// tools.rs — Independent tool management for hvolt.
//
// Manages build tools (hvigor, ohpm) for hvolt without DevEco Studio.
//
// Directory layout:
//   ~/.hvolt/tools/
//     hvigor/
//       bin/hvigorw       ← launcher (npm install)
//       bin/hvigorw.js    ← launcher (npm install)
//       node_modules/     ← @ohos/hvigor + deps (npm install)
//     ohpm/               ← commandline-tools (download from huaweicloud)
//       bin/ohpm
//       lib/
//       node_modules/
//       bin/hvigorw       ← launcher (symlink from IDE or copied)

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── Tool registry ────────────────────────────────────────────────

/// Known tool versions
pub const HVIGOR_VERSION: &str = "6.26.8";
pub const OHPM_VERSION: &str = "5.0.2";

/// npm registry for @ohos scoped packages
const OHOS_NPM_REGISTRY: &str = "https://repo.harmonyos.com/npm";

/// Public download URL for commandline-tools (huaweicloud mirror)
const OHPM_MIRROR_BASE: &str = "https://repo.huaweicloud.com/harmonyos/ohpm";
const COMMANDLINE_TOOLS_URL: &str =
    "https://repo.huaweicloud.com/harmonyos/ohpm/5.0.2/oh-command-line-tools-20240715.zip";

/// Archives above this size are full platform commandline-tools (GB-scale) rather
/// than the small ohpm subtree, so the default install never picks one silently.
const OHPM_SIZE_GATE: u64 = 200_000_000;

fn home_dir() -> PathBuf {
    crate::platform::home_dir()
}

/// Root directory for downloaded tools: ~/.hvolt/tools/
pub fn tools_root() -> PathBuf {
    home_dir().join(".hvolt").join("tools")
}

/// Root for hvigor installation
/// Cache root handed to ohpm/hvigor children: `HVOLT_CACHE_HOME`, or the value
/// `hvolt build --cache-dir` exported before the pipeline runs.
pub fn cache_home() -> Option<PathBuf> {
    let raw = std::env::var("HVOLT_CACHE_HOME").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn hvigor_dir() -> PathBuf {
    tools_root().join("hvigor")
}

/// Root for ohpm installation
/// Where a Code Linter payload lives when DevEco Studio is NOT installed.
///
/// Directory holding the codelinter payload (`~/.hvolt/tools/codelinter`).
///
/// `hvolt lint` drives DevEco's `plugins/codelinter/index.js`. Huawei ships
/// that payload only inside DevEco Studio: npm has no `@ohos/codelinter`
/// (404 on registry.npmjs.org and repo.harmonyos.com/npm) and the official
/// SDK component list carries only ets/js/native/previewer/toolchains. So the
/// the only source is a machine that already has DevEco Studio: copy
/// `plugins/codelinter` (~200 MB, 61 MB of it `typescript`) and install it
/// with `hvolt tools install codelinter --from <dir>`.
pub fn codelinter_dir() -> PathBuf {
    tools_root().join("codelinter")
}

/// Directories inside the payload that the CLI does not need: `lib/` holds the
/// IDE-side Java jars (log4j/poi/xmlbeans + the instrumented IntelliJ plugin).
const CODELINTER_SKIP_DIRS: &[&str] = &["lib"];

/// Identify a native executable's platform flavour from its magic bytes.
/// The C/C++ auditors (performanceAgent/cppaudit, hpaudit) are per-OS
/// binaries; the ArkTS/TS engines are plain JS and portable.
pub fn native_payload_kind(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xCF, 0xFA, 0xED, 0xFE, ..] | [0xFE, 0xED, 0xFA, 0xCF, ..] => "mach-o",
        [0xCE, 0xFA, 0xED, 0xFE, ..] | [0xFE, 0xED, 0xFA, 0xCE, ..] => "mach-o",
        [0x7F, b'E', b'L', b'F', ..] => "elf",
        [b'M', b'Z', ..] => "pe",
        _ => "unknown",
    }
}

