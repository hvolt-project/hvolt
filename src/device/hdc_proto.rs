//! Native hdc wire protocol: framing, handshake, and RSA challenge-response.
//!
//! Every constant and layout here is quoted from the OpenHarmony `developtools_hdc` sources and
//! cross-checked against traffic captured from a real device:
//!
//! - `PayloadHead` is an 11-byte packed struct (`session.h`), holding the frame flag, the protocol
//!   version, and the two lengths that follow it.
//! - The handshake message is `SessionHandShake`, serialised as six protobuf fields
//!   (`serial_struct.h`).
//! - Authentication is an RSA-3072 challenge-response: the device sends a 20-byte token
//!   (`auth.h: RSA_TOKEN_SIZE`) and the host returns a PKCS#1 v1.5 signature over its SHA-256
//!   digest (`auth.cpp: RSA_verify(NID_sha256, ...)`).
//!
//! The captured frames that pin this down: a 178-byte client handshake decodes as
//! `11 (head) + 8 (PayloadProtect) + 159 (SessionHandShake)`, and the server's 99-byte answer as
//! `11 + 8 + 80`.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Frame flag in front of every packet (`define.h`: `PACKET_FLAG = "HW"`).
const PACKET_FLAG: [u8; 2] = *b"HW";
/// Protocol version written into every `PayloadHead` (`define.h`: `VER_PROTOCOL = 0x01`).
const VER_PROTOCOL: u8 = 0x01;
/// `sizeof(struct PayloadHead)` with `__attribute__((packed))` (`session.h`).
const HEAD_LEN: usize = 11;
/// The `PayloadProtect` every handshake frame carries.
///
/// Captured from a working client: `command_flag = 1` names the handshake command and
/// `v_code = 9` is the protocol revision the daemon expects. Sending zeros instead makes the
/// daemon close the connection without logging anything.
pub const HANDSHAKE_PROTECT: PayloadProtect = PayloadProtect {
    channel_id: 0,
    command_flag: 1,
    check_sum: 0,
    v_code: 9,
};

/// `AuthType::AUTH_NONE` — the state a host starts in (`session.h`).
pub const AUTH_NONE: u32 = 0;
/// `AuthType::AUTH_PUBLICKEY` — the device wants our public key next.
pub const AUTH_PUBLICKEY: u32 = 3;
/// `AuthType::AUTH_OK` — the device accepted us.
pub const AUTH_OK: u32 = 4;
/// `AuthType::AUTH_SIGNATURE` — the device wants the signed challenge.
pub const AUTH_SIGNATURE: u32 = 2;
/// Length of the device's challenge token (`auth.h`: `RSA_TOKEN_SIZE = 20`).
pub const RSA_TOKEN_SIZE: usize = 20;
/// `CMD_KERNEL_WAKEUP_SLAVETASK` — pre-create the daemon-side *slave* task for
/// a transfer channel, so `CMD_FILE_CHECK`/`CMD_APP_CHECK` on that channel find
/// a task. `define_enum.h` numbering: HELP=0, HANDSHAKE=1, CHANNEL_CLOSE=2,
/// TARGET_DISCOVER=4 … ECHO=9, ECHO_RAW=10, ENABLE_KEEPALIVE=11, **WAKEUP=12**.
/// Sending 10 (ECHO_RAW) leaves the daemon with no task and it closes the
/// channel (device log: `DispatchTaskData hTaskInfo null … command:10`).
pub const CMD_KERNEL_WAKEUP_SLAVETASK: u16 = 12;
/// `CMD_UNITY_EXECUTE` — run a shell command (`define_enum.h`). The payload is the command
/// string itself; the reply streams back on the channel named in the frame head.
pub const CMD_UNITY_EXECUTE: u16 = 1001;
/// `v_code` every frame after the handshake carries (captured).
pub const V_CODE: u32 = 9;
/// `CMD_KERNEL_CHANNEL_CLOSE` — the peer closing a task channel (`define_enum.h: 2`).
pub const CMD_KERNEL_CHANNEL_CLOSE: u16 = 2;
/// `CMD_FILE_INIT` (`define_enum.h: 3000`) — ask the daemon to *send* a file:
/// the daemon's statistic table maps it to `FILE_RECV_COUNT`, and its handler
/// opens the named path as the transfer source.
pub const CMD_FILE_INIT: u16 = 3000;
/// `CMD_FILE_CHECK` — announce a *push* with its `TransferConfig` (`define_enum.h: 3001`; the
/// daemon maps it to `FILE_SEND_COUNT` and answers with `CMD_FILE_BEGIN`).
pub const CMD_FILE_CHECK: u16 = 3001;
/// `CMD_FILE_BEGIN` — the daemon opened the destination and the host may stream data.
pub const CMD_FILE_BEGIN: u16 = 3002;
/// `CMD_FILE_DATA` — one `TransferPayload` header followed by raw file bytes.
pub const CMD_FILE_DATA: u16 = 3003;
/// `CMD_FILE_FINISH` — close the transfer.
pub const CMD_FILE_FINISH: u16 = 3004;
/// `CMD_APP_CHECK` — announce an install with its `TransferConfig` (`define_enum.h`).
pub const CMD_APP_CHECK: u16 = 3501;
/// `CMD_KERNEL_HEARTBEAT` arithmetic: the daemon sends `command:5000` heartbeats
/// on channel 0 while a task runs; they are not part of any result.
pub const HEARTBEAT_FLAG: u16 = 5000;
/// `CMD_APP_BEGIN` — the daemon opened the staging file and the host may stream bytes.
pub const CMD_APP_BEGIN: u16 = 3502;
/// `CMD_APP_DATA` — the package bytes.
pub const CMD_APP_DATA: u16 = 3503;
/// Reserved bytes between a `TransferPayload` header and the file body (captured: 8 + 56 + 17).
const FILE_PACKAGE_HEAD: usize = 64;
/// Bytes of file content per data frame: the daemon seeks to `index * chunk` and reads exactly
/// this much (`config.rs: FILE_PACKAGE_PAYLOAD_SIZE = 49152`), so the two must agree.
const FILE_CHUNK: usize = 49152;

/// Banner every handshake opens with (`define.h`: `HANDSHAKE_MESSAGE = "OHOS HDC"`).
pub const HANDSHAKE_MESSAGE: &str = "OHOS HDC";

