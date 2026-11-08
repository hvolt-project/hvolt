// Config types

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Root hvolt.toml: [app] + optional [dependencies] + optional [app.signing] +
/// optional product variants (`[products.<name>]`).
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AppConfig {
    pub app: AppFields,
    /// Product variants: `[products.<name>]` overrides the identity fields, the signing
    /// config, and the resource directories. Unwritten fields inherit from the root.
    #[serde(default)]
    pub products: BTreeMap<String, ProductConfig>,
    /// Product a build selects when `--product` is absent. None = `default`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_product: Option<String>,
    /// App-wide dependencies: every member sees them.
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    /// Named dependency sources; a dependency picks one by name, `default` otherwise.
    #[serde(default)]
    pub registries: BTreeMap<String, Registry>,
    /// Set by `with_product`; never read from or written to the file.
    #[serde(skip)]
    pub resolved: Option<ResolvedProduct>,
}

/// One `[products.<name>]` table.
///
/// The override set is deliberately narrow: `bundle` / `name` / `version` / `version_code` /
/// `vendor` / `icon`. `sdk_version`, `runtime_os`, `members` and `dependencies` describe the
/// project (toolchain and module set), not the artifact, so they cannot vary per product.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct ProductConfig {
    #[serde(default)]
    pub app: ProductApp,
    /// Signing materials. Unwritten fields inherit from `[app.signing.<product>]` or
    /// `[app.signing.default]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing: Option<ProductSigning>,
    #[serde(default)]
    pub resources: ProductResources,
}

/// The artifact-level identity fields a product may override.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct ProductApp {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_code: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// `[products.<name>.resources]`: resource directories this product overlays.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct ProductResources {
    /// Project-root-relative directories, in overlay order. A later directory replaces an
    /// entry of the same name from an earlier one.
    #[serde(default)]
    pub directories: Vec<String>,
}

/// `[products.<name>.signing]`: a partial signing config; omitted fields inherit.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct ProductSigning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_alg: Option<String>,
}

impl ProductSigning {
    /// Merge these overrides over `base`: an unwritten field keeps the inherited value.
    fn over(&self, base: SigningConfig) -> SigningConfig {
        fn pick(override_value: &Option<String>, inherited: String) -> String {
            override_value.clone().unwrap_or(inherited)
        }
        SigningConfig {
            store_file: pick(&self.store_file, base.store_file),
            store_password: pick(&self.store_password, base.store_password),
            key_alias: pick(&self.key_alias, base.key_alias),
            key_password: pick(&self.key_password, base.key_password),
            certpath: pick(&self.certpath, base.certpath),
            profile: pick(&self.profile, base.profile),
            sign_alg: pick(&self.sign_alg, base.sign_alg),
        }
    }
}

/// What `with_product` resolved: the selected product and its resource overlay order.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedProduct {
    pub name: String,
    pub resource_directories: Vec<String>,
}

