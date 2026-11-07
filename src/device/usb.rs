//! usb: native USB transport to a device's hdcd, via nusb.
//!
//! hdc's USB envelope is an 11-byte packed `USBHead` (no alignment pad; the
//! Rust daemon observed on real devices emits and parses this layout) followed
//! by the same hdc frame bytes used over TCP (`usb.h`/`usb_types.h` in
//! developtools_hdc):
//!
//! ```text
//! USBHead {
//!     flag[2]    = "UB"
//!     option     = USB_OPTION_HEADER(1) or USB_OPTION_RESET(2)
//!     sessionId  = htonl(session_id)
//!     dataSize   = htonl(data_size)
//! }             // 2 + 1 + 4 + 4 = 11 bytes, packed
//! ```
//!
//! The device side presents this as an `HDC_CLASS(0xff)/HDC_SUBCLASS(0x50)`
//! interface with two bulk endpoints (OUT 0x01, IN 0x82).
//! When a payload is a multiple of the endpoint packet size, an extra dummy
//! header is appended (WinUSB ZLP workaround; harmless elsewhere).

use std::io::{Read, Write};
use std::time::Duration;

use nusb::transfer::{Bulk, In, Out};
use nusb::MaybeFuture;

pub const HDC_CLASS: u8 = 0xff;
pub const HDC_SUBCLASS: u8 = 0x50;
const USB_PACKET_FLAG: [u8; 2] = *b"UB";
/// Bytes we SEND per USB envelope head. The device sends (and, per the fused
/// 185-byte observation, parses) 11-byte packed heads; we send the same.
const USB_HEAD_SIZE: usize = 11;
/// Bytes we expect in an envelope head from the device (observed packed: 11).
const USB_HEAD_RECV: usize = 11;
const USB_OPTION_HEADER: u8 = 1;
const USB_OPTION_RESET: u8 = 2;

/// One HDCD device found on the bus.
#[derive(Debug, Clone)]
pub struct HdcdDevice {
    pub serial: String,
}

fn is_hdcd(info: &nusb::DeviceInfo) -> bool {
    info.interfaces()
        .any(|i| i.class() == HDC_CLASS && i.subclass() == HDC_SUBCLASS)
}

/// Enumerate HDCD devices (interface class 0xff / subclass 0x50).
pub fn enumerate_hdcd() -> Vec<HdcdDevice> {
    let Ok(list) = nusb::list_devices().wait() else {
        return Vec::new();
    };
    list.filter_map(|d| {
        if !is_hdcd(&d) {
            return None;
        }
        Some(HdcdDevice {
            serial: d.serial_number().unwrap_or_default().to_string(),
        })
    })
    .collect()
}

/// A synchronous USB bulk transport speaking the USBHead envelope.
pub struct UsbTransport {
    reader: nusb::io::EndpointRead<Bulk>,
    writer: nusb::io::EndpointWrite<Bulk>,
    /// Endpoint max packet size, used for the ZLP dummy-packet workaround.
    packet_size: u16,
    timeout: Duration,
    /// Host-side session id (written into handshake body).
    session_id: u32,
    /// Session id the *device* uses for its envelopes (read from its
    /// heartbeat/announce packets). The daemon routes USB packets by this id:
    /// a handshake carrying a host-generated id is forwarded and dropped
    /// (observed `SendToHdcStream softreset dataSize:173` with no
    /// `FetchCommand` after it).
    device_sid: u32,
    /// One envelope body read from the device, not yet consumed.
    pending: Vec<u8>,
    pending_pos: usize,
}