/// Install a Code Linter payload into `~/.hvolt/tools/codelinter`.
pub fn install_codelinter(from: Option<&str>) -> Result<(), String> {
    let Some(src) = from else {
        return Err(concat!(
            "no payload source given.\n",
            "Huawei ships the Code Linter only inside DevEco Studio: there is no npm package\n",
            "(measured 404) and the SDK component list has no linter component.\n",
            "Get `plugins/codelinter` from a machine with DevEco Studio, then:\n",
            "  hvolt tools install codelinter --from <that-directory>"
        )
        .to_string());
    };
    let src = PathBuf::from(src);
    if !src.join("index.js").exists() {
        return Err(format!(
            "{} does not look like a codelinter payload (no index.js)",
            src.display()
        ));
    }
    for engine in ["eslintAgent", "performanceAgent"] {
        if !src.join(engine).exists() {
            return Err(format!(
                "{} is missing the {} engine",
                src.display(),
                engine
            ));
        }
    }
    let dst = codelinter_dir();
    std::fs::create_dir_all(&dst).map_err(|e| format!("mkdir {}: {}", dst.display(), e))?;
    let copied = copy_tree_skipping(&src, &dst, CODELINTER_SKIP_DIRS)?;
    println!(
        "✓ codelinter payload installed at {} ({} files, lib/ skipped)",
        dst.display(),
        copied
    );

    // Platform warning: the native auditors are per-OS binaries.
    let native = src.join("performanceAgent").join("cppaudit");
    if let Ok(head) = std::fs::read(&native).map(|b| b.iter().take(4).copied().collect::<Vec<u8>>())
    {
        let kind = native_payload_kind(&head);
        let here = if crate::platform::is_windows() {
            "pe"
        } else if crate::platform::is_macos() {
            "mach-o"
        } else {
            "elf"
        };
        if kind != "unknown" && kind != here {
            println!(
                "  ⚠ payload's native auditors are {} but this machine is {} — \
                 ArkTS/TS rules still work, C/C++ auditing will not",
                kind, here
            );
        }
    }
    println!("  next: hvolt lint (or hvolt lint --all-rules)");
    Ok(())
}

/// Recursive copy that skips the named top-level directories.
fn copy_tree_skipping(src: &Path, dst: &Path, skip: &[&str]) -> Result<usize, String> {
    let mut n = 0usize;
    // the recursive call must create its own destination (the earlier version
    // only created the top level, so the first non-empty subdirectory failed
    // with ENOENT while the error text blamed the SOURCE path)
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {}", dst.display(), e))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("read {}: {}", src.display(), e))?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let from = e.path();
        if from.is_dir() {
            if skip.contains(&name.as_str()) {
                continue;
            }
            n += copy_tree_skipping(&from, &dst.join(&name), &[])?;
        } else {
            std::fs::copy(&from, dst.join(&name))
                .map_err(|e| format!("copy {}: {}", from.display(), e))?;
            n += 1;
        }
    }
    Ok(n)
}

pub fn ohpm_dir() -> PathBuf {
    tools_root().join("ohpm")
}

// ─── Status ───────────────────────────────────────────────────────

/// Print status of all managed tools
pub fn list_tools() {
    {
        let cl = codelinter_dir();
        if cl.join("index.js").exists() {
            println!("codelinter  {} (payload, lint rules)", cl.display());
        }
    }
    println!("Tools directory: {}", tools_root().display());
    println!();

    for (name, dir) in &[("hvigor", &hvigor_dir()), ("ohpm", &ohpm_dir())] {
        let installed = dir.exists();
        // explicit marker: absent tools never print a path (a bare space made
        // "not installed" rows read as installed)
        if !installed {
            println!(
                "  ✗ {}  (not installed — hvolt tools install {})",
                name, name
            );
            continue;
        }
        println!("  ✓ {}  {}", name, dir.display());
        if installed {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() || p.is_symlink() {
                        let name = p.file_name().unwrap_or_default().to_string_lossy();
                        println!("      └── {}/", name);
                    }
                }
            }
        }
    }
    println!();

    // (per-tool ✗ lines above already carry the install hint)
}

// ─── hvigor install (npm packages + launcher) ─────────────────────