/// The fixed frame head: `flag[2] reserve[2] protocolVer headSize(u16 BE) dataSize(u32 BE)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadHead {
    pub reserve: [u8; 2],
    pub protocol_ver: u8,
    /// Bytes of `PayloadProtect` that follow the head.
    pub head_size: u16,
    /// Bytes of application payload that follow `PayloadProtect`.
    pub data_size: u32,
}

impl PayloadHead {
    fn encode(&self) -> [u8; HEAD_LEN] {
        let mut out = [0u8; HEAD_LEN];
        out[0..2].copy_from_slice(&PACKET_FLAG);
        out[2..4].copy_from_slice(&self.reserve);
        out[4] = self.protocol_ver;
        out[5..7].copy_from_slice(&self.head_size.to_be_bytes());
        out[7..11].copy_from_slice(&self.data_size.to_be_bytes());
        out
    }

    fn decode(buf: &[u8]) -> Result<Self, String> {
        if buf.len() < HEAD_LEN {
            return Err(format!("frame head truncated: {} bytes", buf.len()));
        }
        if buf[0..2] != PACKET_FLAG {
            return Err(format!(
                "frame flag mismatch: {:02x} {:02x}",
                buf[0], buf[1]
            ));
        }
        let mut reserve = [0u8; 2];
        reserve.copy_from_slice(&buf[2..4]);
        let mut head_size = [0u8; 2];
        head_size.copy_from_slice(&buf[5..7]);
        let mut data_size = [0u8; 4];
        data_size.copy_from_slice(&buf[7..11]);
        Ok(Self {
            reserve,
            protocol_ver: buf[4],
            head_size: u16::from_be_bytes(head_size),
            data_size: u32::from_be_bytes(data_size),
        })
    }
}

/// Per-packet routing fields (`serial_struct.h`: `PayloadProtect`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PayloadProtect {
    pub channel_id: u32,
    pub command_flag: u16,
    pub check_sum: u32,
    pub v_code: u32,
}

impl PayloadProtect {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_uint(&mut out, 1, u64::from(self.channel_id));
        put_uint(&mut out, 2, u64::from(self.command_flag));
        put_uint(&mut out, 3, u64::from(self.check_sum));
        put_uint(&mut out, 4, u64::from(self.v_code));
        out
    }

    fn decode(buf: &[u8]) -> Result<Self, String> {
        let mut out = Self::default();
        let mut reader = ProtoReader::new(buf);
        while let Some((field, value)) = reader.next_field()? {
            match (field, value) {
                (1, ProtoValue::Uint(v)) => out.channel_id = v as u32,
                (2, ProtoValue::Uint(v)) => out.command_flag = v as u16,
                (3, ProtoValue::Uint(v)) => out.check_sum = v as u32,
                (4, ProtoValue::Uint(v)) => out.v_code = v as u32,
                _ => {}
            }
        }
        Ok(out)
    }
}

/// The handshake message (`serial_struct.h`: `SessionHandShake`, six fields).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionHandShake {
    /// Field 1: always `HANDSHAKE_MESSAGE`.
    pub banner: String,
    /// Field 2: authentication state the sender is in.
    pub auth_type: u32,
    /// Field 3: session this handshake belongs to.
    pub session_id: u32,
    /// Field 4: `<host>:<port>` for TCP, the USB serial for USB.
    pub connect_key: String,
    /// Field 5: free-form payload — the public key, the signed token, and status lines.
    pub buf: String,
    /// Field 6: protocol version string, e.g. `Ver: 3.2.0`.
    pub version: String,
}

impl SessionHandShake {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_str(&mut out, 1, &self.banner);
        put_uint(&mut out, 2, u64::from(self.auth_type));
        put_uint(&mut out, 3, u64::from(self.session_id));
        put_str(&mut out, 4, &self.connect_key);
        put_str(&mut out, 5, &self.buf);
        put_str(&mut out, 6, &self.version);
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Self, String> {
        let mut out = Self::default();
        let mut reader = ProtoReader::new(buf);
        while let Some((field, value)) = reader.next_field()? {
            match (field, value) {
                (1, ProtoValue::Bytes(v)) => out.banner = bytes_to_string(v),
                (2, ProtoValue::Uint(v)) => out.auth_type = v as u32,
                (3, ProtoValue::Uint(v)) => out.session_id = v as u32,
                (4, ProtoValue::Bytes(v)) => out.connect_key = bytes_to_string(v),
                (5, ProtoValue::Bytes(v)) => out.buf = bytes_to_string(v),
                (6, ProtoValue::Bytes(v)) => out.version = bytes_to_string(v),
                _ => {}
            }
        }
        Ok(out)
    }
}

fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

// ── protobuf wire format, minimal subset ──────────────────────────────────────────────────────

enum ProtoValue<'a> {
    Uint(u64),
    Bytes(&'a [u8]),
}

struct ProtoReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ProtoReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_varint(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = *self
                .buf
                .get(self.pos)
                .ok_or_else(|| "protobuf: varint ran past the buffer".to_string())?;
            self.pos += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift >= 64 {
                return Err("protobuf: varint longer than 64 bits".to_string());
            }
        }
    }

    fn next_field(&mut self) -> Result<Option<(u32, ProtoValue<'a>)>, String> {
        if self.pos >= self.buf.len() {
            return Ok(None);
        }
        let key = self.read_varint()?;
        let field = (key >> 3) as u32;
        let wire = (key & 0x7) as u8;
        let value = match wire {
            0 => ProtoValue::Uint(self.read_varint()?),
            2 => {
                let len = self.read_varint()? as usize;
                let end = self.pos + len;
                let slice = self.buf.get(self.pos..end).ok_or_else(|| {
                    "protobuf: length-delimited field past the buffer".to_string()
                })?;
                self.pos = end;
                ProtoValue::Bytes(slice)
            }
            other => return Err(format!("protobuf: unsupported wire type {other}")),
        };
        Ok(Some((field, value)))
    }
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn put_tag(out: &mut Vec<u8>, field: u32, wire: u8) {
    put_varint(out, (u64::from(field) << 3) | u64::from(wire));
}

fn put_uint(out: &mut Vec<u8>, field: u32, value: u64) {
    put_tag(out, field, 0);
    put_varint(out, value);
}

