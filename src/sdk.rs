// SDK detection, hvigorw invocation, and SDK management.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// supportVersion for HarmonyOS SDK API (from DevEco Studio oh-config.json)
const SDK_SUPPORT_VERSION: &str = "26.0-ohos-single-1";

pub struct SdkEnv {
    pub ohos_sdk: PathBuf,
    pub hvigorw: PathBuf,
    pub ohpm: PathBuf,
    pub node_home: PathBuf,
    pub java_home: PathBuf,
    pub hdc: Option<PathBuf>,
    pub emulator: Option<PathBuf>,
}

// ─── Global config ────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// User's runtime config stored in ~/.hvolt/config.json
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct GlobalConfig {
    #[serde(default)]
    sdk_path: String,
    #[serde(default)]
    node_home: String,
    #[serde(default)]
    java_home: String,
    /// Path to ohpm binary or its parent directory.
    /// When pointing to a directory, expects `<dir>/bin/ohpm`.
    /// When pointing to a file, uses it directly.
    #[serde(default)]
    pub(crate) ohpm_path: String,
    /// Persistent install target (device serial or ip:port), recorded by
    /// `hvolt device connect`. Used by push/run when no explicit device given.
    #[serde(default)]
    pub(crate) device_target: String,
    /// getSdkList supportVersion override (e.g. "27.0-ohos-single-1" once the
    /// API adds a newer SDK generation). Empty = hvolt's built-in default.
    #[serde(default)]
    pub(crate) sdk_support_version: String,
    /// Device host for cross-machine install ("user@ip[:port]").
    /// When set, push/run ship the signed HAP via scp and drive the
    /// device with that host's hdc (build host has no USB device).
    #[serde(default)]
    pub(crate) device_host: String,
    /// Absolute path of hdc ON THE DEVICE HOST (paired with device_host).
    #[serde(default)]
    pub(crate) device_hdc: String,
}

pub(crate) fn home_dir() -> PathBuf {
    crate::platform::home_dir()
}

fn global_config_path() -> PathBuf {
    home_dir().join(".hvolt").join("config.json")
}

/// Read config from an explicit path.
///
/// Missing file (or empty file) = defaults. Unreadable / unparsable = Err.
/// The distinction is the whole point: the old lenient loader made "corrupt"
/// indistinguishable from "unset", so any write path (config set/unset)
/// saved a default-filled struct and WIPED every field the user had set
/// (measured: sdk_path=/real/sdk vanished after `config set java_home`).
pub(crate) fn load_global_config_from(path: &std::path::Path) -> Result<GlobalConfig, String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(GlobalConfig::default()),
        Err(e) => return Err(format!("cannot read {}: {}", path.display(), e)),
    };
    if text.trim().is_empty() {
        return Ok(GlobalConfig::default());
    }
    serde_json::from_str(&text).map_err(|e| {
        format!(
            "{} is not valid config JSON: {} — fix or delete that file; \
             refusing to treat it as empty because saving would erase the other keys",
            path.display(),
            e
        )
    })
}

/// Strict loader used by WRITE paths: an existing-but-corrupt config aborts
/// the command instead of being silently replaced by defaults.
pub(crate) fn load_global_config_checked() -> Result<GlobalConfig, String> {
    load_global_config_from(&global_config_path())
}

/// Lenient loader used by READ paths: corrupt config still yields defaults,
/// but never silently — the problem is printed.
pub(crate) fn load_global_config() -> GlobalConfig {
    match load_global_config_checked() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("⚠ {}", e);
            GlobalConfig::default()
        }
    }
}

/// SDK store root as configured (empty string if unset).
pub(crate) fn configured_sdk_path() -> String {
    load_global_config().sdk_path
}

/// Configured node home (empty string if unset).
pub(crate) fn configured_node_home() -> String {
    load_global_config().node_home
}

/// Default SDK store location: ~/.hvolt/sdk.
pub(crate) fn sdk_store_home() -> PathBuf {
    home_dir().join(".hvolt").join("sdk")
}

pub(crate) fn save_global_config(cfg: &GlobalConfig) -> Result<(), String> {
    let path = global_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("write: {}", e))?;
    Ok(())
}

// ─── CLI entry points ─────────────────────────────────────────────

pub fn config_get() {
    let cfg = load_global_config();
    println!("hvolt configuration ({})", global_config_path().display());
    println!(
        "  sdk_path  = {}",
        if cfg.sdk_path.is_empty() {
            "(not set)"
        } else {
            &cfg.sdk_path
        }
    );
    println!(
        "  node_home = {}",
        if cfg.node_home.is_empty() {
            "(not set)"
        } else {
            &cfg.node_home
        }
    );
    println!(
        "  java_home = {}",
        if cfg.java_home.is_empty() {
            "(not set)"
        } else {
            &cfg.java_home
        }
    );
    println!(
        "  ohpm_path = {}",
        if cfg.ohpm_path.is_empty() {
            "(not set)"
        } else {
            &cfg.ohpm_path
        }
    );
    println!(
        "  device_target = {}",
        if cfg.device_target.is_empty() {
            "(not set)"
        } else {
            &cfg.device_target
        }
    );
    println!(
        "  sdk_support_version = {}",
        if cfg.sdk_support_version.is_empty() {
            "(default)"
        } else {
            &cfg.sdk_support_version
        }
    );
    println!(
        "  device_host = {}",
        if cfg.device_host.is_empty() {
            "(not set — devices are local)"
        } else {
            &cfg.device_host
        }
    );
    println!(
        "  device_hdc  = {}",
        if cfg.device_hdc.is_empty() {
            "(not set)"
        } else {
            &cfg.device_hdc
        }
    );
}

/// The single key list (set/unset branches must cover it:
/// the error text and the match arms had drifted into two different sets)
pub const CONFIG_KEYS: &[&str] = &[
    "sdk_path",
    "node_home",
    "java_home",
    "ohpm_path",
    "sdk_support_version",
    "device_target",
    "device_host",
    "device_hdc",
];

fn unknown_key(key: &str) -> String {
    format!(
        "Unknown config key '{}'. Valid: {}",
        key,
        CONFIG_KEYS.join(", ")
    )
}

/// Set a configuration key
pub fn config_set(key: &str, value: &str) -> Result<(), String> {
    let mut cfg = load_global_config_checked()?;
    match key {
        "sdk_path" => cfg.sdk_path = value.to_string(),
        "node_home" => cfg.node_home = value.to_string(),
        "java_home" => cfg.java_home = value.to_string(),
        "ohpm_path" => cfg.ohpm_path = value.to_string(),
        "sdk_support_version" => cfg.sdk_support_version = value.to_string(),
        "device_host" => cfg.device_host = value.to_string(),
        "device_hdc" => cfg.device_hdc = value.to_string(),
        "device_target" => cfg.device_target = value.to_string(),
        _ => return Err(unknown_key(key)),
    }
    save_global_config(&cfg)?;
    println!("✓ {} set to {}", key, value);
    Ok(())
}
/// Unset a configuration key
pub fn config_unset(key: &str) -> Result<(), String> {
    let mut cfg = load_global_config_checked()?;
    match key {
        "sdk_path" => cfg.sdk_path = String::new(),
        "node_home" => cfg.node_home = String::new(),
        "java_home" => cfg.java_home = String::new(),
        "ohpm_path" => cfg.ohpm_path = String::new(),
        "sdk_support_version" => cfg.sdk_support_version = String::new(),
        "device_host" => cfg.device_host = String::new(),
        "device_hdc" => cfg.device_hdc = String::new(),
        "device_target" => cfg.device_target = String::new(),
        _ => return Err(unknown_key(key)),
    }
    save_global_config(&cfg)?;
    println!("✓ {} unset", key);
    Ok(())
}

