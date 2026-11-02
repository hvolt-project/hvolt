// hap: HAP file introspection utilities.
//
// Currently: extract aclPermissions from unsigned HAP's module.json.

use std::fs;
use std::io::Read;
use std::path::Path;

/// Extract aclPermissions from an unsigned HAP's module.json.
#[cfg_attr(not(feature = "agc-local"), allow(dead_code))] // used by the optional AGC backend
pub fn extract_permissions(hap_path: &Path) -> Vec<String> {
    if !hap_path.exists() {
        return vec![];
    }
    let Ok(file) = fs::File::open(hap_path) else {
        return vec![];
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return vec![];
    };
    let Ok(mut entry) = archive.by_name("module.json") else {
        return vec![];
    };
    let mut json_str = String::new();
    if entry.read_to_string(&mut json_str).is_err() {
        return vec![];
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) else {
        return vec![];
    };
    let perms = match json["module"]["requestPermissions"].as_array() {
        Some(arr) => arr,
        None => return vec![],
    };
    perms
        .iter()
        .filter_map(|p| p["name"].as_str().map(|s| s.to_string()))
        .collect()
}

// ─── package stat report (app_check_tool stat.json) ─────────────────

/// Parsed `stat.json` from app_check_tool (package-side quality data).
#[derive(Debug, PartialEq)]
pub struct PackageStat {
    /// byte-size threshold the scan ran with, if --stat-file-size was given
    pub size_limit: Option<u64>,
    /// entries over the threshold: (path, bytes) desc by size
    pub over_limit: Vec<(String, u64)>,
    /// duplicate file groups: (pathA, [pathB...], bytes)
    pub duplicates: Vec<(String, Vec<String>, u64)>,
    /// per-suffix totals: (suffix, total bytes, count) desc by total
    pub suffixes: Vec<(String, u64, usize)>,
}

// ─── hap info (pure Rust, no java needed) ───────────────────────────

