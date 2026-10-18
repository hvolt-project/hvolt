// lint.rs — Code Linter (DevEco's official codelinter engine) integration.
//
// Everything below was MEASURED against DevEco 26.0.0.461 on 2026-10-10;
// none of it is documented by Huawei, so the constraints are recorded here
// so the next person does not have to rediscover them:
//
//  1. The entry point is `plugins/codelinter/index.js` (a minified node CLI
//     driven by commander), NOT a binary and NOT reachable through hvigor
//     (hvigor has no codelinter task — the IDE drives it directly).
//  2. `--dir` is parsed with JSON.parse: a bare path fails with
//     "Dir json parse error". It must be a JSON ARRAY of paths, each of
//     which must live inside `--project` (or an out-module), otherwise the
//     entry is skipped and the run ends with "No files to check!".
//  3. `--sdkNumberVersion` goes through ApiVersion, which switches to
//     DOT_API_VERSION_PATTERN once major >= FIRST_DOT_API_VERSION — so the
//     plain form "26" throws IllegalApiVersionException. The CLI swallows
//     that into its generic BASE_ERROR_MESSAGE ("Some error occurred during
//     linting"), which is why this looks like a mystery failure. Dot form
//     ("26.0.0") is always accepted.
//  4. `--logPath` is a FILE (passing a directory gives EISDIR).
//  5. The official rule config (`plugins/codelinter/arkPerfCheck/
//     code-linter.json5`) ignores `**/build/**/*`, and hvolt's standard tree
//     lives under `build/` — hvolt therefore passes its own config with that
//     ignore removed, stored in a stable location (never a temp dir).
//  6. Output is JSON lines on stdout: `{filePath, defects:[...]}` per checked
//     file plus `{content, messageType, indicator}` progress lines. The exit
//     code is 0 even when defects are found, so the gate is on the defects.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate DevEco's codelinter entry script.
pub fn find_codelinter() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("HVOLT_CODELINTER") {
        let p = PathBuf::from(p);
        let entry = if p.is_dir() { p.join("index.js") } else { p };
        if entry.exists() {
            return Some(entry);
        }
    }
    // hvolt-managed payload (works with NO DevEco install at all)
    let managed = crate::tools::codelinter_dir().join("index.js");
    if managed.exists() {
        return Some(managed);
    }
    // Derive from the discovered SDK (…/Contents/sdk → …/Contents/plugins/…)
    if let Some(root) = crate::sdk::sdk_root() {
        if let Some(contents) = root.parent() {
            let p = contents.join("plugins/codelinter/index.js");
            if p.exists() {
                return Some(p);
            }
        }
    }
    let candidates = [
        "/Applications/DevEco-Studio.app/Contents/plugins/codelinter/index.js",
        "/opt/deveco-studio/plugins/codelinter/index.js",
        "/usr/local/deveco-studio/plugins/codelinter/index.js",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .or_else(|| {
            // Windows installs (both the classic and the per-user location)
            for base in [
                "C:/Program Files/Huawei/DevEco Studio",
                "C:/Program Files/DevEco Studio",
            ] {
                let p = PathBuf::from(base).join("plugins/codelinter/index.js");
                if p.exists() {
                    return Some(p);
                }
            }
            None
        })
}

/// The node binary to run it with: configured node_home → the DevEco
/// install that ships codelinter → PATH.
fn find_node(codelinter_entry: &Path) -> Option<PathBuf> {
    let node_home = crate::sdk::configured_node_home();
    if !node_home.is_empty() {
        let p = crate::sdk::node_bin(Path::new(&node_home));
        if p.exists() {
            return Some(p);
        }
    }
    // …/Contents/plugins/codelinter/index.js → …/Contents/tools/node/bin/node
    if let Some(contents) = codelinter_entry
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
    {
        for rel in [
            "tools/node/bin/node",
            "tools/node/bin/node.exe",
            "tools/node/node",
        ] {
            let p = contents.join(rel);
            if p.exists() {
                return Some(p);
            }
        }
    }
    crate::platform::on_path("node").or_else(|| crate::platform::on_path("node.exe"))
}

/// `--sdkNumberVersion` must be dot form (see note 3 above).
pub fn dot_form_version(v: &str) -> String {
    let v = v.trim();
    if v.is_empty() {
        return "0.0.0".to_string();
    }
    match v.split('.').filter(|s| !s.is_empty()).count() {
        0 => "0.0.0".to_string(),
        1 => format!("{v}.0.0"),
        2 => format!("{v}.0"),
        _ => v.to_string(),
    }
}