pub fn show_info() {
    let global = load_global_config();
    println!("Global config: {}", global_config_path().display());
    println!(
        "  sdk_path: {}",
        if global.sdk_path.is_empty() {
            "(not set)"
        } else {
            &global.sdk_path
        }
    );

    match detect("") {
        Ok(env) => {
            println!();
            println!("Resolved SDK: {}", env.ohos_sdk.display());
            println!("  hvigorw:  {}", env.hvigorw.display());
            println!("  java:     {}", env.java_home.display());
            println!("  node:     {}", env.node_home.display());
            println!(
                "  hdc:      {}",
                env.hdc
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(not found)".to_string())
            );
        }
        Err(e) => {
            // "no build chain" must not read as "no SDK": show
            // what IS present, then the single missing link
            println!("Build chain: incomplete — {}", e);
            let store = sdk_store_home();
            if store.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&store) {
                    let vers: Vec<String> = rd
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .filter(|n| n.starts_with('v') && store.join(n).is_dir())
                        .collect();
                    if !vers.is_empty() {
                        println!("SDKs present in store: {}", vers.join(", "));
                    }
                }
            }
            let deveco = Path::new("/Applications/DevEco-Studio.app/Contents/sdk");
            if deveco.is_dir() {
                println!("DevEco SDK found: {} (hvolt can use it — missing pieces above are what to install)", deveco.display());
            }
            println!("Next: hvolt tools install hvigor   (and ohpm) — or `hvolt doctor` for the full picture");
        }
    }
}

pub fn set_sdk(path: &str, global: bool) {
    let p = Path::new(path);
    if !p.exists() {
        eprintln!("✗ Path not found: {}", path);
        std::process::exit(1);
    }
    if global {
        let mut cfg = load_global_config_checked().unwrap_or_else(|e| {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        });
        cfg.sdk_path = path.to_string();
        save_global_config(&cfg).unwrap_or_else(|e| {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        });
        println!("✓ Global SDK set to: {}", path);
    } else {
        let toml_path = Path::new(".").join("hvolt.toml");
        if !toml_path.exists() {
            eprintln!("✗ No hvolt.toml in current directory. Use --global or run from a project.");
            std::process::exit(1);
        }
        let content = fs::read_to_string(&toml_path).unwrap_or_default();
        let new_content = if content.contains("sdk_path") {
            let re = regex_lite::Regex::new(r#"sdk_path\s*=\s*"[^"]*""#).unwrap();
            re.replace(&content, format!("sdk_path = \"{}\"", path))
                .to_string()
        } else {
            content.replace("[app]", &format!("[app]\nsdk_path = \"{}\"", path))
        };
        fs::write(&toml_path, new_content).unwrap_or_else(|e| {
            eprintln!("✗ write hvolt.toml: {}", e);
            std::process::exit(1);
        });
        println!("✓ Project SDK set to: {}", path);
    }
}

// ─── SDK detection ────────────────────────────────────────────────

pub fn detect(sdk_path_override: &str) -> Result<SdkEnv, String> {
    let ohos_sdk = find_ohos_sdk(sdk_path_override)?;
    let (hvigorw, ohpm, node_home, java_home, hdc, emulator) = find_tools(&ohos_sdk)?;
    Ok(SdkEnv {
        ohos_sdk,
        hvigorw,
        ohpm,
        node_home,
        java_home,
        hdc,
        emulator,
    })
}

/// Root of the hvolt-managed SDK store (`~/.hvolt/sdk`), returned only when it
/// actually carries components: `default/openharmony/ets` or any `v*/openharmony/ets`.
/// The store root is the shape downstream expects — its parent holds `sdk-ohos/`.
pub(crate) fn managed_sdk_root() -> Option<PathBuf> {
    let store = home_dir().join(".hvolt").join("sdk");
    if !store.is_dir() {
        return None;
    }
    if store.join("default/openharmony/ets").is_dir() {
        return Some(store);
    }
    for entry in fs::read_dir(&store).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('v') && entry.path().join("openharmony/ets").is_dir() {
            return Some(store);
        }
    }
    None
}

/// DevEco SDK root, but only when the installation carries components. An
/// uninstalled IDE leaves `<install>/sdk` behind with no components, and that
/// skeleton must not shadow the managed store.
fn deveco_sdk_path(deveco: &Path) -> Option<PathBuf> {
    // Prefer the SDK root that also carries `hms`: pointing DEVECO_SDK_HOME at
    // `<sdk>/default/openharmony` makes hvigor report "SDK component missing" for every
    // runtimeOS=HarmonyOS project, because the HMS components are not under openharmony/.
    let root = deveco.join("sdk");
    let candidates = [
        root.clone(),
        root.join("default"),
        root.join("default/openharmony"),
    ];
    candidates.into_iter().find(|cand| {
        cand.join("ets").is_dir()
            || cand.join("openharmony/ets").is_dir()
            || cand.join("default/openharmony/ets").is_dir()
    })
}

/// SDK component directory only — no tool checks. Used by consumers that need the
/// SDK and nothing else (the language server), where requiring hvigor/ohpm would
/// report an unrelated failure.
pub(crate) fn sdk_dir(toml_override: &str) -> Result<PathBuf, String> {
    find_ohos_sdk(toml_override)
}

/// SDK root resolved through the same order `hvolt build` uses, but without demanding
/// build tools. `doctor` reports this so "DevEco is installed" and "the SDK is missing"
/// stop being told at the same time.
pub(crate) fn discovered_sdk_root(toml_override: &str) -> Option<PathBuf> {
    find_ohos_sdk(toml_override).ok()
}

fn find_ohos_sdk(toml_override: &str) -> Result<PathBuf, String> {
    // 1. hvolt.toml sdk_path
    if !toml_override.is_empty() {
        let p = Path::new(toml_override);
        return if p.exists() {
            Ok(p.to_path_buf())
        } else {
            Err(format!("sdk_path not found: {}", toml_override))
        };
    }
    // 2. Global config
    let global = load_global_config();
    if !global.sdk_path.is_empty() {
        let p = Path::new(&global.sdk_path);
        if p.exists() {
            return Ok(p.to_path_buf());
        }
    }
    // 3. hvolt-managed store (~/.hvolt/sdk): the default version, else the newest
    if let Some(root) = managed_sdk_root() {
        return Ok(root);
    }
    // 4. Env vars
    for var in &["DEVECO_SDK_HOME", "HOS_SDK_HOME", "OHOS_SDK_HOME"] {
        if let Ok(val) = std::env::var(var) {
            let p = Path::new(&val);
            if !val.is_empty() && p.exists() {
                return Ok(p.to_path_buf());
            }
        }
    }
    // 5. DevEco Studio, only when it actually carries components: an uninstalled
    //    IDE leaves an empty skeleton behind, and that skeleton used to win over
    //    the managed store (measured on Windows, 2026-10-18).
    let deveco = Path::new(if cfg!(windows) {
        "C:/Program Files/Huawei/DevEco Studio"
    } else {
        "/Applications/DevEco-Studio.app/Contents"
    });
    if let Some(sdk) = deveco_sdk_path(deveco) {
        return Ok(sdk);
    }
    Err(
        "HarmonyOS SDK not found. Run `hvolt sdk download <version>` or `hvolt sdk set <path>`"
            .into(),
    )
}
/// Tool paths resolved from an installed OpenHarmony SDK.
/// (hvigorw, ohpm, hdc, java, signing tool, node)
type ToolPaths = (
    PathBuf,
    PathBuf,
    PathBuf,
    PathBuf,
    Option<PathBuf>,
    Option<PathBuf>,
);

fn find_tools(ohos_sdk: &Path) -> Result<ToolPaths, String> {
    // hvigorw + ohpm: only from ~/.hvolt/tools/ (independent management)
    let hvigorw = crate::tools::find_hvigorw()
        .ok_or_else(|| "hvigorw not found. Run `hvolt tools install hvigor`.".to_string())?;
    let ohpm = crate::tools::find_ohpm()
        .ok_or_else(|| "ohpm not found. Run `hvolt tools install ohpm`.".to_string())?;

    // Node.js: config → env var → PATH
    let node_home = find_node()?;

    // JDK: config → env var → PATH
    let java_home = find_java()?;

    // hdc: from SDK toolchains, fallback to PATH
    let hdc = find_hdc_from_sdk(ohos_sdk).or_else(crate::hdc::find_hdc);

    // emulator: managed through hdc (not independently discoverable)
    Ok((hvigorw, ohpm, node_home, java_home, hdc, None))
}

