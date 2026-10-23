// doctor.rs — environment & project health check.
//
// `hvolt doctor` reports one line per check: ✓ pass, ⚠ degraded, ✗ broken.
// Every ✗/⚠ carries the concrete fix command. Exit code: 1 if any ✗.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::AppConfig;
use crate::sdk;

pub struct Report {
    issues: std::cell::Cell<usize>,
}

impl Report {
    fn line(&self, ok: bool, warn: bool, section: &str, name: &str, detail: &str, fix: &str) {
        let mark = if ok {
            "✓"
        } else if warn {
            "⚠"
        } else {
            "✗"
        };
        println!("  {} [{}] {:<12} {}", mark, section, name, detail);
        if !ok && !fix.is_empty() {
            println!("      → {}", fix);
            if !warn {
                self.issues.set(self.issues.get() + 1);
            }
        }
    }

    pub fn finish(&self) -> i32 {
        let n = self.issues.get();
        if n == 0 {
            println!("Summary: all checks passed");
            0
        } else {
            println!("Summary: {} issue(s)", n);
            1
        }
    }
}

/// Run all checks. project_dir is the CWD; project-level checks only run
/// when hvolt.toml exists there.
pub fn run(project_dir: &Path) -> Report {
    let rep = Report {
        issues: std::cell::Cell::new(0),
    };

    println!("hvolt doctor");

    // ── env ────────────────────────────────────────────────
    println!("[env]");
    check_executable(
        &rep,
        "node",
        "env",
        sdk::find_node(),
        |home| sdk::node_bin(home),
        "--version",
    );
    check_executable(
        &rep,
        "java",
        "env",
        sdk::find_java(),
        |home| sdk::java_bin(home),
        "-version",
    );

    // ── sdk store ──────────────────────────────────────────
    println!("[sdk]");
    let sdk_root = check_sdk(&rep);

    // ── tools ──────────────────────────────────────────────
    println!("[tools]");
    check_hvigor(&rep);
    check_ohpm(&rep);

    // ── lint ───────────────────────────────────────────────
    println!("[lint]");
    check_lint_readiness(&rep);

    // ── device ─────────────────────────────────────────────
    println!("[device]");
    check_devices(&rep, sdk_root.as_deref());

    // ── auth (advisory only — sign needs it) ───────────────
    println!("[auth]");
    check_auth(&rep);

    // ── project (only inside an hvolt project) ─────────────
    let toml_path = project_dir.join("hvolt.toml");
    if toml_path.exists() {
        println!("[project] {}", project_dir.display());
        check_project(&rep, &toml_path, sdk_root.as_deref());
    }

    rep
}

/// Usable device types = schema-legal ∩ present in this SDK's device-define.
/// Falls back to whichever source exists; with neither, the platform default.
fn intersect_device_types(
    schema: Option<&[String]>,
    dd: &[String],
    fallback: &[&str],
) -> Vec<String> {
    match (schema, dd.is_empty()) {
        (Some(se), false) => se
            .iter()
            .filter(|s| dd.iter().any(|d| d == *s))
            .cloned()
            .collect(),
        (Some(se), true) => se.to_vec(),
        (None, false) => dd.to_vec(),
        (None, true) => fallback.iter().map(|s| s.to_string()).collect(),
    }
}

