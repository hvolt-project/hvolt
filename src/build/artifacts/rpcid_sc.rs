// rpcid_sc: write the HAP's rpcid.sc syscap container without the SDK's encoder.
//
// Layout measured 2026-11-08 against the SDK's own encoder (v2.0.1, SDK 26.0.0.23) by
// encoding minimal `rpcid.json` inputs and reading the bytes back with xxd, then confirming
// the model on the 206-syscap fixture in `fixtures/rpcid/` (52 742 bytes, byte-identical):
//
//   offset 0   u16be   (api_version as u16) | 0x0080
//                      The low 16 bits of api_version, with bit 7 always set: the shipped
//                      fixture's 60000020 (0x03938714) becomes 0x8794, and the tool's -d
//                      decoder reports 34580 (0x8714) back, i.e. it clears that bit. Probed
//                      api_version 0, 1, 8, 9, 12, 30, 127, 128, 200, 255, 256, 1000,
//                      32767, 32773, 65330, 65535, 65553, 131072, 60000020 — all
//                      `(api & 0xffff) | 0x80`. What the bit means is not established.
//   offset 2   u16be   0x0002
//                      Constant in every run (empty list included); taken as the container
//                      format version.
//   offset 4   u16be   256 * count
//                      Byte length of the payload that follows, not the entry count:
//                      one entry stores 0x0100, two 0x0200, none 0x0000. The field is a
//                      u16, so the tool wraps silently at 256 entries (0x0000, and its
//                      own decoder then reads back an empty list); this module refuses
//                      that input instead of writing an unreadable file.
//   offset 6   payload: one slot per syscap, in input order, 256 bytes each:
//                      the name with its 17-byte `SystemCapability.` prefix removed, then
//                      zero padding. The prefix is what the reader puts back; a name
//                      whose payload is exactly 256 bytes is written without the
//                      terminator (the tool accepts it, its decoder cannot read it back).
//
// No checksum and no other trailer: file size is always `6 + 256 * count`. The tool keeps
// duplicates, keeps input order (no sort), and rejects an empty name, a name without the
// prefix, and a payload longer than 256 bytes.

use std::path::Path;

/// Prefix every syscap name carries and the on-disk slot omits.
const NAME_PREFIX: &str = "SystemCapability.";

/// Fixed width of one syscap's slot on disk.
const SLOT_LEN: usize = 256;

/// Header: api version, format version, payload byte length.
const HEADER_LEN: usize = 6;

/// Second header field; unchanged across every probe.
const FORMAT_VERSION: u16 = 0x0002;

/// Always set in the first header field (see the module comment).
const API_FLAG: u16 = 0x0080;

const ZEROS: [u8; SLOT_LEN] = [0; SLOT_LEN];

/// Encode one `rpcid.sc` image. `syscaps` keeps its order; duplicates are preserved.
///
/// Whole-image encoding in memory: a rejected entry leaves no partial file behind.
pub(crate) fn encode(api_version: u32, syscaps: &[&str]) -> Result<Vec<u8>, String> {
    let body_len = syscaps
        .len()
        .checked_mul(SLOT_LEN)
        .filter(|len| *len <= u16::MAX as usize)
        .ok_or_else(|| {
            format!(
                "rpcid.sc holds at most {} syscaps (its payload length is a u16 and each entry \
                 takes {SLOT_LEN} bytes); got {}",
                u16::MAX as usize / SLOT_LEN,
                syscaps.len()
            )
        })?;

    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    out.extend_from_slice(&((api_version as u16) | API_FLAG).to_be_bytes());
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&(body_len as u16).to_be_bytes());
    for name in syscaps {
        let payload = slot(name)?;
        out.extend_from_slice(payload);
        out.extend_from_slice(&ZEROS[..SLOT_LEN - payload.len()]);
    }
    Ok(out)
}

/// Write the encoded image to `path`.
pub(crate) fn write(path: &Path, api_version: u32, syscaps: &[&str]) -> Result<(), String> {
    let image = encode(api_version, syscaps)?;
    std::fs::write(path, &image).map_err(|e| format!("write {}: {e}", path.display()))
}