/// Find hdc binary from SDK toolchains: <sdk_root>/default/openharmony/toolchains/hdc
pub(crate) fn find_hdc_from_sdk(sdk_root: &Path) -> Option<PathBuf> {
    if !sdk_root.exists() {
        return None;
    }

    // Check default/openharmony/toolchains first
    let default_toolchains = sdk_root.join("default/openharmony/toolchains/hdc");
    if default_toolchains.exists() {
        return Some(default_toolchains);
    }

    // Fallback: scan version dirs for toolchains/hdc
    if let Ok(entries) = fs::read_dir(sdk_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join("openharmony/toolchains/hdc");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Resolve the actual node binary from a node_home. node_home has TWO
/// legitimate shapes: install root (…/node-v20/bin/node) or the bin dir
/// itself (PATH discovery returns parent-of-binary, e.g. /opt/homebrew/bin).
/// Probers must not guess a third shape — doctor's unconditional
/// home.join("bin/node") produced /opt/homebrew/bin/bin/node false
/// negatives on the most common macOS install.
pub(crate) fn node_bin(home: &std::path::Path) -> std::path::PathBuf {
    for cand in [
        home.join("bin").join("node"),
        home.join("node"),
        home.join("bin").join("node.exe"),
        home.join("node.exe"),
    ] {
        if cand.exists() {
            return cand;
        }
    }
    home.join("bin").join("node") // nothing found: report the canonical shape
}

/// Same dual-shape contract for java (PATH-discovered /usr/bin must not
/// become /usr/bin/bin/java in diagnostics).
pub(crate) fn java_bin(home: &std::path::Path) -> std::path::PathBuf {
    for cand in [
        home.join("bin").join("java"),
        home.join("java"),
        home.join("bin").join("java.exe"),
        home.join("java.exe"),
    ] {
        if cand.exists() {
            return cand;
        }
    }
    home.join("bin").join("java")
}

/// Find Node.js home: hvolt config → NODE_HOME env → PATH which
pub(crate) fn find_node() -> Result<PathBuf, String> {
    // node_home semantics: the directory from which apply_tool_path can
    // locate the binary — either root/bin/node (posix) or root/node.exe
    // (Windows layouts: pi-node\current, nodejs install dir). tool_bin_dir
    // resolves either, so validity = "binary findable under this root".
    let node_ok = |p: &Path| {
        crate::platform::find_in_dirs("node", [p.join("bin"), p.to_path_buf()]).is_some()
    };
    let cfg = load_global_config();
    if !cfg.node_home.is_empty() {
        let p = Path::new(&cfg.node_home);
        if node_ok(p) {
            return Ok(p.to_path_buf());
        }
    }
    if let Ok(val) = std::env::var("NODE_HOME") {
        if !val.is_empty() {
            let p = Path::new(&val);
            if node_ok(p) {
                return Ok(p.to_path_buf());
            }
        }
    }
    if let Some(np) = crate::platform::on_path("node") {
        // node.exe lives IN its home on Windows; posix /usr/bin/node too
        // (npm side-by-side). parent-of-binary is the correct root here.
        if let Some(parent) = np.parent() {
            return Ok(parent.to_path_buf());
        }
    }
    Err("Node.js not found. Install it with your package manager — `scoop install nodejs-lts` (Windows) or `brew install node@20` (macOS) — or set NODE_HOME / `hvolt config set node_home <path>`".into())
}

/// Find JDK home: hvolt config → JAVA_HOME env → PATH which
pub(crate) fn find_java() -> Result<PathBuf, String> {
    let cfg = load_global_config();
    if !cfg.java_home.is_empty() {
        let p = Path::new(&cfg.java_home);
        if p.join("bin/java").exists() || p.join("bin/java.exe").exists() {
            return Ok(p.to_path_buf());
        }
    }
    if let Ok(val) = std::env::var("JAVA_HOME") {
        if !val.is_empty() {
            let p = Path::new(&val);
            if p.join("bin/java").exists() {
                return Ok(p.to_path_buf());
            }
        }
    }
    if let Some(jp) = crate::platform::on_path("java") {
        let path = jp.to_string_lossy().trim().to_string();
        if !path.is_empty() {
            if let Some(parent) = Path::new(&path).parent() {
                if let Some(grandparent) = parent.parent() {
                    return Ok(grandparent.to_path_buf());
                }
            }
        }
    }
    Err("JDK not found. Install it with your package manager — `scoop install temurin17-jdk-lts` (Windows) or `brew install --cask zulu@17` (macOS) — or set JAVA_HOME / `hvolt config set java_home <path>`".into())
}

// ─── SDK version management ────────────────────────────────────────

/// An installed SDK version discovered on disk
#[derive(Debug, Clone)]
pub struct InstalledSdk {
    pub api_version: String,
    pub platform_version: String,
    pub display_name: String,
    pub release_type: String,
    pub version: String,
    pub dir_name: String,
    pub path: PathBuf,
    pub components: Vec<String>,
}

/// Ensure the SDK root has a `default` symlink pointing to the given version directory.
/// `DEVECO_SDK_HOME` must point to the SDK root (parent of `default/`), or hvigor
/// fails with "SDK component missing".
fn ensure_default_symlink(sdk_root: &Path, version_dir: &Path) -> Result<(), String> {
    let default_link = sdk_root.join("default");
    // replace only the link hvolt created; a REAL dir at default/ is user data
    match default_link.symlink_metadata() {
        Ok(md) if md.file_type().is_symlink() => {
            #[cfg(unix)]
            {
                let _ = fs::remove_file(&default_link);
            }
            #[cfg(windows)]
            {
                let _ = fs::remove_dir(&default_link);
            }
        }
        Ok(md) if md.is_dir() => {
            // empty real dir from old broken runs is safe to drop; non-empty refuse
            let empty = fs::read_dir(&default_link)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if empty {
                let _ = fs::remove_dir(&default_link);
            } else {
                return Err(format!(
                    "{} is a real non-empty dir — not overwriting",
                    default_link.display()
                ));
            }
        }
        Ok(_) => {
            let _ = fs::remove_file(&default_link);
        }
        Err(_) => {}
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(version_dir, &default_link)
        .map_err(|e| format!("create default symlink: {}", e))?;
    #[cfg(windows)]
    crate::build_hvigor::junction(&default_link, version_dir)
        .map_err(|e| format!("create default link: {}", e))?;
    Ok(())
}
/// A remote SDK version available for download
#[derive(Debug, Clone)]
pub struct RemoteSdkVersion {
    pub version: String,
    pub display_name: String,
    pub api_version: String,
    pub release_type: String,
    pub components: Vec<RemoteComponent>,
}

/// A downloadable SDK component within a version
#[derive(Debug, Clone)]
pub struct RemoteComponent {
    pub path: String,
    pub display_name: String,
    pub archive_url: String,
    pub size: u64,
    pub checksum: String,
}

/// Parse sdk-pkg.json from a version directory
fn parse_sdk_pkg(dir: &Path) -> Option<InstalledSdk> {
    let pkg_path = dir.join("sdk-pkg.json");
    if !pkg_path.exists() {
        return None;
    }
    let text = fs::read_to_string(&pkg_path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let data = v.get("data")?;
    let api_version = data.get("apiVersion")?.as_str()?.to_string();
    let platform_version = data.get("platformVersion")?.as_str()?.to_string();
    let display_name = data.get("displayName")?.as_str()?.to_string();
    let version = data.get("version")?.as_str()?.to_string();
    let release_type = data
        .get("releaseType")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Scan openharmony/ subdirectory for components
    let oh = dir.join("openharmony");
    let mut components = Vec::new();
    if oh.exists() && oh.is_dir() {
        if let Ok(entries) = fs::read_dir(&oh) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') {
                        components.push(name);
                    }
                }
            }
        }
    }
    components.sort();

    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    Some(InstalledSdk {
        api_version,
        platform_version,
        display_name,
        release_type,
        version,
        dir_name,
        path: dir.to_path_buf(),
        components,
    })
}

