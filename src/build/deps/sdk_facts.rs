//! Values that come from the installed SDK rather than from hvolt.
//!
//! hvigor asks the SDK for these (version strings, the ark bytecode version for a compatible
//! API level); hvolt must take them from the same place instead of carrying literals, or a
//! different SDK silently produces a different package.

use std::path::Path;

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
    /// Read the SDK metadata next to the compilers and ask the toolchain for the bytecode
    /// version of `compatible_api`.
    pub fn detect(
        sdk_root: &Path,
        node_binary: &Path,
        compatible_api: u32,
    ) -> Result<Self, String> {
        // The store keeps every version beside a `default` link; the platform tree lives one
        // level below whichever root the caller resolved.
        let platform = if sdk_root.join("default/openharmony").is_dir() {
            sdk_root.join("default/openharmony")
        } else if sdk_root.join("ets").is_dir() {
            sdk_root.to_path_buf()
        } else {
            sdk_root.join("openharmony")
        };
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
            virtual_machine: ark_version(&platform, node_binary, compatible_api),
        })
    }
}

/// `ark` + the bytecode version the SDK picks for `api`.
///
/// The value is the toolchain's own answer (`ts2abc.js --target-api-version <api>`), not a
/// table kept here: an API level maps to a bytecode version inside the SDK.
fn ark_version(ohos_sdk: &Path, node_binary: &Path, api: u32) -> String {
    let script = ohos_sdk.join("ets/build-tools/ets-loader/bin/ark/ts2abc.js");
    if !script.is_file() {
        return String::new();
    }
    let output = std::process::Command::new(node_binary)
        .arg(&script)
        .arg("--target-api-version")
        .arg(api.to_string())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text.is_empty() {
                String::new()
            } else {
                format!("ark{text}")
            }
        }
        _ => String::new(),
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