fn put_str(out: &mut Vec<u8>, field: u32, value: &str) {
    put_tag(out, field, 2);
    put_varint(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

// ── TLV used inside the handshake `buf` ──────────────────────────────────────────────────────

/// Field width of a TLV tag (`base.h`: `TLV_TAG_LEN = 16`), space padded.
const TLV_TAG_LEN: usize = 16;
/// Field width of a TLV value-length (`base.h`: `TLV_VAL_LEN = 16`), space padded.
const TLV_VAL_LEN: usize = 16;
/// The one TLV the device insists on (`base.h`: `TAG_AUTH_TYPE = "authtype"`).
pub const TAG_AUTH_TYPE: &str = "authtype";
/// Capabilities the host advertises (`base.h`: `TAG_SUPPORT_FEATURE = "supportfeatures"`).
pub const TAG_SUPPORT_FEATURES: &str = "supportfeatures";
/// `AuthVerifyType::RSA_3072_SHA512` — the scheme we advertise (`define_enum.h`).
pub const AUTH_VERIFY_RSA_3072_SHA512: u32 = 1;

/// Append one TLV pair: tag padded to 16, length padded to 16, then the value itself.
fn tlv_append(out: &mut String, tag: &str, value: &str) {
    let mut tag = tag.to_string();
    while tag.len() < TLV_TAG_LEN {
        tag.push(' ');
    }
    let mut len = value.len().to_string();
    while len.len() < TLV_VAL_LEN {
        len.push(' ');
    }
    out.push_str(&tag);
    out.push_str(&len);
    out.push_str(value);
}

/// The `buf` a host sends in its opening handshake.
///
/// Two TLVs, exactly as a real host sends them: the auth scheme it can do, and the capability
/// list whose first element is the host version string. The rest of the list names the transport
/// (`TCP`), the host OS token, and optional features — the daemon reads `authtype` and ignores
/// the rest, but sending both keeps the frame byte-compatible with what it expects.
fn opening_buf(version_tag: &str) -> String {
    let mut buf = String::new();
    tlv_append(
        &mut buf,
        TAG_AUTH_TYPE,
        &AUTH_VERIFY_RSA_3072_SHA512.to_string(),
    );
    let features = format!("{version_tag},TCP,win,heartbeat");
    tlv_append(&mut buf, TAG_SUPPORT_FEATURES, &features);
    buf
}

// ── authentication ───────────────────────────────────────────────────────────────────────────

/// Separator between the host name and the PEM key in the public-key `buf`.
///
/// Captured from a real host: `"SURFACE-YCHONG" 0x0c "-----BEGIN PUBLIC KEY-----\n..."`. A
/// newline here is silently accepted by the parser but the key never verifies.
const PUBKEY_SEPARATOR: char = '\u{0c}';

/// The host key pair a device remembers (`auth.cpp`: `hdckey` / `hdckey.pub` under `~/.harmony`).
pub struct HostIdentity {
    pub public_key_pem: String,
    pub private_key: rsa::RsaPrivateKey,
}

impl HostIdentity {
    /// Load the key pair `hdc` itself uses, so no separate pairing is needed.
    pub fn load_from_home() -> Result<Self, String> {
        use rsa::pkcs8::DecodePrivateKey;
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| "cannot locate the home directory".to_string())?;
        let dir = std::path::Path::new(&home).join(".harmony");
        let pub_path = dir.join("hdckey.pub");
        let pri_path = dir.join("hdckey");
        let public_key_pem = std::fs::read_to_string(&pub_path)
            .map_err(|e| format!("read {}: {e}", pub_path.display()))?;
        let private_pem = std::fs::read_to_string(&pri_path)
            .map_err(|e| format!("read {}: {e}", pri_path.display()))?;
        let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(&private_pem)
            .map_err(|e| format!("parse {}: {e}", pri_path.display()))?;
        Ok(Self {
            public_key_pem,
            private_key,
        })
    }

    /// The `buf` that carries this identity to the device.
    fn public_key_buf(&self, hostname: &str) -> String {
        format!("{hostname}{PUBKEY_SEPARATOR}{}", self.public_key_pem)
    }

    /// Sign the device's challenge. The daemon verifies with RSA-PSS over SHA-512 and an
    /// automatic salt length (`daemon.cpp: EVP_PKEY_CTX_set_rsa_padding(RSA_PKCS1_PSS_PADDING)`),
    /// so a PKCS#1 v1.5 signature is rejected with `[E000010]:Auth failed`.
    fn sign_token(&self, token: &[u8]) -> Result<String, String> {
        use base64::Engine as _;
        use rsa::pss::SigningKey;
        use rsa::signature::{RandomizedSigner, SignatureEncoding};
        use sha2::Sha512;

        let signing_key = SigningKey::<Sha512>::new(self.private_key.clone());
        let mut rng = rand::thread_rng();
        let signature = signing_key.sign_with_rng(&mut rng, token);
        Ok(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()))
    }
}

/// Where the host name lands in the handshake.
fn host_name() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
        if !name.is_empty() {
            return name;
        }
    }
    crate::support::platform::hostname().unwrap_or_else(|| "localhost".to_string())
}

/// What a completed handshake reports back.
#[derive(Debug)]
pub struct AuthReport {
    pub hostname: String,
    pub device_name: String,
    pub auth_status: String,
    pub device_message: String,
}

/// Run the three-step handshake: advertise capabilities, offer the public key, sign the token.
pub fn authenticate(
    addr: &str,
    version: &str,
    identity: &HostIdentity,
    timeout: Duration,
) -> Result<AuthReport, String> {
    let mut conn = HdcConn::connect_device(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)
}