/// Find the DevEco Studio SDK root directory (parent of version dirs).
pub(crate) fn sdk_root() -> Option<PathBuf> {
    // 1. Global config sdk_path
    let global = load_global_config();
    if !global.sdk_path.is_empty() {
        let p = Path::new(&global.sdk_path);
        if p.exists() && (p.join("sdk-pkg.json").exists() || p.join("openharmony").exists()) {
            return p.parent().map(|pp| pp.to_path_buf());
        }
    }
    // 2. User-managed SDK store ~/.hvolt/sdk/
    let user_sdk = home_dir().join(".hvolt").join("sdk");
    if user_sdk.exists() {
        return Some(user_sdk);
    }
    // 3. Auto-detect from DevEco Studio (mac bundle or windows install)
    let mut deveco_roots: Vec<std::path::PathBuf> =
        vec![PathBuf::from("/Applications/DevEco-Studio.app/Contents")];
    if cfg!(windows) {
        deveco_roots.push(PathBuf::from("C:/Program Files/Huawei/DevEco Studio"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            deveco_roots.push(std::path::Path::new(&l).join("Huawei/DevEcoStudio"));
        }
    }
    for root in deveco_roots {
        let sdk = root.join("sdk");
        if sdk.exists() {
            return Some(sdk);
        }
    }
    // 4. Env vars
    for var in &["DEVECO_SDK_HOME", "HOS_SDK_HOME", "OHOS_SDK_HOME"] {
        if let Ok(val) = std::env::var(var) {
            let p = Path::new(&val);
            if p.exists() {
                return Some(p.to_path_buf());
            }
        }
    }
    None
}

/// List all installed SDK versions
pub fn list_installed() {
    let root = match sdk_root() {
        Some(r) => r,
        None => {
            println!("No SDK installation found. Install DevEco Studio or set SDK path.");
            return;
        }
    };

    let mut versions: Vec<InstalledSdk> = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(sdk) = parse_sdk_pkg(&path) {
                    versions.push(sdk);
                }
            }
        }
    }

    if versions.is_empty() {
        println!("No SDK versions found in {}", root.display());
        return;
    }

    versions.sort_by(|a, b| b.version.cmp(&a.version));

    println!("Installed SDK versions (in {}):", root.display());
    for (i, sdk) in versions.iter().enumerate() {
        let active_mark = if sdk.dir_name == "default" {
            " *"
        } else {
            "  "
        };
        println!(
            "{}{}. {} ({})",
            active_mark,
            i + 1,
            sdk.display_name,
            sdk.version
        );
        println!(
            "     platform: {}, api: {}, type: {}",
            sdk.platform_version, sdk.api_version, sdk.release_type
        );
        println!("     path: {}", sdk.path.display());
        if !sdk.components.is_empty() {
            println!("     components: {}", sdk.components.join(", "));
        }
    }
    if versions.iter().any(|v| v.dir_name == "default") {
        println!("\n  * default = currently active version (via DevEco Studio)");
    }
}

