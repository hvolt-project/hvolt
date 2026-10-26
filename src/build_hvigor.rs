// build_hvigor: hvigorw build invocation and JSON5 config surgery.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::sdk::SdkEnv;

/// Replace the `signingConfigs` array with `[]`, string-aware.
///
/// Returns `None` when there is nothing to strip (or the array is unbalanced, in which case
/// the file is left alone rather than corrupted). Kept pure so the two cases most likely to be
/// wrong — nested brackets and brackets inside string literals — are testable without files.
pub(crate) fn strip_signing_configs(content: &str) -> Option<String> {
    let start = content.find("\"signingConfigs\"")?;
    let bracket_start = content[start..].find('[')?;
    let abs_start = start + bracket_start;
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = abs_start;
    for (i, ch) in content[abs_start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = abs_start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    Some(format!(
        "{}\"signingConfigs\": []{}",
        &content[..start],
        &content[end..]
    ))
}

/// Remove signingConfigs from build-profile.json5 so hvigor skips SignHap.
pub fn strip_signing_config(project_dir: &Path) {
    let bp = project_dir.join("build/build-profile.json5");
    if !bp.exists() {
        return;
    }
    let Ok(content) = fs::read_to_string(&bp) else {
        return;
    };
    let Some(fixed) = strip_signing_configs(&content) else {
        return;
    };
    if let Err(e) = fs::write(&bp, &fixed) {
        eprintln!("Warning: strip signingConfigs failed: {}", e);
    }
}

pub fn run_ohpm_install(sdk: &SdkEnv, project_dir: &Path) -> Result<(), String> {
    println!("  ohpm install (resolve dependencies)...");
    // prefer node-direct launch (pm-cli.js beside the wrapper): identical to
    // what the sh wrapper execs, but shell-dialect-free on every platform
    let mut cmd = ohpm_command(sdk);
    cmd.arg("install")
        .current_dir(project_dir)
        .env("DEVECO_SDK_HOME", &sdk.ohos_sdk);
    apply_tool_path(&mut cmd, sdk);
    apply_cache_home(&mut cmd);

    let status = cmd.status().map_err(|e| format!("ohpm: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ohpm exit: {}", status.code().unwrap_or(-1)))
    }
}

/// Command to run ohpm on this platform.
fn ohpm_command(sdk: &SdkEnv) -> Command {
    let bin_dir = sdk.ohpm.parent().unwrap_or(&sdk.ohpm).to_path_buf();
    if let Some(js) = crate::platform::ohpm_via_node(&bin_dir) {
        if let Some(node) = crate::platform::on_path("node") {
            let mut c = Command::new(node);
            c.arg(&js);
            return c;
        }
    }
    Command::new(&sdk.ohpm)
}

/// Point the child process's home at a caller-chosen directory.
///
/// ohpm keeps its store under `$HOME/.ohpm` and hvigor under `$HOME/.hvigor`; in a sandbox,
/// a container with a read-only home, or CI with an ephemeral HOME, both fail with `EPERM`
/// that reads like a project error. `HVOLT_CACHE_HOME` (or `hvolt build --cache-dir`)
/// relocates them without touching hvolt's own `~/.hvolt` (tools and SDK stay put).
fn apply_cache_home(cmd: &mut Command) {
    let Some(dir) = crate::tools::cache_home() else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("Warning: cannot create cache dir {}: {}", dir.display(), e);
        return;
    }
    // ohpm/hvigor/npm all derive their cache path from the home directory.
    cmd.env("HOME", &dir);
    if crate::platform::is_windows() {
        cmd.env("USERPROFILE", &dir);
    }
}