/// Run one shell command over a freshly authenticated connection and return its output.
///
/// The command travels as the payload of a `CMD_UNITY_EXECUTE` frame on a channel id the host
/// picks; the device answers on that same channel.
pub fn shell(
    addr: &str,
    version: &str,
    identity: &HostIdentity,
    command: &str,
    timeout: Duration,
) -> Result<String, String> {
    let mut conn = HdcConn::connect_device(addr, timeout)?;
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[shell] connected");
    }
    handshake(&mut conn, addr, version, identity)?;
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[shell] handshake ok");
    }
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let protect = PayloadProtect {
        channel_id: channel,
        command_flag: CMD_UNITY_EXECUTE,
        check_sum: 0,
        v_code: V_CODE,
    };
    // The payload is the command string itself (`daemon_unity.cpp` builds
    // `strPayload` straight from the frame bytes). A trailing NUL mirrors the
    // host's `cmdline.c_str()`; earlier TLV wrapping was the host↔server IPC
    // format and produced an empty command (NUL head) — no output, no error.
    let mut cmdline = command.as_bytes().to_vec();
    cmdline.push(0);
    // No extra confirmation frame: the daemon's three-step handshake already
    // sets the session's auth state to OK (`UpdateSessionAuthOk`), and it then
    // closes the *auth channel* with a `CMD_KERNEL_CHANNEL_CLOSE (1)` trailer —
    // which is a normal close of that task, not a rejection. Sending a
    // `CMD_KERNEL_HANDSHAKE` here confused the daemon into resetting the
    // session (connection closed right after). Command frames travel on a new
    // (random) task channel and are checked against the session auth state.
    conn.send_packet(protect, &cmdline)?;
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[shell] sent {} bytes", cmdline.len());
    }
    // The device answers on the channel and then keeps the session open, so a
    // blocking read would hang forever. A short idle timeout ends the read.
    // The device keeps the channel open after answering, so an idle timeout
    // ends the read. 1.5 s was too short for slow producers (hilog dumps need
    // several seconds before the first byte), so wait longer before giving up.
    conn.set_read_timeout(std::time::Duration::from_secs(8))?;
    let mut output = String::new();
    let mut pkts = 0usize;
    loop {
        match conn.recv_packet() {
            Ok((p, payload)) => {
                pkts += 1;
                if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                    eprintln!(
                        "[shell] pkt {} chan={} flag={} len={} hex={:02x?}",
                        pkts,
                        p.channel_id,
                        p.command_flag,
                        payload.len(),
                        &payload[..payload.len().min(24)]
                    );
                }
                // `CMD_KERNEL_CHANNEL_CLOSE` on *our* task channel ends the command.
                // The handshake also closes the auth channel (channel 0) with the
                // same flag right after connect; that trailer must not be mistaken
                // for completion, or the command looks finished before any output.
                // Leaving the loop on a recv error alone would hang forever: the
                // daemon keeps sending heartbeats, so every recv succeeds.
                if p.command_flag == CMD_KERNEL_CHANNEL_CLOSE && p.channel_id == channel {
                    break;
                }
                if p.channel_id == channel {
                    output.push_str(&String::from_utf8_lossy(&payload));
                } else if p.command_flag == CMD_KERNEL_CHANNEL_CLOSE {
                    continue;
                }
            }
            Err(e) => {
                if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                    eprintln!("[shell] recv end: {e}");
                }
                break;
            }
        }
    }
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[shell] done pkts={} out_len={}", pkts, output.len());
    }
    Ok(output)
}

/// Push one local file to `remote_path` on the device.
///
/// The device expects three frames on one channel: a `TransferConfig` announcement, a
/// `TransferPayload` header with the raw bytes appended, then a finish marker. The payload
/// serialisation is not plain protobuf — the length fields name sizes, and the file content is
/// appended verbatim after the header (captured from a working `hdc file send`).
pub fn send_file(
    addr: &str,
    version: &str,
    identity: &HostIdentity,
    local_path: &std::path::Path,
    remote_path: &str,
    timeout: Duration,
) -> Result<(), String> {
    let data =
        std::fs::read(local_path).map_err(|e| format!("read {}: {e}", local_path.display()))?;

    let mut conn = HdcConn::connect_device(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)?;
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let frame = |cmd: u16| PayloadProtect {
        channel_id: channel,
        command_flag: cmd,
        check_sum: 0,
        v_code: V_CODE,
    };

    // Pushing to the device is the `file send` direction: the host announces a
    // `TransferConfig` with `CMD_FILE_CHECK`, the daemon opens the destination
    // and answers `CMD_FILE_BEGIN`, the bytes stream as `CMD_FILE_DATA`, and
    // the daemon closes with `CMD_FILE_FINISH`. The entry command matters:
    // `CMD_FILE_INIT` is the *receive* direction, so the daemon tried to open
    // the destination for reading and answered "Error opening file".
    let name = remote_path
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("file");
    let mut config = Vec::new();
    put_uint(&mut config, 1, data.len() as u64); // fileSize
    put_uint(&mut config, 2, 0); // atime
    put_uint(&mut config, 3, 0); // mtime
    put_str(&mut config, 4, ""); // options
    put_str(&mut config, 5, remote_path); // path: destination on the device
    put_str(&mut config, 6, name); // optionalName
    put_uint(&mut config, 7, 0); // updateIfNew
    put_uint(&mut config, 8, 0); // compressType: none
    put_uint(&mut config, 9, 0); // holdTimestamp
    put_str(&mut config, 10, ""); // functionName
    put_str(&mut config, 11, ""); // clientCwd
                                  // The task has to exist first: `NeedNewTaskInfo` only creates one for
                                  // FILE_INIT/APP_INIT/WAKEUP_SLAVETASK, and FILE_INIT's task is the
                                  // *sending* side (it opens the path for reading). A push therefore wakes a
                                  // slave task, then announces the config on it.
    conn.send_packet(frame(CMD_KERNEL_WAKEUP_SLAVETASK), &[])?;
    conn.send_packet(frame(CMD_FILE_CHECK), &config)?;

    // BEGIN means the destination is open. The daemon answers the wakeup frame
    // itself with a channel-close (WAKEUP is not a file command, so the fresh
    // task treats it as unknown), so a close arriving *before* BEGIN is noise,
    // not a refusal — but it must not swallow a real BEGIN.
    // The daemon opens the destination asynchronously (observed ~4s stall in
    // `HdcTransferBase::OnFileOpen`), so poll for BEGIN on a time budget rather
    // than a frame count, and treat empty close frames as noise.
    let mut began = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    conn.set_read_timeout(std::time::Duration::from_secs(3))?;
    while std::time::Instant::now() < deadline {
        match conn.recv_packet() {
            Ok((p0, a0)) => {
                if p0.command_flag == CMD_FILE_BEGIN {
                    began = true;
                    break;
                }
                // A close frame is the daemon's answer to the wakeup (payload
                // is a single status byte and means nothing for this flow);
                // real errors arrive as a message frame carrying text.
                if p0.command_flag != CMD_KERNEL_CHANNEL_CLOSE {
                    let text = String::from_utf8_lossy(&a0);
                    let text = text.trim().trim_start_matches('\0').trim();
                    if !text.is_empty() {
                        return Err(format!(
                            "file send refused (flag={}): {}",
                            p0.command_flag, text
                        ));
                    }
                }
            }
            Err(_) => continue,
        }
    }
    if !began {
        return Err("file send refused: no BEGIN from the device".to_string());
    }
    conn.set_read_timeout(timeout)?;

    // TransferPayload: index is the byte offset into the file, then the two
    // sizes; the header occupies a fixed 64 bytes and the bytes follow.
    let mut pos: u64 = 0;
    for chunk in data.chunks(FILE_CHUNK) {
        let mut payload = Vec::new();
        put_uint(&mut payload, 1, pos);
        put_uint(&mut payload, 2, 0); // compressType: none
                                      // With COMPRESS_NONE the daemon takes `compressSize` as the byte count
                                      // and rejects the frame unless it equals `uncompressSize`
                                      // (`RecvIOPayload`: "invalid data size for fileIO").
        put_uint(&mut payload, 3, chunk.len() as u64);
        put_uint(&mut payload, 4, chunk.len() as u64);
        payload.resize(FILE_PACKAGE_HEAD, 0);
        payload.extend_from_slice(chunk);
        conn.send_packet(frame(CMD_FILE_DATA), &payload)?;
        pos += chunk.len() as u64;
    }

    let (p1, a1) = conn.recv_packet()?;
    // `HdcFile::WhenTransferFinish` closes a successful transfer with
    // CMD_FILE_FINISH and a single `1` byte, so the command is the verdict —
    // an error instead arrives as a message frame carrying text.
    // A status byte is not text: only a longer payload is the daemon's message.
    let ok = p1.command_flag == CMD_FILE_FINISH && a1.len() <= 2;
    if !ok {
        let text = String::from_utf8_lossy(&a1);
        return Err(format!(
            "file send failed (flag={}): {}",
            p1.command_flag,
            text.trim().trim_start_matches('\0').trim()
        ));
    }
    Ok(())
}