impl UsbTransport {
    /// Open the HDCD interface of the device with `serial` (empty matches the
    /// first HDCD device).
    pub fn open(serial: &str, timeout: Duration) -> Result<Self, String> {
        let mut list = nusb::list_devices()
            .wait()
            .map_err(|e| format!("usb enumerate: {e}"))?;
        let info = list
            .find(|d| is_hdcd(d) && (serial.is_empty() || d.serial_number() == Some(serial)))
            .ok_or_else(|| {
                if serial.is_empty() {
                    "no HDCD device found over USB".to_string()
                } else {
                    format!("HDCD device {serial} not found over USB")
                }
            })?;

        let dev = info.open().wait().map_err(|e| format!("usb open: {e}"))?;

        // The HDC interface's bulk endpoint addresses come from the active
        // configuration's descriptors (they differ from the dev-board layout
        // on commercial ROMs).
        let config = dev
            .active_configuration()
            .map_err(|e| format!("active configuration: {e}"))?;
        let mut iface: Option<u8> = None;
        let mut out_ep: Option<u8> = None;
        let mut in_ep: Option<u8> = None;
        for id in config.interfaces() {
            for alt in id.alt_settings() {
                if alt.class() != HDC_CLASS || alt.subclass() != HDC_SUBCLASS {
                    continue;
                }
                iface = Some(id.interface_number());
                for ep in alt.endpoints() {
                    match (ep.direction(), ep.transfer_type()) {
                        (nusb::transfer::Direction::In, nusb::descriptors::TransferType::Bulk) => {
                            in_ep = Some(ep.address());
                        }
                        (nusb::transfer::Direction::Out, nusb::descriptors::TransferType::Bulk) => {
                            out_ep = Some(ep.address());
                        }
                        _ => {}
                    }
                }
            }
        }
        let out_ep = out_ep.ok_or_else(|| "no bulk OUT endpoint on HDCD interface".to_string())?;
        let in_ep = in_ep.ok_or_else(|| "no bulk IN endpoint on HDCD interface".to_string())?;
        let iface = iface.ok_or_else(|| "HDCD interface not found".to_string())?;
        let interface = dev
            .claim_interface(iface)
            .wait()
            .map_err(|e| format!("claim interface {iface}: {e}"))?;

        let writer = interface
            .endpoint::<Bulk, Out>(out_ep)
            .map_err(|e| format!("endpoint out 0x{out_ep:02x}: {e}"))?
            .writer(64 * 1024);
        let mut reader = interface
            .endpoint::<Bulk, In>(in_ep)
            .map_err(|e| format!("endpoint in 0x{in_ep:02x}: {e}"))?
            .reader(64 * 1024);
        reader.set_read_timeout(timeout);

        let mut t = Self {
            reader,
            writer,
            packet_size: 512,
            timeout,
            session_id: 0,
            device_sid: 0,
            pending: Vec::new(),
            pending_pos: 0,
        };
        t.reset_and_clear()?;
        Ok(t)
    }

