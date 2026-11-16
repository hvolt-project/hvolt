//! Validate a generated manifest against the SDK's own modulecheck schema.
//!
//! The schema ships with the SDK (`toolchains/modulecheck/*.json`), so the rules stay those of
//! the platform hvolt builds for instead of a copy that drifts. Reading it per build costs one
//! file read; keeping a copy would trade that for a silent disagreement with a newer SDK.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Where the SDK keeps the schemas its own validator uses.
///
/// `sdk_root` is the store root, so the version is reached through the `default` link, and the
/// toolchain matches the runtime the project targets — the hms copy carries the stricter rules.
fn schema_path(sdk_root: &Path, runtime_os: &str, schema_name: &str) -> PathBuf {
    let toolchain = if crate::support::config::is_open_harmony(runtime_os) {
        "openharmony"
    } else {
        "hms"
    };
    sdk_root
        .join("default")
        .join(toolchain)
        .join("toolchains")
        .join("modulecheck")
        .join(schema_name)
}

/// `camelCase` -> `snake_case`, the rule `parse_module`/`parse_app` read toml with.
fn snake(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len() + 4);
    for (i, ch) in segment.char_indices() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// `/abilities/0/label` -> `abilities[0].label`, so a report names the field a person wrote.
fn toml_key(instance_path: &str) -> String {
    let mut out = String::new();
    for segment in instance_path.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        if let Ok(index) = segment.parse::<usize>() {
            out.push_str(&format!("[{index}]"));
            continue;
        }
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&snake(segment));
    }
    if out.is_empty() {
        "(root)".to_string()
    } else {
        out
    }
}

/// Check `instance` against `schema_name` (e.g. `module.json`).
///
/// Fails with every violation at once, each naming the manifest path, the toml key that writes
/// it, and the schema constraint that was broken.
pub fn check(
    sdk_root: &Path,
    runtime_os: &str,
    schema_name: &str,
    instance: &Value,
) -> Result<(), String> {
    let path = schema_path(sdk_root, runtime_os, schema_name);
    // The schema is an optional SDK asset: an OpenHarmony-only SDK carries no hms
    // `toolchains/modulecheck/` at all. Treating that as a hard error failed every build on such
    // a machine, so a missing file skips the check while any other read error still fails.
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let schema: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|e| format!("build a validator from {}: {e}", path.display()))?;

    let mut problems: Vec<String> = Vec::new();
    for error in validator.iter_errors(instance) {
        let at = error.instance_path().to_string();
        let shown = if at.is_empty() {
            "(root)".to_string()
        } else {
            at.clone()
        };
        problems.push(format!("  {} (toml: {}) — {}", shown, toml_key(&at), error));
    }
    if problems.is_empty() {
        return Ok(());
    }
    problems.sort();
    Err(format!(
        "{schema_name} does not satisfy {}\n{}",
        path.display(),
        problems.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::{check, snake, toml_key};
    use serde_json::json;

    /// Lay down the SDK-side schema the check reads, under the layout `schema_path` expects.
    fn write_schema(
        root: &std::path::Path,
        runtime_os: &str,
        name: &str,
        schema: &serde_json::Value,
    ) {
        let toolchain = if crate::support::config::is_open_harmony(runtime_os) {
            "openharmony"
        } else {
            "hms"
        };
        let dir = root
            .join("default")
            .join(toolchain)
            .join("toolchains")
            .join("modulecheck");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), schema.to_string()).unwrap();
    }

    /// An SDK that ships no modulecheck copy must not fail the build: this is the shape an
    /// OpenHarmony-only SDK has, and it used to abort every build with a read error.
    #[test]
    fn a_missing_schema_skips_the_check() {
        let root = std::env::temp_dir().join(format!("hvolt-schema-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let instance = json!({"name": "entry"});
        assert!(check(&root, "OpenHarmony", "module.json", &instance).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A conforming manifest passes against the SDK's own schema.
    #[test]
    fn a_conforming_manifest_passes() {
        let root = std::env::temp_dir().join(format!("hvolt-schema-ok-{}", std::process::id()));
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {"name": {"type": "string"}}
        });
        write_schema(&root, "OpenHarmony", "module.json", &schema);
        let instance = json!({"name": "entry"});
        assert!(check(&root, "OpenHarmony", "module.json", &instance).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A violation is reported with both the manifest path and the toml key that writes it, so
    /// the message names something a person can find in hvolt.toml.
    #[test]
    fn a_violation_names_the_path_and_the_toml_key() {
        let root = std::env::temp_dir().join(format!("hvolt-schema-bad-{}", std::process::id()));
        let schema = json!({
            "type": "object",
            "properties": {
                "abilities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {"label": {"type": "string"}}
                    }
                }
            }
        });
        write_schema(&root, "OpenHarmony", "module.json", &schema);
        let instance = json!({"abilities": [{"label": 42}]});
        let err = check(&root, "OpenHarmony", "module.json", &instance).unwrap_err();
        assert!(err.contains("/abilities/0/label"), "{err}");
        assert!(err.contains("abilities[0].label"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn camel_case_maps_back_to_the_toml_key() {
        assert_eq!(snake("backgroundModes"), "background_modes");
        assert_eq!(snake("startWindowIcon"), "start_window_icon");
        assert_eq!(snake("name"), "name");
    }

    #[test]
    fn array_indexes_survive_the_mapping() {
        assert_eq!(toml_key("/abilities/0/label"), "abilities[0].label");
        assert_eq!(toml_key("/deviceTypes"), "device_types");
        assert_eq!(toml_key(""), "(root)");
    }
}