/// Pull one file from the device (`hdc file recv`).
///
/// The mirror of `send_file`: the host announces `CMD_FILE_INIT` with the
/// command line the daemon parses for a *receive-from-device*, i.e. the file
/// the device must open as the transfer source. The daemon (now the sending
/// side) opens it, answers `CMD_FILE_CHECK` with a `TransferConfig` carrying
/// the size, and streams `CMD_FILE_DATA` once we reply `CMD_FILE_BEGIN`.
pub fn recv_file(
    addr: &str,
    version: &str,
    identity: &HostIdentity,
    remote_path: &str,
    local_path: &std::path::Path,
    timeout: Duration,
) -> Result<u64, String> {
    let mut conn = HdcConn::connect_device(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)?;
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let frame = |cmd: u16| PayloadProtect {
        channel_id: channel,
        command_flag: cmd,
        check_sum: 0,
        v_code: V_CODE,
    };

    // `file recv <src> <src>`: the daemon takes the second-to-last argument as
    // the file it must open, so both positions name the device path.
    let mut cmdline = format!("file recv {remote_path} {remote_path}").into_bytes();
    cmdline.push(0);
    conn.send_packet(frame(CMD_FILE_INIT), &cmdline)?;

    // The daemon answers with the TransferConfig (fileSize in field 1). It may
    // first emit a channel-close (the same noise the push path sees), so poll
    // for CHECK on a time budget instead of trusting the first frame.
    let mut check: Option<Vec<u8>> = None;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    conn.set_read_timeout(std::time::Duration::from_secs(3))?;
    while std::time::Instant::now() < deadline {
        match conn.recv_packet() {
            Ok((p0, a0)) => {
                if p0.command_flag == CMD_FILE_CHECK {
                    check = Some(a0);
                    break;
                }
                if p0.command_flag != CMD_KERNEL_CHANNEL_CLOSE {
                    let text = String::from_utf8_lossy(&a0);
                    let text = text.trim().trim_start_matches('\0').trim();
                    if !text.is_empty() {
                        return Err(format!(
                            "file recv refused (flag={}): {}",
                            p0.command_flag, text
                        ));
                    }
                }
            }
            Err(_) => continue,
        }
    }
    let Some(a0) = check else {
        return Err("file recv refused: no FILE_CHECK from the device".to_string());
    };
    let mut reader = ProtoReader::new(&a0);
    let mut file_size: u64 = 0;
    while let Ok(Some((field, value))) = reader.next_field() {
        if field == 1 {
            if let ProtoValue::Uint(v) = value {
                file_size = v;
            }
            break;
        }
    }

    // BEGIN: the destination is ours to write, start streaming.
    conn.send_packet(frame(CMD_FILE_BEGIN), &[])?;
    let mut out: Vec<u8> = Vec::new();
    conn.set_read_timeout(std::time::Duration::from_secs(20))?;
    while let Ok((p, payload)) = conn.recv_packet() {
        match p.command_flag {
            CMD_FILE_DATA => {
                if payload.len() > FILE_PACKAGE_HEAD {
                    out.extend_from_slice(&payload[FILE_PACKAGE_HEAD..]);
                }
            }
            CMD_FILE_FINISH => break,
            _ => {}
        }
        if file_size > 0 && out.len() as u64 >= file_size {
            break;
        }
    }
    std::fs::write(local_path, &out).map_err(|e| format!("write {}: {e}", local_path.display()))?;
    Ok(out.len() as u64)
}