/// Switch to a specific SDK version by storing its path in global config.
pub fn use_version(version_id: &str) -> Result<(), String> {
    let root = sdk_root().ok_or_else(|| "SDK not found".to_string())?;

    let mut found: Option<InstalledSdk> = None;
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(sdk) = parse_sdk_pkg(&path) {
                    if sdk.version == version_id
                        || sdk.dir_name == version_id
                        || sdk.api_version == version_id
                    {
                        found = Some(sdk);
                        break;
                    }
                }
            }
        }
    }

    let sdk = found.ok_or_else(|| {
        format!(
            "SDK version '{}' not found. Use `hvolt sdk list` to see installed versions.",
            version_id
        )
    })?;

    let version_path = &sdk.path;
    let sdk_root = version_path
        .parent()
        .ok_or_else(|| format!("cannot determine SDK root from {}", version_path.display()))?;

    ensure_default_symlink(sdk_root, version_path)?;

    let mut cfg = load_global_config_checked()?;
    cfg.sdk_path = sdk_root.to_string_lossy().to_string();
    save_global_config(&cfg)?;

    println!(
        "✓ Switched to SDK version {} ({})",
        sdk.version, sdk.display_name
    );
    println!("  Path: {}", version_path.display());
    Ok(())
}
/// Fetch the SDK component list from the remote API.
fn get_sdk_list() -> Result<Vec<serde_json::Value>, String> {
    let url = "https://repo.harmonyos.com/sdkmanager/v5/ohos/getSdkList";

    // API 实测支持的 osType/osArch 组合（2026-10-09 服务器实测）：
    //   darwin:arm64 ✅   linux:x64 ✅   linux:x86_64 ✗(134012 osArch参数异常)
    //   osType=centos/ubuntu/ohos/openharmony ✗(134004 osType参数异常)
    // 此前 linux 借用 darwin 参数是因 osArch 传错值（x86_64）导致的误判——
    // API 实际返回按 osType/osArch 区分平台的构建（linux/x64 的 toolchains 是 ELF 构建）。
    let (os_type, os_arch) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => ("darwin", "arm64"),
        ("linux", "x86_64") => ("linux", "x64"),
        ("linux", "aarch64") => ("linux", "arm64"),
        ("windows", "x86_64") => ("windows", "x64"),
        _ => {
            return Err(format!(
                "unsupported platform: {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ))
        }
    };

    let cfg = load_global_config();
    let support_version = if cfg.sdk_support_version.is_empty() {
        SDK_SUPPORT_VERSION.to_string()
    } else {
        cfg.sdk_support_version.clone()
    };
    let body = serde_json::json!({
        "osType": os_type,
        "osArch": os_arch,
        "supportVersion": support_version,
    });

    // Body via @file: Windows argv quote round-tripping (Rust escape × MSVCRT
    // unescape) mangles inline JSON when handed to curl.exe — measured.
    let body_file = std::env::temp_dir().join(format!("hvolt-sdklist-{}.json", std::process::id()));
    fs::write(&body_file, body.to_string()).map_err(|e| format!("write body: {}", e))?;
    let darg = format!("-d@{}", body_file.to_string_lossy());
    let response = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            url,
            "-H",
            "Content-Type: application/json",
            &darg,
        ])
        .output()
        .map_err(|e| format!("curl request failed: {}", e))?;
    let _ = fs::remove_file(&body_file);

    if !response.status.success() {
        let stderr = String::from_utf8_lossy(&response.stderr);
        return Err(format!("API request failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&response.stdout);
    let data: Vec<serde_json::Value> = serde_json::from_str(&stdout).map_err(|e| {
        format!(
            "parse API response: {} ({})",
            e,
            stdout.chars().take(200).collect::<String>()
        )
    })?;

    Ok(data)
}

/// List available SDK versions from remote.
pub fn list_remote() -> Result<Vec<RemoteSdkVersion>, String> {
    let data = get_sdk_list()?;

    // Group components by version key (apiVersion + version)
    let mut groups: std::collections::BTreeMap<String, Vec<&serde_json::Value>> =
        std::collections::BTreeMap::new();
    for item in &data {
        let api_ver = item["apiVersion"].as_str().unwrap_or("");
        let version = item["version"].as_str().unwrap_or("");
        let _display_name = item["displayName"].as_str().unwrap_or("");
        let key = format!("{}-{}", api_ver, version);
        groups.entry(key).or_default().push(item);
    }

    let mut versions = Vec::new();
    // Reverse order: latest first
    for (_key, items) in groups.iter().rev() {
        let first = items[0];
        let ver = first["version"].as_str().unwrap_or("?").to_string();
        let display = first["displayName"].as_str().unwrap_or("?").to_string();
        let api_ver = first["apiVersion"].as_str().unwrap_or("?").to_string();
        let release_type = first["releaseType"].as_str().unwrap_or("?").to_string();

        let mut seen_paths = std::collections::HashSet::new();
        let mut components = Vec::new();
        for item in items {
            let path = item["path"].as_str().unwrap_or("");
            if !seen_paths.insert(path.to_string()) {
                continue; // deduplicate by path
            }
            let archive = &item["archive"];
            components.push(RemoteComponent {
                path: path.to_string(),
                display_name: item["displayName"].as_str().unwrap_or("").to_string(),
                archive_url: archive["url"].as_str().unwrap_or("").to_string(),
                size: archive["size"].as_str().unwrap_or("0").parse().unwrap_or(0),
                checksum: archive["checksum"].as_str().unwrap_or("").to_string(),
            });
        }
        versions.push(RemoteSdkVersion {
            version: ver,
            display_name: display,
            api_version: api_ver,
            release_type,
            components,
        });
    }

    Ok(versions)
}

/// Download and install a specific SDK version.
/// dry_run: only report components + sizes, touch nothing.
pub fn download_sdk(
    version: &str,
    components_filter: &[String],
    dry_run: bool,
) -> Result<(), String> {
    let versions = list_remote()?;

    // Find matching version (by version string or apiVersion prefix)
    let target = versions
        .iter()
        .find(|v| v.version == version || v.api_version == version)
        .ok_or_else(|| {
            let available: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
            format!(
                "SDK version '{}' not found. Available: {}",
                version,
                available.join(", ")
            )
        })?;

    let to_download = select_components(&target.components, components_filter);
    if to_download.is_empty() {
        let known: Vec<&str> = target.components.iter().map(|c| c.path.as_str()).collect();
        return Err(format!(
            "No matching components to download. Known: {}",
            known.join(", ")
        ));
    }
    let total: u64 = to_download.iter().map(|c| c.size).sum();

    if dry_run {
        println!(
            "SDK {} ({}, {} components, {} total):",
            target.version,
            target.release_type,
            to_download.len(),
            fmt_size(total)
        );
        for c in &to_download {
            println!(
                "  {:<12} {:>10}  {}",
                c.path,
                fmt_size(c.size),
                c.display_name
            );
        }
        let user_sdk = home_dir()
            .join(".hvolt")
            .join("sdk")
            .join(format!("v{}", target.version));
        println!("Would install to: {}", user_sdk.display());
        return Ok(());
    }

    let user_sdk = home_dir().join(".hvolt").join("sdk");
    let version_dir_name = format!("v{}", target.version);
    let target_dir = user_sdk.join(&version_dir_name);

    let present = |c: &RemoteComponent| component_dir(&target_dir, &c.path).is_some();
    if target_dir.join("sdk-pkg.json").exists() {
        if target.components.iter().all(&present) {
            // idempotent convergence: finish whatever an interrupted earlier
            // run left undone (default link / view) instead of refusing
            if let Some(parent) = user_sdk.parent() {
                ensure_default_symlink(&user_sdk, &target_dir)?;
                let ohos_view = parent.join("sdk-ohos").join(&target.api_version);
                let all_refs: Vec<&RemoteComponent> = target.components.iter().collect();
                if let Err(e) = create_ohos_view(&ohos_view, &target_dir, &all_refs) {
                    eprintln!("  ⚠ OpenHarmony runtime view not created: {}", e);
                }
            }
            println!(
                "✓ SDK {} already installed; default/view links converged",
                version
            );
            return Ok(());
        }
        return Err(format!(
            "SDK version {} already installed at {}. Use `hvolt sdk use {}`.",
            version,
            target_dir.display(),
            target.version
        ));
    }

    // A pre-existing dir without sdk-pkg.json = interrupted run: resume into it
    // (partial .tmp files resume per component, extracted ones are skipped).
    fs::create_dir_all(&target_dir).map_err(|e| format!("mkdir: {}", e))?;

    println!(
        "Downloading SDK {} ({}, {} components, {} total)...",
        target.version,
        target.release_type,
        to_download.len(),
        fmt_size(total)
    );

    let mut downloaded: Vec<&RemoteComponent> = Vec::new();
    for component in &to_download {
        if component_dir(&target_dir, &component.path).is_some() {
            println!("  ✓ {} — already extracted, skipping", component.path);
            downloaded.push(component);
            continue;
        }
        download_component(component, &target_dir)?;
        downloaded.push(component);
    }
    let to_download = downloaded;

    // Create openharmony/ directory structure
    // Each component ZIP is already structured: when extracted, toolchains -> toolchains/
    // We need to rename to match the SDK layout convention
    let oh_dir = target_dir.join("openharmony");
    fs::create_dir_all(&oh_dir).map_err(|e| format!("mkdir openharmony: {}", e))?;

    // Move each component dir into openharmony/
    for component in &to_download {
        let src = target_dir.join(&component.path);
        let dst = oh_dir.join(&component.path);
        if src.exists() && !dst.exists() {
            fs::rename(&src, &dst)
                .map_err(|e| format!("move {} to openharmony/: {}", component.path, e))?;
        }
    }

    // Set as default only when THIS becomes a complete package.
    let all_present = target.components.iter().all(&present);
    if all_present {
        // Set as default
        ensure_default_symlink(&user_sdk, &target_dir)?;
    } else {
        let have: Vec<&str> = target
            .components
            .iter()
            .filter(|c| {
                component_dir(&target_dir, &c.path)
                    .map(|p| p.join("oh-uni-package.json").exists() || p.exists())
                    .unwrap_or(false)
            })
            .map(|c| c.path.as_str())
            .collect();
        println!(
            "  ⚠ partial install ({} of {} components: {}) — default NOT switched;",
            have.len(),
            target.components.len(),
            have.join(",")
        );
        println!("    complete it later with the same `hvolt sdk download {}` (remaining resume automatically)", version);
    }

    // Build the OpenHarmony runtime view alongside the store. Gate: either
    // the full manifest is present, or THIS run's selection is complete
    // (demo-class installs pick 3 of 5 deliberately — refusing to build the
    // view would strand them; links are re-pointed by later downloads and
    // self-healed on remove, so a selected-partial view is safe).
    let selection_present = !to_download.is_empty() && to_download.iter().all(|c| present(c));
    if all_present || selection_present {
        if let Some(parent) = user_sdk.parent() {
            let ohos_view = parent.join("sdk-ohos").join(&target.api_version);
            // ALL components, not just this run's — re-running download with
            // nothing new must still (re)build the view (self-heal path)
            let all_refs: Vec<&RemoteComponent> = target.components.iter().collect();
            if let Err(e) = create_ohos_view(&ohos_view, &target_dir, &all_refs) {
                eprintln!("  ⚠ OpenHarmony runtime view not created: {}", e);
            } else {
                println!(
                    "  ✓ OpenHarmony view: {}",
                    parent.join("sdk-ohos").display()
                );
            }
        }
    }

    // Completeness marker LAST: default+view above succeeded, so re-runs can
    // converge via the already-installed path; a failure before this point
    // leaves the resume entry (no sdk-pkg.json) intact.
    if all_present && !target_dir.join("sdk-pkg.json").exists() {
        let pkg = sdk_pkg_json(&target.api_version, &target.version, &target.release_type);
        fs::write(
            target_dir.join("sdk-pkg.json"),
            serde_json::to_string_pretty(&pkg).map_err(|e| format!("write sdk-pkg.json: {}", e))?,
        )
        .map_err(|e| format!("write sdk-pkg.json: {}", e))?;
    }

    // Update global config
    let mut cfg = load_global_config_checked()?;
    cfg.sdk_path = user_sdk.to_string_lossy().to_string();
    save_global_config(&cfg)?;

    println!(
        "✓ SDK {} downloaded to {}",
        target.version,
        target_dir.display()
    );
    println!(
        "  Components: {}",
        to_download
            .iter()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

/// <root>/{api_version}/{component} -> {sdk version dir}/openharmony/{component}
/// Platform-aware directory link: symlink (unix) or junction (windows).
/// View maintenance uses this rather than a unix-only symlink.
fn link_component(link: PathBuf, target: &Path) -> Result<(), String> {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, &link)
        .map_err(|e| format!("link {}: {}", link.display(), e));
    #[cfg(windows)]
    return crate::build_hvigor::junction(&link, target);
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (link, target);
        Err("linking unsupported on this platform".into())
    }
}

/// Where a component actually landed for this platform's package shape:
/// `openharmony/<comp>` (linux/darwin zips) or `<comp>` at the version root
/// (some windows zips — measured: native).
pub(crate) fn component_dir(version_dir: &Path, comp: &str) -> Option<PathBuf> {
    let a = version_dir.join("openharmony").join(comp);
    if a.exists() {
        return Some(a);
    }
    let b = version_dir.join(comp);
    if b.exists() {
        return Some(b);
    }
    None
}

fn create_ohos_view(
    view_dir: &Path,
    sdk_version_dir: &Path,
    components: &[&RemoteComponent],
) -> Result<(), String> {
    fs::create_dir_all(view_dir).map_err(|e| format!("mkdir {}: {}", view_dir.display(), e))?;
    for c in components {
        let src = match component_dir(sdk_version_dir, &c.path) {
            Some(p) => p,
            None => continue,
        };
        let dst = view_dir.join(&c.path);
        // Replace only links hvolt created; never touch real dirs or files
        // the user may have placed at the view path.
        match dst.symlink_metadata() {
            Ok(md) if md.file_type().is_symlink() => {
                // directory links (unix symlink-to-dir / Windows junction)
                // must be removed via remove_dir: remove_file fails against
                // a Windows reparse point and left the view stuck (measured)
                #[cfg(unix)]
                {
                    let _ = fs::remove_file(&dst);
                }
                #[cfg(windows)]
                {
                    let _ = fs::remove_dir(&dst);
                }
            }
            Ok(_) => {
                return Err(format!(
                    "{} exists as a real file/dir — not overwriting",
                    dst.display()
                ))
            }
            Err(_) => {} // absent: proceed
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&src, &dst).map_err(|e| format!("link {}: {}", c.path, e))?;
        #[cfg(windows)]
        crate::build_hvigor::junction(&dst, &src).map_err(|e| format!("link {}: {}", c.path, e))?;
    }
    Ok(())
}

/// Download a single component ZIP, verify checksum, and extract.
/// Resume is attempted first (`-C -`); if the server rejects ranged requests
/// curl fails, the next attempt starts fresh, so a partial download cannot wedge the flow.
fn download_component(component: &RemoteComponent, target_dir: &Path) -> Result<(), String> {
    let label = &component.path;
    println!(
        "  {} ({}) — {}",
        component.display_name,
        component.path,
        fmt_size(component.size)
    );
    println!("    URL: {}", component.archive_url);

    let dest = target_dir.join(format!("_{}.tmp", label));
    let has_partial = dest.exists();

    // attempt 1: resume if a partial file is on disk, else fresh
    let mut status = Command::new("curl")
        .args(curl_download_args(
            &component.archive_url,
            &dest,
            has_partial,
        ))
        .status()
        .map_err(|e| format!("curl {}: {}", label, e))?;

    // resume attempt failed with a partial on disk → server may not accept
    // ranges or the partial is stale: wipe and retry from zero, once.
    if !status.success() && has_partial {
        eprintln!("    resume failed, retrying from scratch...");
        let _ = fs::remove_file(&dest);
        status = Command::new("curl")
            .args(curl_download_args(&component.archive_url, &dest, false))
            .status()
            .map_err(|e| format!("curl {}: {}", label, e))?;
    }

    if !status.success() {
        // keep whatever bytes arrived — the next run resumes from them.
        // (checksum gate below still protects integrity)
        return Err(format!(
            "download {} failed (partial kept at {} — rerun to resume)",
            label,
            dest.display()
        ));
    }

    // Verify checksum — pure Rust, always on (partial resume also passes
    // through this gate before extraction)
    if !component.checksum.is_empty() {
        match sha256_file(&dest) {
            Ok(actual) if actual == component.checksum => {}
            Ok(actual) => {
                let _ = fs::remove_file(&dest);
                return Err(format!(
                    "checksum mismatch for {}: expected {}, got {}",
                    label, component.checksum, actual
                ));
            }
            Err(e) => {
                let _ = fs::remove_file(&dest);
                return Err(format!("checksum for {}: {}", label, e));
            }
        }
    }

    // Extract ZIP with the embedded pure-Rust engine (no unzip dependency —
    // Windows hosts don't ship one)
    if let Err(e) = crate::platform::extract_zip(&dest, target_dir) {
        let _ = fs::remove_file(&dest);
        return Err(format!("extract {} failed: {}", label, e));
    }

    let _ = fs::remove_file(&dest);
    println!("    ✓ {} ({})", component.display_name, component.path);
    Ok(())
}

/// curl argument list for component downloads (pure, tested).
pub(crate) fn curl_download_args(url: &str, dest: &Path, resume: bool) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-L".into(),
        "--progress-bar".into(),
        // no silent infinite hangs: connect probe + hard ceiling for the
        // largest components (measured ~1GB at LAN speed ⇒ 2h slack)
        "--connect-timeout".into(),
        "15".into(),
        "--max-time".into(),
        "7200".into(),
    ];
    if resume {
        a.push("-C".into());
        a.push("-".into());
    }
    a.push("-o".into());
    a.push(dest.to_string_lossy().into_owned());
    a.push(url.into());
    a
}