pub fn install_hvigor(version: Option<&str>) -> Result<(), String> {
    let version = version.unwrap_or(HVIGOR_VERSION);
    let target = hvigor_dir();
    // idempotent reinstall: usable tree short-circuits; partial tree
    // (left by a failed npm) is repaired in place — never demand rm -rf
    if launcher_present_for_hvigor() {
        println!(
            "hvigor already installed and usable at {}",
            target.display()
        );
        return Ok(());
    }

    // Need Node.js on PATH
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !node_ok {
        return Err(
            "Node.js not found on PATH. Install Node.js (https://nodejs.org) first.".into(),
        );
    }

    fs::create_dir_all(&target).map_err(|e| format!("mkdir: {}", e))?;

    // Create .npmrc: @ohos scope uses HarmonyOS registry, others use public npm
    let npmrc = format!(
        "@ohos:registry={}\nregistry=https://registry.npmjs.org/\n",
        OHOS_NPM_REGISTRY
    );
    fs::write(target.join(".npmrc"), &npmrc).map_err(|e| format!("write .npmrc: {}", e))?;

    let pkg_json = serde_json::json!({
        "name": "hvolt-hvigor",
        "private": true,
        "dependencies": {
            "@ohos/hvigor": version,
            "@ohos/hvigor-ohos-plugin": version,
        }
    });
    let pkg_str = serde_json::to_string_pretty(&pkg_json)
        .map_err(|e| format!("serialize package.json: {}", e))?;
    fs::write(target.join("package.json"), &pkg_str)
        .map_err(|e| format!("write package.json: {}", e))?;

    println!("  Installing @ohos/hvigor v{} and dependencies...", version);
    println!(
        "  (@ohos packages from {}, others from npmjs.org)",
        OHOS_NPM_REGISTRY
    );

    let status = Command::new(
        crate::platform::npm_launcher().unwrap_or_else(|| std::ffi::OsString::from("npm").into()),
    )
    .args(["install", "--no-optional"])
    .current_dir(&target)
    .status()
    .map_err(|e| format!("npm install: {}", e))?;

    if !status.success() {
        return Err("npm install failed".into());
    }

    // Create hvigorw launcher that uses node from PATH
    fs::create_dir_all(target.join("bin")).map_err(|e| format!("mkdir bin: {}", e))?;

    // npm package entry points (in order of preference):
    //   - @ohos/hvigor/bin/hvigor.js      (npm @ohos/hvigor 6.x CLI entry)
    //   - @ohos/hvigor/bin/hvigorw.js     (legacy layout)
    // The launcher must invoke the script IN PLACE: it requires "../src/..."
    // relative paths, so copying it to bin/ would break module resolution.
    let candidates = [
        target.join("node_modules/@ohos/hvigor/bin/hvigor.js"),
        target.join("node_modules/@ohos/hvigor/bin/hvigorw.js"),
    ];
    let entry = candidates.iter().find(|p| p.exists());

    match entry {
        Some(entry) => {
            let rel = entry
                .strip_prefix(&target)
                .unwrap_or(entry.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            let wrapper = format!(
                r##"#!/usr/bin/env bash
DIR="$(cd "$(dirname "$0")/.." && pwd)"
exec node "$DIR/{rel}" "$@"
"##
            );
            fs::write(target.join("bin/hvigorw"), &wrapper)
                .map_err(|e| format!("write hvigorw: {}", e))?;
            // Windows sibling launcher (cmd shim) — same semantics, no sh
            let cmd_shim = format!(
                "@echo off\r\nsetlocal\r\nset \"DIR=%~dp0..\"\r\nnode \"%DIR%\\{rel}\" %*\r\n"
            );
            let _ = fs::write(target.join("bin/hvigorw.cmd"), cmd_shim);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    target.join("bin/hvigorw"),
                    fs::Permissions::from_mode(0o755),
                )
                .ok();
            }

            println!(
                "  ✓ hvigor v{} + launcher installed at {}",
                version,
                target.display()
            );
            println!("    entry: {}", entry.display());
        }
        None => {
            println!(
                "  ✓ hvigor v{} npm packages installed at {}",
                version,
                target.display()
            );
            println!("  ⚠ no CLI entry found in node_modules/@ohos/hvigor/bin/ — launcher not available.");
        }
    }

    Ok(())
}

// ─── ohpm install ──────────────────────────────────────────
//
// ohpm is distributed as part of commandline-tools. We download from
// the public huaweicloud mirror (ohpm 5.0.2, the latest publicly
// available cross-platform version).
//
// Newer versions (26.x) require a Huawei Developer account login at
// https://developer.huawei.com — users with a newer version can set:
//   hvolt config set ohpm_path /path/to/command-line-tools/ohpm
//
// Installation:
//   1. Download commandline-tools zip from huaweicloud mirror
//   2. Extract and copy ohpm/ to ~/.hvolt/tools/ohpm/
//   3. Fallback: copy from DevEco Studio if present