    /// USB session bootstrap (host_usb.cpp `ClearUsbChannel`): send a soft
    /// reset (RESET option, session 0) and then drain whatever the device
    /// still has queued from a previous session until the read goes quiet.
    fn reset_and_clear(&mut self) -> Result<(), String> {
        let reset = self.build_head(USB_OPTION_RESET, 0);
        self.writer
            .write_all(&reset)
            .map_err(|e| format!("usb soft reset: {e}"))?;
        // The reset must reach the device as its own envelope — nusb buffers
        // writes, and without this flush the reset would only be submitted
        // together with the first handshake packet (observed on the device as
        // one fused 185-byte "softreset" block that never matches RESET).
        self.writer
            .flush()
            .map_err(|e| format!("usb soft reset flush: {e}"))?;
        self.reader
            .set_read_timeout(std::time::Duration::from_millis(200));
        let mut sink = [0u8; 4096];
        let mut drained = 0usize;
        let start = std::time::Instant::now();
        let mut last_data = start;
        // Drain whatever the previous session left queued, then stop at the
        // first quiet window: the device answers the reset with one stale
        // envelope and goes quiet, so waiting the full budget only adds dead
        // time to every connection.
        while start.elapsed() < std::time::Duration::from_millis(2500) {
            match self.reader.read(&mut sink) {
                Ok(n) if n > 0 => {
                    drained += n;
                    last_data = std::time::Instant::now();
                    // Announced envelopes ("UB" + 8-byte sid/size) carry the
                    // device-side session id, kept for diagnostics only: the
                    // head of every packet we send uses the host's own id.
                    if n >= 7 && sink[0] == USB_PACKET_FLAG[0] && sink[1] == USB_PACKET_FLAG[1] {
                        let sid = u32::from_be_bytes([sink[3], sink[4], sink[5], sink[6]]);
                        if sid != 0 && self.device_sid != sid {
                            self.device_sid = sid;
                            if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                                eprintln!("[usb] device sid seen: {sid}");
                            }
                        }
                    }
                    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                        eprintln!("[usb] drain {} bytes hex={:02x?}", n, &sink[..n.min(16)]);
                    }
                }
                _ => {
                    if last_data.elapsed() >= std::time::Duration::from_millis(500) {
                        break;
                    }
                }
            }
        }
        if std::env::var("HVOLT_USB_DEBUG").is_ok() {
            eprintln!("[usb] drain done total={drained} bytes");
        }
        self.reader.set_read_timeout(self.timeout);
        Ok(())
    }

    fn build_head(&self, option: u8, data_size: u32) -> [u8; USB_HEAD_SIZE] {
        let mut h = [0u8; USB_HEAD_SIZE];
        h[0] = USB_PACKET_FLAG[0];
        h[1] = USB_PACKET_FLAG[1];
        h[2] = option;
        // Always the host's own session id: the official host builds every
        // envelope with `BuildPacketHeader(hSession->sessionId, ...)`
        // (usb_common.cpp). The device adopts that id — its heartbeat/ack
        // envelopes come back carrying it — and routes commands to the
        // session that authenticated. Routing by a *learned* device id sent
        // commands to a session that never authenticated (device log:
        // `CheckAuthStatus auth failed ... for command 1001`).
        h[3..7].copy_from_slice(&self.session_id.to_be_bytes());
        h[7..11].copy_from_slice(&data_size.to_be_bytes());
        h
    }

    /// One USB packet: header + body, plus the ZLP dummy when the body
    /// exactly fills endpoint packets.
    fn send_block(&mut self, data: &[u8]) -> Result<(), String> {
        if std::env::var("HVOLT_USB_DEBUG").is_ok() {
            eprintln!(
                "[usb] send block len={} sid={} opt=1 head={:02x?}",
                data.len(),
                self.session_id,
                self.build_head(USB_OPTION_HEADER, data.len() as u32)
            );
        }
        let head = self.build_head(USB_OPTION_HEADER, data.len() as u32);
        self.writer
            .write_all(&head)
            .map_err(|e| format!("usb send head: {e}"))?;
        // The head must reach the device as its OWN bulk transfer: the peer's
        // `IsUsbPacketHeader` rejects any transfer whose length is not exactly
        // one header (`ioBytes != sizeof(USBHead)`), and then treats the whole
        // blob as stale session data and drops it (observed as
        // `SendToHdcStream softreset dataSize:173` == our 11-byte head fused
        // with the 162-byte frame). nusb coalesces queued writes, so flush
        // here — the official host issues one `SendUSBRaw` per part.
        self.writer
            .flush()
            .map_err(|e| format!("usb send head flush: {e}"))?;
        if !data.is_empty() {
            self.writer
                .write_all(data)
                .map_err(|e| format!("usb send body: {e}"))?;
            if (data.len() as u64) % u64::from(self.packet_size) == 0 {
                let dummy = self.build_head(0, 0);
                self.writer
                    .write_all(&dummy)
                    .map_err(|e| format!("usb send dummy: {e}"))?;
            }
        }
        // nusb buffers writes; submit them to the endpoint now so the device
        // actually sees the packet.
        self.writer.flush().map_err(|e| format!("usb flush: {e}"))?;
        Ok(())
    }

    /// Read one USB envelope (header + body) into `pending`.
    fn refill(&mut self) -> Result<(), String> {
        let mut head = [0u8; USB_HEAD_RECV];
        self.reader
            .read_exact(&mut head)
            .map_err(|e| format!("usb recv head: {e}"))?;
        if head[0] == USB_PACKET_FLAG[0] && head[1] == USB_PACKET_FLAG[1] {
            let sid = u32::from_be_bytes([head[3], head[4], head[5], head[6]]);
            if sid != 0 && self.device_sid != sid {
                self.device_sid = sid;
                if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                    eprintln!("[usb] device sid learned: {sid}");
                }
            }
        }
        if std::env::var("HVOLT_USB_DEBUG").is_ok() {
            eprintln!("[usb] recv head {:02x?}", head);
        }
        if head[0] != USB_PACKET_FLAG[0] || head[1] != USB_PACKET_FLAG[1] {
            return Err(format!(
                "usb packet flag mismatch: {:02x}{:02x}",
                head[0], head[1]
            ));
        }
        let data_size = u32::from_be_bytes([head[7], head[8], head[9], head[10]]) as usize;
        if data_size > 64 * 1024 * 1024 {
            return Err(format!("usb packet too large: {data_size}"));
        }
        let mut body = vec![0u8; data_size];
        if data_size > 0 {
            self.reader
                .read_exact(&mut body)
                .map_err(|e| format!("usb recv body: {e}"))?;
            if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                eprintln!(
                    "[usb] recv body {} bytes hex={:02x?}",
                    body.len(),
                    &body[..body.len().min(20)]
                );
            }
        }
        self.pending = body;
        self.pending_pos = 0;
        Ok(())
    }
}

impl Read for UsbTransport {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pending_pos >= self.pending.len() {
            self.refill().map_err(std::io::Error::other)?;
        }
        let avail = &self.pending[self.pending_pos..];
        let n = avail.len().min(buf.len());
        buf[..n].copy_from_slice(&avail[..n]);
        self.pending_pos += n;
        Ok(n)
    }
}

impl Write for UsbTransport {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.send_block(buf).map_err(std::io::Error::other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // nusb's EndpointWrite buffers; an explicit flush is what actually
        // submits the queued transfers to the endpoint.
        self.writer.flush().map_err(std::io::Error::other)
    }
}

impl crate::device::hdc_proto::Transport for UsbTransport {
    fn set_read_timeout(&mut self, d: Duration) -> Result<(), String> {
        self.reader.set_read_timeout(d);
        Ok(())
    }

    fn set_session_id(&mut self, session_id: u32) {
        self.session_id = session_id;
    }
}
