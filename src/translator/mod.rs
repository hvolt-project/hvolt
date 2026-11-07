// Translator: user structure → HarmonyOS standard layout
//
// Public API:
//   discover_modules()   — scan for modules
//   full_translate()     — generate entire build/ directory from scratch
//   normalize_ets()      — strip .ets suffix
//   with_ets_suffix()    — ensure .ets suffix

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::support::config::AppConfig;
pub use crate::support::config::{ModuleConfig, NativeModuleConfig};

/// `type = "native"` detection, parsed rather than string-matched:
/// the old literal `contains("type = \"native\"")` missed `type="native"`,
/// single quotes, and any other spacing.
fn is_native_module(raw: &str) -> bool {
    raw.parse::<toml::Value>()
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(|t| t.as_str())
                .map(|s| s == "native")
        })
        .unwrap_or(false)
}

/// One module file's parsed form.
// The ETS variant holds the whole module config (now including native flags); boxing it would
// only move the allocation, and discovery handles a handful of modules at a time.
#[allow(clippy::large_enum_variant)]
enum ModuleKind {
    Ets(ModuleConfig),
    Native(NativeModuleConfig),
}

/// Parse a module `hvolt.toml` on ANY discovery path.
///
/// The design rule (`type = "native"` → native module, never in `modules[]`)
/// used to be implemented only in the `[dependencies]` path; a native MEMBER
/// was therefore parsed as an ETS module and silently skipped (and under the
/// strict-parse change it failed with a confusing "missing field `module`").
fn parse_module_file(path: &Path) -> Result<ModuleKind, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    if is_native_module(&raw) {
        toml::from_str::<NativeModuleConfig>(&raw)
            .map(ModuleKind::Native)
            .map_err(|e| format!("native module {} is invalid: {}", path.display(), e))
    } else {
        ModuleConfig::from_file(path)
            .map(ModuleKind::Ets)
            .map_err(|e| format!("module {} is invalid: {}", path.display(), e))
    }
}

/// Discover the project's modules.
///
/// Returns Err when any declared module/dependency cannot be read or parsed,
/// or when the loaded set is empty. The old version printed a warning and
/// `continue`d, so `assemble` reported "✓ 0 modules" with exit 0 on a
/// mistyped config.
/// Modules and native modules as declared by the project config.
type ModuleSets = (
    Vec<(String, ModuleConfig)>,
    Vec<(String, NativeModuleConfig)>,
);

pub fn discover_modules(root: &Path, app_config: &AppConfig) -> Result<ModuleSets, String> {
    let mut errors: Vec<String> = Vec::new();
    let mut modules = Vec::new();
    let mut declared: HashSet<String> = HashSet::new();
    let mut native_modules = Vec::new();

    // Dependencies from [dependencies]: HSP, HAR, or native modules
    for (dep_name, dep) in &app_config.dependencies {
        // A registry dependency has no local tree to discover: it is resolved by `dep sync`
        // into the package cache, not into the project.
        let Some(dep_path) = dep.path.as_deref() else {
            continue;
        };
        let dep_dir = root.join(dep_path);
        // .har files are pre-built — skip module discovery
        if dep_path.ends_with(".har") {
            continue;
        }
        let toml = dep_dir.join("hvolt.toml");
        if !toml.exists() {
            errors.push(format!(
                "dependency '{}' declared but no hvolt.toml at {}",
                dep_name,
                dep_dir.display()
            ));
            continue;
        }
        // Read raw toml to check type first
        let raw = match std::fs::read_to_string(&toml) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("cannot read {}: {}", toml.display(), e));
                continue;
            }
        };
        if is_native_module(&raw) {
            match toml::from_str::<NativeModuleConfig>(&raw) {
                Ok(cfg) => {
                    native_modules.push((dep_name.clone(), cfg));
                }
                Err(e) => {
                    errors.push(format!(
                        "native module {} is invalid: {}",
                        toml.display(),
                        e
                    ));
                }
            }
        } else {
            // Legacy ETS/HSP module
            match toml::from_str::<ModuleConfig>(&raw) {
                Ok(cfg) => {
                    let dir_name = dep_name.clone();
                    declared.insert(dir_name.clone());
                    modules.push((dir_name, cfg));
                }
                Err(e) => errors.push(format!("module {} is invalid: {}", toml.display(), e)),
            }
        }
    }

    // Members: explicit module list, or scan for subdirs with hvolt.toml not in [dependencies]
    if !app_config.app.members.is_empty() {
        for member in &app_config.app.members {
            if declared.contains(member) {
                continue;
            }
            let member_dir = root.join(member);
            let toml = member_dir.join("hvolt.toml");
            if !toml.exists() {
                errors.push(format!(
                    "member '{}' declared but no hvolt.toml at {}",
                    member,
                    member_dir.display()
                ));
                continue;
            }
            match parse_module_file(&toml) {
                Ok(ModuleKind::Ets(cfg)) => modules.push((member.clone(), cfg)),
                Ok(ModuleKind::Native(cfg)) => native_modules.push((member.clone(), cfg)),
                Err(e) => errors.push(e),
            }
        }
    } else {
        // Fallback: scan for subdirs with hvolt.toml
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
                if declared.contains(&dir_name) {
                    continue;
                }
                let toml = path.join("hvolt.toml");
                if toml.exists() {
                    match parse_module_file(&toml) {
                        Ok(ModuleKind::Ets(cfg)) => modules.push((dir_name, cfg)),
                        Ok(ModuleKind::Native(cfg)) => native_modules.push((dir_name, cfg)),
                        Err(e) => errors.push(e),
                    }
                }
            }
        }
    }

    // Combination rules (runtimeOS default, bundle_type, device_types enums) live in
    // config.rs as pure functions; the translator only applies them.
    errors.extend(crate::support::config::validate_combination(
        &app_config.app,
        &modules,
    ));

    if !errors.is_empty() {
        return Err(errors.join("\n  "));
    }
    if modules.is_empty() && native_modules.is_empty() {
        return Err(format!(
            "no module could be loaded (members: {:?}, dependencies: {:?}) — \
             every module needs a readable hvolt.toml",
            app_config.app.members,
            app_config.dependencies.keys().collect::<Vec<_>>()
        ));
    }
    Ok((modules, native_modules))
}

pub(crate) fn generate_main_pages(config: &ModuleConfig) -> String {
    let pages: Vec<String> = config
        .module
        .routes
        .iter()
        .map(|p| {
            let path = p.trim_end_matches(".ets");
            format!(r#"    "{}""#, path)
        })
        .collect();
    format!(
        r#"{{
  "src": [
{}
  ]
}}"#,
        pages.join(",\n")
    )
}

#[cfg(test)]
mod native_detect_tests {
    use super::is_native_module;

    /// The structured check must accept every legal spelling of the marker
    /// (the old literal match only knew the double-quoted, space-padded one).
    #[test]
    fn native_marker_is_recognized_in_all_legal_spellings() {
        assert!(is_native_module("name = \"x\"\ntype = \"native\"\n"));
        assert!(is_native_module("type=\"native\""));
        assert!(is_native_module("type = 'native'"));
        assert!(is_native_module(
            "name=\"x\"\n type = \"native\"\n version=\"1\""
        ));
    }

    #[test]
    fn non_native_and_garbage_are_not_native() {
        assert!(!is_native_module("[module]\nname = \"src\"\n"));
        assert!(!is_native_module("type = \"ets\""));
        assert!(!is_native_module("type = 12"));
        assert!(!is_native_module("this is not toml at all ]["));
    }
}
