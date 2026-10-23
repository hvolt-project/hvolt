// Config types

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Root hvolt.toml: [app] + optional [dependencies] + optional [app.signing]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub app: AppFields,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AppFields {
    pub bundle: String,
    pub name: String,
    pub version: String,
    #[serde(default = "default_vendor")]
    pub vendor: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    /// SDK installation path. None = auto-detect from env / global config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk_path: Option<String>,
    #[serde(default = "default_sdk_version")]
    pub sdk_version: String,
    #[serde(default = "default_sdk_version")]
    pub compile_sdk_version: String,
    /// Minimum compatible SDK version. Empty = same as sdk_version.
    #[serde(default)]
    pub compatible_sdk_version: String,
    /// Bundle type: "app" or "atomicService"
    #[serde(default = "default_bundle_type")]
    pub bundle_type: String,
    /// Products to build (default: ["default"])
    #[serde(default = "default_products")]
    pub products: Vec<String>,
    /// Runtime OS written into build-profile products: "HarmonyOS" (default) or "OpenHarmony".
    /// "OpenHarmony" uses the OH SDK channel (OHOS_BASE_SDK_HOME) and does not require
    /// hmscore SDK components — those are not published for linux by the getSdkList API.
    #[serde(default = "default_runtime_os")]
    pub runtime_os: String,
    /// Module member directories (relative to project root).
    #[serde(default)]
    pub members: Vec<String>,
    /// Signing configs per product (keyed by product name).
    /// Supports legacy format `[app.signing]` (maps to "default") and new `[app.signing.default]`.
    #[serde(default, deserialize_with = "deserialize_signing")]
    pub signing: BTreeMap<String, SigningConfig>,
}

/// Custom deserializer: accept both `[app.signing]` (legacy single) and `[app.signing.default]` (new map).
fn deserialize_signing<'de, D>(deserializer: D) -> Result<BTreeMap<String, SigningConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SigningFormat {
        Map(BTreeMap<String, SigningConfig>),
        Single(SigningConfig),
    }

    match SigningFormat::deserialize(deserializer)? {
        SigningFormat::Map(m) => Ok(m),
        SigningFormat::Single(s) => {
            let mut m = BTreeMap::new();
            m.insert("default".to_string(), s);
            Ok(m)
        }
    }
}

/// Signing materials for one build mode (debug/release).
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SigningConfig {
    pub store_file: String,
    pub store_password: String,
    pub key_alias: String,
    pub key_password: String,
    pub certpath: String,
    pub profile: String,
    #[serde(default = "default_sign_alg")]
    pub sign_alg: String,
}

impl AppFields {
    /// Derive versionCode from semantic version string.
    /// "1.0.0" → 1_000_000, "2.3.4" → 2_003_004
    pub fn version_code(&self) -> u32 {
        let parts: Vec<&str> = self.version.split('.').collect();
        let major: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
        let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        major * 1_000_000 + minor * 1_000 + patch
    }

    /// Get signing config for a product, falling back to "default".
    pub fn signing_for(&self, product: &str) -> Option<&SigningConfig> {
        self.signing
            .get(product)
            .or_else(|| self.signing.get("default"))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Dependency {
    pub path: String,
    pub version: String,
}

/// Module hvolt.toml (in subdirectory): [module]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ModuleConfig {
    pub module: ModuleInfo,
}

/// Skill declares actions, entities, and optional URI filters for an ability.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Skill {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub uris: Vec<UriScheme>,
    /// Permissions required by other apps to access this ability via this skill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    /// Enable domain verification for deep linking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_verify: Option<bool>,
}