/// One dependency source: a public registry or a local directory served as one.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct Registry {
    /// `https://…` or `file:///…`.
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AppFields {
    pub bundle: String,
    pub name: String,
    pub version: String,
    /// versionCode. 0 = derive from `version` (semver → NNNN); DevEco tracks it separately.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub version_code: u32,
    /// app.json5 buildVersion. Empty = "1" (DevEco's default for new projects).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub build_version: String,
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
    /// targetSdkVersion; empty = same as compile_sdk_version. A project may pin the compile
    /// SDK and declare a lower target (6.1.1(24)), and that decides runtime behaviour.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target_sdk_version: String,
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
impl SigningConfig {
    /// The all-empty value a product's partial signing merges over when it inherits nothing.
    fn empty() -> Self {
        Self {
            store_file: String::new(),
            store_password: String::new(),
            key_alias: String::new(),
            key_password: String::new(),
            certpath: String::new(),
            profile: String::new(),
            sign_alg: default_sign_alg(),
        }
    }

    /// Material fields a signing config cannot do without (empty = the field is missing).
    fn missing_materials(&self) -> Vec<&'static str> {
        [
            ("store_file", &self.store_file),
            ("store_password", &self.store_password),
            ("key_alias", &self.key_alias),
            ("key_password", &self.key_password),
            ("certpath", &self.certpath),
            ("profile", &self.profile),
        ]
        .into_iter()
        .filter(|(_, value)| value.trim().is_empty())
        .map(|(name, _)| name)
        .collect()
    }

    /// DevEco Studio stores signing passwords encoded (long hex), and hap-sign-tool needs the
    /// plaintext — such a config can never sign, no matter how the build is driven.
    pub fn passwords_look_encoded(&self) -> bool {
        [&self.store_password, &self.key_password]
            .into_iter()
            .any(|pw| Self::password_looks_encoded(pw))
    }

    /// One value: 32+ hex characters is DevEco's encoded form, never a real password.
    pub fn password_looks_encoded(value: &str) -> bool {
        value.len() >= 32 && value.chars().all(|c| c.is_ascii_hexdigit())
    }
}

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
    /// targetAPIVersion: HarmonyOS's `apiTransform` concatenates every dotted part
    /// left-padded to two digits plus the major version padded to three, then strips
    /// leading zeros: "26.0.0" + 26 → "26"+"00"+"00"+"026" → 260000026. OpenHarmony
    /// keeps the bare major (26). Mirrors merge-profile.js `resolveApi`.
    pub fn target_api_version(&self) -> u32 {
        // targetSdkVersion is its own statement: a project may compile against one API level
        // and target another, and the package records the target.
        let pinned = if self.target_sdk_version.trim().is_empty() {
            &self.compile_sdk_version
        } else {
            &self.target_sdk_version
        };
        if self.runtime_os.eq_ignore_ascii_case("openharmony") {
            pinned
                .split('.')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(26)
        } else {
            api_transform(pinned)
        }
    }
    /// The API level a version string names: `5.0.5(17)` carries 17, `6.0.0` carries the
    /// major number. pack.info records this plain number, not the encoded form.
    pub fn api_level(version: &str) -> Option<u32> {
        if let (Some(open), Some(close)) = (version.find('('), version.rfind(')')) {
            if close > open {
                return version[open + 1..close].trim().parse().ok();
            }
        }
        version
            .split('.')
            .next()
            .and_then(|s| s.trim().parse().ok())
    }
    pub fn compatible_api_level(&self) -> Option<u32> {
        Self::api_level(&self.compatible_sdk_version)
    }
    pub fn target_api_level(&self) -> Option<u32> {
        let pinned = if self.target_sdk_version.trim().is_empty() {
            &self.compile_sdk_version
        } else {
            &self.target_sdk_version
        };
        Self::api_level(pinned)
    }
    /// minAPIVersion: the same encoding, from the compatible SDK pin.
    pub fn min_api_version(&self) -> u32 {
        if self.runtime_os.eq_ignore_ascii_case("openharmony") {
            self.compatible_sdk_version
                .split('.')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(26)
        } else {
            api_transform(&self.compatible_sdk_version)
        }
    }
    /// compileSdkType: the SDK family the project builds against.
    pub fn compile_sdk_type(&self) -> String {
        if self.runtime_os.eq_ignore_ascii_case("openharmony") {
            "OpenHarmony".into()
        } else {
            "HarmonyOS".into()
        }
    }

    /// Derive versionCode from semantic version string.
    /// "1.0.0" → 1_000_000, "2.3.4" → 2_003_004
    pub fn version_code(&self) -> u32 {
        // An explicit versionCode wins: DevEco tracks it independently of versionName, and
        // deriving it from semver only reproduces the source by coincidence.
        if self.version_code != 0 {
            return self.version_code;
        }
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

/// One entry of `[dependencies]`.
///
/// Cargo-shaped: `yaml = "^2.8.0"` names a version from a registry, while a table form
/// (`{ path = "deps/x", type = "arkts" }`) adds the fields a local package needs. Every field
/// is optional because the source decides which are meaningful: a path dependency needs no
/// version, a registry dependency needs no path.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
#[serde(from = "DependencyRepr")]
pub struct Dependency {
    /// Directory of a local package, relative to the project root. `None` = from a registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// semver range (`^1.2.3`, `~`, `>=`, `*`). `None` for a path dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Registry this dependency resolves through; `None` = `default`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    /// `arkts` / `js` / `native`. `None` = detect from the package contents.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Package entry file. `None` = the convention for its kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Import name when it differs from the dependency's key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Missing package is recorded as a gap instead of failing the build.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

impl Dependency {
    /// The name imports use for this dependency.
    pub fn import_name<'a>(&'a self, key: &'a str) -> &'a str {
        self.alias.as_deref().unwrap_or(key)
    }
}

/// Deserialization surface: accept the shorthand string and the table form.
#[derive(Deserialize)]
#[serde(untagged)]
enum DependencyRepr {
    Version(String),
    Table(DependencyTable),
}

#[derive(Deserialize)]
struct DependencyTable {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    registry: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    optional: bool,
}

impl From<DependencyRepr> for Dependency {
    fn from(repr: DependencyRepr) -> Self {
        match repr {
            // `"1.2.3"` is the version, exactly like Cargo's shorthand.
            DependencyRepr::Version(version) => Self {
                version: Some(version),
                ..Self::default()
            },
            DependencyRepr::Table(t) => Self {
                path: t.path,
                version: t.version,
                registry: t.registry,
                kind: t.kind,
                entry: t.entry,
                alias: t.alias,
                optional: t.optional,
            },
        }
    }
}

/// Module hvolt.toml (in subdirectory): [module]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ModuleConfig {
    pub module: ModuleInfo,
    /// Module-scoped `[dependencies]`: what this module itself imports. Declaring them here
    /// keeps `oh-package.json5` honest for shared modules, which used to inherit nothing.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, Dependency>,
}

/// HarmonyOS API-version encoding: every dotted part left-padded to 2 digits, then
/// the major version padded to 3, then leading zeros stripped ("26.0.0"+26 → 260000026).
/// HarmonyOS encodes an SDK pin as `major*10^7 + minor*10^5 + patch*10^3 + release`,
/// where `release` comes from the parenthesised build in `5.0.5(17)`.
///
/// Verified against a reference package: `6.0.0(20)` → 60000020, `5.0.5(17)` → 50005017.
fn api_transform(version: &str) -> u32 {
    let (core, release) = match version.split_once('(') {
        Some((core, tail)) => (
            core.trim(),
            tail.trim_end_matches(')')
                .trim()
                .parse::<u32>()
                .unwrap_or(0),
        ),
        None => (version.trim(), 0),
    };
    let mut parts = core
        .split('.')
        .map(|p| p.trim().parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    major * 10_000_000 + minor * 100_000 + patch * 1_000 + release
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
    /// Resource reference for the ability label (e.g. "$string:My_label").
    /// Empty = generated from the ability name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Resource reference for the ability description. Empty = generated.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Directory under `ets/` this ability lives in (e.g. "entrybackupability").
    /// Empty = the type default (entryability for UIAbility, entryformability for extensions).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dir: String,
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

/// One widget a module declares (`form_config.json` `forms[]`).
///
/// The field set mirrors what the device's form manager reads; defaults follow the manifest
/// the reference project ships, so a field omitted in `hvolt.toml` still lands with the value
/// hvigor would have written.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FormDef {
    pub name: String,
    /// e.g. `$string:widget_display_name`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Module-relative source of the widget page.
    pub src: String,
    #[serde(default = "default_ui_syntax")]
    pub ui_syntax: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<FormWindow>,
    #[serde(default = "default_color_mode")]
    pub color_mode: String,
    #[serde(default)]
    pub is_dynamic: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub update_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_update_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_duration: Option<u32>,
    pub default_dimension: String,
    #[serde(default)]
    pub support_dimensions: Vec<String>,
}

/// The `window` block of a widget definition.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FormWindow {
    #[serde(default)]
    pub design_width: u32,
    #[serde(default)]
    pub auto_design_width: bool,
}

/// `backup_config.json`. The only field the device reads is whether the app's data may be
/// restored onto a new device.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BackupConfig {
    #[serde(default)]
    pub allow_to_backup_restore: bool,
}