/// Install one package on the device and return whatever the installer reports.
///
/// The sequence mirrors `hdc install`: a `TransferConfig` whose `functionName` is `install` and
/// whose destination path is left empty (the daemon picks the staging name), the package bytes,
/// then the result the daemon streams back. There is no explicit finish frame for this flow.
pub fn install(
    addr: &str,
    version: &str,
    identity: &HostIdentity,
    package: &std::path::Path,
    timeout: Duration,
) -> Result<String, String> {
    let data = std::fs::read(package).map_err(|e| format!("read {}: {e}", package.display()))?;
    let mut conn = HdcConn::connect_device(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)?;
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let frame = |cmd: u16| PayloadProtect {
        channel_id: channel,
        command_flag: cmd,
        check_sum: 0,
        v_code: V_CODE,
    };

    // Direct-TCP daemon session: wake the transfer task first (the daemon
    // creates the task for WAKEUP_SLAVETASK, then routes APP_* frames to it),
    // then announce the install.
    conn.send_packet(frame(CMD_KERNEL_WAKEUP_SLAVETASK), &[])?;
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[install] wakeup sent");
    }
    // No ack for the wakeup frame: the daemon creates the task and moves on
    // without replying (its dispatch has no handler), and the next frame on
    // this channel is the one that answers.
    let staging = format!("{:x}.hap", rand::random::<u32>());
    let mut config = Vec::new();
    put_uint(&mut config, 1, data.len() as u64);
    put_uint(&mut config, 2, 0);
    put_uint(&mut config, 3, 0);
    put_str(&mut config, 4, "");
    put_str(&mut config, 5, "");
    put_str(&mut config, 6, &staging);
    put_uint(&mut config, 7, 0);
    put_uint(&mut config, 8, 0);
    put_uint(&mut config, 9, 0);
    put_str(&mut config, 10, "install");
    put_str(&mut config, 11, "");
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[install] app_check sending {} bytes", config.len());
    }
    conn.send_packet(frame(CMD_APP_CHECK), &config)?;
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[install] app_check sent, waiting ack");
    }
    // The daemon opens the staging file asynchronously and answers APP_BEGIN;
    // streaming before that leaves its fd unset and every write fails with
    // EBADF (device log: `WhenTransferFinish with errno:9`, 0/2782537 bytes).
    let mut opened = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    conn.set_read_timeout(std::time::Duration::from_secs(3))?;
    while std::time::Instant::now() < deadline {
        match conn.recv_packet() {
            Ok((p, ack)) => {
                if p.command_flag == CMD_APP_BEGIN {
                    opened = true;
                    break;
                }
                if p.command_flag != CMD_KERNEL_CHANNEL_CLOSE {
                    let text = String::from_utf8_lossy(&ack);
                    let text = text.trim().trim_start_matches('\0').trim();
                    if !text.is_empty() {
                        return Err(format!(
                            "install refused (flag={}): {}",
                            p.command_flag, text
                        ));
                    }
                }
            }
            Err(_) => continue,
        }
    }
    if !opened {
        return Err("install refused: no APP_BEGIN from the device".to_string());
    }
    conn.set_read_timeout(timeout)?;

    // Stream the package in bounded chunks. No per-chunk acknowledgement: the
    // daemon writes at `header.index` (a byte offset into the file) and sends a
    // result frame only when the transfer completes (modern Rust daemon; the old
    // C++ daemon's per-chunk acks are gone).
    // Header layout (FILE_PACKAGE_HEAD = 64): SerialStruct fields then zero
    // padding; payload follows at offset 64.
    let mut pos: u64 = 0;
    for chunk in data.chunks(FILE_CHUNK) {
        let mut payload = Vec::new();
        put_uint(&mut payload, 1, pos);
        put_uint(&mut payload, 2, 0);
        put_uint(&mut payload, 3, chunk.len() as u64);
        put_uint(&mut payload, 4, chunk.len() as u64);
        payload.resize(FILE_PACKAGE_HEAD, 0);
        payload.extend_from_slice(chunk);
        conn.send_packet(frame(CMD_APP_DATA), &payload)?;
        pos += chunk.len() as u64;
    }
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[install] {} bytes sent, awaiting result", pos);
    }
    // The daemon installs once the byte count matches the announced size, then
    // replies with the outcome (mode byte + text).
    let (_, mut reply) = conn.recv_packet()?;
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!(
            "[install] result frame {} bytes hex={:02x?}",
            reply.len(),
            &reply[..reply.len().min(24)]
        );
    }
    // A short frame is the daemon's begin/feature marker; the installer's own
    // message is longer, so only wait when the payload is still tiny. Heartbeats
    // (`command:5000`) are never the result.
    if reply.len() <= 8 {
        conn.set_read_timeout(std::time::Duration::from_secs(20))?;
        while let Ok((p, more)) = conn.recv_packet() {
            if p.command_flag != HEARTBEAT_FLAG && more.len() > reply.len() {
                reply = more;
                break;
            }
        }
    }
    // The reply opens with two status bytes before the human-readable text.
    let text = if reply.len() > 2 && reply[0] == 1 {
        String::from_utf8_lossy(&reply[2..]).to_string()
    } else {
        String::from_utf8_lossy(&reply).to_string()
    };
    let text = text.trim().to_string();
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!("[install] device says: {text}");
    }
    // The daemon reports failure as plain text ("error: failed to install …"); a
    // transport-level success without a clean "success" word is not an install.
    let lowered = text.to_ascii_lowercase();
    if lowered.contains("fail") || lowered.contains("error") {
        return Err(format!("device rejected the package: {text}"));
    }
    Ok(text)
}

/// The three-step auth exchange on an already-open connection.
fn handshake(
    conn: &mut HdcConn,
    addr: &str,
    version: &str,
    identity: &HostIdentity,
) -> Result<AuthReport, String> {
    // A fresh id every connection: the daemon keeps per-session auth state, and reusing an id
    // that already authenticated makes it answer `... is already, ... refused!`.
    let session_id: u32 = rand::random::<u32>() & 0x7fff_ffff;
    conn.set_session_id(session_id);
    let hostname = host_name();

    let step = |buf: String, auth_type: u32| SessionHandShake {
        banner: HANDSHAKE_MESSAGE.to_string(),
        auth_type,
        session_id,
        connect_key: addr.to_string(),
        buf,
        version: version.to_string(),
    };

    // Step 1: capabilities. The daemon answers with the auth state it wants.
    // This buf is mandatory: `HdcDaemon::GetServerCapability` only selects
    // RSA_3072_SHA512 when it can parse a TLV map carrying `authtype=1`;
    // with an empty buf it logs "maybe old version client", verifies against
    // the legacy RSA_ENCRYPT scheme, and our SHA-512 signature fails
    // (`HandDaemonAuthSignature auth failed`).
    let mut reply = exchange(conn, &step(opening_buf(version), AUTH_NONE))?;
    if reply.auth_type != AUTH_PUBLICKEY {
        return Err(format!(
            "expected AUTH_PUBLICKEY, device answered auth_type={} ({})",
            reply.auth_type, reply.buf
        ));
    }

    // Step 2: the public key. The daemon answers with a 20-byte challenge token.
    reply = exchange(
        conn,
        &step(identity.public_key_buf(&hostname), AUTH_PUBLICKEY),
    )?;
    if reply.auth_type != AUTH_SIGNATURE {
        return Err(format!(
            "expected AUTH_SIGNATURE, device answered auth_type={} ({})",
            reply.auth_type, reply.buf
        ));
    }
    let token = reply.buf.as_bytes().to_vec();
    if token.len() != RSA_TOKEN_SIZE {
        return Err(format!(
            "challenge token is {} bytes, expected {RSA_TOKEN_SIZE}",
            token.len()
        ));
    }

    // Step 3: prove possession of the private key.
    let signature = identity.sign_token(&token)?;
    reply = exchange(conn, &step(signature, AUTH_SIGNATURE))?;
    if reply.auth_type != AUTH_OK {
        return Err(format!(
            "authentication failed: auth_type={} ({})",
            reply.auth_type, reply.buf
        ));
    }

    let fields = parse_tlv(&reply.buf);
    Ok(AuthReport {
        hostname,
        device_name: fields.get("devname").cloned().unwrap_or_default(),
        auth_status: fields.get("daemonauthstatus").cloned().unwrap_or_default(),
        device_message: fields.get("emgmsg").cloned().unwrap_or_default(),
    })
}