/// Known DevEco Studio ohpm paths by platform
fn deveco_ohpm_paths() -> Vec<PathBuf> {
    let dirs = [
        "/Applications/DevEco-Studio.app/Contents/tools/ohpm",
        "C:/Program Files/Huawei/DevEco Studio/tools/ohpm",
    ];
    let mut paths: Vec<PathBuf> = dirs
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    // macOS per-user tools location
    if let Some(home) = home_dir().parent() {
        let user = home.join("Library/Application Support/DevEco-Studio/ohpm");
        if user.exists() {
            paths.push(user);
        }
    }
    paths
}

/// Newest-first ordering for dotted versions, without pulling in a semver crate.
fn newest_first(versions: &mut [String]) {
    versions.sort_by(|a, b| {
        let key = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
        key(b).cmp(&key(a))
    });
}

/// Default ohpm archive: the pinned build, or — when the mirror no longer serves
/// it — the newest version that still ships the small, cross-platform package.
/// Without this the default install hard-failed once the pinned archive was
/// removed (the mirror keeps only recent versions).
fn resolve_default_ohpm() -> Result<(String, String), String> {
    if remote_content_length(COMMANDLINE_TOOLS_URL) > 0 {
        return Ok((COMMANDLINE_TOOLS_URL.to_string(), OHPM_VERSION.to_string()));
    }
    eprintln!(
        "  pinned ohpm {} archive is gone from the mirror; looking for the newest small build",
        OHPM_VERSION
    );
    let listing = ureq_get_string(OHPM_MIRROR_BASE)?;
    let mut versions = parse_ohpm_versions(&listing);
    newest_first(&mut versions);
    for v in versions {
        if v == OHPM_VERSION {
            continue;
        }
        let Ok((url, host_match)) = ohpm_mirror_zip_url2(&v) else {
            continue;
        };
        if !host_match {
            continue;
        }
        let size = remote_content_length(&url);
        if size > 0 && size <= OHPM_SIZE_GATE {
            eprintln!(
                "  falling back to ohpm {} ({})",
                v,
                crate::sdk::fmt_size(size)
            );
            return Ok((url, v));
        }
    }
    Err(format!(
        "no usable ohpm archive found: pinned {} is gone and the mirror has no small \
cross-platform build. Pick one explicitly with `hvolt tools install ohpm --version <v>` \
(see `hvolt tools list-remote`).",
        OHPM_VERSION
    ))
}