fn default_ui_syntax() -> String {
    "arkts".to_string()
}

fn default_color_mode() -> String {
    "auto".to_string()
}

/// One `NavPathStack` route a module registers.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RouterMapDef {
    /// The name `pushPathByName` is called with.
    pub name: String,
    /// Module-relative page (`components/More/Log.ets`). Derived when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// The `@Builder export function` the installer wraps. Derived when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_function: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ModuleInfo {
    pub name: String,
    /// Page routes for main_pages.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
    /// `NavPathStack` routes this module registers.
    ///
    /// `page` and `build_function` may be omitted: they are then derived from the module's
    /// sources by matching the route name against each file's `@Builder export function`
    /// (case-insensitive, underscore-sensitive) and its file stem. Of clashbox's nineteen
    /// routes, thirteen omit both fields, `LogDetails` omits the builder, `EditArrayConfig`
    /// omits the page, and the remaining four (`route-address`, `compatible`,
    /// `access_control`, `online_app_search`) write both out because the comparison above
    /// does not match their builders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub router_map: Vec<RouterMapDef>,
    /// The module's `main` file, which is the name the device addresses it by
    /// (`Index.ets` → `Index`). Absent = the module's own entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    /// Widget definitions, emitted as `resources/base/profile/form_config.json`.
    ///
    /// That manifest is a carrier: nothing in the build reads it, only the device does. It is
    /// expressed here so a migrated project carries no json config of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<FormDef>,
    /// `resources/base/profile/backup_config.json`, emitted from this table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<BackupConfig>,
    /// Root of the supervisual sources. When set, a page whose `.visual` sibling exists is
    /// compiled from that file instead of the `.ets` one (the reference resolves it in
    /// `findVisualFile`: same relative path, extension swapped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ace_super_visual_path: Option<String>,
    /// Entry ability file (e.g. "EntryAbility.ets"). Presence of entry implies entry module.
    #[serde(default = "default_entry_ets")]
    pub entry: String,
    /// The module's own entry file, which the device loads as the application entry point
    /// (`module.srcEntry`). Absent = the module declares none, and the runtime has to derive
    /// the entry from the main ability instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_entry: Option<String>,
    /// Skills for the entry ability. Empty = default home skill added automatically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<Skill>,
    /// Module-level metadata, e.g. ohos.arkui.UIMaterial.state = enable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<MetadataItem>,
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
    /// Native compile flags forwarded to `externalNativeOptions.cppFlags`
    /// (DevEco stores them in the module build-profile).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cpp_flags: String,
    /// CMake `arguments` forwarded to `externalNativeOptions.arguments`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub native_arguments: String,
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
    /// Entry ability orientation. Empty = omit the key (DevEco leaves it out too).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub orientation: String,
    /// Resource reference for the entry ability label. Empty = generated.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry_label: String,
    /// Resource reference for the entry ability description. Empty = generated.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry_description: String,
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
    /// `resource` and `value` are mutually exclusive in module.json5: `resource` points at a
    /// profile, `value` carries an inline string (e.g. ohos.arkui.UIMaterial.state = enable).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resource: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
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
    /// versionCode. 0 = derive from `version` (semver → NNNN); DevEco tracks it separately.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub version_code: u32,
    /// app.json5 buildVersion. Empty = "1" (DevEco's default for new projects).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub build_version: String,
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
            registries: BTreeMap::new(),
            app: AppFields {
                bundle: format!("com.example.{}", name.replace('-', "_")),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                version_code: 0,
                build_version: String::new(),
                vendor: default_vendor(),
                icon: default_icon(),
                sdk_path: None,
                sdk_version: default_sdk_version(),
                compile_sdk_version: default_sdk_version(),
                target_sdk_version: String::new(),
                compatible_sdk_version: String::new(),
                signing: BTreeMap::new(),
                bundle_type: default_bundle_type(),
                products: default_products(),
                runtime_os: default_runtime_os(),
                members: vec!["src".into()],
            },
            products: BTreeMap::new(),
            default_product: None,
            resolved: None,
            dependencies: BTreeMap::new(),
        }
    }

    /// Every product name this config can build: `[products.*]` keys, the `[app] products`
    /// list, and `default` (always addressable, so a project without variants still resolves).
    pub fn product_names(&self) -> Vec<String> {
        let mut names: std::collections::BTreeSet<String> =
            self.app.products.iter().cloned().collect();
        names.extend(self.products.keys().cloned());
        names.insert("default".to_string());
        names.into_iter().collect()
    }

    /// The product a build selects when none is named: `default_product`, else `default`.
    pub fn default_product_name(&self) -> &str {
        self.default_product
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("default")
    }

    /// Resolve one product over the root config: identity overrides, signing, resource dirs.
    ///
    /// This is the only place the inheritance rule lives. `from_file` keeps returning the file
    /// as written, so callers and tests can tell the declared config from the effective one.
    /// An unknown name is rejected with the names that would have worked.
    pub fn with_product(&self, name: &str) -> Result<AppConfig, String> {
        let name = name.trim();
        let names = self.product_names();
        if !names.iter().any(|known| known == name) {
            return Err(format!(
                "unknown product `{name}` — available products: {}",
                names.join(", ")
            ));
        }
        let product = self.products.get(name);
        let mut effective = self.clone();
        if let Some(product) = product {
            let overrides = &product.app;
            if let Some(bundle) = &overrides.bundle {
                effective.app.bundle = bundle.clone();
            }
            if let Some(display) = &overrides.name {
                effective.app.name = display.clone();
            }
            if let Some(version) = &overrides.version {
                effective.app.version = version.clone();
            }
            if let Some(code) = overrides.version_code {
                effective.app.version_code = code;
            }
            if let Some(vendor) = &overrides.vendor {
                effective.app.vendor = vendor.clone();
            }
            if let Some(icon) = &overrides.icon {
                effective.app.icon = icon.clone();
            }
        }
        let resource_directories = product
            .map(|product| product.resources.directories.clone())
            .unwrap_or_default();
        // The selected product's signing is the only entry an effective config carries:
        // `signing_for` then has exactly one answer instead of a fallback chain.
        let signing = self.resolve_signing(name, product.and_then(|p| p.signing.as_ref()))?;
        effective.app.signing = signing
            .map(|config| BTreeMap::from([(name.to_string(), config)]))
            .unwrap_or_default();
        effective.resolved = Some(ResolvedProduct {
            name: name.to_string(),
            resource_directories,
        });
        Ok(effective)
    }

    /// Merge a product's partial signing over its inherited base.
    ///
    /// Base order: `[app.signing.<product>]` (a migrated signingConfig keeps its own name),
    /// then `[app.signing.default]`. A product table that names no base and leaves required
    /// fields out is rejected here rather than at the keystore.
    fn resolve_signing(
        &self,
        product: &str,
        partial: Option<&ProductSigning>,
    ) -> Result<Option<SigningConfig>, String> {
        let base = self
            .app
            .signing
            .get(product)
            .or_else(|| self.app.signing.get("default"));
        let Some(partial) = partial else {
            return Ok(base.cloned());
        };
        let merged = partial.over(base.cloned().unwrap_or_else(SigningConfig::empty));
        let missing = merged.missing_materials();
        if !missing.is_empty() {
            return Err(format!(
                "product `{product}`: signing references no base to inherit from and leaves {} \
                 unset — write them under [products.{product}.signing] or add \
                 [app.signing.default]",
                missing.join(", ")
            ));
        }
        Ok(Some(merged))
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        toml::from_str(&content).map_err(|e| format!("parse {}: {}", path.display(), e))
    }
}