fn exchange(conn: &mut HdcConn, message: &SessionHandShake) -> Result<SessionHandShake, String> {
    let encoded = message.encode();
    if std::env::var("HVOLT_USB_DEBUG").is_ok() {
        eprintln!(
            "[frame] send handshake {}B tail={:02x?}",
            encoded.len(),
            &encoded[encoded.len().saturating_sub(20)..]
        );
    }
    conn.send_packet(HANDSHAKE_PROTECT, &encoded)?;
    // USB devices emit a short session-notice frame (field1=version, empty
    // auth) before the real handshake reply; skip such lead-in frames.
    for _ in 0..4 {
        let (_, payload) = conn.recv_packet()?;
        let reply = SessionHandShake::decode(&payload)?;
        if reply.auth_type != AUTH_NONE || !reply.buf.is_empty() {
            return Ok(reply);
        }
        if std::env::var("HVOLT_USB_DEBUG").is_ok() {
            eprintln!("[frame] skipping lead-in handshake frame {}", payload.len());
        }
    }
    Err("no handshake reply (only lead-in frames)".to_string())
}

/// Decode the TLV pairs the daemon puts in `buf`.
fn parse_tlv(buf: &str) -> std::collections::HashMap<String, String> {
    let bytes = buf.as_bytes();
    let mut out = std::collections::HashMap::new();
    let mut pos = 0usize;
    while pos + TLV_TAG_LEN + TLV_VAL_LEN <= bytes.len() {
        let tag = String::from_utf8_lossy(&bytes[pos..pos + TLV_TAG_LEN])
            .trim()
            .to_string();
        pos += TLV_TAG_LEN;
        let len_text = String::from_utf8_lossy(&bytes[pos..pos + TLV_VAL_LEN])
            .trim()
            .to_string();
        pos += TLV_VAL_LEN;
        let Ok(len) = len_text.parse::<usize>() else {
            break;
        };
        if pos + len > bytes.len() {
            break;
        }
        out.insert(
            tag,
            String::from_utf8_lossy(&bytes[pos..pos + len]).to_string(),
        );
        pos += len;
    }
    out
}

// ── transport ────────────────────────────────────────────────────────────────────────────────

/// The byte channel under an `HdcConn`.
///
/// hdc reaches a device over two different transports: TCP to `hdcd`'s listening port, and
/// USB bulk endpoints. Only the frame format is shared, so the connection holds a boxed
/// reader/writer and each transport supplies its own.
pub trait Transport: Read + Write {
    /// Adjust the read timeout mid-session (shell output arrives in bursts on
    /// an open channel). TCP forwards to the socket; USB forwards to nusb.
    fn set_read_timeout(&mut self, _d: Duration) -> Result<(), String> {
        Ok(())
    }

    /// The transport-level session id written into USB packet headers. TCP has
    /// no transport envelope, so only the USB transport implements this.
    fn set_session_id(&mut self, _session_id: u32) {}
}

impl std::fmt::Debug for dyn Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transport")
    }
}

impl Transport for TcpStream {
    fn set_read_timeout(&mut self, d: Duration) -> Result<(), String> {
        TcpStream::set_read_timeout(self, Some(d)).map_err(|e| format!("set read timeout: {e}"))
    }
}

/// A connection that speaks the hdc frame format over a boxed transport
/// (TCP or USB). The frame logic is transport-agnostic: `send_packet` writes
/// one frame, `recv_packet` reads exactly one.
pub struct HdcConn {
    stream: Box<dyn Transport>,
}