pub fn install_ohpm(version: Option<&str>, assume_yes: bool) -> Result<(), String> {
    // Resolve the archive URL: explicit version → query the mirror listing;
    // default → the verified 5.0.2 cross-platform package.
    let (url, ver) = match version {
        None => resolve_default_ohpm()?,
        Some(v) => {
            let (url, host_match) = ohpm_mirror_zip_url2(v)?;
            if !host_match {
                let (os, arch) = ohpm_platform_tag();
                return Err(format!(
                    "mirror has no commandline-tools-{}-{} build for {} (would ship a foreign-platform package) — try `hvolt tools list-remote`",
                    os, arch, v));
            }
            // 5.0.3+ archives are full platform commandline-tools (GB-scale)
            // for a ~60MB ohpm subset — gate with the real size before burning bandwidth.
            let size = remote_content_length(&url);
            if size > OHPM_SIZE_GATE && !assume_yes {
                eprintln!("  ohpm {} ships as {} (full commandline-tools; ohpm is a small subtree inside).",
                    v, crate::sdk::fmt_size(size));
                eprint!("  Download it anyway? [y/N] ");
                use std::io::Write;
                std::io::stderr().flush().ok();
                let mut ans = String::new();
                std::io::stdin().read_line(&mut ans).unwrap_or(0);
                let a = ans.trim().to_lowercase();
                if a != "y" && a != "yes" {
                    return Err(format!(
                        "aborted. Default stays at {}: `hvolt tools install ohpm` ({} cross-platform, small).",
                        OHPM_VERSION, OHPM_VERSION));
                }
            }
            (url, v.to_string())
        }
    };

    let target = ohpm_dir();
    if target.exists() {
        return Err(format!(
            "ohpm already installed at {}. Remove it first.",
            target.display()
        ));
    }

    // Need Node.js on PATH (ohpm is a Node.js CLI)
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !node_ok {
        return Err(
            "Node.js not found on PATH. Install Node.js (https://nodejs.org) first.".into(),
        );
    }

    fs::create_dir_all(&target).map_err(|e| format!("mkdir: {}", e))?;

    // Strategy 1: download commandline-tools and extract the ohpm/ subtree.
    // The subtree prefix differs across package generations
    // (oh-command-line-tools-20240715.zip → "oh-command-line-tools/ohpm/",
    //  commandline-tools-<os>-<arch>-*.zip → "<dir>/ohpm/"), so discover the
    // prefix from the archive listing instead of assuming it.
    println!("  Downloading ohpm {} from mirror...", ver);
    let tmp_zip = target.join("_commandline-tools.zip");
    let dl_status = Command::new("curl")
        .args(["-sL", "-o", &tmp_zip.to_string_lossy(), &url])
        .status()
        .map_err(|e| format!("curl download: {}", e))?;
    if dl_status.success() {
        if let Some(prefix) = ohpm_prefix_in_zip(&tmp_zip) {
            // selective subtree extraction, pure Rust (no unzip/cp/chmod —
            // none of which a bare Windows host ships)
            match extract_subtree(&tmp_zip, &prefix, target.as_path()) {
                Ok(()) => {
                    if launcher_present(target.as_path()) {
                        let _ = fs::remove_file(&tmp_zip);
                        println!("  ✓ ohpm v{} installed at {}", ver, target.display());
                        return Ok(());
                    }
                    eprintln!("  ⚠ extracted but launcher missing under {}", prefix);
                }
                Err(e) => eprintln!("  ⚠ extract failed: {}", e),
            }
        } else {
            eprintln!("  ⚠ archive has no recognizable ohpm subtree (new package shape?)");
        }
        let _ = fs::remove_file(&tmp_zip);
        let _ = fs::remove_dir_all(target.join("_extract"));
    }

    // Strategy 2: copy from DevEco Studio (fallback)
    for src in deveco_ohpm_paths() {
        println!("  Found ohpm in DevEco Studio at: {}", src.display());
        println!("  Copying to {}...", target.display());
        let status = Command::new("cp")
            .arg("-a")
            .arg(src.to_string_lossy().to_string() + "/.")
            .arg(target.to_string_lossy().to_string())
            .status()
            .map_err(|e| format!("cp: {}", e))?;

        if status.success() && target.join("bin/ohpm").exists() {
            println!(
                "  ✓ ohpm (from DevEco Studio) installed at {}",
                target.display()
            );
            return Ok(());
        }
    }

    let _ = fs::remove_dir_all(&target);

    Err(format!(
        "ohpm install failed (version {ver}).

Mirror URL tried: {url}
Known versions: `hvolt tools list-remote`

To install ohpm manually:
  1. Download Command Line Tools from:
     https://developer.huawei.com/consumer/en/download/
  2. Extract and note the ohpm/ directory path
  3. Run: hvolt config set ohpm_path /path/to/ohpm
",
        ver = ver,
        url = url
    ))
}

/// Total object size via HEAD (content-length, falling back to the
/// content-range total from a 1-byte range probe — mirrors without HEAD
/// support still answer range GETs).
pub(crate) fn remote_content_length(url: &str) -> u64 {
    if let Ok(resp) = crate::http::agent().request("HEAD", url).call() {
        if let Some(v) = resp.header("content-length").and_then(|s| s.parse().ok()) {
            return v;
        }
    }
    if let Ok(resp) = crate::http::agent()
        .get(url)
        .set("Range", "bytes=0-0")
        .call()
    {
        if let Some(v) = resp
            .header("content-range")
            .and_then(|s| s.rsplit('/').next())
            .and_then(|s| s.trim().parse().ok())
        {
            return v;
        }
    }
    0
}

/// Returns (url, host_platform_match) — an explicit mismatch is reported
/// rather than silently shipping a foreign-platform archive.
pub(crate) fn ohpm_mirror_zip_url2(version: &str) -> Result<(String, bool), String> {
    let listing_url = format!("{}/{}/", OHPM_MIRROR_BASE, version);
    let resp = ureq::get(&listing_url)
        .call()
        .map_err(|e| format!("query {} failed: {}", listing_url, e))?;
    let mut buf = String::new();
    resp.into_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("read listing: {}", e))?;
    let zip = parse_ohpm_zip(&buf)
        .ok_or_else(|| format!("no .zip archive found under {}", listing_url))?;
    let (os, arch) = ohpm_platform_tag();
    let host_match = !zip.contains("commandline-tools-")
        || zip.contains(&format!("commandline-tools-{}-{}", os, arch));
    Ok((format!("{}{}", listing_url, zip), host_match))
}