/// Additional (non-entry) ability. type="" = UIAbility, type="form" = ExtensionAbility.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct AbilityConfig {
    pub name: String,
    /// Source entry file (e.g. "EntryFormAbility.ets").
    pub entry: String,
    /// Ability type: "" (UIAbility) or "form" etc. (ExtensionAbility).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[serde(rename = "type")]
    pub ability_type: String,
    #[serde(default)]
    pub skills: Vec<Skill>,
    /// Whether the ability is exported. None = omit from json5 output.
    #[serde(default)]
    pub exported: Option<bool>,
    /// Orientation: "auto_rotation", "portrait", "landscape", etc.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub orientation: String,
    /// Extension ability profile config (written to resources/base/profile/{type}_config.json).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_profile: Option<serde_json::Value>,
    /// Launch type: "singleton", "multiton", "specified".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[serde(rename = "launchType")]
    pub launch_type: String,
    /// Permissions required by other apps to access this ability.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ability_permissions: Vec<String>,
    /// Ability-level metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<MetadataItem>,
    /// Whether this ability can be continued on another device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuable: Option<bool>,
    /// Enhanced startup window profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "startWindow")]
    pub start_window: Option<String>,
    /// Whether to remove from recent missions after termination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove_mission_after_terminate: Option<bool>,
    /// Whether self-redirect is allowed (API 23+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_self_redirect: Option<bool>,
    /// Implicit query priority (0-10, system apps only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    /// Supported window modes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub support_window_mode: Vec<String>,
    /// Maximum window aspect ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_window_ratio: Option<f64>,
    /// Minimum window aspect ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_window_ratio: Option<f64>,
    /// Maximum window width in vp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_window_width: Option<u32>,
    /// Minimum window width in vp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_window_width: Option<u32>,
    /// Maximum window height in vp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_window_height: Option<u32>,
    /// Minimum window height in vp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_window_height: Option<u32>,
    /// Background modes for long-running tasks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub background_modes: Vec<String>,
    /// Whether to exclude from recent missions (system apps only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_from_missions: Option<bool>,
    /// Whether the mission is unclearable (system apps only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unclearable_mission: Option<bool>,
    /// Whether to run in an isolated process (2-in-1/tablet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_process: Option<bool>,
    /// Whether to exclude from dock (not effective).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_from_dock: Option<bool>,
    /// Whether the ability is recoverable after a crash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recoverable: Option<bool>,
    /// Preferred orientation in multi-window mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefer_multi_window_orientation: Option<String>,
    /// Continuation types for cross-device migration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub continue_type: Vec<String>,
    /// Other app bundle names for cross-device migration (API 13+).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub continue_bundle_name: Vec<String>,
    /// Process name (API 14+, 2-in-1/tablet only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ModuleInfo {
    pub name: String,
    /// Page routes for main_pages.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
    /// Entry ability file (e.g. "EntryAbility.ets"). Presence of entry implies entry module.
    #[serde(default = "default_entry_ets")]
    pub entry: String,
    /// Skills for the entry ability. Empty = default home skill added automatically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<Skill>,
    /// Module type: "hsp" / "shared" for non-entry modules. Entry modules omit this.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[serde(rename = "type")]
    pub module_type: String,
    /// Additional abilities (UIAbility or ExtensionAbility).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abilities: Vec<AbilityConfig>,
    /// Supported device types (required).
    pub device_types: Vec<String>,
    /// Runtime permissions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<ModulePermission>,
    /// Enable code obfuscation.
    #[serde(default)]
    pub obfuscation: bool,
    /// Custom obfuscation rules, relative to the module directory.
    /// Defaults to a file named `obfuscation-rules.txt` beside `hvolt.toml` when it exists.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub obfuscation_rules: String,
    /// Entry ability icon (default: the app icon).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// Entry ability start-window icon (default: `icon`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_window_icon: String,
    /// Entry ability orientation (default: `auto_rotation`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub orientation: String,
    /// Override `deliveryWithInstall` (default: derived from `bundle_type`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_with_install: Option<bool>,
    /// Override `installationFree` (default: derived from `bundle_type`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_free: Option<bool>,
    /// ABIs compiled for native modules (default: ["arm64-v8a"]).
    #[serde(default = "default_abi_filters")]
    pub abi_filters: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct UriScheme {
    pub scheme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "pathStartWith")]
    pub path_start_with: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "pathRegex")]
    pub path_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxFileSupported")]
    pub max_file_supported: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "linkFeature")]
    pub link_feature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct MetadataItem {
    pub name: String,
    pub resource: String,
}

/// Native module hvolt.toml (e.g. libxxx/hvolt.toml)
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NativeModuleConfig {
    pub name: String,
    /// Always "native".
    #[serde(rename = "type")]
    pub module_type: String,
    /// Path to the Index.d.ts type declaration file, relative to native module directory.
    pub types: String,
    pub version: String,
    #[serde(default = "default_native_description")]
    pub description: String,
}

fn default_native_description() -> String {
    "native module".to_string()
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ModulePermission {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(default)]
    pub abilities: Vec<String>,
    #[serde(default = "default_permission_when")]
    pub when: String,
}