/// The `--dir` value: a JSON array of paths (note 2 above).
pub fn dir_json(paths: &[String]) -> String {
    let items: Vec<String> = paths
        .iter()
        .map(|p| format!("\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", items.join(","))
}

/// Full argv for the codelinter CLI (pure, so the measured constraints are
/// testable without an IDE install).
#[allow(clippy::too_many_arguments)] // one parameter per codelinter flag, forwarded verbatim
pub fn build_args(
    project: &Path,
    check_paths: &[String],
    codelinter_root: &Path,
    sdk_path: &str,
    sdk_string_version: &str,
    sdk_number_version: &str,
    fix: bool,
    config: &Path,
    log_path: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        codelinter_root
            .join("index.js")
            .to_string_lossy()
            .to_string(),
        "--dir".into(),
        dir_json(check_paths),
        "--workdir".into(),
        codelinter_root.to_string_lossy().to_string(),
        "--project".into(),
        normal_path(project).to_string_lossy().to_string(),
        "--sdkPath".into(),
        sdk_path.to_string(),
        "--sdkStringVersion".into(),
        sdk_string_version.to_string(),
        "--sdkNumberVersion".into(),
        dot_form_version(sdk_number_version),
        "--config".into(),
        config.to_string_lossy().to_string(),
        "--logPath".into(),
        log_path.to_string_lossy().to_string(),
        "--product".into(),
        "default".into(),
    ];
    // codelinter applies fixes to whatever --dir points at; hvolt only offers
    // this with --fix, which points --dir at the project's real files.
    if fix {
        a.push("--fix".into());
        a.push("true".into());
    }
    a.push("--inIde".into());
    a.push("true".into());
    a
}

/// One checked file's result.
#[derive(Debug, PartialEq)]
pub struct FileDefects {
    pub file: String,
    pub defects: Vec<serde_json::Value>,
}

/// Parse the JSON-line stream into (messages, per-file defects).
/// Progress lines carry an `indicator`; anything with `filePath` is a result.
pub fn parse_output(stdout: &str) -> (Vec<String>, Vec<FileDefects>) {
    let mut messages = Vec::new();
    let mut results = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(fp) = v.get("filePath").and_then(|x| x.as_str()) {
            let defects = v
                .get("defects")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            results.push(FileDefects {
                file: fp.to_string(),
                defects,
            });
            continue;
        }
        let Some(content) = v.get("content").and_then(|c| c.as_str()) else {
            continue;
        };
        let mt = v.get("messageType").and_then(|m| m.as_i64()).unwrap_or(0);
        // 1 with an indicator = progress noise; keep the informational ones
        let is_progress = mt == 1 && v.get("indicator").is_some();
        if !is_progress {
            messages.push(content.to_string());
        }
    }
    (messages, results)
}

/// Stable location for the generated config: it must survive reboots and temp
/// cleaners, so it lives under `~/.hvolt/lint/`, not in a temp directory.
pub fn hvolt_lint_config_path() -> PathBuf {
    crate::sdk::home_dir()
        .join(".hvolt")
        .join("lint")
        .join("code-linter.json5")
}

/// Write hvolt's lint config: the official one minus the `**/build/**/*`
/// ignore (hvolt lints its translated tree, which IS under build/).
pub fn ensure_config(codelinter_entry: &Path) -> Result<PathBuf, String> {
    let out = hvolt_lint_config_path();
    if out.exists() {
        return Ok(out);
    }
    let official = codelinter_entry
        .parent()
        .map(|p| p.join("arkPerfCheck/code-linter.json5"));
    let mut text = match official.as_ref().filter(|p| p.exists()) {
        Some(p) => {
            std::fs::read_to_string(p).map_err(|e| format!("read {}: {}", p.display(), e))?
        }
        None => DEFAULT_LINT_CONFIG.to_string(),
    };
    // drop the build/ ignore so the translated tree is actually checked
    text = text
        .lines()
        .filter(|l| !l.contains("**/build/**"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
    }
    std::fs::write(&out, text).map_err(|e| format!("write {}: {}", out.display(), e))?;
    Ok(out)
}

const DEFAULT_LINT_CONFIG: &str = r#"{
  "files": [
    "**/*.ets"
  ],
  "ignore": [
    "**/ohosTest/**/*",
    "**/node_modules/**/*",
    "**/hvigorfile.ts",
    "**/oh_modules/**/*",
    "**/.preview/**/*"
  ],
  "ruleSet": [
    "plugin:@performance/recommended",
    "plugin:@cross-device-app-dev/recommended"
  ],
  "rules": {},
  "overrides": []
}
"#;

/// Stable location for the config used by `--all-rules`.
pub fn hvolt_lint_all_rules_config_path() -> PathBuf {
    crate::sdk::home_dir()
        .join(".hvolt")
        .join("lint")
        .join("code-linter-all.json5")
}