/// First entry ending in ohpm/bin/ohpm → its subtree prefix ending "ohpm/".
pub(crate) fn ohpm_prefix_in_zip(zip: &Path) -> Option<String> {
    let names = crate::platform::zip_names(zip).ok()?;
    const MARKER: &str = "ohpm/bin/ohpm";
    names.iter().find_map(|l| {
        let l = l.trim();
        if l.ends_with(MARKER) {
            Some(l[..l.len() - MARKER.len() + "ohpm/".len()].to_string())
        } else {
            None
        }
    })
}

// ─── Remote inventory ─────────────────────────────────────────────

/// Print remotely-available versions for each managed tool + current defaults.
pub fn list_remote() -> Result<(), String> {
    println!("hvigor  @ohos/hvigor ({})", OHOS_NPM_REGISTRY);
    let hvigor_json = ureq_get_string(&format!("{}/@ohos%2Fhvigor", OHOS_NPM_REGISTRY))?;
    match parse_npm_latest(&hvigor_json) {
        Some(latest) => println!(
            "  dist-tags latest: {}   (hvolt default: {})",
            latest, HVIGOR_VERSION
        ),
        None => println!("  ⚠ could not parse registry response"),
    }

    println!("ohpm    {}", OHPM_MIRROR_BASE);
    let listing = ureq_get_string(OHPM_MIRROR_BASE)?;
    let versions = parse_ohpm_versions(&listing);
    println!("  mirror versions   (hvolt default: {})", OHPM_VERSION);
    for v in &versions {
        match ohpm_mirror_zip_url2(v) {
            Ok((url, host_match)) => {
                let sz = remote_content_length(&url);
                let name = url.rsplit('/').next().unwrap_or("?");
                println!(
                    "    {:<7} {:>10}  {}{}",
                    v,
                    crate::sdk::fmt_size(sz),
                    name,
                    if host_match {
                        ""
                    } else {
                        "   (not this platform — mirror lacks a build)"
                    }
                );
            }
            Err(_) => println!("    {:<7}  (no archive found)", v),
        }
    }
    println!("  note: 5.0.3+ packages are platform-specific commandline-tools;");
    println!(
        "        `hvolt tools install ohpm --version <v>` discovers the archive automatically"
    );
    Ok(())
}

fn ureq_get_string(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("GET {}: {}", url, e))?;
    let mut buf = String::new();
    resp.into_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("read {}: {}", url, e))?;
    Ok(buf)
}

// ─── Detection helpers ────────────────────────────────────────────

