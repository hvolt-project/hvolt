// pkg_context: render pkgContextInfo.json from hvolt.toml, standalone.
//
// pkgContextInfo.json is the runtime dependency-lookup table inside the HAP. The
// reference shape comes from the reference's `GeneratePkgContextInfo` task (verified against
// asr-ondevice's `loader/default/pkgContextInfo.json`): one entry for the module
// itself, one per declared dependency, and one per `.so` on disk.
//
// hvolt.toml has no oh-package.json5, so dev dependencies (hypium/hamock) never
// appear — they are test-only and not part of the shipped package anyway.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::support::config::{AppConfig, NativeModuleConfig};

/// Dependency entries collected from a project: path dependencies declared in
/// hvolt.toml, plus the `.so` files a native module builds.
pub struct PkgContextInput<'a> {
    /// module directory in the project (e.g. "src") — the entry's own packageName.
    pub module_name: &'a str,
    /// Root project dir; dependency paths resolve against it.
    pub project_dir: &'a std::path::Path,
    /// Every native dependency declared in the root hvolt.toml.
    pub native_deps: &'a [(String, NativeModuleConfig)],
    /// Version every record carries; the device matches `<record>&<version>`.
    pub module_version: &'a str,
    /// ETS dependency modules linked into the same abc (name, srcEntry). Without an entry
    /// the device expands `from '<pkg>'` with its own default root and the composed name
    /// does not match the record the module actually carries.
    pub ets_deps: &'a [(String, String)],
    /// Third-party packages compiled into the same abc (`yaml`, `@mcui/mccharts`, …).
    /// Each one needs an entry, or the runtime cannot resolve an import of its package root.
    pub packages: &'a [crate::build::deps::npm::Package],
    /// The app config (bundle type etc.).
    pub app: &'a AppConfig,
}

/// One context entry.
fn entry(package_name: &str, version: &str, entry_path: &str, is_so: bool) -> Value {
    json!({
        "packageName": package_name,
        "bundleName": "",
        "moduleName": "",
        "version": version,
        "entryPath": entry_path,
        "isSO": is_so,
        "dependencyAlias": "",
    })
}