impl AppConfig {
    pub fn default_for(name: &str) -> Self {
        AppConfig {
            app: AppFields {
                bundle: format!("com.example.{}", name.replace('-', "_")),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                vendor: default_vendor(),
                icon: default_icon(),
                sdk_path: None,
                sdk_version: default_sdk_version(),
                compile_sdk_version: default_sdk_version(),
                compatible_sdk_version: String::new(),
                signing: BTreeMap::new(),
                bundle_type: default_bundle_type(),
                products: default_products(),
                runtime_os: default_runtime_os(),
                members: vec!["src".into()],
            },
            dependencies: BTreeMap::new(),
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        toml::from_str(&content).map_err(|e| format!("parse {}: {}", path.display(), e))
    }
}

fn default_sdk_version() -> String {
    "26.0.0".to_string()
}

fn default_vendor() -> String {
    "example".to_string()
}

fn default_icon() -> String {
    "$media:layered_image".to_string()
}

fn default_sign_alg() -> String {
    "SHA256withECDSA".to_string()
}

fn default_bundle_type() -> String {
    "app".to_string()
}

fn default_abi_filters() -> Vec<String> {
    vec!["arm64-v8a".to_string()]
}

fn default_products() -> Vec<String> {
    vec!["default".to_string()]
}

fn default_runtime_os() -> String {
    "HarmonyOS".to_string()
}

/// Official deviceTypes enums, MEASURED from the SDK's own validators
/// (DevEco 26.0.0.461, 2026-10-10):
///   sdk/default/openharmony/toolchains/modulecheck/module.json
///   sdk/default/hms/toolchains/modulecheck/module.json
/// They differ: OpenHarmony has NO "phone". Emitting "phone" for an
/// OpenHarmony build produces a module.json5 its validator rejects
/// (measured).
pub const DEVICE_TYPES_OPENHARMONY: &[&str] =
    &["default", "tablet", "tv", "wearable", "car", "2in1"];
pub const DEVICE_TYPES_HARMONYOS: &[&str] = &[
    "default", "phone", "tablet", "tv", "wearable", "car", "2in1",
];

/// The enum that applies to a runtimeOS. Unknown/empty → HarmonyOS, which
/// matches the platform default (default_runtime_os).
pub fn device_types_for_runtime(runtime_os: &str) -> &'static [&'static str] {
    if runtime_os.eq_ignore_ascii_case("openharmony") {
        DEVICE_TYPES_OPENHARMONY
    } else {
        DEVICE_TYPES_HARMONYOS
    }
}

/// Declared entries the runtimeOS's official enum does not allow (empty = ok).
pub fn illegal_device_types(declared: &[String], runtime_os: &str) -> Vec<String> {
    let allowed = device_types_for_runtime(runtime_os);
    declared
        .iter()
        .filter(|d| !allowed.iter().any(|a| a.eq_ignore_ascii_case(d)))
        .cloned()
        .collect()
}

fn default_permission_when() -> String {
    "inuse".to_string()
}

fn default_entry_ets() -> String {
    "EntryAbility.ets".to_string()
}

impl ModuleConfig {
    pub fn default_entry() -> Self {
        ModuleConfig {
            module: ModuleInfo {
                name: "src".to_string(),
                module_type: String::new(),
                entry: "EntryAbility.ets".to_string(),
                routes: vec!["pages/Index.ets".to_string()],
                skills: vec![],
                abilities: vec![],
                device_types: vec!["phone".into(), "tablet".into(), "2in1".into(), "car".into()],
                permissions: vec![],
                obfuscation: false,
                obfuscation_rules: String::new(),
                icon: String::new(),
                start_window_icon: String::new(),
                orientation: String::new(),
                delivery_with_install: None,
                installation_free: None,
                abi_filters: default_abi_filters(),
            },
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        toml::from_str(&content).map_err(|e| format!("parse {}: {}", path.display(), e))
    }
}

#[cfg(test)]
mod device_type_tests {
    use super::*;

    /// Pins the two official enums (the constants above are the fix for
    /// "hvolt emits schema-illegal deviceTypes"). Re-measure if the SDK
    /// changes: sdk/default/{openharmony,hms}/toolchains/modulecheck/module.json
    #[test]
    fn official_enums_differ_by_runtime() {
        assert!(DEVICE_TYPES_OPENHARMONY.contains(&"tablet"));
        assert!(
            !DEVICE_TYPES_OPENHARMONY.contains(&"phone"),
            "OpenHarmony has no phone device type"
        );
        assert!(DEVICE_TYPES_HARMONYOS.contains(&"phone"));
        assert_eq!(device_types_for_runtime(""), DEVICE_TYPES_HARMONYOS);
        assert_eq!(
            device_types_for_runtime("OpenHarmony"),
            DEVICE_TYPES_OPENHARMONY
        );
        assert_eq!(
            device_types_for_runtime("openharmony"),
            DEVICE_TYPES_OPENHARMONY
        );
    }

    #[test]
    fn illegal_entries_are_reported() {
        let scaffold_default: Vec<String> = ["phone", "tablet", "2in1", "car"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // legal for HarmonyOS (the scaffold's default runtime)...
        assert!(illegal_device_types(&scaffold_default, "HarmonyOS").is_empty());
        // ...illegal for OpenHarmony: phone is not in its enum
        assert_eq!(
            illegal_device_types(&scaffold_default, "OpenHarmony"),
            vec!["phone".to_string()]
        );
    }
}