/// Find hvigorw binary in hvolt tools
pub fn find_hvigorw() -> Option<PathBuf> {
    let hvigor = hvigor_dir();
    if !hvigor.exists() {
        return None;
    }

    // the wrappers: sh (posix) / cmd shim (windows), then the upstream js entry
    let bin = hvigor.join("bin");
    let order: Vec<std::path::PathBuf> = if crate::platform::is_windows() {
        vec![
            bin.join("hvigorw.cmd"),
            bin.join("hvigorw"),
            bin.join("hvigorw.js"),
        ]
    } else {
        vec![
            bin.join("hvigorw"),
            bin.join("hvigorw.cmd"),
            bin.join("hvigorw.js"),
        ]
    };
    order.into_iter().find(|p| p.exists())
}
/// Find ohpm binary: config ohpm_path → PATH which → ~/.hvolt/tools/ohpm/bin/ohpm
/// Extract entries under `prefix` from a zip into `dest`, flattening the
/// prefix. Pure Rust; preserves unix exec bits where the platform has them.
pub(crate) fn extract_subtree(zip: &Path, prefix: &str, dest: &Path) -> Result<(), String> {
    use std::io;
    let zf = fs::File::open(zip).map_err(|e| format!("open: {}", e))?;
    let mut ar = ::zip::ZipArchive::new(zf).map_err(|e| e.to_string())?;
    for i in 0..ar.len() {
        let mut f = ar.by_index(i).map_err(|e| e.to_string())?;
        let name = match f.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => return Err(format!("unsafe entry {:?}", f.name())),
        };
        let ns = name.to_string_lossy().to_string();
        if !ns.starts_with(prefix) || f.is_dir() {
            continue;
        }
        let rel = &ns[prefix.len()..];
        let out = crate::platform::win_long(&dest.join(rel.trim_start_matches('/')));
        if let Some(par) = out.parent() {
            fs::create_dir_all(par).map_err(|e| format!("mkdir: {}", e))?;
        }
        let mut w = io::BufWriter::with_capacity(
            1 << 20,
            fs::File::create(&out).map_err(|e| e.to_string())?,
        );
        io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = f.unix_mode() {
                let _ = fs::set_permissions(&out, fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

/// Any usable launcher present (sh wrapper or .bat)?
fn launcher_present_for_hvigor() -> bool {
    crate::tools::find_hvigorw().is_some()
}

pub(crate) fn launcher_present(ohpm_root: &Path) -> bool {
    crate::platform::ohpm_launcher(&ohpm_root.join("bin")).is_some()
}

pub fn find_ohpm() -> Option<PathBuf> {
    use crate::sdk::load_global_config;
    use std::path::Path;

    // 1. Config ohpm_path — launcher variants (sh wrapper / .bat / root)
    let cfg = load_global_config();
    if !cfg.ohpm_path.is_empty() {
        let bin = Path::new(&cfg.ohpm_path).join("bin");
        if let Some(l) = crate::platform::ohpm_launcher(&bin) {
            return Some(l);
        }
        let direct = Path::new(&cfg.ohpm_path);
        if direct.exists() {
            return Some(direct.to_path_buf());
        }
    }

    // 2. PATH
    if let Some(p) = crate::platform::on_path("ohpm") {
        return Some(p);
    }

    // 3. ~/.hvolt/tools/ohpm/bin launcher (platform-correct)
    let bin = ohpm_dir().join("bin");
    crate::platform::ohpm_launcher(&bin)
}

// ─── Internal helpers ─────────────────────────────────────────────

/// Extract href="..." values from listing HTML (no full parser needed —
/// the mirror is a plain directory table).
pub(crate) fn extract_hrefs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("href=\"") {
        rest = &rest[i + 6..];
        if let Some(j) = rest.find('"') {
            out.push(rest[..j].to_string());
            rest = &rest[j + 1..];
        }
    }
    out
}

/// Parse version dirs from the huaweicloud ohpm mirror listing HTML.
/// Returns sorted-descending version strings ("5.0.5","5.0.3","5.0.2").
pub(crate) fn parse_ohpm_versions(html: &str) -> Vec<String> {
    let mut out: Vec<String> = extract_hrefs(html)
        .into_iter()
        .filter_map(|h| {
            let h = h.trim_end_matches('/');
            if h.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                Some(h.to_string())
            } else {
                None
            }
        })
        .collect();
    out.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
    out
}

fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Pick this host's platform archive from a version-dir listing.
/// 5.0.3+ dirs ship one zip per platform (commandline-tools-<os>-<arch>);
/// older cross-platform packages (oh-command-line-tools-*) have no marker.
pub(crate) fn parse_ohpm_zip(html: &str) -> Option<String> {
    let zips: Vec<String> = extract_hrefs(html)
        .into_iter()
        .filter(|h| h.ends_with(".zip"))
        .collect();
    let (os, arch) = ohpm_platform_tag();
    zips.iter()
        .find(|z| z.contains(&format!("commandline-tools-{}-{}", os, arch)))
        .cloned()
        .or_else(|| {
            zips.iter()
                .find(|z| !z.contains("commandline-tools-"))
                .cloned()
        })
        .or_else(|| zips.first().cloned())
}

pub(crate) fn ohpm_platform_tag() -> (&'static str, &'static str) {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => ("windows", "x64"),
        ("macos", "aarch64") => ("darwin", "arm64"),
        ("macos", "x86_64") => ("darwin", "x64"),
        ("linux", "x86_64") => ("linux", "x64"),
        ("linux", "aarch64") => ("linux", "arm64"),
        _ => ("linux", "x64"),
    }
}