/// The `.so` files a native module builds: `lib{name}.so` in its build output.
/// the reference scans the ninja working dir; hvolt's native modules are prebuilt, so the
/// convention is `libs/<abi>/lib{name}.so` under the dependency's directory.
fn so_files_for(dep_name: &str, dep_dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let libs = dep_dir.join("libs");
    if !libs.is_dir() {
        // No bundled libs/ tree — the module may keep prebuilt .so elsewhere;
        // the name convention still applies.
        out.push(format!("lib{dep_name}.so"));
        return out;
    }
    let mut abis: Vec<_> = std::fs::read_dir(&libs)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    abis.sort();
    for abi in abis {
        let dir = libs.join(&abi);
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .filter(|n| n.ends_with(".so"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        for name in names {
            let key = format!("{abi}/{name}");
            if !out.contains(&key) {
                out.push(key);
            }
        }
    }
    if out.is_empty() {
        out.push(format!("lib{dep_name}.so"));
    }
    out
}

/// Render the whole pkgContextInfo.json for one module.
pub fn render(input: &PkgContextInput<'_>) -> Value {
    let mut map: BTreeMap<String, Value> = BTreeMap::new();

    // The module's own entry. `entryPath` is the source root; the device composes the
    // lookup name as `<module>/<entryPath with dots>/ets/<srcEntry path>`.
    // The reference spells the source root as a slash path here (`src/main/`); the device
    // composes `<pkg>/<entryPath><relative>` from it.
    // The entry's own version stays empty: the device appends `&<version>` to the entry
    // request only when the context carries one, and the entry record has none.
    let own = entry(input.module_name, "", "src/main/", false);
    map.insert(input.module_name.to_string(), own);

    // ETS dependency modules share the module's own path model, so they carry the same
    // entryPath kind as the entry module.
    // A dependency module is addressed through its package entry (`from 'proxy_core'`),
    // so its context entryPath names that entry file, not the source root.
    for (name, src_entry) in input.ets_deps {
        if name == input.module_name {
            continue;
        }
        map.insert(
            name.clone(),
            entry(name, input.module_version, src_entry, false),
        );
    }

    // Third-party packages: `entryPath` names the file the package's own entry resolves to,
    // relative to the package directory.
    for package in input.packages {
        // The declared entry file is authoritative: a package may ship `index.ets`,
        // `index.ts` and `index.js` beside each other, and only the manifest says which one
        // the runtime should load.
        let declared = crate::build::deps::npm::entry_file_name(package);
        let ext = match package.kind {
            crate::build::deps::npm::Kind::Arkts => "ets",
            crate::build::deps::npm::Kind::ByteCode => "",
            _ => "js",
        };
        let path = if let Some(declared) = declared {
            match package.source_root.strip_prefix(&package.dir) {
                Ok(rest) if !rest.as_os_str().is_empty() => {
                    format!("{}/{}", rest.to_string_lossy().replace('\\', "/"), declared)
                }
                _ => declared,
            }
        } else if package.kind == crate::build::deps::npm::Kind::ByteCode {
            package.entry.clone()
        } else {
            match package.source_root.strip_prefix(&package.dir) {
                Ok(rest) if !rest.as_os_str().is_empty() => format!(
                    "{}/{}.{}",
                    rest.to_string_lossy().replace('\\', "/"),
                    package.entry,
                    ext
                ),
                _ => format!("{}.{}", package.entry, ext),
            }
        };
        map.insert(
            package.name.clone(),
            entry(&package.name, &package.version, &path, false),
        );
    }

    // Declared native dependencies: `{name}.so` entry with the types-file stem as
    // entryPath (the main file minus .d.ets/.d.ts). The `.d.ts` suffix has to be
    // stripped by a pattern, not file_stem — "Index.d.ts" would give "Index.d".
    // `version` stays empty for SO dependencies (the reference's `getVersion` only fills it
    // for HAR entries).
    for (name, cfg) in input.native_deps {
        let mut entry_path = cfg.types.clone();
        for suffix in [".d.ets", ".d.ts"] {
            if let Some(rest) = entry_path.strip_suffix(suffix) {
                entry_path = rest.to_string();
                break;
            }
        }
        map.insert(
            format!("{name}.so"),
            entry(&format!("{name}.so"), "", &entry_path, true),
        );
        let dep_dir = input.project_dir.join(name);
        for so in so_files_for(name, &dep_dir) {
            let key = so.split('/').next_back().unwrap_or(&so).to_string();
            // The declared entry above wins; on-disk duplicates are skipped.
            if !map.contains_key(&key) {
                map.insert(key.clone(), entry(&key, "", "", true));
            }
        }
    }

    let _ = input.app;
    Value::Object(map.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::config::NativeModuleConfig;

    fn native(name: &str, types: &str) -> (String, NativeModuleConfig) {
        (
            name.to_string(),
            NativeModuleConfig {
                name: name.into(),
                module_type: "native".into(),
                types: types.into(),
                version: "1.0.0".into(),
                version_code: 0,
                build_version: String::new(),
                description: "native module".into(),
            },
        )
    }

    /// The module carries its own entry, whose `entryPath` is the ability's source entry
    /// without its extension — the runtime composes the bytecode path from it.
    #[test]
    fn the_module_carries_its_own_entry() {
        let app = AppConfig::default_for("x");
        let out = render(&PkgContextInput {
            module_name: "src",
            project_dir: std::path::Path::new("/tmp/nonexistent"),
            native_deps: &[],
            module_version: "1.0.0",
            ets_deps: &[],
            packages: &[],
            app: &app,
        });
        let obj = out.as_object().unwrap();
        assert_eq!(obj["src"]["packageName"], "src");
        assert_eq!(obj["src"]["entryPath"], "src/main/");
        assert_eq!(obj["src"]["isSO"], false);
    }

    /// A native dependency contributes `{name}.so` with the types stem as entryPath,
    /// mirroring `getDependenciesPkgContextInfo`'s main-file rule.
    #[test]
    fn a_native_dependency_uses_the_types_stem() {
        let app = AppConfig::default_for("x");
        let deps = vec![native("asrprobe", "Index.d.ts")];
        let out = render(&PkgContextInput {
            module_name: "src",
            project_dir: std::path::Path::new("/tmp/nonexistent"),
            native_deps: &deps,
            module_version: "1.0.0",
            ets_deps: &[],
            packages: &[],
            app: &app,
        });
        let obj = out.as_object().unwrap();
        assert!(obj.contains_key("asrprobe.so"));
        assert_eq!(obj["asrprobe.so"]["entryPath"], "Index");
        assert_eq!(obj["asrprobe.so"]["isSO"], true);
        // SO dependencies carry no version (the reference's getVersion only fills HAR).
        assert_eq!(obj["asrprobe.so"]["version"], "");
    }

    /// No oh-package.json5 means no dev dependencies — the table only carries the
    /// module and declared deps.
    #[test]
    fn dev_dependencies_never_appear() {
        let app = AppConfig::default_for("x");
        let out = render(&PkgContextInput {
            module_name: "src",
            project_dir: std::path::Path::new("/tmp/nonexistent"),
            native_deps: &[],
            module_version: "1.0.0",
            ets_deps: &[],
            packages: &[],
            app: &app,
        });
        let obj = out.as_object().unwrap();
        assert!(!obj.contains_key("@ohos/hypium"));
    }
}