/// The broad rule sets (MEASURED: the official `recommended` set stays silent
/// on samples the `all` sets flag, e.g. @performance/hp-arkui-avoid-empty-callback).
const ALL_RULE_SETS: &str = r#"{
  "files": [
    "**/*.ets"
  ],
  "ignore": [
    "**/ohosTest/**/*",
    "**/node_modules/**/*",
    "**/hvigorfile.ts",
    "**/oh_modules/**/*",
    "**/.preview/**/*"
  ],
  "ruleSet": [
    "plugin:@performance/all",
    "plugin:@typescript-eslint/all"
  ],
  "rules": {},
  "overrides": []
}
"#;

/// Write (once) the `--all-rules` config.
pub fn ensure_all_rules_config() -> Result<PathBuf, String> {
    let out = hvolt_lint_all_rules_config_path();
    if out.exists() {
        return Ok(out);
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
    }
    std::fs::write(&out, ALL_RULE_SETS).map_err(|e| format!("write {}: {}", out.display(), e))?;
    Ok(out)
}

/// Human-readable one-liner for a defect (the engine returns raw JSON).
pub fn format_defect(d: &serde_json::Value) -> String {
    let get_str = |k: &str| d.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let rule = get_str("ruleId");
    let desc = get_str("description");
    let engine = get_str("engineName");
    let line = d.get("reportLine").and_then(|v| v.as_i64()).unwrap_or(0);
    let col = d.get("reportColumn").and_then(|v| v.as_i64()).unwrap_or(0);
    let sev = d.get("severity").and_then(|v| v.as_i64()).unwrap_or(0);
    format!(
        "  {line:>4}:{col:<3} sev{sev}  {desc}  [{rule}]{}",
        if engine.is_empty() {
            String::new()
        } else {
            format!(" ({engine})")
        }
    )
}

/// Reporting/gating options — they never change how the engine is driven.
#[derive(Clone, Copy)]
pub struct LintOptions {
    pub json: bool,
    pub fix: bool,
    pub fail_on: u8,
    pub max_defects: usize,
}

impl Default for LintOptions {
    fn default() -> Self {
        Self {
            json: false,
            fix: false,
            fail_on: 1,
            max_defects: 0,
        }
    }
}

