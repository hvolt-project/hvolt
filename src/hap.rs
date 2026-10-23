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

/// Parse app_check_tool stat.json. Unknown taskType sections are ignored
/// (forward compatible); missing sections become empty, not errors.
pub fn parse_package_stat(json_str: &str) -> Result<PackageStat, String> {
    let v: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("stat.json: {}", e))?;
    let tasks = v
        .as_array()
        .ok_or("stat.json: top level is not a list".to_string())?;
    let mut st = PackageStat {
        size_limit: None,
        over_limit: vec![],
        duplicates: vec![],
        suffixes: vec![],
    };
    for t in tasks {
        match t["taskType"].as_i64() {
            Some(1) => {
                // duplicated files: result entries list a group of paths + size
                if let Some(groups) = t["result"].as_array() {
                    for g in groups {
                        let paths: Vec<String> = g["files"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|f| f["file"].as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let size = g["size"].as_u64().unwrap_or(0);
                        if let Some(first) = paths.first() {
                            st.duplicates
                                .push((first.clone(), paths[1..].to_vec(), size));
                        }
                    }
                }
            }
            Some(2) => {
                st.size_limit = t["param"]
                    .as_str()
                    .and_then(|p| p.split_whitespace().nth(1))
                    .and_then(|n| n.parse().ok());
                if let Some(files) = t["result"].as_array() {
                    for f in files {
                        if let (Some(p), Some(sz)) = (f["file"].as_str(), f["size"].as_u64()) {
                            st.over_limit.push((p.to_string(), sz));
                        }
                    }
                    st.over_limit.sort_by_key(|e| std::cmp::Reverse(e.1));
                }
            }
            Some(3) => {
                if let Some(groups) = t["result"].as_array() {
                    for g in groups {
                        let suffix = g["suffix"].as_str().unwrap_or("?").to_string();
                        let files = g["files"].as_array().map(|a| a.len()).unwrap_or(0);
                        let total = g["files"]
                            .as_array()
                            .map(|a| a.iter().filter_map(|f| f["size"].as_u64()).sum())
                            .unwrap_or(0);
                        st.suffixes.push((suffix, total, files));
                    }
                    st.suffixes.sort_by_key(|e| std::cmp::Reverse(e.1));
                }
            }
            _ => {} // unknown task type: ignore
        }
    }
    Ok(st)
}

// ─── hap info (pure Rust, no java needed) ───────────────────────────

/// Read the module.json embedded in a (signed or unsigned) HAP and print
/// identity facts straight from the package — the acceptance discipline
/// says trust the package, not build logs.
pub fn hap_info(hap_path: &Path) -> Result<String, String> {
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

    const REAL_STAT: &str = r#"[
      {"param":"--stat-duplicate","result":[{"files":[{"file":"a.png","size":100},{"file":"b/a.png","size":100}],"size":100}],"taskType":1},
      {"param":"--stat-file-size 500","result":[{"file":"big.abc","size":900},{"file":"bigger.so","size":1500}],"taskType":2},
      {"param":"--stat-suffix","result":[{"suffix":".png","files":[{"file":"x.png","size":300},{"file":"y.png","size":100}]},{"suffix":".abc","files":[{"file":"z.abc","size":900}]}],"taskType":3}
    ]"#;

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

    #[test]
    fn parse_stat_sections_roundtrip() {
        let st = parse_package_stat(REAL_STAT).unwrap();
        assert_eq!(st.duplicates.len(), 1);
        assert_eq!(st.duplicates[0].0, "a.png");
        assert_eq!(st.duplicates[0].1, vec!["b/a.png".to_string()]);
        assert_eq!(st.size_limit, Some(500));
        // over_limit sorted desc
        assert_eq!(st.over_limit[0], ("bigger.so".to_string(), 1500));
        assert_eq!(st.suffixes[0], (".abc".to_string(), 900, 1)); // by total desc
        assert_eq!(st.suffixes[1], (".png".to_string(), 400, 2));
    }

    #[test]
    fn parse_stat_tolerates_missing_sections() {
        let st = parse_package_stat(r#"[{"taskType":3,"result":[]}]"#).unwrap();
        assert_eq!(st.size_limit, None);
        assert!(st.over_limit.is_empty() && st.duplicates.is_empty() && st.suffixes.is_empty());
        // unknown task types ignored
        let st2 = parse_package_stat(r#"[{"taskType":99,"result":[1,2]}]"#).unwrap();
        assert!(st2.suffixes.is_empty());
        assert!(parse_package_stat("not json").is_err());
        assert!(parse_package_stat("{}").is_err()); // top-level list expected
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
}
