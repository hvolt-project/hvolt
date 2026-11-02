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
/// `CMD_UNITY_EXECUTE` — run a shell command (`define_enum.h`). The payload is the command
/// string itself; the reply streams back on the channel named in the frame head.
pub const CMD_UNITY_EXECUTE: u16 = 1001;
/// `v_code` every frame after the handshake carries (captured).
pub const V_CODE: u32 = 9;
/// `CMD_KERNEL_HANDSHAKE` — the confirmation a host sends once authenticated (`cmd=2`).
pub const CMD_KERNEL_HANDSHAKE: u16 = 2;
/// Opens a data channel before a file transfer (captured: `cmd=12`).
pub const CMD_CHANNEL_OPEN: u16 = 12;
/// `CMD_FILE_INIT` — announce a transfer with its `TransferConfig` (`define_enum.h`).
pub const CMD_FILE_INIT: u16 = 3001;
/// `CMD_FILE_DATA` — one `TransferPayload` header followed by raw file bytes.
pub const CMD_FILE_DATA: u16 = 3003;
/// `CMD_FILE_FINISH` — close the transfer.
pub const CMD_FILE_FINISH: u16 = 3004;
/// `CMD_APP_CHECK` — announce an install with its `TransferConfig` (`define_enum.h`).
pub const CMD_APP_CHECK: u16 = 3501;
/// `CMD_APP_DATA` — the package bytes.
pub const CMD_APP_DATA: u16 = 3503;
/// Reserved bytes between a `TransferPayload` header and the file body (captured: 8 + 56 + 17).
const FILE_DATA_RESERVED: usize = 56;
/// Bytes of file content per data frame. The daemon caps a single IO buffer at 511 KB
/// (`define.h: MAX_SIZE_IOBUF`); staying well under it keeps one frame within one read.
const FILE_CHUNK: usize = 256 * 1024;

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
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .map_err(|e| e.to_string())
        })
        .unwrap_or_else(|_| "localhost".to_string())
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
    let mut conn = HdcConn::connect(addr, timeout)?;
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
    let mut conn = HdcConn::connect(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)?;
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let protect = PayloadProtect {
        channel_id: channel,
        command_flag: CMD_UNITY_EXECUTE,
        check_sum: 0,
        v_code: V_CODE,
    };
    conn.send_packet(protect, command.as_bytes())?;
    let mut output = String::new();
    loop {
        match conn.recv_packet() {
            Ok((p, payload)) if p.channel_id == channel => {
                output.push_str(&String::from_utf8_lossy(&payload));
            }
            Ok(_) => continue,
            Err(_) => break,
        }
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
    let name = local_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let mut conn = HdcConn::connect(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)?;
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let frame = |cmd: u16| PayloadProtect {
        channel_id: channel,
        command_flag: cmd,
        check_sum: 0,
        v_code: V_CODE,
    };

    // The device answers every step below; skipping a read leaves it discarding the next frame,
    // which is exactly how a transfer silently lands as a zero-byte file.
    conn.send_packet(frame(CMD_KERNEL_HANDSHAKE), &[0])?;
    let _ = conn.recv_packet()?;
    conn.send_packet(frame(CMD_CHANNEL_OPEN), &[])?;

    // TransferConfig: only the size, the destination path and the display name are meaningful
    // here; every other field was zero in the captured frame.
    let mut config = Vec::new();
    put_uint(&mut config, 1, data.len() as u64);
    put_uint(&mut config, 2, 0);
    put_uint(&mut config, 3, 0);
    put_str(&mut config, 4, "");
    put_str(&mut config, 5, remote_path);
    put_str(&mut config, 6, &name);
    put_uint(&mut config, 7, 0);
    put_uint(&mut config, 8, 0);
    put_uint(&mut config, 9, 0);
    put_str(&mut config, 10, "");
    put_str(&mut config, 11, "");
    conn.send_packet(frame(CMD_FILE_INIT), &config)?;
    let _ = conn.recv_packet()?;

    // TransferPayload: four varint fields, then a 56-byte reserved block, then the bytes.
    // The reserved block is what makes the header 64 bytes in the captured frame
    // (8 + 56 + file size = frame data size); omitting it leaves the device writing an empty file.
    let mut payload = Vec::new();
    put_uint(&mut payload, 1, 0);
    put_uint(&mut payload, 2, 0);
    put_uint(&mut payload, 3, data.len() as u64);
    put_uint(&mut payload, 4, data.len() as u64);
    payload.resize(payload.len() + FILE_DATA_RESERVED, 0);
    payload.extend_from_slice(&data);
    conn.send_packet(frame(CMD_FILE_DATA), &payload)?;
    let _ = conn.recv_packet()?;

    conn.send_packet(frame(CMD_FILE_FINISH), &[0])?;
    let _ = conn.recv_packet()?;
    Ok(())
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
    let mut conn = HdcConn::connect(addr, timeout)?;
    handshake(&mut conn, addr, version, identity)?;
    let channel = rand::random::<u32>() & 0x7fff_ffff;
    let frame = |cmd: u16| PayloadProtect {
        channel_id: channel,
        command_flag: cmd,
        check_sum: 0,
        v_code: V_CODE,
    };

    conn.send_packet(frame(CMD_KERNEL_HANDSHAKE), &[0])?;
    let _ = conn.recv_packet()?;
    conn.send_packet(frame(CMD_CHANNEL_OPEN), &[])?;

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
    conn.send_packet(frame(CMD_APP_CHECK), &config)?;
    let _ = conn.recv_packet()?;

    // Stream the package in bounded chunks, waiting for each acknowledgement: one oversized
    // frame makes the device reset the connection (measured with a 46 MB package).
    let mut reply = Vec::new();
    for (index, chunk) in data.chunks(FILE_CHUNK).enumerate() {
        let mut payload = Vec::new();
        put_uint(&mut payload, 1, index as u64);
        put_uint(&mut payload, 2, 0);
        put_uint(&mut payload, 3, chunk.len() as u64);
        put_uint(&mut payload, 4, chunk.len() as u64);
        payload.resize(payload.len() + FILE_DATA_RESERVED, 0);
        payload.extend_from_slice(chunk);
        conn.send_packet(frame(CMD_APP_DATA), &payload)?;
        let (_, ack) = conn.recv_packet()?;
        reply = ack;
    }
    // The reply opens with two status bytes before the human-readable text.
    let text = if reply.len() > 2 && reply[0] == 1 {
        String::from_utf8_lossy(&reply[2..]).to_string()
    } else {
        String::from_utf8_lossy(&reply).to_string()
    };
    Ok(text.trim().to_string())
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
    conn.send_packet(HANDSHAKE_PROTECT, &message.encode())?;
    let (_, payload) = conn.recv_packet()?;
    SessionHandShake::decode(&payload)
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

/// A connection that speaks the hdc frame format.
pub struct HdcConn {
    stream: TcpStream,
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
        Ok(Self { stream })
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
        let mut protect_buf = vec![0u8; usize::from(head.head_size)];
        self.stream
            .read_exact(&mut protect_buf)
            .map_err(|e| format!("recv protect: {e}"))?;
        let mut payload = vec![0u8; head.data_size as usize];
        self.stream
            .read_exact(&mut payload)
            .map_err(|e| format!("recv payload: {e}"))?;
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