/// Read the module.json embedded in a (signed or unsigned) HAP and print
/// identity facts straight from the package — the acceptance discipline
/// says trust the package, not build logs.
/// The manifest a package carries: `module.json`, or `bundle-info` in the older layout.
///
/// Both readers below need it, so the archive walk lives here once.
fn read_manifest(hap_path: &Path) -> Result<(serde_json::Value, usize), String> {
    let file = fs::File::open(hap_path).map_err(|e| {
        if hap_path.exists() {
            format!("open {}: {}", hap_path.display(), e)
        } else {
            format!(
                "{} not found — pass a .hap path or run `hvolt build` first",
                hap_path.display()
            )
        }
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {}", e))?;
    let name = ["module.json", "bundle-info"]
        .iter()
        .find(|n| archive.by_name(n).is_ok())
        .ok_or("module.json not found in hap".to_string())?
        .to_string();
    let n_entries = archive.len();
    let mut entry = archive.by_name(&name).map_err(|e| e.to_string())?;
    let mut raw = String::new();
    entry.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let j: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("module.json: {}", e))?;
    Ok((j, n_entries))
}

/// bundleName as the package itself declares it.
///
/// Read from the package rather than from a project file: a package that came from elsewhere
/// must have its profile bound to its own identity, and there is no other source for it.
pub fn bundle_name(hap_path: &Path) -> Result<String, String> {
    let (j, _) = read_manifest(hap_path)?;
    let bundle = j["app"]["bundleName"]
        .as_str()
        .or_else(|| j["bundleName"].as_str())
        .or_else(|| j["bundle"]["name"].as_str())
        .ok_or_else(|| "bundleName not found in module.json".to_string())?;
    Ok(bundle.to_string())
}

/// Whether the package already carries a signature block.
///
/// Judged from the archive, not from the file name: an unsigned package may well be named
/// `app-signed.hap`, and refusing to sign it on the strength of its name would be wrong.
pub fn is_signed(hap_path: &Path) -> Result<bool, String> {
    let file =
        fs::File::open(hap_path).map_err(|e| format!("open {}: {}", hap_path.display(), e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip: {}", e))?;
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let name = entry.name().to_ascii_lowercase();
        if name.starts_with("meta-inf/")
            && (name.ends_with(".sig") || name.ends_with(".rsa") || name.ends_with(".ec"))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn hap_info(hap_path: &Path) -> Result<String, String> {
    let (j, n_entries) = read_manifest(hap_path)?;
    Ok(render_hap_info(&j, n_entries))
}

/// Render from the two-domain layout a signed HAP's module.json actually has:
/// app.{bundleName,versionCode,versionName,minAPIVersion,targetAPIVersion,
/// apiReleaseType} + module.{name,type,deviceTypes,mainElement,abilities,...}.
/// Tolerates the unsigned/older shapes by falling back key by key.
pub(crate) fn render_hap_info(j: &serde_json::Value, n_entries: usize) -> String {
    let app = &j["app"];
    let m = &j["module"];
    let g = |h: &serde_json::Value, k: &str| h[k].as_str().map(|s| s.to_string());
    let bundle = g(app, "bundleName")
        .or_else(|| g(j, "bundleName"))
        .or_else(|| g(&j["bundle"], "name"))
        .unwrap_or_else(|| "?".into());
    let ver = g(app, "versionName")
        .or_else(|| g(j, "versionName"))
        .unwrap_or_else(|| "?".into());
    let vcode = app["versionCode"]
        .as_u64()
        .or(j["versionCode"].as_u64())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".into());
    let min_api = app["minAPIVersion"]
        .as_u64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".into());
    let tgt_api = app["targetAPIVersion"]
        .as_u64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".into());
    let release = g(app, "apiReleaseType").unwrap_or_else(|| "?".into());
    let fmt_arr = |k: &str| {
        m[k].as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "-".into())
    };
    let mut lines = Vec::new();
    lines.push(format!(
        "bundle: {} (version {} / code {})",
        bundle, ver, vcode
    ));
    lines.push(format!(
        "api: min {} target {} ({})",
        min_api, tgt_api, release
    ));
    lines.push(format!(
        "module: {} type={} runtimeOS={}",
        g(m, "name").unwrap_or_else(|| "?".into()),
        g(m, "type").unwrap_or_else(|| "?".into()),
        g(j, "compileSdkVersion")
            .or_else(|| g(&j["app"], "compileSdkVersion"))
            .unwrap_or_else(|| "?".into())
    ));
    lines.push(format!("deviceTypes: {}", fmt_arr("deviceTypes")));
    // DevEco module.json5 keeps srcEntry inside each ability; fall back to the main element's
    // own entry so `hap info` still answers "where does this module start".
    let main_element = g(m, "mainElement")
        .or_else(|| g(m, "mainAbility"))
        .unwrap_or_else(|| "?".into());
    let src_entry = g(m, "srcEntry")
        .or_else(|| {
            m["abilities"]
                .as_array()
                .and_then(|a| a.iter().find(|x| x["name"] == main_element.as_str()))
                .and_then(|x| x["srcEntry"].as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".into());
    lines.push(format!(
        "mainElement: {} | srcEntry: {}",
        main_element, src_entry
    ));
    let abilities = m["abilities"].as_array().map(|a| a.len()).unwrap_or(0);
    let ext = m["extensionAbilities"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    lines.push(format!("abilities: {} (+{} extension)", abilities, ext));
    lines.push(format!(
        "permissions: {}",
        m["requestPermissions"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    ));
    if let Some(v) = m["installationFree"].as_bool() {
        lines.push(format!("installationFree: {}", v));
    }
    lines.push(format!("entries: {} files", n_entries));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hap_info_renders_signed_layout() {
        let j: serde_json::Value = serde_json::from_str(r#"{
          "app": {"bundleName":"com.example.demo","versionCode":2,"versionName":"1.0.1","minAPIVersion":20000017,"targetAPIVersion":20000017,"apiReleaseType":"Release"},
          "module": {"name":"entry","type":"entry","deviceTypes":["phone","tablet"],"mainElement":"EntryAbility","srcEntry":"./ets/EntryAbility.ets","abilities":[{"name":"EntryAbility"}],"extensionAbilities":[{"name":"Ext"}],"requestPermissions":[{"name":"ohos.permission.CAMERA"}]}
        }"#).unwrap();
        let out = render_hap_info(&j, 42);
        assert!(
            out.contains("com.example.demo (version 1.0.1 / code 2)"),
            "{}",
            out
        );
        assert!(out.contains("min 20000017 target 20000017 (Release)"));
        assert!(out.contains("deviceTypes: phone, tablet"));
        assert!(out.contains("abilities: 1 (+1 extension)"));
        assert!(out.contains("permissions: 1"));
        assert!(out.contains("entries: 42 files"));
    }
}

#[cfg(test)]
mod src_entry_fallback_tests {
    use super::render_hap_info;

    /// Entry modules carry srcEntry per ability; `hap info` must resolve it from mainElement.
    #[test]
    fn src_entry_falls_back_to_main_element() {
        let j: serde_json::Value = serde_json::from_str(
            r#"{
          "app": {"bundleName":"com.example.demo","versionCode":2,"versionName":"1.0.1"},
          "module": {"name":"entry","type":"entry","deviceTypes":["phone"],
                     "mainElement":"EntryAbility",
                     "abilities":[{"name":"EntryAbility","srcEntry":"./ets/entryability/EntryAbility.ets"}]}
        }"#,
        )
        .unwrap();
        let out = render_hap_info(&j, 0);
        assert!(
            out.contains("./ets/entryability/EntryAbility.ets"),
            "{}",
            out
        );
        assert!(!out.contains("srcEntry: ?"), "{}", out);
    }

    /// Build a throwaway .hap-shaped archive so the signature reader can be exercised without
    /// an SDK: only the entry names matter to it.
    fn write_archive(name: &str, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("hvolt-issigned-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (entry, body) in entries {
            writer.start_file(*entry, options).unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    /// A package with no signature block is unsigned no matter what it is called, which is the
    /// whole point of reading the archive instead of the file name.
    #[test]
    fn unsigned_archive_is_not_reported_as_signed() {
        let path = write_archive("app-signed.hap", &[("module.json", b"{}")]);
        assert!(!crate::hap::is_signed(&path).unwrap());
    }

    /// The signature block lives under META-INF with a `.sig` suffix; its presence is what marks
    /// a package signed.
    #[test]
    fn signature_block_marks_a_package_signed() {
        let path = write_archive(
            "app.hap",
            &[("module.json", b"{}"), ("META-INF/APP.sig", b"x")],
        );
        assert!(crate::hap::is_signed(&path).unwrap());
    }

    /// An unrelated META-INF entry must not be mistaken for a signature.
    #[test]
    fn meta_inf_without_signature_is_not_signed() {
        let path = write_archive(
            "app-plain.hap",
            &[("module.json", b"{}"), ("META-INF/MANIFEST.MF", b"x")],
        );
        assert!(!crate::hap::is_signed(&path).unwrap());
    }
}