/// Filter components by path OR display name; empty filter = all (pure, tested).
pub(crate) fn select_components<'a>(
    all: &'a [RemoteComponent],
    filter: &[String],
) -> Vec<&'a RemoteComponent> {
    if filter.is_empty() {
        return all.iter().collect();
    }
    all.iter()
        .filter(|c| filter.iter().any(|f| f == &c.path || f == &c.display_name))
        .collect()
}

/// Byte count to human string (pure, tested).
pub(crate) fn fmt_size(bytes: u64) -> String {
    const GB: u64 = 1_000_000_000;
    const MB: u64 = 1_000_000;
    const KB: u64 = 1_000;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
/// Import an existing SDK installation into ~/.hvolt/sdk/.
///
/// Copies an SDK version directory (containing sdk-pkg.json and openharmony/)
/// into the user-managed SDK store, then activates it.
pub fn import_sdk(from_path: &str) -> Result<(), String> {
    let src = std::path::Path::new(from_path);
    if !src.exists() {
        return Err(format!("Path not found: {}", from_path));
    }
    if !src.join("sdk-pkg.json").exists() && !src.join("openharmony").exists() {
        return Err(format!(
            "{} does not look like an SDK version directory\n\
             Expected: sdk-pkg.json and/or openharmony/ subdirectory.\n\
             An SDK version directory looks like:\n  \
               /path/to/sdk/v26.0.0.23/\n  \
               ├── sdk-pkg.json\n  \
               └── openharmony/",
            from_path
        ));
    }

    let user_sdk = home_dir().join(".hvolt").join("sdk");
    fs::create_dir_all(&user_sdk).map_err(|e| format!("mkdir ~/.hvolt/sdk: {}", e))?;

    // Determine version from sdk-pkg.json or directory name
    let version_dir_name = if let Ok(meta_file) = fs::read_to_string(src.join("sdk-pkg.json")) {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_file) {
            if let Some(ver) = meta.pointer("/data/version").and_then(|v| v.as_str()) {
                format!("v{}", ver)
            } else if let Some(path) = meta.pointer("/data/path").and_then(|v| v.as_str()) {
                path.to_string()
            } else {
                let dir = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                format!("v{}", dir)
            }
        } else {
            format!(
                "v{}",
                src.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
            )
        }
    } else {
        format!(
            "v{}",
            src.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        )
    };

    let target_dir = user_sdk.join(&version_dir_name);

    if target_dir.exists() {
        return Err(format!(
            "SDK version {} already exists at {}. Remove it first with `hvolt sdk remove`.",
            version_dir_name,
            target_dir.display()
        ));
    }

    // Copy entire SDK version directory
    fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
        fs::create_dir_all(dst).map_err(|e| format!("mkdir: {}", e))?;
        if let Ok(entries) = fs::read_dir(src) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                let name = entry.file_name();
                let dest_path = dst.join(&name);
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    copy_dir(&entry_path, &dest_path)?;
                } else {
                    fs::copy(&entry_path, &dest_path)
                        .map_err(|e| format!("copy {}: {}", name.to_string_lossy(), e))?;
                }
            }
        }
        Ok(())
    }

    copy_dir(src, &target_dir)?;

    // SDK root = ~/.hvolt/sdk/ (not version dir) — hvigor requires DEVECO_SDK_HOME
    // to point to a directory containing a `default/` subdirectory.
    let sdk_root = user_sdk;
    ensure_default_symlink(&sdk_root, &target_dir)?;

    let mut cfg = load_global_config_checked()?;
    cfg.sdk_path = sdk_root.to_string_lossy().to_string();
    save_global_config(&cfg)?;

    println!(
        "✓ Imported SDK {} to {}",
        version_dir_name,
        target_dir.display()
    );
    println!("  Use `hvolt sdk use <version>` to switch versions.");
    Ok(())
}

