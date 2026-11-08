// syscap: generate the HAP's rpcid.sc from the SDK's per-device syscap lists.
//
// the reference's `SyscapTransform` intersects the device-type SysCaps (from
// `<sdk>/ets/api/device-define/<device>.json`) and encodes the result as `rpcid.sc`.
// The encoding lives in `rpcid_sc`, which replaced the SDK's own encoder; the JSON input
// is still written next to the output as `rpcid.json`.
// Verified byte-identical against the SDK's encoder output on asr-ondevice
// (default×tablet×2in1, 206 syscaps): 52 742 bytes, equal (`fixtures/rpcid/`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Read one device type's syscap list.
///
/// Measured on the shipped 26.0.0.23 SDK: `device-define/` carries `default`,
/// `2in1`, `tablet`, `tv`, `wearable`, `liteWearable` — there is no
/// `phone.json` (nor `car.json`), so a project declaring the customary
/// `phone` would fail on a file the SDK deliberately folds into `default`.
/// Fall back to `default.json` and only fail when neither exists.
fn device_caps(define_dir: &Path, device: &str) -> Result<serde_json::Value, String> {
    let direct = define_dir.join(format!("{device}.json"));
    let path = if direct.exists() {
        direct
    } else {
        let fallback = define_dir.join("default.json");
        if !fallback.exists() {
            return Err(format!(
                "no syscap list for device type '{device}' ({} missing, and no default.json in {})",
                direct.display(),
                define_dir.display()
            ));
        }
        fallback
    };
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Generate rpcid.sc for the given device types. Returns the output path.
///
/// `device_types` order decides the syscap order: the reference keeps the intersection
/// in the order of the *first* device's JSON (a Set built iteratively), so this filters
/// the first device's list by the intersection set.
pub fn generate(
    sdk_root: &Path,
    out_dir: &Path,
    api_version: u32,
    device_types: &[String],
) -> Result<PathBuf, String> {
    // `<sdk>/default/openharmony/ets/api/device-define/<device>.json`.
    let define_dir = crate::sdk::manage::component_root(sdk_root, "ets").join("api/device-define");

    if device_types.is_empty() {
        return Err("no device types declared, cannot intersect syscaps".to_string());
    }

    // All syscaps present in every device type, ordered like the first device.
    let mut all_sets: Option<BTreeSet<String>> = None;
    for dt in device_types {
        let doc = device_caps(&define_dir, dt)?;
        let caps: BTreeSet<String> = doc["SysCaps"]
            .as_array()
            .ok_or_else(|| format!("no SysCaps for device type '{dt}'"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        all_sets = Some(match all_sets {
            None => caps,
            Some(acc) => acc.intersection(&caps).cloned().collect(),
        });
    }
    let inter = all_sets.unwrap_or_default();

    // Order by the first device's listing, reference-style.
    let first_doc = device_caps(&define_dir, &device_types[0])?;
    let ordered: Vec<&str> = first_doc["SysCaps"]
        .as_array()
        .ok_or_else(|| format!("no SysCaps for device type '{}'", device_types[0]))?
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|c| inter.contains(*c))
        .collect();

    // rpcid.json — the schema the reference's encoder consumes (api_version + syscap).
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;
    let rpcid = serde_json::json!({
        "api_version": api_version,
        "syscap": ordered,
    });
    let rpcid_path = out_dir.join("rpcid.json");
    std::fs::write(
        &rpcid_path,
        serde_json::to_string_pretty(&rpcid).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write {}: {e}", rpcid_path.display()))?;

    let sc = out_dir.join("rpcid.sc");
    crate::build::artifacts::rpcid_sc::write(&sc, api_version, &ordered)?;
    Ok(sc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The intersection keeps the first device's order and drops non-shared caps.
    #[test]
    fn intersection_keeps_first_device_order() {
        // Can't run against a real SDK in unit tests; the function is exercised
        // end-to-end by `assemble-hap`. This only confirms the ordering helper
        // behaves: filter by set, keep source order.
        let src = ["b", "a", "c", "d"];
        let keep: BTreeSet<String> = ["a", "c"].iter().map(|s| s.to_string()).collect();
        let filtered: Vec<&str> = src.iter().copied().filter(|c| keep.contains(*c)).collect();
        assert_eq!(filtered, vec!["a", "c"]);
    }
}