pub fn lint_with(
    root: &Path,
    check_paths: &[String],
    config_override: Option<&Path>,
    all_rules: bool,
    opts: &LintOptions,
) -> Result<(), String> {
    let entry = find_codelinter()
        .ok_or("codelinter not found — install DevEco Studio or set HVOLT_CODELINTER=<path to plugins/codelinter/index.js>")?;
    let node = find_node(&entry).ok_or(
        "node not found — set node_home (`hvolt config set node_home <dir>`) or install node",
    )?;
    let codelinter_root = entry
        .parent()
        .ok_or("codelinter path has no parent")?
        .to_path_buf();

    // Absolute paths are mandatory here: the engine resolves `--project`
    // against the process cwd, so a relative `./build` turns into
    // `<tree>/build/build` and it dies with "Failed to find the
    // build-profile.json5 file in the project root directory …".
    let root =
        std::fs::canonicalize(root).map_err(|e| format!("resolve {}: {}", root.display(), e))?;
    // The standard (DevEco-shaped) tree is what the linter understands.
    let std_tree = {
        let t = root.join("build");
        std::fs::canonicalize(&t).unwrap_or(t)
    };
    if !std_tree.join("build-profile.json5").exists() {
        return Err(
            "no translated tree — run `hvolt assemble` first (lint checks build/)".to_string(),
        );
    }

    // The engine ignores anything under a directory NAMED `build` (measured:
    // identical content is checked at /tmp/x but yields "No files to check"
    // at /tmp/x/build) — and hvolt's translated tree lives in build/. So lint
    // a stable copy under hvolt's home, versioned by the assemble manifest
    // hash: a stable location (never a temp dir), and content changes get a
    // new directory so no stale file can produce a phantom defect.
    let fingerprint = std::fs::read_to_string(std_tree.join(".hvolt_manifest"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let slug = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    if opts.fix {
        // Measured 2026-10-10: codelinter only rewrites files INSIDE its project
        // root, and drives its validator off `<project>/build-profile.json5`.
        // hvolt's real sources live outside that root (the root is the flat
        // hvolt.toml + src/, the profile is inside build/), and the translated
        // mirror under build/<member>/src/main/ets/<role>/ invents structure, so
        // a changed mirror file cannot be mapped back to its source by path.
        // Refuse rather than "run and change nothing".
        return Err("--fix is not available yet: codelinter rewrites only files inside its project root, \
                   while hvolt's sources sit outside it and the translated mirror is not reverse-mappable \
                   (measured 2026-10-10). Needs a source↔mirror map recorded at assemble time; until then \
                   use `hvolt lint --json` and apply fixes by hand"
            .into());
    }
    // `--fix` works IN PLACE (same tree as the project); everything else lints
    // the managed copy, because the engine ignores any path segment named `build`
    // and the translated tree lives under exactly that.
    let lint_tree = if opts.fix {
        root.to_path_buf()
    } else {
        lint_tree_for(&slug, fingerprint)
    };
    if !opts.fix {
        let linked = sync_tree(&std_tree, &lint_tree)?;
        if linked > 0 && !opts.json {
            println!(
                "  ({linked} linked entr{} skipped — links are not followed)",
                if linked == 1 { "y" } else { "ies" }
            );
        }
    }

    let config = match config_override {
        Some(p) => p.to_path_buf(),
        None if all_rules => ensure_all_rules_config()?,
        None => ensure_config(&entry)?,
    };
    let log = crate::sdk::home_dir()
        .join(".hvolt")
        .join("lint")
        .join("codelinter.log");
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let paths: Vec<String> = if check_paths.is_empty() {
        vec![lint_tree.to_string_lossy().to_string()]
    } else {
        // every entry must be absolute and inside the project, otherwise the
        // engine silently skips it ("<path> will not be checked"); rebase it
        // onto the lint copy, which is what actually gets checked.
        let mut out = Vec::new();
        // `root` is already canonicalized above; on Windows that adds a verbatim
        // prefix the engine cannot use, so normalize it for the comparison.
        let root_abs = normal_path(&root);
        for p in check_paths {
            let pb = PathBuf::from(p);
            let abs = if pb.is_absolute() { pb } else { root.join(pb) };
            let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
            let abs = normal_path(&abs);
            let rel = abs
                .strip_prefix(&root_abs)
                .or_else(|_| abs.strip_prefix(&std_tree))
                .map_err(|_| format!("check path is outside the project: {}", abs.display()))?;
            if opts.fix {
                // in --fix mode the tree IS the project: keep the real path so
                // the engine's fixes land in the user's files
                out.push(abs.to_string_lossy().to_string());
            } else {
                out.push(lint_tree.join(rel).to_string_lossy().to_string());
            }
        }
        out
    };

    let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let sdk_path = {
        let p = crate::sdk::configured_sdk_path();
        if p.is_empty() {
            crate::sdk::sdk_root()
                .map(|r| r.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            p
        }
    };
    // The SDK version string the project targets (hvolt.toml [app] sdk_version);
    // the linter cross-checks it against its own HOS version mapper.
    let sdk_string = resolve_sdk_version(&root);
    let sdk_number = sdk_string
        .split('(')
        .next()
        .unwrap_or(&sdk_string)
        .to_string();

    // codelinter's OpenHarmony validator only accepts a component whose
    // directory is exactly `<sdkRoot>/<platformVersion>/<component>` (measured
    // with a probe on 2026-10-10: the store's component sat at
    // `<store>/<version>/openharmony/ets` while the validator expected
    // `<sdkRoot>/26.0.0/ets`, so it found NOTHING, skipped eslintAgent and
    // arkPerfCheck, and the run reported a false "0 defects"). hvolt's store
    // has the components, not that shape — build the shape with links and tell
    // the validator about it.
    if let Some(overlay) = ensure_open_sdk_overlay(&sdk_number) {
        let value = overlay.to_string_lossy().replace('\\', "/");
        let lp = lint_tree.join("local.properties");
        // The managed copy is ours; a real project's file is not.
        if !(opts.fix && lp.exists()) {
            let existed = lp.exists();
            let _ = std::fs::write(&lp, format!("sdk.dir={}\n", value));
            if opts.fix && !existed && !opts.json {
                println!(
                    "  wrote {} (DevEco Studio creates this too; sdk.dir={})",
                    lp.display(),
                    value
                );
            }
        }
    }

    let args = build_args(
        &lint_tree,
        &paths,
        &codelinter_root,
        &sdk_path,
        &sdk_string,
        &sdk_number,
        opts.fix,
        &config,
        &log,
    );

    let before = if opts.fix {
        snapshot(&path_bufs)
    } else {
        Vec::new()
    };

    // The engine APPENDS to this log, so a failure from an earlier run would
    // make the false-green guard below fire on a later clean run. Clear the
    // transient artifact so the guard only ever reads the current run.
    let _ = std::fs::write(&log, b"");

    let out = Command::new(&node)
        .args(&args)
        .current_dir(&lint_tree)
        .output()
        .map_err(|e| format!("run codelinter: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let (messages, results) = parse_output(&stdout);

    // --fix: report what changed (mtime/size before vs after), so "fixed" is a
    // readout and not a claim.
    if opts.fix {
        let after = snapshot(&path_bufs);
        let changed: Vec<String> = after
            .iter()
            .filter(|(p, t, l)| {
                before
                    .iter()
                    .find(|(bp, _, _)| bp == p)
                    .map(|(_, bt, bl)| bt != t || bl != l)
                    .unwrap_or(true)
            })
            .map(|(p, _, _)| p.to_string_lossy().to_string())
            .collect();
        if !opts.json {
            if changed.is_empty() {
                println!("  (--fix: no file changed)");
            } else {
                println!("  (--fix changed {} file(s))", changed.len());
                for c in &changed {
                    println!("    {c}");
                }
            }
        }
    }

    if !opts.json {
        for m in &messages {
            println!("  {}", m);
        }
    }
    let total: usize = results.iter().map(|r| r.defects.len()).sum();
    let checked = results.len();
    if !opts.json {
        for r in &results {
            if r.defects.is_empty() {
                continue;
            }
            println!("{}", r.file);
            for d in &r.defects {
                println!("{}", format_defect(d));
            }
        }
    }
    // Severity is numeric (1 most severe … 3 info observed); `--fail-on` sets
    // the level that fails, `--max-defects` how many are tolerated.
    let qualifying: usize = results
        .iter()
        .flat_map(|r| r.defects.iter())
        .filter(|d| defect_severity(d) <= opts.fail_on)
        .count();

    let emit_json = |error: Option<&str>| {
        let defects: Vec<serde_json::Value> = results
            .iter()
            .flat_map(|r| {
                r.defects.iter().map(|d| {
                    let g = |k: &str| d.get(k).cloned().unwrap_or(serde_json::Value::Null);
                    serde_json::json!({
                        "file": r.file,
                        "line": g("reportLine"),
                        "column": g("reportColumn"),
                        "severity": g("severity"),
                        "rule": g("ruleId"),
                        "message": g("description"),
                        "engine": g("engineName"),
                    })
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "command": "lint",
                "project": root.to_string_lossy(),
                "files": checked,
                "total": total,
                "qualifying": qualifying,
                "fail_on": opts.fail_on,
                "max_defects": opts.max_defects,
                "passed": error.is_none(),
                "error": error,
                "defects": defects,
            })
        );
    };

    if out.status.code().unwrap_or(-1) != 0 && checked == 0 {
        // The engine swallows its real exception into a generic message and
        // only records it in the log — surface that tail instead of hiding it.
        let tail = std::fs::read_to_string(&log)
            .map(|t| {
                t.lines()
                    .filter(|l| l.contains("[ERROR]"))
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if opts.json {
            emit_json(Some("engine failed"));
        }
        return Err(format!(
            "codelinter failed (rc={}). Log: {}\n{}",
            out.status.code().unwrap_or(-1),
            log.display(),
            tail
        ));
    }
    if total == 0 {
        // FALSE-GREEN GUARD. codelinter exits 0 with an empty defect list when
        // its own validator fails: it logs "Project validate failed", then
        // eslintAgent/arkPerfCheck log "no check file" and skip themselves.
        // Measured 2026-10-10 with one payload and one project: a DevEco
        // machine reports 61 real defects, a DevEco-less box reported
        // "✓ 0 files, 0 defects" — a lie, so refuse to print it.
        let log_text = std::fs::read_to_string(&log).unwrap_or_default();
        // Only these two mean "the engines did not run": the validator abort
        // (any project without a usable SDK shape) and a per-engine skip. The
        // generic "Some error occurred during linting" is NOT a marker — the
        // engine emits it for its own log4js stack-print hiccup on healthy
        // runs, which made this guard fire on a genuinely clean project.
        let markers = ["Project validate failed", "no check file"];
        let hits: Vec<&str> = log_text
            .lines()
            .filter(|l| markers.iter().any(|m| l.contains(m)))
            .collect();
        if !hits.is_empty() {
            let tail = hits
                .iter()
                .rev()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n  ");
            if opts.json {
                emit_json(Some("engines skipped"));
            }
            return Err(format!(
                "codelinter reported NO defects but skipped its engines — not a clean run.\n  {}\n  \
                 No rule ran, so this says nothing about the code. The usual cause is an SDK the \
                 validator cannot resolve (see `hvolt doctor`, section [lint]).\n  Full log: {}",
                tail, log.display()
            ));
        }
        if opts.json {
            emit_json(None);
        } else {
            println!(
                "✓ Lint clean: {} files, 0 defects (rule set: {})",
                checked,
                config.display()
            );
        }
        return Ok(());
    }
    if qualifying > opts.max_defects {
        if opts.json {
            emit_json(Some("defects"));
        }
        return Err(format!(
            "lint found {} defect(s) in {} file(s), {} at or above severity {} (log: {})",
            total,
            results.iter().filter(|r| !r.defects.is_empty()).count(),
            qualifying,
            opts.fail_on,
            log.display()
        ));
    }
    if opts.json {
        emit_json(None);
    } else {
        println!(
            "✓ no defect at or above severity {} ({} lower-severity reported; rule set: {})",
            opts.fail_on,
            total,
            config.display()
        );
    }
    Ok(())
}

/// Stable per-project copy location for linting (see the `build`-name note
/// in `lint`). This is hvolt's own managed cache under ~/.hvolt — never a
/// temp dir, and versioned by the assemble manifest hash so a changed tree
/// gets a clean directory instead of inheriting stale files.
fn lint_tree_for(slug: &str, fingerprint: u64) -> PathBuf {
    crate::sdk::home_dir()
        .join(".hvolt")
        .join("lint")
        .join("tree")
        .join(format!("{}-{:016x}", slug, fingerprint))
}

/// Recursive copy that does NOT follow links. Measured on Windows: following a
/// junction into hvigor's `node_modules` turned a 177-file tree into 18337
/// copied files; a link is not source to duplicate. Returns how
/// many linked entries were skipped.
fn sync_tree(src: &Path, dst: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {}", dst.display(), e))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("read {}: {}", src.display(), e))?;
    let mut linked = 0usize;
    for e in entries.flatten() {
        let from = e.path();
        let is_link = std::fs::symlink_metadata(&from)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if is_link {
            linked += 1;
            continue;
        }
        let to = dst.join(e.file_name());
        if from.is_dir() {
            linked += sync_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("copy {} → {}: {}", from.display(), to.display(), e))?;
        }
    }
    Ok(linked)
}

/// Windows `canonicalize` yields a verbatim `\\?\C:\…` path, which the JS
/// engine cannot resolve — measured: "--fix" runs failed with
/// `Failed to find the build-profile.json5 file in the project root directory
/// \\?\C:\…`. Strip the prefix wherever a path goes to the engine.
pub fn normal_path(p: &std::path::Path) -> std::path::PathBuf {
    let s = p.to_string_lossy().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        if let Some(unc) = rest.strip_prefix("UNC\\") {
            return std::path::PathBuf::from(format!(r"\\{}", unc));
        }
        return std::path::PathBuf::from(rest);
    }
    std::path::PathBuf::from(s)
}

/// Severity as codelinter reports it (1 = most severe). An unknown/absent
/// severity is treated as severe rather than as harmless.
pub fn defect_severity(d: &serde_json::Value) -> u8 {
    d.get("severity")
        .and_then(|v| v.as_i64())
        .map(|s| s.clamp(1, 3) as u8)
        .unwrap_or(1)
}

/// (path, mtime secs, size) for every regular file under `targets`, without
/// following links — the baseline for reporting what `--fix` changed.
fn snapshot(targets: &[PathBuf]) -> Vec<(PathBuf, u64, u64)> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = targets.to_vec();
    while let Some(p) = stack.pop() {
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {}
            Ok(m) if m.is_file() => {
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                out.push((p, t, m.len()));
            }
            Ok(_) => {
                if let Ok(rd) = std::fs::read_dir(&p) {
                    stack.extend(rd.flatten().map(|e| e.path()));
                }
            }
            Err(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Measured: "26" throws IllegalApiVersionException; dot form is accepted.
    #[test]
    fn sdk_number_version_is_normalized_to_dot_form() {
        assert_eq!(dot_form_version("26"), "26.0.0");
        assert_eq!(dot_form_version("26.1"), "26.1.0");
        assert_eq!(dot_form_version("26.0.0"), "26.0.0");
        assert_eq!(dot_form_version(" 26.0.0.38 "), "26.0.0.38");
        assert_eq!(dot_form_version(""), "0.0.0");
    }

    /// Measured: a bare path fails ("Dir json parse error") — it must be JSON.
    #[test]
    fn dir_is_json_array() {
        assert_eq!(dir_json(&["/a/b".to_string()]), "[\"/a/b\"]");
        assert_eq!(
            dir_json(&["/a b".to_string(), "/c\\d\"e".to_string()]),
            "[\"/a b\",\"/c\\\\d\\\"e\"]"
        );
        assert_eq!(dir_json(&[]), "[]");
    }

    #[test]
    fn args_carry_every_measured_requirement() {
        let args = build_args(
            Path::new("/proj/build"),
            &["/proj/build".to_string()],
            Path::new("/ide/plugins/codelinter"),
            "/ide/sdk",
            "26.0.0.38",
            "26",
            false,
            Path::new("/home/u/.hvolt/lint/code-linter.json5"),
            Path::new("/home/u/.hvolt/lint/codelinter.log"),
        );
        let joined = args.join(" ");
        assert!(joined.contains("--dir [\"/proj/build\"]"));
        assert!(joined.contains("--sdkNumberVersion 26.0.0"), "{joined}");
        assert!(!joined.contains("--fix"), "fix must be opt-in: {joined}");
        assert!(joined.contains("--sdkStringVersion 26.0.0.38"));
        assert!(joined.contains("--workdir /ide/plugins/codelinter"));
        assert!(joined.contains("--project /proj/build"));
        assert!(joined.contains("--config /home/u/.hvolt/lint/code-linter.json5"));
        // logPath must be a file path, never a directory
        assert!(joined.ends_with(
            "--logPath /home/u/.hvolt/lint/codelinter.log --product default --inIde true"
        ));
    }

    /// Real stdout captured from the measured run.
    #[test]
    fn output_parsing_separates_progress_from_defects() {
        let raw = r#"{"content":"The configuration file x is in use.","messageType":-3}
{"content":"init checkFile","messageType":1,"indicator":0.025}
{"content":"init finish","messageType":1,"indicator":0.05}
{"content":"The HarmonyOS SDK is not found","messageType":-1}
{"content":"/tmp/p/a.ets","messageType":1,"indicator":0.5}
{"defects":[],"filePath":"/tmp/p/a.ets"}
{"defects":[{"ruleId":"@performance/hp-arkui-use-reusable-component","severity":"warn","line":3}],"filePath":"/tmp/p/b.ets"}
{"content":"report finish","messageType":1,"indicator":1}"#;
        let (msgs, res) = parse_output(raw);
        // progress lines (messageType 1 + indicator) are not messages
        assert_eq!(
            msgs,
            vec![
                "The configuration file x is in use.".to_string(),
                "The HarmonyOS SDK is not found".to_string(),
            ]
        );
        assert_eq!(res.len(), 2);
        assert!(res[0].defects.is_empty());
        assert_eq!(res[1].defects.len(), 1);
        assert_eq!(res[1].file, "/tmp/p/b.ets");
    }

    #[test]
    fn config_drops_the_build_ignore() {
        let text = DEFAULT_LINT_CONFIG;
        assert!(
            !text.contains("**/build/**"),
            "the fallback config must lint build/"
        );
        assert!(text.contains("**/*.ets"));
    }
}
#[cfg(test)]
mod lint_render_tests {
    use super::*;

    /// A real defect payload captured from the measured run.
    #[test]
    fn defect_line_is_readable() {
        let d = serde_json::json!({
            "ruleId": "@performance/hp-arkui-avoid-empty-callback",
            "description": "Avoid setting empty system callback listeners",
            "engineName": "performanceAgent",
            "reportLine": 6, "reportColumn": 7, "severity": 3
        });
        let s = format_defect(&d);
        assert!(s.contains("6:7"), "{s}");
        assert!(s.contains("sev3"), "{s}");
        assert!(
            s.contains("@performance/hp-arkui-avoid-empty-callback"),
            "{s}"
        );
        assert!(s.contains("performanceAgent"), "{s}");
    }

    /// Measured: these sets flag a file the official `recommended` set passes.
    #[test]
    fn all_rules_config_uses_the_broad_sets() {
        assert!(ALL_RULE_SETS.contains("plugin:@performance/all"));
        assert!(ALL_RULE_SETS.contains("plugin:@typescript-eslint/all"));
        assert!(
            !ALL_RULE_SETS.contains("**/build/**"),
            "must lint the translated tree"
        );
    }
}

/// SDK version string to hand codelinter. `hvolt.toml [app] sdk_version` wins;
/// when that is empty, fall back to the translated build profile's
/// `compileSdkVersion` — the very field codelinter cross-checks.
///
/// Measured 2026-10-10: passing an EMPTY version makes codelinter's validator
/// fail with "Sdk path parse error", abort validation, skip eslintAgent and
/// arkPerfCheck entirely, and STILL exit 0 with an empty defect list. The same
/// payload on a project whose version resolved reported 61 real defects.
pub fn resolve_sdk_version(root: &std::path::Path) -> String {
    let from_toml = crate::config::AppConfig::from_file(&root.join("hvolt.toml"))
        .map(|c| c.app.sdk_version.clone())
        .unwrap_or_default();
    if !from_toml.trim().is_empty() {
        return from_toml;
    }
    let profile = root.join("build").join("build-profile.json5");
    if let Ok(text) = std::fs::read_to_string(&profile) {
        if let Some(v) = version_from_profile_text(&text) {
            return v;
        }
    }
    from_toml
}

/// First `compileSdkVersion` (else `compatibleSdkVersion`) string value in a
/// build profile. Dependency-free so it also copes with the JSON5 comments a
/// hand-edited profile carries.
pub fn version_from_profile_text(text: &str) -> Option<String> {
    for key in ["compileSdkVersion", "compatibleSdkVersion"] {
        if let Some(i) = text.find(&format!("\"{}\"", key)) {
            let rest = &text[i..];
            if let Some(colon) = rest.find(':') {
                let after = &rest[colon + 1..];
                if let Some(q1) = after.find('"') {
                    let tail = &after[q1 + 1..];
                    if let Some(q2) = tail.find('"') {
                        let v = tail[..q2].trim().to_string();
                        if !v.is_empty() {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Platform-version dir inside the overlay, e.g. `<root>/26.0.0`.
pub fn overlay_version_dir(root: &std::path::Path, platform_version: &str) -> std::path::PathBuf {
    root.join(platform_version)
}

/// Whether the codelinter-shaped SDK root already exists for this version
/// (read-only counterpart of `ensure_open_sdk_overlay`, for `hvolt doctor`).
pub fn open_sdk_overlay_ready(platform_version: &str) -> bool {
    let root = crate::sdk::home_dir()
        .join(".hvolt")
        .join("lint")
        .join("sdkroot");
    overlay_version_dir(&root, platform_version)
        .join("ets")
        .join("oh-uni-package.json")
        .exists()
}

/// Build (once) a codelinter-shaped SDK root: `<root>/<platformVersion>/<component>`
/// for every component of hvolt's SDK store, each an OS link to the store's own
/// component dir. Returns the root to put in `sdk.dir`, or None when no
/// OpenHarmony component set is available (e.g. an hms-only install).
///
/// Components and their manifest name come from the DevEco layout measured on
/// 2026-10-10: `{ets,js,native,previewer,toolchains}/oh-uni-package.json`.
pub fn ensure_open_sdk_overlay(platform_version: &str) -> Option<std::path::PathBuf> {
    if platform_version.trim().is_empty() {
        return None;
    }
    let store = crate::sdk::sdk_root()?;
    // Components live under `<store>/<version>/openharmony` (the `default` entry
    // is a link to a version dir). Prefer `default`, else scan for a dir that
    // actually contains the ets component.
    let mut parent = store.join("default").join("openharmony");
    if !parent.join("ets").join("oh-uni-package.json").exists() {
        parent = std::fs::read_dir(&store)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path().join("openharmony"))
            .find(|p| p.join("ets").join("oh-uni-package.json").exists())?;
    }

    let root = crate::sdk::home_dir()
        .join(".hvolt")
        .join("lint")
        .join("sdkroot");
    let version_dir = overlay_version_dir(&root, platform_version);
    std::fs::create_dir_all(&version_dir).ok()?;

    for comp in ["ets", "js", "native", "previewer", "toolchains"] {
        let target = parent.join(comp);
        if !target.exists() {
            continue;
        }
        let link = version_dir.join(comp);
        if link.exists() || std::fs::symlink_metadata(&link).is_ok() {
            continue;
        }
        if link_dir(&target, &link).is_err() {
            return None;
        }
    }
    Some(root)
}

/// Directory link, preferring the OS call and falling back to `mklink /J` on
/// Windows, where symlink creation needs a privilege an ordinary user lacks.
fn link_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return Ok(());
        }
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("mklink /J failed"))
        }
    }
}

#[cfg(test)]
mod sdk_version_tests {
    use super::*;

    #[test]
    fn version_from_profile_prefers_compile_and_copes_with_json5() {
        let t = r#"{
  // hand-edited comment
  "app": {
    "products": [
      { "name": "default", "compatibleSdkVersion": "5.0.0(12)", "compileSdkVersion": "26.0.0" }
    ]
  }
}"#;
        assert_eq!(version_from_profile_text(t), Some("26.0.0".to_string()));
        let only_compat = r#"{ "compatibleSdkVersion": "5.0.0(12)" }"#;
        assert_eq!(
            version_from_profile_text(only_compat),
            Some("5.0.0(12)".to_string())
        );
        assert_eq!(
            version_from_profile_text(r#"{ "compileSdkVersion": "" }"#),
            None
        );
        assert_eq!(version_from_profile_text("{}"), None);
    }

    #[test]
    fn verbatim_windows_paths_are_stripped() {
        assert_eq!(normal_path(Path::new("/tmp/p")), PathBuf::from("/tmp/p"));
        #[cfg(windows)]
        {
            assert_eq!(
                normal_path(Path::new(r"\\?\C:\proj")),
                PathBuf::from(r"C:\proj")
            );
            assert_eq!(
                normal_path(Path::new(r"\\?\UNC\srv\share")),
                PathBuf::from(r"\\srv\share")
            );
        }
    }

    #[test]
    fn severity_defaults_to_severe() {
        // 1 = most severe … 3 = info; unknown must not be treated as harmless
        assert_eq!(defect_severity(&serde_json::json!({"severity": 2})), 2);
        assert_eq!(defect_severity(&serde_json::json!({"severity": 0})), 1);
        assert_eq!(defect_severity(&serde_json::json!({"severity": 9})), 3);
        assert_eq!(defect_severity(&serde_json::json!({})), 1);
        assert_eq!(defect_severity(&serde_json::json!({"severity": "2"})), 1);
    }

    #[test]
    fn overlay_version_dir_is_platform_scoped() {
        let r = std::path::Path::new("/tmp/root");
        assert_eq!(
            overlay_version_dir(r, "26.0.0"),
            std::path::Path::new("/tmp/root/26.0.0")
        );
    }
}