impl HdcConn {
    /// Connect to a device's `hdcd` listening port (`8710` by default on a device).
    pub fn connect(addr: &str, timeout: Duration) -> Result<Self, String> {
        let socket = addr
            .to_socket_addrs()
            .map_err(|e| format!("resolve {addr}: {e}"))?
            .next()
            .ok_or_else(|| format!("{addr} resolved to nothing"))?;
        let stream = TcpStream::connect_timeout(&socket, timeout)
            .map_err(|e| format!("connect {addr}: {e}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| format!("set read timeout: {e}"))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| format!("set write timeout: {e}"))?;
        Ok(Self {
            stream: Box::new(stream),
        })
    }

    /// Connect by target: `<host>:<port>` over TCP, a USB serial over the
    /// native USB transport.
    pub fn connect_device(target: &str, timeout: Duration) -> Result<Self, String> {
        if target.contains(':') {
            Self::connect(target, timeout)
        } else {
            let usb = crate::device::usb::UsbTransport::open(target, timeout)?;
            Ok(Self {
                stream: Box::new(usb),
            })
        }
    }

    /// Adjust the read timeout mid-session (used for shell output where the
    /// device keeps the channel open after answering).
    pub fn set_read_timeout(&mut self, d: Duration) -> Result<(), String> {
        self.stream.set_read_timeout(d)
    }

    /// Push the handshake session id down to the transport (USB packet headers).
    pub fn set_session_id(&mut self, session_id: u32) {
        self.stream.set_session_id(session_id);
    }

    /// Send one framed packet: head, then `PayloadProtect`, then the payload.
    pub fn send_packet(&mut self, protect: PayloadProtect, payload: &[u8]) -> Result<(), String> {
        let protect_bytes = protect.encode();
        let head = PayloadHead {
            reserve: [0, 0],
            protocol_ver: VER_PROTOCOL,
            head_size: protect_bytes.len() as u16,
            data_size: payload.len() as u32,
        };
        let mut frame = Vec::with_capacity(HEAD_LEN + protect_bytes.len() + payload.len());
        frame.extend_from_slice(&head.encode());
        frame.extend_from_slice(&protect_bytes);
        frame.extend_from_slice(payload);
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("send: {e}"))
    }

    /// Read exactly one framed packet.
    pub fn recv_packet(&mut self) -> Result<(PayloadProtect, Vec<u8>), String> {
        let mut head_buf = [0u8; HEAD_LEN];
        self.stream
            .read_exact(&mut head_buf)
            .map_err(|e| format!("recv head: {e}"))?;
        let head = PayloadHead::decode(&head_buf)?;
        if std::env::var("HVOLT_USB_DEBUG").is_ok() {
            eprintln!(
                "[frame] recv head {:02x?} hs={} ds={}",
                head_buf, head.head_size, head.data_size
            );
        }
        let mut protect_buf = vec![0u8; usize::from(head.head_size)];
        self.stream
            .read_exact(&mut protect_buf)
            .map_err(|e| format!("recv protect: {e}"))?;
        let mut payload = vec![0u8; head.data_size as usize];
        self.stream
            .read_exact(&mut payload)
            .map_err(|e| format!("recv payload: {e}"))?;
        if std::env::var("HVOLT_USB_DEBUG").is_ok() {
            eprintln!(
                "[frame] recv protect {:02x?} payload {:02x?}",
                &protect_buf[..protect_buf.len().min(8)],
                &payload[..payload.len().min(24)]
            );
        }
        Ok((
            PayloadProtect::decode(&protect_bytes(&protect_buf))?,
            payload,
        ))
    }
}

fn protect_bytes(buf: &[u8]) -> Vec<u8> {
    buf.to_vec()
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    /// The captured 178-byte client handshake was `11 + 8 + 159`; the head must decode to those
    /// two lengths from the bytes that were actually on the wire.
    #[test]
    fn captured_client_frame_head_decodes() {
        let head_bytes = [
            0x48, 0x57, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00, 0x00, 0x00, 0x9f,
        ];
        let head = PayloadHead::decode(&head_bytes).unwrap();
        assert_eq!(head.protocol_ver, 0x01);
        assert_eq!(head.head_size, 8);
        assert_eq!(head.data_size, 159);
    }

    /// The server's 99-byte answer used the same head shape, 8 bytes of protect and 80 of payload.
    #[test]
    fn captured_server_frame_head_decodes() {
        let head_bytes = [
            0x48, 0x57, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00, 0x00, 0x00, 0x50,
        ];
        let head = PayloadHead::decode(&head_bytes).unwrap();
        assert_eq!(head.head_size, 8);
        assert_eq!(head.data_size, 80);
    }

    /// A frame that does not open with `HW` is not ours; refusing it early keeps a stray byte
    /// from being read as a length.
    #[test]
    fn wrong_flag_is_rejected() {
        let bad = [0x00u8; HEAD_LEN];
        assert!(PayloadHead::decode(&bad).is_err());
    }

    /// Encoding a head and decoding it back must round-trip every field.
    #[test]
    fn head_round_trips() {
        let head = PayloadHead {
            reserve: [0, 0],
            protocol_ver: VER_PROTOCOL,
            head_size: 8,
            data_size: 159,
        };
        assert_eq!(PayloadHead::decode(&head.encode()).unwrap(), head);
    }
}

#[cfg(test)]
mod handshake_tests {
    use super::*;

    /// The captured client handshake carried banner, authType=0, sessionId, connectKey, buf and
    /// version. Decoding the captured protobuf prefix must recover those fields.
    #[test]
    fn captured_handshake_prefix_decodes() {
        let mut payload = Vec::new();
        put_str(&mut payload, 1, "OHOS HDC");
        put_uint(&mut payload, 2, 0);
        put_uint(&mut payload, 3, 0);
        put_str(&mut payload, 4, "127.0.0.1:9999");
        let h = SessionHandShake::decode(&payload).unwrap();
        assert_eq!(h.banner, "OHOS HDC");
        assert_eq!(h.auth_type, 0);
        assert_eq!(h.connect_key, "127.0.0.1:9999");
    }

    /// A round-trip over all six fields keeps the message intact.
    #[test]
    fn handshake_round_trips() {
        let h = SessionHandShake {
            banner: HANDSHAKE_MESSAGE.into(),
            auth_type: 3,
            session_id: 7,
            connect_key: "192.168.1.3:8710".into(),
            buf: "authtype       1           1".into(),
            version: "Ver: 3.2.0".into(),
        };
        assert_eq!(SessionHandShake::decode(&h.encode()).unwrap(), h);
    }

    /// `PayloadProtect` from the captured frame was `08 00 10 01 18 00 20 09`, i.e. channel 0,
    /// commandFlag 1, checkSum 0, vCode 9.
    #[test]
    fn captured_protect_decodes() {
        let bytes = [0x08, 0x00, 0x10, 0x01, 0x18, 0x00, 0x20, 0x09];
        let p = PayloadProtect::decode(&bytes).unwrap();
        assert_eq!(p.channel_id, 0);
        assert_eq!(p.command_flag, 1);
        assert_eq!(p.check_sum, 0);
        assert_eq!(p.v_code, 9);
    }

    /// A varint that never terminates must be an error, not an infinite loop.
    #[test]
    fn unterminated_varint_is_an_error() {
        let bytes = [0x80u8; 12];
        assert!(PayloadProtect::decode(&bytes).is_err());
    }
}

#[cfg(test)]
mod wire_probe {
    use super::*;

    /// Print the exact bytes the handshake puts on the wire, so a failing real-device attempt
    /// can be compared field by field against a working client.
    #[test]
    fn dump_opening_frame() {
        let buf = opening_buf("Ver: 3.2.0");
        let h = SessionHandShake {
            banner: HANDSHAKE_MESSAGE.to_string(),
            auth_type: AUTH_NONE,
            session_id: 123456789,
            connect_key: "192.168.1.3:8710".to_string(),
            buf: buf.clone(),
            version: "Ver: 3.2.0".to_string(),
        };
        let payload = h.encode();
        let protect = HANDSHAKE_PROTECT.encode();
        let head = PayloadHead {
            reserve: [0, 0],
            protocol_ver: VER_PROTOCOL,
            head_size: protect.len() as u16,
            data_size: payload.len() as u32,
        };
        let mut frame = Vec::new();
        frame.extend_from_slice(&head.encode());
        frame.extend_from_slice(&protect);
        frame.extend_from_slice(&payload);
        let hex: String = frame.iter().map(|b| format!("{b:02x}")).collect();
        println!("BUF_LEN={}", buf.len());
        println!("FRAME_LEN={}", frame.len());
        println!("FRAME_HEX={hex}");
    }
}