/// Prepend node/bin and java/bin to PATH for child processes:
/// ohpm's launcher does `exec node`, and hvigor spawns bare `java` for the
/// PackageHap/SignHap jar tools — both resolve via PATH, not NODE_HOME/JAVA_HOME.
fn apply_tool_path(cmd: &mut Command, sdk: &SdkEnv) {
    // layout-aware (Windows node zip has no bin/) and suffix-aware
    let node_bin = crate::platform::tool_bin_dir(&sdk.node_home, "node");
    let java_bin = crate::platform::tool_bin_dir(&sdk.java_home, "java");
    let mut prefixes: Vec<std::path::PathBuf> = Vec::new();
    if crate::platform::find_in_dirs("node", [node_bin.clone()]).is_some() {
        prefixes.push(node_bin);
    }
    if crate::platform::find_in_dirs("java", [java_bin.clone()]).is_some() {
        prefixes.push(java_bin);
    }
    if !prefixes.is_empty() {
        let joined = std::env::join_paths(prefixes.into_iter().chain(std::env::split_paths(
            &std::env::var("PATH").unwrap_or_default(),
        )))
        .ok();
        if let Some(p) = joined {
            cmd.env("PATH", p);
        }
    }
}

/// The project's hvigorfile.ts imports `@ohos/hvigor-ohos-plugin` by module
/// name; node resolves from the project dir upward. The npm-installed hvigor
/// tool lives in ~/.hvolt/tools/hvigor/node_modules, so link it into the
/// translated project before invoking hvigorw.
/// Windows directory junction (no privilege, unlike symlinks). Public so
/// the SDK OH-view builder shares the mechanism.
#[cfg(windows)]
pub(crate) fn junction(link: &Path, target: &Path) -> Result<(), String> {
    let out = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.to_string_lossy(),
            &target.to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("mklink spawn: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "junction {}: {}",
            link.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn link_hvigor_node_modules(build_dir: &Path) -> Result<(), String> {
    let link = build_dir.join("node_modules");
    if link.symlink_metadata().is_ok() {
        return Ok(()); // already present (symlink or real dir) — leave as is
    }
    let tools_nm = crate::tools::hvigor_dir().join("node_modules");
    if !tools_nm.exists() {
        return Err("hvigor node_modules not found — run `hvolt tools install hvigor`".into());
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&tools_nm, &link)
        .map_err(|e| format!("link node_modules: {}", e))?;
    #[cfg(windows)]
    junction(&link, &tools_nm)?;
    Ok(())
}

/// Module-level hqf packaging task (quickfix incremental bundle).
/// Measured requirements: OHOS_BASE_SDK_HOME must point at the sdk-ohos
/// VIEW (management-mode check on the hqf chain; assembleHap does not
/// enforce it) — run_assemble already injects the view when present.
pub fn run_hqf(sdk: &SdkEnv, build_dir: &Path, module: &str) -> Result<(), String> {
    let mut cmd = Command::new(&sdk.hvigorw);
    cmd.args([
        "--mode",
        "module",
        "-p",
        "product=default",
        "-p",
        &format!("module={}@default", module),
        "assembleDevHqf",
        "--no-daemon",
    ])
    .current_dir(build_dir)
    .env("DEVECO_SDK_HOME", &sdk.ohos_sdk)
    .env("JAVA_HOME", &sdk.java_home);
    if let Some(parent) = sdk.ohos_sdk.parent() {
        let ohos_view = parent.join("sdk-ohos");
        if ohos_view.is_dir() {
            cmd.env("OHOS_BASE_SDK_HOME", &ohos_view);
        }
    }
    apply_tool_path(&mut cmd, sdk);
    apply_cache_home(&mut cmd);
    apply_cache_home(&mut cmd);
    let status = cmd.status().map_err(|e| format!("hvigorw hqf: {}", e))?;
    if !status.success() {
        return Err(format!(
            "assembleDevHqf exit: {}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(())
}

/// First module name of the translated build-profile (hvolt flat model
/// keeps one module; read it, don't assume "src").
pub fn first_module(build_dir: &Path) -> Result<String, String> {
    let bp = build_dir.join("build-profile.json5");
    let t = std::fs::read_to_string(&bp).map_err(|e| format!("build-profile: {}", e))?;
    let after = t
        .find("\"modules\"")
        .ok_or("no modules in build-profile".to_string())?;
    let rest = &t[after..];
    let ni = rest.find("\"name\"").ok_or("no module name".to_string())?;
    let seg = &rest[ni..];
    // "name" : "value" — grab the next quoted token after the key
    let colon = seg.find(':').ok_or("no colon after name".to_string())?;
    let vs = seg[colon..]
        .find('\"')
        .ok_or("no value quote".to_string())?
        + colon
        + 1;
    let ve = seg[vs..].find('\"').ok_or("unterminated".to_string())?;
    Ok(seg[vs..vs + ve].to_string())
}

/// Content for `local.properties` (sdk.dir). hvigor resolves the SDK from this file as well
/// as DEVECO_SDK_HOME; when only the env var is set the module tasks abort with
/// 00303208 "Unable to find 'sdk.dir'". DevEco Studio writes the same file.
pub(crate) fn local_properties_content(sdk_dir: &str) -> String {
    format!("sdk.dir={}\n", sdk_dir)
}

/// hvigor-config.json5 that binds the declared plugin dependency to the vendored copy.
///
/// The hvigor wrapper resolves plugin dependencies through pnpm; @ohos/hvigor-ohos-plugin
/// is not on the public registry (ERR_PNPM_FETCH_404), so the declared dependency in
/// `strategy::hvigor_config` gets rebound to `file:<vendored>` here at build time.
pub(crate) fn vendored_plugin_config(dep: &str) -> String {
    // pnpm file: deps need forward slashes even on Windows; a raw `C:\...` path would be
    // parsed as an escape sequence (or left verbatim and fail to resolve).
    let dep = dep.replace('\\', "/");
    format!(
        r#"{{
  "modelVersion": "26.0.0",
  "dependencies": {{
    "@ohos/hvigor-ohos-plugin": "file:{dep}"
  }},
  "execution": {{}},
  "logging": {{}},
  "debugging": {{}},
  "nodeOptions": {{}}
}}
"#
    )
}

pub fn run_assemble(sdk: &SdkEnv, project_dir: &Path) -> Result<(), String> {
    link_hvigor_node_modules(project_dir)?;

    // hvigor resolves the SDK from local.properties (sdk.dir) as well as
    // DEVECO_SDK_HOME; when only the env var is set the module tasks abort
    // with 00303208 "Unable to find 'sdk.dir'". DevEco writes this file too.
    let local_props = project_dir.join("local.properties");
    let sdk_dir_disp = sdk.ohos_sdk.display().to_string();
    if let Err(e) = std::fs::write(&local_props, local_properties_content(&sdk_dir_disp)) {
        eprintln!("Warning: write {} failed: {}", local_props.display(), e);
    }

    // The wrapper resolves plugin dependencies through pnpm. @ohos/hvigor-ohos-plugin
    // is not on the public registry (ERR_PNPM_FETCH_404), so bind the declared
    // dependency to the vendored copy that ships with hvolt instead.
    let vendored = crate::tools::hvigor_dir().join("node_modules/@ohos/hvigor-ohos-plugin");
    if vendored.exists() {
        let cfg = vendored_plugin_config(&vendored.display().to_string());
        let cfg_path = project_dir.join("hvigor/hvigor-config.json5");
        if let Err(e) = std::fs::write(&cfg_path, cfg) {
            eprintln!("Warning: write {} failed: {}", cfg_path.display(), e);
        }
    }

    let mut cmd = Command::new(&sdk.hvigorw);
    cmd.arg("assembleApp")
        .arg("--no-daemon")
        .current_dir(project_dir)
        .env("DEVECO_SDK_HOME", &sdk.ohos_sdk)
        .env("JAVA_HOME", &sdk.java_home);

    // OpenHarmony runtime view (created by `hvolt sdk download` next to the
    // SDK store). Harmless for runtimeOS=HarmonyOS builds; required when a
    // product uses runtimeOS "OpenHarmony".
    if let Some(parent) = sdk.ohos_sdk.parent() {
        let ohos_view = parent.join("sdk-ohos");
        if ohos_view.is_dir() {
            cmd.env("OHOS_BASE_SDK_HOME", &ohos_view);
        }
    }

    apply_tool_path(&mut cmd, sdk);
    apply_cache_home(&mut cmd);

    println!("  hvigorw: {}", sdk.hvigorw.display());
    println!("  DEVECO_SDK_HOME: {}", sdk.ohos_sdk.display());
    let injected: Vec<String> = cmd
        .get_envs()
        .filter(|&(k, _v)| k == "OHOS_BASE_SDK_HOME")
        .map(|(_k, v)| {
            v.map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "<cleared>".into())
        })
        .collect();
    println!(
        "  OHOS_BASE_SDK_HOME: {}",
        injected
            .first()
            .map(String::as_str)
            .unwrap_or("(not injected)")
    );

    let status = cmd.status().map_err(|e| format!("hvigorw: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("hvigorw exit: {}", status.code().unwrap_or(-1)))
    }
}

#[cfg(test)]
mod strip_tests {
    use super::strip_signing_configs;

    /// The normal shape: a populated array becomes an empty one, the rest survives.
    #[test]
    fn array_is_emptied_and_rest_preserved() {
        let src = r#"{"app": {"signingConfigs": [{"name": "default"}]}, "x": 1}"#;
        let out = strip_signing_configs(src).expect("should strip");
        assert_eq!(out, r#"{"app": {"signingConfigs": []}, "x": 1}"#);
    }

    /// Nested arrays inside a config must not terminate the scan early.
    #[test]
    fn nested_brackets_are_balanced() {
        let src = r#"{"signingConfigs": [{"a": [1, 2]}], "tail": true}"#;
        let out = strip_signing_configs(src).unwrap();
        assert_eq!(out, r#"{"signingConfigs": [], "tail": true}"#);
    }

    /// A `]` inside a string literal must not close the array.
    #[test]
    fn bracket_inside_a_string_is_ignored() {
        let src = r#"{"signingConfigs": [{"pwd": "a]b"}], "tail": 1}"#;
        let out = strip_signing_configs(src).unwrap();
        assert_eq!(out, r#"{"signingConfigs": [], "tail": 1}"#);
    }

    /// An escaped quote does not end the literal, so a later `]` stays inside it.
    #[test]
    fn escaped_quote_does_not_end_the_string() {
        let src = r#"{"signingConfigs": [{"pwd": "a\"]b"}], "tail": 1}"#;
        let out = strip_signing_configs(src).unwrap();
        assert_eq!(out, r#"{"signingConfigs": [], "tail": 1}"#);
    }

    /// Nothing to strip, or an unbalanced array: leave the caller's file untouched.
    #[test]
    fn missing_key_or_unbalanced_array_is_left_alone() {
        assert!(strip_signing_configs(r#"{"app": {}}"#).is_none());
        assert!(strip_signing_configs(r#"{"signingConfigs": [1, 2"#).is_none());
    }
}

#[cfg(test)]
mod assemble_env_tests {
    use super::{local_properties_content, vendored_plugin_config};

    /// hvigor wants a plain `sdk.dir=` line; the trailing newline is conventional.
    #[test]
    fn local_properties_sets_sdk_dir() {
        assert_eq!(
            local_properties_content("/opt/harmony/sdk/26.0.0.38"),
            "sdk.dir=/opt/harmony/sdk/26.0.0.38\n"
        );
    }

    /// The vendored rebind must keep the declared modelVersion and target exactly the
    /// hvolt-shipped copy, with a forward-slash path (pnpm file: deps on Windows too).
    #[test]
    fn vendored_config_rebinds_the_plugin_dependency() {
        let out =
            vendored_plugin_config("/home/u/.hvolt/tools/node_modules/@ohos/hvigor-ohos-plugin");
        assert!(out.contains(r#""modelVersion": "26.0.0""#), "{}", out);
        assert!(
            out.contains(
                r#""@ohos/hvigor-ohos-plugin": "file:/home/u/.hvolt/tools/node_modules/@ohos/hvigor-ohos-plugin""#
            ),
            "{}",
            out
        );
    }

    /// A backslash path (Windows) must be normalised to forward slashes before interpolation.
    #[test]
    fn vendored_config_normalises_windows_paths() {
        let out = vendored_plugin_config(
            r"C:\Users\u\.hvolt\tools\node_modules\@ohos\hvigor-ohos-plugin",
        );
        assert!(
            out.contains("file:C:/Users/u/.hvolt/tools/node_modules/@ohos/hvigor-ohos-plugin"),
            "{}",
            out
        );
    }
}
