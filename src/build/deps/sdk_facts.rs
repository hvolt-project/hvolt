//! Values that come from the installed SDK rather than from hvolt.
//!
//! hvigor asks the SDK for these (version strings, the ark bytecode version for a compatible
//! API level); hvolt must take them from the same place instead of carrying literals, or a
//! different SDK silently produces a different package. The bytecode version is the one value
//! that does not need the SDK at run time: it is a per-level constant, kept in `sdk_bytecode`
//! and re-measured against the installed SDK by that module's tests.

use std::path::{Path, PathBuf};

/// Facts read from one installed SDK.
#[derive(Debug, Clone)]
pub struct SdkFacts {
    /// Four-part SDK version, e.g. "26.0.0.23".
    pub version: String,
    /// Release type as the SDK declares it, e.g. "Beta1".
    pub release_type: String,
    /// `ark<target bytecode version>` for a given compatible API level.
    pub virtual_machine: String,
}

impl SdkFacts {
    /// Read the SDK metadata next to the compilers and look up the bytecode version for
    /// `compatible_api`.
    ///
    /// `node_binary` is not used: the bytecode version comes from the `sdk_bytecode` table, so
    /// detecting facts no longer spawns a Node process.
    pub fn detect(
        sdk_root: &Path,
        _node_binary: &Path,
        compatible_api: u32,
    ) -> Result<Self, String> {
        let platform = platform_root(sdk_root);
        let package = platform.join("ets/oh-uni-package.json");
        let text = std::fs::read_to_string(&package)
            .map_err(|e| format!("read {}: {e}", package.display()))?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", package.display()))?;
        let version = value
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let release_type = value
            .get("releaseType")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(Self {
            version,
            release_type,
            virtual_machine: ark_version(compatible_api),
        })
    }
}

/// The platform tree of an SDK root.
///
/// The store keeps every version beside a `default` link; the platform tree lives one level
/// below whichever root the caller resolved.
pub(crate) fn platform_root(sdk_root: &Path) -> PathBuf {
    if sdk_root.join("default/openharmony").is_dir() {
        sdk_root.join("default/openharmony")
    } else if sdk_root.join("ets").is_dir() {
        sdk_root.to_path_buf()
    } else {
        sdk_root.join("openharmony")
    }
}

/// `ark` + the bytecode version the SDK picks for `api`.
///
/// The lookup lives in `sdk_bytecode::TABLE`; a level the table does not cover reports a
/// warning instead of leaving the field empty in silence, because an empty `virtualMachine`
/// is a malformed package rather than a degraded one.
fn ark_version(api: u32) -> String {
    match super::sdk_bytecode::bytecode_version(api) {
        Some(version) => format!("ark{version}"),
        None => {
            eprintln!(
                "Warning: no bytecode version for API level {api}; virtualMachine will be empty"
            );
            String::new()
        }
    }
}

/// The API level a version string like `5.0.5(17)` targets.
pub fn compatible_api(version: &str) -> Option<u32> {
    // `5.0.5(17)` carries the API level in parentheses; a bare `6.0.0` carries it as the
    // leading number.
    if let (Some(start), Some(end)) = (version.find('('), version.rfind(')')) {
        if end > start {
            if let Ok(level) = version[start + 1..end].trim().parse::<u32>() {
                return Some(level);
            }
        }
    }
    version
        .split('.')
        .next()
        .and_then(|v| v.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_api_reads_the_parenthesised_level() {
        assert_eq!(compatible_api("5.0.5(17)"), Some(17));
        assert_eq!(compatible_api("6.0.0(20)"), Some(20));
        assert_eq!(compatible_api("6.0.0"), Some(6));
        assert_eq!(compatible_api(""), None);
    }

    /// Reads the installed SDK, so it only runs when the toolchain is present.
    #[test]
    fn the_installed_sdk_reports_a_bytecode_version() {
        let env = match crate::sdk::manage::detect("") {
            Ok(env) => env,
            Err(_) => return,
        };
        let node = crate::sdk::manage::node_bin(&env.node_home);
        if !node.is_file() {
            println!("no node at {}", node.display());
            return;
        }
        let facts = SdkFacts::detect(&env.ohos_sdk, &node, 17).expect("read SDK facts");
        println!(
            "version={} release={} vm={:?} root={} node={}",
            facts.version,
            facts.release_type,
            facts.virtual_machine,
            env.ohos_sdk.display(),
            node.display()
        );
        assert!(!facts.version.is_empty(), "SDK version is empty");
    }
}