/// Parse npm registry JSON dist-tags.latest.
pub(crate) fn parse_npm_latest(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v["dist-tags"]["latest"].as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_zip_selection() {
        let html = r#"<a href="commandline-tools-linux-x64-5.0.5.310.zip">l</a>
<a href="commandline-tools-windows-x64-5.0.5.310.zip">w</a>
<a href="commandline-tools-darwin-arm64-5.0.5.310.zip">d</a>"#;
        let (os, arch) = ohpm_platform_tag();
        let picked = parse_ohpm_zip(html).unwrap();
        assert!(
            picked.contains(&format!("commandline-tools-{}-{}", os, arch)),
            "{} vs host {}/{}",
            picked,
            os,
            arch
        );
        // cross-platform legacy package still resolves when no marker exists
        let legacy = r#"<a href="oh-command-line-tools-20240715.zip">x</a>"#;
        assert_eq!(
            parse_ohpm_zip(legacy).unwrap(),
            "oh-command-line-tools-20240715.zip"
        );
    }

    #[test]
    fn ohpm_versions_parsed_sorted_descending() {
        let html = r#"<a href="../">..</a>
<a href="5.0.2/">5.0.2/</a>
<a href="5.0.5/">5.0.5/</a>
<a href="5.0.3/">5.0.3/</a>
<a href="?C=N&O=A">Name</a>
<a href="/repository/static/css/style.css">css</a>"#;
        assert_eq!(parse_ohpm_versions(html), vec!["5.0.5", "5.0.3", "5.0.2"]);
    }

    #[test]
    fn ohpm_zip_filename_extracted() {
        let html =
            r#"<a href="../">..</a><a href="commandline-tools-linux-x64-5.0.5.310.zip">z</a>"#;
        assert_eq!(
            parse_ohpm_zip(html).as_deref(),
            Some("commandline-tools-linux-x64-5.0.5.310.zip")
        );
        assert_eq!(parse_ohpm_zip("<a href=\"index.html\">"), None);
    }

    #[test]
    fn npm_latest_from_dist_tags() {
        let j = r#"{"dist-tags":{"legacy":"6.24.5","latest":"6.26.8"},"versions":{}}"#;
        assert_eq!(parse_npm_latest(j).as_deref(), Some("6.26.8"));
        assert_eq!(parse_npm_latest("not json"), None);
        assert_eq!(parse_npm_latest("{}"), None);
    }

    #[test]
    fn version_key_orders_correctly() {
        assert!(version_key("5.0.10") > version_key("5.0.9"));
        assert!(version_key("6.26.8") > version_key("6.24.5"));
    }
}

#[cfg(test)]
mod codelinter_payload_tests {
    use super::*;

    #[test]
    fn native_kind_is_classified_from_magic() {
        assert_eq!(
            native_payload_kind(&[0xCF, 0xFA, 0xED, 0xFE, 0x07, 0x00]),
            "mach-o"
        );
        assert_eq!(native_payload_kind(&[0x7F, b'E', b'L', b'F', 2, 1]), "elf");
        assert_eq!(native_payload_kind(&[b'M', b'Z', 0x90, 0x00]), "pe");
        assert_eq!(native_payload_kind(&[0x00, 0x01, 0x02, 0x03]), "unknown");
    }

    /// The IDE-only jar directory must stay in the skip list (13 MB of
    /// log4j/poi/xmlbeans + the IntelliJ plugin the CLI never loads).
    #[test]
    fn ide_jars_are_skipped() {
        assert!(CODELINTER_SKIP_DIRS.contains(&"lib"));
    }
}

#[cfg(test)]
mod payload_copy_tests {
    use super::*;

    /// Nested subdirectories must be created — the first version copied into
    /// nonexistent dirs and failed on any non-empty subdirectory.
    #[test]
    fn copy_skips_ide_jars_and_keeps_nested_files() {
        let base = std::env::temp_dir().join(format!("hvolt-cl-copy-{}", std::process::id()));
        let src = base.join("src");
        std::fs::create_dir_all(src.join("eslintAgent")).unwrap();
        std::fs::create_dir_all(src.join("lib")).unwrap();
        std::fs::write(src.join("index.js"), "// payload").unwrap();
        std::fs::write(src.join("eslintAgent/eslint.js"), "// engine").unwrap();
        std::fs::write(src.join("lib/huge.jar"), "jar").unwrap();

        let dst = base.join("dst");
        let n = copy_tree_skipping(&src, &dst, CODELINTER_SKIP_DIRS).unwrap();
        assert_eq!(n, 2, "index.js + eslintAgent/eslint.js");
        assert!(dst.join("index.js").exists());
        assert!(
            dst.join("eslintAgent/eslint.js").exists(),
            "nested file must land"
        );
        assert!(!dst.join("lib").exists(), "IDE jars skipped");

        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod ohpm_resolution_tests {
    use super::newest_first;

    #[test]
    fn newest_first_orders_numerically() {
        let mut v: Vec<String> = ["5.0.2", "5.0.10", "4.9.9", "5.0.3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        newest_first(&mut v);
        assert_eq!(v, ["5.0.10", "5.0.3", "5.0.2", "4.9.9"]);
    }
}