/// Remove an installed SDK version from ~/.hvolt/sdk/.
pub fn remove_version(version: &str) -> Result<(), String> {
    let user_sdk = home_dir().join(".hvolt").join("sdk");
    let version_dir = format!("v{}", version);
    let target_dir = user_sdk.join(&version_dir);

    if !target_dir.exists() {
        return Err(format!(
            "SDK version {} not found at {}",
            version,
            target_dir.display()
        ));
    }

    // Check if this version is currently the active one in global config
    let cfg = load_global_config();
    if !cfg.sdk_path.is_empty() {
        let active = Path::new(&cfg.sdk_path);
        if active.exists() && active == target_dir {
            eprintln!(
                "  (SDK version {} is currently active — clearing from global config)",
                version
            );
            let mut new_cfg = cfg;
            new_cfg.sdk_path = String::new();
            save_global_config(&new_cfg)?;
        }
    }

    // Read the API version BEFORE deleting (the view cleanup needs it)
    let pkg_api = fs::read_to_string(target_dir.join("sdk-pkg.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["data"]["platformVersion"].as_str().map(|s| s.to_string()));

    fs::remove_dir_all(&target_dir)
        .map_err(|e| format!("remove {}: {}", target_dir.display(), e))?;

    // Drop the OpenHarmony runtime view entries that pointed into this version
    // (created by download_sdk; their symlinks would otherwise linger dead).
    // The view dir is SHARED per API level (26.0.0.25 and 26.0.0.38 both live
    // under sdk-ohos/26.0.0) — so prune link-by-link, only removing links
    // whose target died with this version, and never another version's links.
    if let (Some(parent), Some(api)) = (user_sdk.parent(), pkg_api) {
        let view_api = parent.join("sdk-ohos").join(&api);
        if view_api.is_dir() {
            let mut pruned: Vec<String> = Vec::new();
            if let Ok(entries) = fs::read_dir(&view_api) {
                for e in entries.flatten() {
                    let dead = std::fs::canonicalize(e.path()).is_err();
                    if dead {
                        if let Some(n) = e.file_name().to_str() {
                            pruned.push(n.to_string());
                        }
                        // junctions (Windows) are DIRECTORIES to remove_dir;
                        // symlinks (unix) are remove_file. The old unconditional
                        // remove_file left dead junctions behind
                        #[cfg(windows)]
                        {
                            let _ = fs::remove_dir(e.path());
                        }
                        #[cfg(not(windows))]
                        {
                            let _ = fs::remove_file(e.path());
                        }
                    }
                }
            }
            // Re-point pruned components at the newest OTHER installed build of
            // the same API — removing one version must not leave the view of a
            // still-installed version incomplete.
            for comp in &pruned {
                if let Some(src) = newest_component_link(&user_sdk, &api, comp, &[&target_dir]) {
                    link_component(view_api.join(comp), &src)?;
                }
            }
            // backfill components missing from the view entirely (e.g. one
            // pointing at a since-removed build left no entry to prune).
            // NOTE: check link PRESENCE via symlink_metadata — exists() follows
            // links and would treat a dead backfill as absent, deleting the dir.
            for comp in &["ets", "js", "native", "previewer", "toolchains"] {
                if view_api.join(comp).symlink_metadata().is_ok() {
                    continue;
                }
                if let Some(src) = newest_component_link(&user_sdk, &api, comp, &[&target_dir]) {
                    link_component(view_api.join(comp), &src)?;
                }
            }
            if !pruned.is_empty() {
                println!(
                    "  (pruned dead view links {:?}, re-pointed to newest installed build)",
                    pruned
                );
            }
            // nothing left and nothing to backfill → drop the empty view dir
            if fs::read_dir(&view_api)
                .map(|mut rd| rd.next().is_none())
                .unwrap_or(false)
            {
                let _ = fs::remove_dir_all(&view_api);
            }
        }
    }

    println!(
        "✓ Removed SDK version {} from {}",
        version,
        target_dir.display()
    );
    Ok(())
}

/// Newest installed build (by data.version) whose platformVersion matches
/// `api` and that contains openharmony/{comp}, excluding `skip` dirs.
fn newest_component_link(
    user_sdk: &Path,
    api: &str,
    comp: &str,
    skip: &[&Path],
) -> Option<PathBuf> {
    let mut best: Option<(String, PathBuf)> = None;
    let entries = fs::read_dir(user_sdk).ok()?;
    for e in entries.flatten() {
        let dir = e.path();
        if skip.contains(&dir.as_path()) {
            continue;
        }
        if !dir.is_dir() || dir.file_name().map(|n| n == "default").unwrap_or(false) {
            continue;
        }
        let text = fs::read_to_string(dir.join("sdk-pkg.json")).ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        if v["data"]["platformVersion"].as_str() != Some(api) {
            continue;
        }
        if !dir.join("openharmony").join(comp).exists() {
            continue;
        }
        let ver = v["data"]["version"].as_str().unwrap_or("").to_string();
        if best
            .as_ref()
            .map(|(b, _)| ver.as_str() > b.as_str())
            .unwrap_or(true)
        {
            best = Some((ver, dir.join("openharmony").join(comp)));
        }
    }
    best.map(|(_, p)| p)
}

/// Build the sdk-pkg.json content for a downloaded version dir.
/// Format contract (verified against DevEco's bundled SDK and hvigor's
/// hos-sdkmanager-common scanner):
///   - meta.version "1.0.0" — in hvigor's SUPPORT_META_VERSION_LIST; without
///     meta the component dto fails validation.
///   - data.apiVersion is the bare API major ("26"); the getSdkList API
///     returns "26.0.0" for it — split on '.' and take the first component.
///   - data.platformVersion is the dotted platform version ("26.0.0"); for
///     legacy "23"-style api entries (no dot) fall back to the build version.
///   - data.path "HarmonyOS-{platform}" — the scanner's location check passes
///     via the `basename == "default"` rule (SDK root carries a default symlink).
pub(crate) fn sdk_pkg_json(
    api_version: &str,
    version: &str,
    release_type: &str,
) -> serde_json::Value {
    let api_parts: Vec<&str> = api_version.split('.').collect();
    let api_major = api_parts[0];
    let platform_version = if api_parts.len() > 1 {
        api_version.to_string()
    } else {
        version.to_string()
    };
    serde_json::json!({
        "meta": {
            "version": "1.0.0"
        },
        "data": {
            "apiVersion": api_major,
            "version": version,
            "displayName": format!("HarmonyOS {}", platform_version),
            "path": format!("HarmonyOS-{}", platform_version),
            "platformVersion": platform_version,
            "releaseType": release_type,
            "stage": release_type,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_pkg_json_modern_api_entry() {
        // getSdkList 26.0.0.38 returns apiVersion "26.0.0"
        let v = sdk_pkg_json("26.0.0", "26.0.0.38", "Release");
        assert_eq!(v["meta"]["version"], "1.0.0");
        assert_eq!(v["data"]["apiVersion"], "26"); // bare major, NOT "26.0.0"
        assert_eq!(v["data"]["platformVersion"], "26.0.0");
        assert_eq!(v["data"]["path"], "HarmonyOS-26.0.0");
        assert_eq!(v["data"]["version"], "26.0.0.38");
        assert_eq!(v["data"]["stage"], "Release");
    }

    #[test]
    fn sdk_pkg_json_legacy_bare_api() {
        // older entries report apiVersion "23" — platformVersion mirrors build version
        let v = sdk_pkg_json("23", "6.1.0.32", "Release");
        assert_eq!(v["data"]["apiVersion"], "23");
        assert_eq!(v["data"]["platformVersion"], "6.1.0.32");
        assert_eq!(v["data"]["path"], "HarmonyOS-6.1.0.32");
    }

    #[test]
    fn sdk_pkg_json_beta_release_type() {
        let v = sdk_pkg_json("26.0.0", "26.0.0.25", "Beta");
        assert_eq!(v["data"]["releaseType"], "Beta");
        assert_eq!(v["data"]["stage"], "Beta");
    }

    // ─── download UX (TDD: written before curl_download_args etc. existed) ───

    #[test]
    fn curl_args_resume_and_progress() {
        let a = curl_download_args("https://x/y.zip", Path::new("/tmp/z.zip"), true);
        assert!(a.iter().any(|s| s == "-L"), "must follow redirects");
        assert!(
            a.windows(2).any(|w| w == ["-C", "-"]),
            "resume must be requested"
        );
        assert!(a.iter().any(|s| s == "--progress-bar"), "visible progress");
        assert_eq!(a.last().unwrap(), "https://x/y.zip");
        let i = a.iter().position(|s| s == "-o").unwrap();
        assert_eq!(a[i + 1], "/tmp/z.zip");
    }

    #[test]
    fn curl_args_fresh_no_resume() {
        let a = curl_download_args("https://x/y.zip", Path::new("/tmp/z.zip"), false);
        assert!(
            !a.iter().any(|s| s == "-C"),
            "fresh download must not resume"
        );
        assert!(!a.iter().any(|s| s == "--fail-early"));
    }

    #[test]
    fn select_components_by_path_or_display_name() {
        let all: Vec<RemoteComponent> = ["ets", "js", "toolchains"]
            .iter()
            .map(|p| RemoteComponent {
                path: p.to_string(),
                display_name: p.to_uppercase(),
                archive_url: format!("https://x/{}", p),
                size: 100,
                checksum: String::new(),
            })
            .collect();
        // empty filter = all
        assert_eq!(select_components(&all, &[]).len(), 3);
        // by path
        assert_eq!(select_components(&all, &["ets".into()]).len(), 1);
        // by display name
        assert_eq!(select_components(&all, &["TOOLCHAINS".into()]).len(), 1);
        // unknown name selects nothing (caller must report)
        assert_eq!(select_components(&all, &["nope".into()]).len(), 0);
    }

    #[test]
    fn fmt_size_humans() {
        assert_eq!(fmt_size(74454307), "74.5 MB");
        assert_eq!(fmt_size(937100000), "937.1 MB");
        assert_eq!(fmt_size(2_100_000_000), "2.10 GB");
        assert_eq!(fmt_size(1500), "1.5 KB");
        assert_eq!(fmt_size(999), "999 B");
    }

    #[test]
    fn newest_component_link_picks_other_installed_build() {
        // temp store: v26.0.0.25 (native only, to be skipped) + v26.0.0.38 (native+ets)
        let root = std::env::temp_dir().join(format!("hvolt-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mk = |name: &str, ver: &str, comps: &[&str]| {
            let d = root.join(name);
            for c in comps {
                fs::create_dir_all(d.join("openharmony").join(c)).unwrap();
            }
            fs::write(
                d.join("sdk-pkg.json"),
                sdk_pkg_json("26.0.0", ver, "Release").to_string(),
            )
            .unwrap();
            d
        };
        let va = mk("v26.0.0.25", "26.0.0.25", &["native"]);
        let vb = mk("v26.0.0.38", "26.0.0.38", &["native", "ets"]);

        let r = newest_component_link(&root, "26.0.0", "native", &[&va])
            .expect("should re-point to the other build");
        assert!(
            r.starts_with(&vb),
            "pointed at wrong build: {}",
            r.display()
        );

        assert!(
            newest_component_link(&root, "26.0.0", "previewer", &[&vb]).is_none(),
            "component absent everywhere must yield None"
        );
        assert!(
            newest_component_link(&root, "18.0.0", "native", &[&va]).is_none(),
            "other API level must not match"
        );

        let _ = fs::remove_dir_all(&root);
    }
}

/// SHA-256 of a file, hex lowercase.
pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut f = fs::File::open(path).map_err(|e| format!("open: {}", e))?;
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("read: {}", e))?;
        if n == 0 {
            break;
        }
        sha2::Digest::update(&mut h, &buf[..n]);
    }
    Ok(format!("{:x}", sha2::Digest::finalize(h)))
}

#[cfg(test)]
mod config_loader_tests {
    use super::*;

    /// missing/empty must be Ok(default); corrupt must be Err —
    /// conflating them is what let a write path erase the user's config.
    #[test]
    fn missing_empty_and_corrupt_are_distinguished() {
        let dir = std::env::temp_dir().join(format!("hvolt-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.json");

        let _ = std::fs::remove_file(&p);
        assert!(
            load_global_config_from(&p).is_ok(),
            "missing file = default"
        );

        std::fs::write(&p, "   \n").unwrap();
        assert!(load_global_config_from(&p).is_ok(), "empty file = default");

        std::fs::write(&p, "{ this is not json").unwrap();
        assert!(
            load_global_config_from(&p).is_err(),
            "garbage must be an error"
        );

        // valid JSON but a field of the wrong type must NOT silently become defaults
        std::fs::write(&p, r#"{"sdk_path": "/real/sdk", "device_target": ["x"]}"#).unwrap();
        assert!(
            load_global_config_from(&p).is_err(),
            "type mismatch must be an error"
        );

        std::fs::write(&p, r#"{"sdk_path": "/good/sdk"}"#).unwrap();
        assert_eq!(load_global_config_from(&p).unwrap().sdk_path, "/good/sdk");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod sdk_resolution_tests {
    use super::deveco_sdk_path;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hvolt-sdk-{}-{}", tag, std::process::id()));
        std::fs::remove_dir_all(&p).ok();
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn deveco_skeleton_without_components_is_rejected() {
        let root = scratch("skeleton");
        let deveco = root.join("DevEco Studio");
        std::fs::create_dir_all(deveco.join("sdk")).unwrap();
        assert!(deveco_sdk_path(&deveco).is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    /// The SDK root wins over its `default/openharmony` child: HMS components live beside
    /// openharmony, and handing hvigor the child alone makes runtimeOS=HarmonyOS builds
    /// fail with "SDK component missing".
    #[test]
    fn deveco_install_prefers_the_root_that_carries_hms() {
        let root = scratch("full");
        let deveco = root.join("DevEco Studio");
        std::fs::create_dir_all(deveco.join("sdk/default/openharmony/ets")).unwrap();
        std::fs::create_dir_all(deveco.join("sdk/default/hms")).unwrap();
        let found = deveco_sdk_path(&deveco).expect("component dir");
        assert_eq!(found, deveco.join("sdk"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn deveco_single_tree_install_is_accepted() {
        let root = scratch("single");
        let deveco = root.join("DevEco Studio");
        std::fs::create_dir_all(deveco.join("sdk/openharmony/ets")).unwrap();
        let found = deveco_sdk_path(&deveco).expect("component dir");
        assert!(found.ends_with("sdk"));
        std::fs::remove_dir_all(&root).ok();
    }
}