/// Official deviceTypes enum from an SDK `modulecheck/module.json`
/// (the validator the toolchain itself uses), if that file is present.
fn read_device_type_enum(path: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    fn find(o: &serde_json::Value) -> Option<Vec<String>> {
        match o {
            serde_json::Value::Object(m) => {
                if let Some(dt) = m.get("deviceTypes") {
                    let items = dt
                        .get("enum")
                        .or_else(|| dt.get("items").and_then(|i| i.get("enum")));
                    if let Some(serde_json::Value::Array(a)) = items {
                        let v: Vec<String> = a
                            .iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect();
                        if !v.is_empty() {
                            return Some(v);
                        }
                    }
                }
                for (_, val) in m {
                    if let Some(r) = find(val) {
                        return Some(r);
                    }
                }
                None
            }
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    find(&v)
}

// ─── individual checks ───────────────────────────────────────────

/// What `hvolt lint` needs, and what fails LATE and silently without it: the
/// codelinter payload, and an SDK root in the shape its OpenHarmony validator
/// accepts (`<root>/<platformVersion>/<component>` holding oh-uni-package.json).
/// Measured 2026-10-10: without that shape the engines skip themselves and lint
/// reports a false "0 defects", so surface it here rather than after a run.
/// Reported as warning-only: build-only users need no linter.
fn check_lint_readiness(rep: &Report) {
    let payload = crate::lint::find_codelinter();
    match &payload {
        Some(p) => rep.line(true, false, "lint", "codelinter", &p.to_string_lossy(), ""),
        None => rep.line(
            false,
            true,
            "lint",
            "codelinter",
            "payload not found (no DevEco install and no hvolt-managed copy)",
            "hvolt tools install codelinter --from <DevEco>/plugins/codelinter",
        ),
    }
    let version = crate::lint::resolve_sdk_version(std::path::Path::new("."));
    if version.trim().is_empty() {
        rep.line(
            false,
            true,
            "lint",
            "sdk version",
            "no sdk_version in hvolt.toml and none in the translated build profile",
            "set [app] sdk_version in hvolt.toml",
        );
    } else if crate::lint::open_sdk_overlay_ready(&version) {
        rep.line(
            true,
            false,
            "lint",
            "sdk bridge",
            &format!("{} — codelinter-shaped root present", version),
            "",
        );
    } else {
        rep.line(
            true,
            false,
            "lint",
            "sdk bridge",
            &format!(
                "{} — will be created on the first `hvolt lint` run",
                version
            ),
            "",
        );
    }
}

/// Locate via hvolt's resolution chain then EXECUTE it: an existing but
/// non-functional binary (e.g. macOS /usr/bin/java stub) must fail here,
/// not at build time.
fn check_executable(
    rep: &Report,
    label: &str,
    section: &str,
    found: Result<PathBuf, String>,
    bin_of: fn(&PathBuf) -> PathBuf,
    version_arg: &str,
) -> Option<PathBuf> {
    match found {
        Err(e) => {
            rep.line(
                false,
                false,
                section,
                label,
                &e,
                &format!("hvolt config set {}_home <path>", label),
            );
            None
        }
        Ok(home) => {
            let bin = bin_of(&home);
            match Command::new(&bin).arg(version_arg).output() {
                Ok(out) if out.status.success() => {
                    let out_s = String::from_utf8_lossy(&out.stdout).to_string();
                    let err_s = String::from_utf8_lossy(&out.stderr).to_string();
                    let first = out_s
                        .lines()
                        .next()
                        .or_else(|| err_s.lines().next())
                        .unwrap_or("")
                        .trim()
                        .chars()
                        .take(40)
                        .collect::<String>();
                    rep.line(
                        true,
                        false,
                        section,
                        label,
                        &format!("{} ({})", first, bin.display()),
                        "",
                    );
                    Some(home)
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    rep.line(
                        false,
                        false,
                        section,
                        label,
                        &format!("runs but fails: {}", err),
                        &format!(
                            "hvolt config set {}_home <path to a working {}>",
                            label, label
                        ),
                    );
                    None
                }
                Err(e) => {
                    rep.line(
                        false,
                        false,
                        section,
                        label,
                        &format!("cannot execute {}: {}", bin.display(), e),
                        &format!(
                            "check {} installation or hvolt config set {}_home <path>",
                            label, label
                        ),
                    );
                    None
                }
            }
        }
    }
}

fn check_sdk(rep: &Report) -> Option<PathBuf> {
    // Resolution order is the same one `hvolt build` follows, including env vars and an
    // installed DevEco Studio — doctor used to look only at the managed store and reported
    // "no SDK" on machines where the build then succeeded with the IDE's SDK.
    let Some(root) = sdk::discovered_sdk_root("") else {
        rep.line(
            false,
            false,
            "sdk",
            "sdk_path",
            "not found (no sdk_path, no ~/.hvolt/sdk, no DevEco install)",
            "hvolt sdk download <version>  (or hvolt sdk set <path>)",
        );
        return None;
    };
    let managed = root == sdk::sdk_store_home();
    let default_link = root.join("default");
    rep.line(
        true,
        false,
        "sdk",
        if managed { "store" } else { "detected" },
        &format!(
            "{}{}",
            root.display(),
            if managed {
                format!(
                    " (default -> {})",
                    std::fs::canonicalize(&default_link)
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .unwrap_or_else(|| "?".into())
                )
            } else {
                " (external install: DevEco Studio / env var)".to_string()
            }
        ),
        "",
    );

    // Component root: managed store keeps them under default/openharmony, an external
    // install may be the SDK root itself or already point at the openharmony tree.
    let oh = [
        default_link.join("openharmony"),
        root.join("openharmony"),
        root.clone(),
    ]
    .into_iter()
    .find(|p| p.join("ets").is_dir() || p.join("native").is_dir())
    .unwrap_or_else(|| default_link.join("openharmony"));

    let mut comps: Vec<String> = Vec::new();
    for c in &["ets", "js", "native", "previewer", "toolchains"] {
        if oh.join(c).exists() {
            comps.push(c.to_string());
        }
    }
    if comps.is_empty() {
        rep.line(
            false,
            false,
            "sdk",
            "components",
            &format!("{} has no components", oh.display()),
            "hvolt sdk download <version>",
        );
    } else {
        rep.line(true, false, "sdk", "components", &comps.join(", "), "");
    }

    // toolchains runnable on THIS platform (the cross-OS Mach-O/ELF trap)
    let restool = oh.join("toolchains/restool");
    if restool.exists() {
        match Command::new(&restool).arg("--help").output() {
            Ok(o) if o.status.success() => rep.line(
                true,
                false,
                "sdk",
                "toolchains",
                "executables run on this platform",
                "",
            ),
            _ => rep.line(
                false,
                false,
                "sdk",
                "toolchains",
                "restool does NOT run here (wrong-platform build from an old import?)",
                "hvolt sdk download <version>  (fetches the build for this OS)",
            ),
        }
    }

    // hms presence decides runtimeOS=HarmonyOS viability
    // hms sits next to openharmony: <sdk>/hms for a managed store, <install>/sdk/hms for
    // DevEco, which hands us <install>/sdk/default/openharmony as the component root.
    let has_hms = [
        default_link.join("hms"),
        root.join("hms"),
        root.parent().map(|p| p.join("hms")).unwrap_or_default(),
        root.parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("hms"))
            .unwrap_or_default(),
    ]
    .iter()
    .any(|p| !p.as_os_str().is_empty() && p.exists());
    if has_hms {
        rep.line(
            true,
            false,
            "sdk",
            "hms",
            "present — runtimeOS=HarmonyOS available",
            "",
        );
    } else {
        rep.line(false, true, "sdk", "hms",
            "absent — projects with runtimeOS=HarmonyOS will fail 'SDK component missing'",
            "no public hms distribution for this platform; set runtime_os = \"OpenHarmony\" in [app]");
    }

    // OpenHarmony runtime view (only the managed store has one)
    if managed {
        let view = root.parent().map(|p| p.join("sdk-ohos"));
        match view.as_deref() {
            Some(v) if v.exists() => {
                let apis: Vec<String> = std::fs::read_dir(v)
                    .map(|rd| {
                        rd.flatten()
                            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                rep.line(
                    true,
                    false,
                    "sdk",
                    "sdk-ohos",
                    &format!("view for api(s): {}", apis.join(",")),
                    "",
                );
            }
            _ => rep.line(
                false,
                true,
                "sdk",
                "sdk-ohos",
                "missing — OpenHarmony runtime builds will fail",
                "re-run: hvolt sdk download <version> (creates the view)",
            ),
        }
    }
    Some(root)
}

fn check_hvigor(rep: &Report) {
    let dir = crate::tools::hvigor_dir();
    match crate::tools::find_hvigorw() {
        None => rep.line(
            false,
            false,
            "tools",
            "hvigor",
            "not installed",
            "hvolt tools install hvigor",
        ),
        Some(w) => {
            let plugin = dir.join("node_modules/@ohos/hvigor-ohos-plugin");
            if plugin.exists() {
                rep.line(
                    true,
                    false,
                    "tools",
                    "hvigor",
                    &format!("{} + plugin", w.display()),
                    "",
                );
            } else {
                rep.line(
                    false,
                    false,
                    "tools",
                    "hvigor",
                    "launcher found but @ohos/hvigor-ohos-plugin missing",
                    "rm -rf ~/.hvolt/tools/hvigor && hvolt tools install hvigor",
                );
            }
        }
    }
}

fn check_ohpm(rep: &Report) {
    match crate::tools::find_ohpm() {
        Some(b) => match Command::new(&b).arg("--version").output() {
            Ok(o) if o.status.success() => rep.line(true, false, "tools", "ohpm",
                &format!("{} ({})", String::from_utf8_lossy(&o.stdout).trim(), b.display()), ""),
            _ => rep.line(false, false, "tools", "ohpm", &format!("runs but fails: {}", b.display()),
                "ohpm's launcher needs node on PATH — add hvolt config set node_home's bin to PATH, or build via hvolt (it injects PATH itself)"),
        },
        None => rep.line(false, false, "tools", "ohpm", "not found", "hvolt tools install ohpm"),
    }
}

fn check_devices(rep: &Report, sdk_root: Option<&Path>) {
    // remote (cross-machine) target takes precedence — local hdc presence is moot
    let cfg = crate::sdk::load_global_config();
    if !cfg.device_host.is_empty() {
        rep.line(
            true,
            false,
            "device",
            "remote",
            &format!(
                "host={} serial={} hdc={}",
                cfg.device_host, cfg.device_target, cfg.device_hdc
            ),
            "",
        );
        let probe = crate::target::ssh_args(&cfg.device_host, "echo hvolt-ok");
        match std::process::Command::new("ssh").args(&probe).output() {
            Ok(o)
                if o.status.success()
                    && String::from_utf8_lossy(&o.stdout).contains("hvolt-ok") =>
            {
                rep.line(true, false, "device", "ssh", "device host reachable", "")
            }
            Ok(o) => rep.line(
                false,
                false,
                "device",
                "ssh",
                &format!(
                    "{} unreachable: {}",
                    cfg.device_host,
                    String::from_utf8_lossy(&o.stderr)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                ),
                "enable Remote Login on the device host and authorize this machine's ssh key",
            ),
            Err(e) => rep.line(
                false,
                false,
                "device",
                "ssh",
                &format!("probe failed: {}", e),
                "",
            ),
        }
        return;
    }
    let hdc = sdk_root
        .and_then(sdk::find_hdc_from_sdk)
        .or_else(crate::hdc::find_hdc);
    match hdc {
        None => rep.line(
            false,
            true,
            "device",
            "hdc",
            "not found (PATH or SDK toolchains)",
            "hvolt sdk download <version> includes toolchains/hdc",
        ),
        Some(h) => match Command::new(&h).arg("list").arg("targets").output() {
            Ok(o) => {
                let targets: Vec<String> = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty() && l != "[Empty]")
                    .collect();
                if targets.is_empty() {
                    rep.line(
                        false,
                        true,
                        "device",
                        "targets",
                        "no device connected",
                        "connect USB, or hvolt device connect <ip:port>",
                    );
                } else {
                    rep.line(
                        true,
                        false,
                        "device",
                        "targets",
                        &format!("{} connected: {}", targets.len(), targets.join(", ")),
                        "",
                    );
                }
            }
            Err(e) => rep.line(
                false,
                true,
                "device",
                "hdc",
                &format!("{}: {}", h.display(), e),
                "",
            ),
        },
    }
}

fn check_auth(rep: &Report) {
    match crate::backend::token_state() {
        crate::backend::TokenState::NotBuilt => rep.line(
            false,
            true,
            "auth",
            "agc",
            "no AGC backend in this build (local signing only)",
            "hvolt sign --local  (or build with --features agc-local)",
        ),
        crate::backend::TokenState::Missing => rep.line(
            false,
            true,
            "auth",
            "agc",
            "no token (~/.hvolt/auth.json missing)",
            "hvolt agc login  (only needed for remote signing)",
        ),
        crate::backend::TokenState::Valid { minutes_left } => rep.line(
            true,
            false,
            "auth",
            "agc",
            &format!("token valid, {}m left (refresh automatic)", minutes_left),
            "",
        ),
        crate::backend::TokenState::ExpiredRefreshable => rep.line(
            false,
            true,
            "auth",
            "agc",
            "token expired but refresh_token present",
            "next remote signing refreshes automatically — run: hvolt agc login only if it fails",
        ),
        crate::backend::TokenState::Expired => rep.line(
            false,
            false,
            "auth",
            "agc",
            "token expired, no refresh",
            "hvolt agc login",
        ),
    }
}

fn check_project(rep: &Report, toml_path: &Path, sdk_root: Option<&Path>) {
    let app = match AppConfig::from_file(toml_path) {
        Ok(a) => a,
        Err(e) => {
            rep.line(
                false,
                false,
                "project",
                "hvolt.toml",
                &e,
                "fix the config file",
            );
            return;
        }
    };
    let Some(root) = sdk_root else {
        rep.line(
            false,
            true,
            "project",
            "sdk",
            "cannot cross-check project vs SDK (none detected)",
            "",
        );
        return;
    };
    let default_link = root.join("default");

    // runtimeOS vs installed components
    let has_hms = default_link.join("hms").exists();
    if app.app.runtime_os == "HarmonyOS" && !has_hms {
        rep.line(false, false, "project", "runtime_os",
            "runtimeOS=HarmonyOS but installed SDK has no hms/ — hvigor will fail 'SDK component missing'",
            "set runtime_os = \"OpenHarmony\" in [app] (see hvolt doctor [sdk] hms line)");
    } else {
        rep.line(
            true,
            false,
            "project",
            "runtime_os",
            &format!("{} matches installed SDK", app.app.runtime_os),
            "",
        );
    }

    // device_types against the official enum for this runtimeOS.
    // The two toolchains carry DIFFERENT enums (openharmony has no "phone",
    // hms does) — reading the openharmony one while runtimeOS=HarmonyOS made
    // doctor report a false ✗ for the default scaffold.
    let rt = if app.app.runtime_os.is_empty() {
        "HarmonyOS"
    } else {
        app.app.runtime_os.as_str()
    };
    let toolchain = if rt.eq_ignore_ascii_case("OpenHarmony") {
        "openharmony"
    } else {
        "hms"
    };
    let mc_json = default_link
        .join(toolchain)
        .join("toolchains/modulecheck/module.json");
    let schema_enum = read_device_type_enum(&mc_json);
    // This SDK's device-define set = what hvigor can actually build for.
    // MEASURED (Windows Surface, OpenHarmony-only SDK): the modulecheck schema
    // lists "car", but device-define has no car.json and the build dies with
    // 00303060 "the intersection of the system capability sets ... is empty".
    // So a device type is usable only if it is BOTH schema-legal AND present
    // in device-define — the previous code trusted the schema alone and would
    // have passed a project that cannot build.
    let dd_set: Vec<String> =
        std::fs::read_dir(default_link.join(toolchain).join("ets/api/device-define"))
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        e.file_name()
                            .to_str()
                            .and_then(|s| s.strip_suffix(".json"))
                            .map(|s| s.trim_end_matches("-hmos").to_string())
                    })
                    .filter(|s| {
                        !matches!(
                            s.as_str(),
                            "api-white-list" | "kit-version" | "syscap-version"
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
    let available: Vec<String> = intersect_device_types(
        schema_enum.as_deref(),
        &dd_set,
        crate::config::device_types_for_runtime(rt),
    );
    {
        let mut missing: Vec<String> = Vec::new();
        // scan each member's module hvolt.toml for device_types
        for member in &app.app.members {
            let m_toml = toml_path.parent().unwrap().join(member).join("hvolt.toml");
            if let Ok(content) = std::fs::read_to_string(&m_toml) {
                if let Ok(mc) = content.parse::<toml::Value>() {
                    if let Some(dts) = mc
                        .get("module")
                        .and_then(|m| m.get("device_types"))
                        .and_then(|d| d.as_array())
                    {
                        for dt in dts {
                            if let Some(s) = dt.as_str() {
                                if !available.iter().any(|a| a == s) {
                                    missing.push(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        if missing.is_empty() {
            rep.line(
                true,
                false,
                "project",
                "device_types",
                &format!("buildable here ({})", available.join(",")),
                "",
            );
        } else {
            let detailed = match schema_enum.as_ref() {
                Some(se) => {
                    let illegal: Vec<String> = missing
                        .iter()
                        .filter(|m| !se.iter().any(|s| s == *m))
                        .cloned()
                        .collect();
                    let no_dd: Vec<String> = missing
                        .iter()
                        .filter(|m| !illegal.contains(m))
                        .cloned()
                        .collect();
                    let mut parts: Vec<String> = Vec::new();
                    if !illegal.is_empty() {
                        parts.push(format!(
                            "not legal for runtimeOS={}: {} (official enum: {})",
                            rt,
                            illegal.join(","),
                            se.join(",")
                        ));
                    }
                    if !no_dd.is_empty() {
                        parts.push(format!(
                            "this SDK has no device-define for {} — hvigor will fail SyscapTransform (00303060)",
                            no_dd.join(",")));
                    }
                    format!(
                        "{} — buildable here: {}",
                        parts.join("; "),
                        available.join(",")
                    )
                }
                None => format!(
                    "not in this SDK: {} — available: {}",
                    missing.join(","),
                    available.join(",")
                ),
            };
            rep.line(
                false,
                false,
                "project",
                "device_types",
                &detailed,
                &format!(
                    "edit src/hvolt.toml device_types to a subset of [{}]",
                    available.join(", ")
                ),
            );
        }
    }

    // signing files exist?
    if let Some(signing) = app.app.signing_for("default") {
        let base = toml_path.parent().unwrap();
        let mut absent = Vec::new();
        for f in [&signing.store_file, &signing.certpath, &signing.profile] {
            if !base.join(f).exists() {
                absent.push(f.clone());
            }
        }
        if absent.is_empty() {
            rep.line(true, false, "project", "signing", "materials present", "");
        } else {
            rep.line(
                false,
                false,
                "project",
                "signing",
                &format!("config references missing files: {}", absent.join(", ")),
                "run `hvolt sign` to (re)generate them",
            );
        }
    }
}

#[cfg(test)]
mod device_type_intersect_tests {
    use super::intersect_device_types;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    /// The Surface case: schema says "car" is legal for OpenHarmony, but the
    /// SDK has no car.json → hvigor fails 00303060. car must NOT be offered.
    #[test]
    fn schema_and_sdk_are_intersected() {
        let schema = v(&["default", "tablet", "tv", "wearable", "car", "2in1"]);
        let dd = v(&[
            "2in1",
            "default",
            "liteWearable",
            "tablet",
            "tv",
            "wearable",
        ]);
        let got = intersect_device_types(Some(&schema), &dd, &["default"]);
        assert!(got.contains(&"tablet".to_string()));
        assert!(got.contains(&"2in1".to_string()));
        assert!(
            !got.contains(&"car".to_string()),
            "car has no device-define here: {got:?}"
        );
        assert!(
            !got.contains(&"liteWearable".to_string()),
            "not in the schema enum: {got:?}"
        );
    }

    #[test]
    fn falls_back_when_one_source_is_missing() {
        let schema = v(&["default", "phone"]);
        assert_eq!(
            intersect_device_types(Some(&schema), &[], &["default"]),
            schema
        );
        let dd = v(&["default", "tablet"]);
        assert_eq!(intersect_device_types(None, &dd, &["default"]), dd);
        assert_eq!(
            intersect_device_types(None, &[], &["default", "tablet"]),
            v(&["default", "tablet"])
        );
    }
}