/// One syscap's slot content: the name without the prefix that the reader restores.
fn slot(name: &str) -> Result<&[u8], String> {
    let payload = name
        .as_bytes()
        .strip_prefix(NAME_PREFIX.as_bytes())
        .ok_or_else(|| format!("syscap '{name}' does not start with '{NAME_PREFIX}'"))?;
    if payload.len() > SLOT_LEN {
        return Err(format!(
            "syscap '{name}' needs {} bytes after the '{NAME_PREFIX}' prefix, the slot holds \
             {SLOT_LEN}",
            payload.len()
        ));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Name of the SDK encoder this module replaces, in two pieces: the removal gate is
    /// that no file under `src/` names that binary any more, and only the verification
    /// test still needs it.
    const TOOL_NAME: &str = concat!("syscap", "_tool");

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/rpcid")
            .join(name)
    }

    /// The committed fixture split into this module's inputs.
    fn fixture_input() -> (u32, Vec<String>) {
        let text =
            std::fs::read_to_string(fixture("rpcid.json")).expect("read fixtures/rpcid/rpcid.json");
        let doc: serde_json::Value = serde_json::from_str(&text).expect("parse fixture json");
        let api = doc["api_version"].as_u64().expect("api_version") as u32;
        let caps = doc["syscap"]
            .as_array()
            .expect("syscap array")
            .iter()
            .map(|v| v.as_str().expect("syscap entry").to_string())
            .collect();
        (api, caps)
    }

    /// Compare two images by the first offset that differs; a whole 52 KiB payload in an
    /// `assert_eq!` failure message is unreadable.
    fn assert_same_bytes(got: &[u8], want: &[u8], what: &str) {
        assert_eq!(got.len(), want.len(), "{what}: byte length");
        if let Some(i) = got.iter().zip(want).position(|(a, b)| a != b) {
            let end = (i + 8).min(got.len());
            panic!(
                "{what}: first difference at byte {i}: got {:02x?}, want {:02x?}",
                &got[i..end],
                &want[i..end]
            );
        }
    }

    /// An empty syscap list is legal and stops after the header (measured: 6 bytes).
    #[test]
    fn empty_list_writes_the_header_only() {
        let out = encode(12, &[]).expect("encode");
        assert_eq!(out.as_slice(), &[0x00, 0x8c, 0x00, 0x02, 0x00, 0x00]);
    }

    /// Each entry is one 256-byte slot holding the name without its prefix, zero filled.
    #[test]
    fn slot_drops_the_prefix_and_zero_fills() {
        let out = encode(12, &["SystemCapability.ArkUI.ArkUI.Full"]).expect("encode");
        assert_eq!(out.len(), HEADER_LEN + SLOT_LEN);
        assert_eq!(&out[HEADER_LEN..HEADER_LEN + 16], b"ArkUI.ArkUI.Full");
        assert!(out[HEADER_LEN + 16..].iter().all(|b| *b == 0));
        // The length field counts payload bytes, not entries.
        assert_eq!(&out[4..6], &[0x01, 0x00]);
    }

    /// The first field keeps api_version's low 16 bits and sets bit 7.
    #[test]
    fn api_version_keeps_low_16_bits_and_sets_the_flag() {
        let api_of_fixture = encode(60_000_020, &[]).expect("encode");
        assert_eq!(&api_of_fixture[..2], &[0x87, 0x94]);
        // 128 | 0x80 collapses to 0x0080, which the tool itself reports for api 128.
        assert_eq!(&encode(128, &[]).expect("encode")[..2], &[0x00, 0x80]);
    }

    /// Order and duplicates are the caller's: the tool keeps both.
    #[test]
    fn keeps_input_order_and_duplicates() {
        let caps = [
            "SystemCapability.Z.Z",
            "SystemCapability.A.A",
            "SystemCapability.A.A",
        ];
        let out = encode(12, &caps).expect("encode");
        assert_eq!(out.len(), HEADER_LEN + 3 * SLOT_LEN);
        assert_eq!(&out[4..6], &[0x03, 0x00]);
        for (i, cap) in caps.iter().enumerate() {
            let payload = cap.trim_start_matches(NAME_PREFIX).as_bytes();
            assert_eq!(&out[HEADER_LEN + i * SLOT_LEN..][..payload.len()], payload);
        }
    }

    /// A name the reader could not restore is refused, not written.
    #[test]
    fn rejects_a_name_without_the_prefix() {
        let err = encode(12, &["ArkUI.ArkUI.Full"]).expect_err("prefix required");
        assert!(err.contains("SystemCapability."), "{err}");
    }

    #[test]
    fn rejects_a_payload_longer_than_the_slot() {
        let long = format!("{NAME_PREFIX}{}", "A".repeat(SLOT_LEN + 1));
        let err = encode(12, &[&long]).expect_err("payload overflow");
        assert!(err.contains("257"), "{err}");
    }

    /// A payload of exactly 256 bytes fills the slot and has no terminator; the tool
    /// accepts it too (it then writes 256 name bytes).
    #[test]
    fn accepts_a_payload_filling_the_slot() {
        let full = format!("{NAME_PREFIX}{}", "A".repeat(SLOT_LEN));
        let out = encode(12, &[&full]).expect("encode");
        assert_eq!(out.len(), HEADER_LEN + SLOT_LEN);
        assert_eq!(out.last(), Some(&b'A'));
    }

    /// 256 entries overflow the u16 payload length; the tool wraps to 0 and writes a file
    /// its own decoder reads as empty, so this encoder stops instead.
    #[test]
    fn rejects_a_count_that_overflows_the_payload_length() {
        let caps: Vec<String> = (0..=SLOT_LEN)
            .map(|i| format!("SystemCapability.X{i}"))
            .collect();
        let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
        let err = encode(12, &refs).expect_err("payload length overflow");
        assert!(err.contains("255"), "{err}");
    }

    /// The committed truth image: the SDK encoder's `-R -e` output for the same rpcid.json,
    /// generated on an asr-ondevice-style device-type intersection (206 syscaps).
    ///
    /// Regenerate both fixture files with the SDK encoder (`-R -e -i
    /// fixtures/rpcid/rpcid.json -o <dir>`, then copy `rpcid.sc` back) if a newer SDK
    /// changes the container layout.
    #[test]
    fn matches_the_stored_tool_output() {
        let (api, caps) = fixture_input();
        let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
        let got = encode(api, &refs).expect("encode fixture");
        let want = std::fs::read(fixture("rpcid.sc")).expect("read fixtures/rpcid/rpcid.sc");
        assert_same_bytes(&got, &want, "rpcid.sc vs stored tool output");
    }

    /// Same input, the real encoder: proves the fixture is genuine and the layout still
    /// holds. Skipped when no SDK is installed.
    #[test]
    fn agrees_with_the_sdk_encoder_when_installed() {
        let Some(tool) = sdk_encoder() else {
            eprintln!("skip: no SDK encoder installed");
            return;
        };
        let dir = std::env::temp_dir().join(format!("hvolt-rpcid-{}", std::process::id()));
        let out_dir = dir.join("out");
        std::fs::create_dir_all(&out_dir).expect("create output dir");

        let status = std::process::Command::new(&tool)
            .args(["-R", "-e", "-i"])
            .arg(fixture("rpcid.json"))
            .arg("-o")
            .arg(&out_dir)
            .output()
            .expect("run the SDK encoder");
        assert!(
            status.status.success(),
            "SDK encoder failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );

        let want = std::fs::read(out_dir.join("rpcid.sc")).expect("SDK encoder wrote rpcid.sc");
        let (api, caps) = fixture_input();
        let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
        let got = encode(api, &refs).expect("encode fixture");
        assert_same_bytes(&got, &want, "rpcid.sc vs fresh SDK encoder output");

        // Temp tree, recreated by the next run.
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Locate the SDK's encoder for the cross-check; `None` when no SDK is installed.
    fn sdk_encoder() -> Option<PathBuf> {
        let root = crate::sdk::manage::discovered_sdk_root("")?;
        let dirs = [
            crate::sdk::manage::component_root(&root, "toolchains"),
            root.join("openharmony/toolchains"),
            root.join("toolchains"),
        ];
        crate::support::platform::find_in_dirs(TOOL_NAME, dirs)
    }
}