/// SDK platform version written into generated projects (`compileSdkVersion` / `sdkVersion`).
///
/// The full build id (e.g. 26.0.0.38) is the downloadable artifact; the platform version is
/// what build-profile carries. Single source: cli's `--version` stamp and `import`'s defaults
/// both derive from this value, and a test pins the stamp against it.
pub const SDK_PLATFORM_VERSION: &str = "26.0.0";

fn default_sdk_version() -> String {
    SDK_PLATFORM_VERSION.to_string()
}

fn is_zero(v: &u32) -> bool {
    *v == 0
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
    DEFAULT_RUNTIME_OS.to_string()
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

/// True when this runtimeOS builds against the OpenHarmony SDK channel.
///
/// One predicate for the case-insensitive comparison; `doctor` and the enum tables both use it.
pub fn is_open_harmony(runtime_os: &str) -> bool {
    runtime_os.eq_ignore_ascii_case("OpenHarmony")
}

/// The enum that applies to a runtimeOS. Unknown/empty → HarmonyOS, which
/// matches the platform default (default_runtime_os).
pub fn device_types_for_runtime(runtime_os: &str) -> &'static [&'static str] {
    if is_open_harmony(runtime_os) {
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

/// Why this (bundle_type, runtime_os) combination cannot be built, when it cannot.
///
/// Atomic services need the HarmonyOS SDK (`hms` components) with `runtime_os =
/// "HarmonyOS"`. This toolchain resolves only the OpenHarmony SDK, so the combination is
/// rejected here — with an explicit message — instead of failing later inside the build with
/// "Atomic service development is not supported for OpenHarmony stage model" or the
/// opaque "SDK component missing" (measured 2026-10-24 on the 26.0.0 toolchain).
pub fn unsupported_bundle_type(bundle_type: &str, runtime_os: &str) -> Option<String> {
    if !bundle_type.eq_ignore_ascii_case("atomicService") {
        return None;
    }
    Some(format!(
        "bundle_type = \"atomicService\" is not supported: atomic services require the HarmonyOS \
         SDK (hms components), and this toolchain ships only the OpenHarmony SDK, so the \
         project's runtime_os = \"{}\" cannot be satisfied. Build in an environment that provides \
         the HarmonyOS SDK, or set bundle_type = \"app\" in [app] — that changes the distribution \
         form (installation-free atomic service becomes a regular app)",
        runtime_os
    ))
}

/// The runtimeOS used when a project leaves `runtime_os` empty.
pub const DEFAULT_RUNTIME_OS: &str = "HarmonyOS";

/// The runtimeOS a project actually builds for: its declared value, or the default when empty.
///
/// Single source of truth. `translator` used to derive this inline, so a change to the default
/// would have silently drifted between the two copies.
pub fn effective_runtime_os(declared: &str) -> &str {
    if declared.is_empty() {
        DEFAULT_RUNTIME_OS
    } else {
        declared
    }
}

/// Combination checks that need both the app fields and the loaded module set.
///
/// Empty result means the combination is buildable. Kept pure so the rules can be tested
/// without a filesystem.
pub fn validate_combination(app: &AppFields, modules: &[(String, ModuleConfig)]) -> Vec<String> {
    let runtime_os = effective_runtime_os(&app.runtime_os);
    let mut errors = Vec::new();
    if let Some(reason) = unsupported_bundle_type(&app.bundle_type, runtime_os) {
        errors.push(reason);
    }
    for (name, cfg) in modules {
        let illegal = illegal_device_types(&cfg.module.device_types, runtime_os);
        if !illegal.is_empty() {
            errors.push(format!(
                "module '{}' declares device_types {:?} that runtime_os={} does not allow \
                 (official enum: [{}]) — edit {}/hvolt.toml",
                name,
                illegal,
                runtime_os,
                device_types_for_runtime(runtime_os).join(", "),
                name
            ));
        }
    }
    errors
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
            dependencies: BTreeMap::new(),
            module: ModuleInfo {
                router_map: Vec::new(),
                main: None,
                forms: Vec::new(),
                backup: None,
                src_entry: None,
                // The module name is not the source directory: the device resolves
                // `srcEntry` against it, so `src` produced records like
                // `src/src/main/EntryAbility` and the ability was never found.
                name: "entry".to_string(),
                module_type: String::new(),
                metadata: Vec::new(),
                entry: "EntryAbility.ets".to_string(),
                routes: vec!["pages/Index.ets".to_string()],
                ace_super_visual_path: None,
                skills: vec![],
                abilities: vec![],
                device_types: vec!["phone".into(), "tablet".into(), "2in1".into(), "car".into()],
                permissions: vec![],
                obfuscation: false,
                cpp_flags: String::new(),
                native_arguments: String::new(),
                obfuscation_rules: String::new(),
                icon: String::new(),
                start_window_icon: String::new(),
                orientation: String::new(),
                entry_label: String::new(),
                entry_description: String::new(),
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

    #[test]
    fn atomic_service_is_rejected_with_an_explicit_reason() {
        // The combination check must fire for an atomic service under either runtimeOS,
        // because this toolchain cannot resolve the HarmonyOS SDK.
        for runtime in ["OpenHarmony", "HarmonyOS"] {
            let reason = unsupported_bundle_type("atomicService", runtime)
                .expect("atomic service must be rejected");
            assert!(reason.contains("atomicService"));
            assert!(reason.contains("HarmonyOS SDK"));
            assert!(
                reason.contains(runtime),
                "reason names the declared runtimeOS"
            );
        }
    }

    #[test]
    fn plain_apps_pass_the_combination_check() {
        assert!(unsupported_bundle_type("app", "OpenHarmony").is_none());
        assert!(unsupported_bundle_type("app", "HarmonyOS").is_none());
        assert!(unsupported_bundle_type("", "OpenHarmony").is_none());
    }
}

#[cfg(test)]
mod signing_password_tests {
    use super::SigningConfig;

    fn cfg(store: &str, key: &str) -> SigningConfig {
        SigningConfig {
            store_file: "a.p12".into(),
            store_password: store.into(),
            key_alias: "k".into(),
            key_password: key.into(),
            certpath: "a.cer".into(),
            profile: "a.p7b".into(),
            sign_alg: "SHA256withECDSA".into(),
        }
    }

    /// DevEco writes long hex strings; those can never open the keystore.
    #[test]
    fn encoded_passwords_are_detected() {
        assert!(cfg(&"0".repeat(64), "plain").passwords_look_encoded());
        assert!(cfg("plain", &"a".repeat(48)).passwords_look_encoded());
    }

    /// A genuinely short/human password must not be mistaken for an encoded one.
    #[test]
    fn plain_passwords_pass_through() {
        assert!(!cfg("hunter2", "hunter2").passwords_look_encoded());
        assert!(!cfg("", "").passwords_look_encoded());
    }
}

#[cfg(test)]
mod version_code_tests {
    use super::AppConfig;

    fn app(version: &str, code: u32) -> AppConfig {
        let mut app = AppConfig::default_for("x");
        app.app.version = version.into();
        app.app.version_code = code;
        app
    }

    /// An explicit versionCode is authoritative: DevEco keeps it separate from versionName.
    #[test]
    fn explicit_code_wins_over_semver() {
        assert_eq!(app("1.2.3", 2005).app.version_code(), 2005);
    }

    /// Without one, the semver derivation still applies.
    #[test]
    fn semver_derivation_is_the_fallback() {
        assert_eq!(app("2.3.4", 0).app.version_code(), 2_003_004);
    }
}

#[cfg(test)]
mod combination_tests {
    use super::{
        effective_runtime_os, validate_combination, AppConfig, ModuleConfig, DEFAULT_RUNTIME_OS,
    };

    fn module(name: &str, device_types: &[&str]) -> (String, ModuleConfig) {
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = name.to_string();
        cfg.module.device_types = device_types.iter().map(|d| d.to_string()).collect();
        (name.to_string(), cfg)
    }

    /// An empty runtime_os resolves to the documented default, in exactly one place.
    #[test]
    fn empty_runtime_os_resolves_to_the_default() {
        assert_eq!(effective_runtime_os(""), DEFAULT_RUNTIME_OS);
        assert_eq!(effective_runtime_os("OpenHarmony"), "OpenHarmony");
        assert_eq!(effective_runtime_os("HarmonyOS"), "HarmonyOS");
    }

    /// An atomic service cannot be built here, under either runtimeOS.
    #[test]
    fn atomic_service_fails_the_combination() {
        for runtime in ["", "OpenHarmony", "HarmonyOS"] {
            let mut app = AppConfig::default_for("x");
            app.app.bundle_type = "atomicService".to_string();
            app.app.runtime_os = runtime.to_string();
            let errors = validate_combination(&app.app, &[]);
            assert_eq!(errors.len(), 1, "runtime_os={runtime:?}");
            assert!(errors[0].contains("atomicService"));
            assert!(errors[0].contains("HarmonyOS SDK"));
        }
    }

    /// phone is legal for HarmonyOS and illegal for OpenHarmony: the enum follows runtimeOS.
    #[test]
    fn device_types_follow_the_runtime_os() {
        let modules = vec![module("entry", &["phone"])];

        let mut harmony = AppConfig::default_for("x");
        harmony.app.runtime_os = "HarmonyOS".to_string();
        assert!(validate_combination(&harmony.app, &modules).is_empty());

        let mut open_harmony = AppConfig::default_for("x");
        open_harmony.app.runtime_os = "OpenHarmony".to_string();
        let errors = validate_combination(&open_harmony.app, &modules);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("phone"));
        assert!(errors[0].contains("entry/hvolt.toml"));
    }

    /// A plain app with legal device types passes: the checker must not fire on healthy input.
    #[test]
    fn a_buildable_combination_has_no_errors() {
        let modules = vec![module("entry", &["default"])];
        let mut app = AppConfig::default_for("x");
        app.app.runtime_os = "OpenHarmony".to_string();
        assert!(validate_combination(&app.app, &modules).is_empty());
    }
}

#[cfg(test)]
mod runtime_predicate_tests {
    use super::{device_types_for_runtime, is_open_harmony};

    /// The comparison is case-insensitive, matching how DevEco writes the field.
    #[test]
    fn open_harmony_is_matched_case_insensitively() {
        assert!(is_open_harmony("OpenHarmony"));
        assert!(is_open_harmony("openharmony"));
        assert!(is_open_harmony("OPENHARMONY"));
        assert!(!is_open_harmony("HarmonyOS"));
        assert!(!is_open_harmony(""));
    }

    /// Unknown/empty values fall back to the HarmonyOS enum, as before.
    #[test]
    fn enum_follows_the_predicate() {
        assert_eq!(
            device_types_for_runtime("openharmony"),
            super::DEVICE_TYPES_OPENHARMONY
        );
        assert_eq!(device_types_for_runtime(""), super::DEVICE_TYPES_HARMONYOS);
        assert_eq!(
            device_types_for_runtime("HarmonyOS"),
            super::DEVICE_TYPES_HARMONYOS
        );
    }
}

#[cfg(test)]
mod product_tests {
    use super::AppConfig;

    /// A project with one variant: the release product overrides identity + signing + resources.
    const VARIANT_TOML: &str = r#"
default_product = "release"

[app]
bundle = "org.example.app"
name = "app"
version = "1.0.0"
version_code = 1000000
vendor = "example"
icon = "$media:base"
products = ["default", "release"]

[app.signing.default]
store_file = "signing/default.p12"
store_password = "plain"
key_alias = "debugKey"
key_password = "plain"
certpath = "signing/default.cer"
profile = "signing/default.p7b"

[products.release.app]
bundle = "org.example.app.release"
version_code = 2000000

[products.release.resources]
directories = ["res/base", "res/release"]

[products.release.signing]
store_file = "signing/release.p12"
"#;

    fn variant_config() -> AppConfig {
        toml::from_str(VARIANT_TOML).expect("parse variant hvolt.toml")
    }

    /// Fields the product leaves out keep the root value; only the six named fields vary.
    #[test]
    fn unwritten_fields_inherit_from_root() {
        let root = variant_config();
        let release = root.with_product("release").expect("release resolves");
        assert_eq!(release.app.bundle, "org.example.app.release");
        assert_eq!(release.app.version_code, 2000000);
        // Not overridden -> root values.
        assert_eq!(release.app.name, root.app.name);
        assert_eq!(release.app.version, root.app.version);
        assert_eq!(release.app.vendor, root.app.vendor);
        assert_eq!(release.app.icon, root.app.icon);
        // Project-level fields never vary by product.
        assert_eq!(release.app.sdk_version, root.app.sdk_version);
        assert_eq!(release.app.runtime_os, root.app.runtime_os);
        assert_eq!(release.app.members, root.app.members);
        assert_eq!(release.dependencies, root.dependencies);
    }

    /// The root config stays as written; resolving produces a new value.
    #[test]
    fn resolution_does_not_mutate_the_declared_config() {
        let root = variant_config();
        let _ = root.with_product("release").expect("release resolves");
        assert_eq!(root.app.bundle, "org.example.app");
        assert!(root.resolved.is_none());
    }

    /// A product signing table is partial: unwritten fields come from `[app.signing.default]`.
    #[test]
    fn signing_fields_inherit_from_the_default_config() {
        let release = variant_config()
            .with_product("release")
            .expect("release resolves");
        let signing = release
            .app
            .signing_for("release")
            .expect("release signing resolved");
        assert_eq!(signing.store_file, "signing/release.p12");
        assert_eq!(signing.store_password, "plain");
        assert_eq!(signing.certpath, "signing/default.cer");
        assert_eq!(signing.profile, "signing/default.p7b");
    }

    /// A product with no signing table of its own inherits the root default unchanged.
    #[test]
    fn product_without_signing_table_inherits_the_root_default() {
        let toml_text = r#"
[app]
bundle = "org.example.app"
name = "app"
version = "1.0.0"
products = ["default", "release"]

[app.signing.default]
store_file = "signing/default.p12"
store_password = "plain"
key_alias = "k"
key_password = "plain"
certpath = "signing/default.cer"
profile = "signing/default.p7b"

[products.release.app]
bundle = "org.example.app.release"
"#;
        let root: AppConfig = toml::from_str(toml_text).expect("parse");
        let release = root.with_product("release").expect("release resolves");
        let signing = release.app.signing_for("release").expect("signing");
        assert_eq!(signing.store_file, "signing/default.p12");
    }

    /// A partial product signing with nothing to inherit from fails by name, not at the keystore.
    #[test]
    fn incomplete_product_signing_without_a_base_is_rejected() {
        let toml_text = r#"
[app]
bundle = "org.example.app"
name = "app"
version = "1.0.0"
products = ["default", "release"]

[products.release.signing]
store_file = "signing/release.p12"
"#;
        let root: AppConfig = toml::from_str(toml_text).expect("parse");
        let error = root
            .with_product("release")
            .expect_err("incomplete signing must be rejected");
        assert!(error.contains("store_password"), "{error}");
        assert!(error.contains("products.release.signing"), "{error}");
    }

    /// The old shape — a bare name list in `[app] products` — still parses and resolves.
    #[test]
    fn legacy_name_list_still_parses() {
        let toml_text = r#"
[app]
bundle = "org.example.app"
name = "app"
version = "1.0.0"
products = ["default", "release"]
"#;
        let root: AppConfig = toml::from_str(toml_text).expect("parse the legacy list");
        assert!(root.products.is_empty());
        assert_eq!(root.product_names(), vec!["default", "release"]);
        let release = root.with_product("release").expect("release resolves");
        assert_eq!(release.app.bundle, root.app.bundle);
        assert_eq!(release.resolved.expect("resolved").name, "release");
    }

    /// A product the config never declares is rejected with the names it does declare.
    #[test]
    fn unknown_product_names_the_available_ones() {
        let error = variant_config()
            .with_product("typo")
            .expect_err("unknown product must not resolve");
        assert!(error.contains("typo"), "{error}");
        assert!(error.contains("available products"), "{error}");
        assert!(error.contains("default"), "{error}");
        assert!(error.contains("release"), "{error}");
    }

    /// `default_product` decides which product a build selects without `--product`.
    #[test]
    fn default_product_is_the_configured_one() {
        assert_eq!(variant_config().default_product_name(), "release");
        let mut plain = variant_config();
        plain.default_product = None;
        assert_eq!(plain.default_product_name(), "default");
        plain.default_product = Some("  ".into());
        assert_eq!(plain.default_product_name(), "default");
    }

    /// The overlay order is the declaration order; later directories win at staging time.
    #[test]
    fn resource_directories_keep_their_declaration_order() {
        let release = variant_config()
            .with_product("release")
            .expect("release resolves");
        let resolved = release.resolved.expect("resolved");
        assert_eq!(resolved.resource_directories, ["res/base", "res/release"]);
        // A product that declares none overlays nothing.
        let default = variant_config()
            .with_product("default")
            .expect("default resolves");
        assert!(default
            .resolved
            .expect("resolved")
            .resource_directories
            .is_empty());
    }

    /// A config survives a serializer round trip: the writer (`hvolt migrate`) and the reader
    /// agree, including the product tables and the root-level `default_product`.
    #[test]
    fn config_round_trips_through_toml() {
        let mut original = AppConfig::default_for("roundtrip");
        original.default_product = Some("release".into());
        original.products.insert(
            "release".into(),
            toml::from_str(
                r#"
[app]
bundle = "com.example.roundtrip.release"

[resources]
directories = ["res/release"]
"#,
            )
            .expect("parse product fixture"),
        );
        let text = toml::to_string(&original).expect("serialize hvolt.toml");
        assert!(text.contains("default_product"), "{text}");
        let parsed: AppConfig = toml::from_str(&text).expect("parse hvolt.toml");
        assert_eq!(parsed, original);
    }
}
