// abc_compile: drive the arkts es2abc merge from hvolt, standalone.
//
// Pipeline (mirrors `gen_merged_abc.js` in the SDK ets-loader, verified against
// asr-ondevice's reference cache):
//   1. a pre-pass (rollup+arkts-compose, not this module) expands each .ets into
//      a .ts with OHM-URL imports — e.g. `@kit.AbilityKit` becomes
//      `@ohos:app.ability.UIAbility` and `libasrprobe.so` becomes
//      `@normalized:Y&&&libasrprobe.so&`.
//   2. this module writes `filesInfo.txt` (one line per file:
//      `<ts>;&<recordName>&;esm;<src|pkg|ver|rel>;<pkg>;false;<ets|ts>`) and
//      `npmEntries.txt` (the SDK's fixed ohos/system module remap).
//   3. `es2abc --merge-abc "@filesInfo.txt" --npm-module-entry-list <npmEntries>
//      --output modules.abc --file-threads N`.
//
// Verified: re-running the merge from the reference's own cache reproduces a modules.abc
// whose record set is identical (byte diff is only internal literal ids).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The SDK's fixed module remap for the es2abc merge. Sampled from the reference's
/// `npmEntries.txt` on asr-ondevice; these map the legacy `@system.*`/`@ohos.*`
/// virtual modules to their native stubs.
pub const NPM_ENTRIES: &str = "\
@system.app:@native.system.app
@ohos.app:@native.ohos.app
@system.router:@native.system.router
@system.curves:@native.system.curves
@ohos.curves:@native.ohos.curves
@system.matrix4:@native.system.matrix4
@ohos.matrix4:@native.ohos.matrix4
";

/// Inputs for the es2abc merge stage.
pub struct MergeInput<'a> {
    /// The pre-pass output directory that holds the expanded `.ts` files.
    pub ts_root: &'a Path,
    /// Absolute path to the SDK es2abc binary.
    pub es2abc: &'a Path,
    /// Output modules.abc path (its parent must exist).
    pub out_abc: &'a Path,
    /// Threads for the `--file-threads` flag.
    pub file_threads: u32,
    /// compatible API version number for `--target-api-version` (26, not "26.0.0").
    pub target_api_version: u32,
    /// Debug build: the reference passes `--debug-info`, which makes the compiler emit the
    /// named lexical environments and debug records a debuggable package carries.
    pub debug: bool,
    /// Package version; records carry it as a `&<version>` suffix.
    pub version: &'a str,
    /// Addressable path of the module entry, whose record carries no version.
    pub entry_record: Option<&'a str>,
    /// Name of the entry module: the device addresses it without a version.
    pub entry_module: &'a str,
    /// Third-party packages compiled into the same abc; their records are `<pkg>.<path>`
    /// and carry the package's own version.
    pub npm: &'a [crate::build::deps::npm::Package],
    /// Byte-code HARs whose prebuilt `ets/modules.abc` is merged into the result.
    pub abc_inputs: &'a [std::path::PathBuf],
    /// `pkgContextInfo.json` for `--compile-context-info`: without it the compiler cannot
    /// place package-internal modules (system modules such as `@ohos:file.fs` then fail to
    /// resolve at load time, because the module has no package context to resolve against).
    pub compile_context_info: Option<&'a Path>,
    /// Staged `.ts` files whose source declared `"use shared"`. The flag is a column of the
    /// filesInfo table (`field 6`), not of the source text the compiler sees.
    pub shared: &'a std::collections::BTreeSet<PathBuf>,
}

/// One fileInfo line, split out for the tests.
pub fn files_info_line(
    ts_path: &Path,
    record_name: &str,
    entry_module: &str,
    package_name: &str,
    source_rel: &str,
    lang: &str,
    shared: bool,
) -> String {
    files_info_line_v(
        ts_path,
        record_name,
        entry_module,
        package_name,
        "1.0.0",
        source_rel,
        lang,
        shared,
    )
}

/// As [`files_info_line`], for a package whose version is its own rather than the module's.
///
/// The fourth field starts with the *carrying module*, not the package name: the reference
/// writes `entry|@mcui/mccharts|2.8.9|src/main/ets/pageView/Tooltip.ts` (measured by building
/// clashbox with hvigor, 2026-11-06). es2abc groups shared modules by that first segment, so
/// writing the package name there merges every file of one package into a single shared module
/// and their exports collide.
///
/// Field 6 is the shared-module flag: the reference writes `true` for a file whose source
/// declares `"use shared"` and `false` otherwise (measured: `fixtures/use-shared`, where the
/// page line carries `true` while the ability and resource-table lines carry `false`).
#[allow(clippy::too_many_arguments)]
pub fn files_info_line_v(
    ts_path: &Path,
    record_name: &str,
    carrier: &str,
    package_name: &str,
    version: &str,
    source_rel: &str,
    lang: &str,
    shared: bool,
) -> String {
    format!(
        "{};{};esm;{}|{}|{}|{};{};{};{}\n",
        ts_path.display(),
        record_name,
        carrier,
        package_name,
        version,
        source_rel,
        package_name,
        shared,
        lang
    )
}

/// The source-root segment the device prepends when it addresses a module-internal
/// record (`ability_runtime` composes `<module>/<this>/<srcEntry-relative path>` and
/// `module_path_helper` strips exactly this prefix when normalizing).
///
/// It lives here as the single spelling of that segment in this crate; every other
/// place that needs an addressable path goes through [`module_path`].
pub(crate) const MODULE_SOURCE_ROOT: &str = "src/main/ets";

/// The entry name a module carries when its manifest declares no `main`.
///
/// A member module (`import … from '<member>'`) is addressed through its package entry, and
/// that entry is the file the module's own `oh-package.json5` `main` names — the value the
/// build records as `pkgContextInfo.<member>.entryPath`. `getNormalizedOhmUrlByAliasName`
/// (`ark_utils.js:186`) composes the request as `removeSuffix(alias + "/" + entryPath)`. The
/// reference stages that one file at the module root and every other file of the module under
/// the source root, so the two record forms differ:
///
/// ```text
/// proxy_core/Index.ts                  → &proxy_core/Index&1.0.0
/// proxy_core/src/main/ets/fileUtils.ts → &proxy_core/src/main/ets/fileUtils&1.0.0
/// ```
///
/// (both lines are from the reference build's `filesInfo.txt`). This constant is the fallback
/// for a module whose manifest is absent, which is every module of a `hvolt.toml` project.
pub(crate) const DEFAULT_MODULE_ENTRY: &str = "Index";

/// Module name → the entry name its own manifest declares, filled once per build.
///
/// A global rather than a `MergeInput` field: `cli/build.rs` builds a `MergeInput` and is
/// outside this refactor's write scope, and every other project-wide table in this crate
/// (`component_index`, the source page) is filled the same way.
static MODULE_ENTRIES: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
    std::sync::OnceLock::new();

/// Publish the module→entry table for this build.
pub(crate) fn set_module_entries(entries: std::collections::BTreeMap<String, String>) {
    let _ = MODULE_ENTRIES.set(entries);
}

/// The entry name one module declares, or [`DEFAULT_MODULE_ENTRY`] when it declares none.
pub(crate) fn module_entry(module: &str) -> &'static str {
    MODULE_ENTRIES
        .get()
        .and_then(|entries| entries.get(module).map(String::as_str))
        .unwrap_or(DEFAULT_MODULE_ENTRY)
}

/// The addressable name of a member module's package entry (`proxy_core/Index`).
pub(crate) fn module_entry_path(module: &str) -> String {
    format!("{module}/{}", module_entry(module))
}

/// The module this build assembles — the reference's `projectConfig.moduleName`.
///
/// Every route registration the task writes records it, including one for a file that belongs
/// to a sibling module: measured on clashbox, `xb_components`' pages register under `entry`
/// (`{ moduleName: "entry", pagePath: "xb_components/src/main/ets/view/Xb_Studio" }`).
///
/// A global rather than a lowering parameter, for the same reason as [`MODULE_ENTRIES`]: the
/// emitter sits several stages below `ets_assemble::assemble`, the one place that knows which
/// module the task packs.
static ASSEMBLE_MODULE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Publish the module this build assembles.
pub(crate) fn set_assemble_module(name: &str) {
    // An assemble with no modules has nothing to name; publishing `""` would make a route
    // registration record an empty `moduleName` instead of the page's own module.
    if name.is_empty() {
        return;
    }
    let _ = ASSEMBLE_MODULE.set(name.to_string());
}

/// The assembled module, or `None` when the caller lowers a single file instead of assembling
/// a project (the CLI's `compile-ets`), in which case the page's own module stands.
pub(crate) fn assemble_module() -> Option<&'static str> {
    ASSEMBLE_MODULE.get().map(String::as_str)
}

/// The module root above an ETS directory: `<root>/src/main/ets` → `<root>`.
pub(crate) fn module_root(ets_dir: &Path) -> &Path {
    let mut root = ets_dir;
    for segment in [MODULE_SOURCE_ROOT]
        .iter()
        .flat_map(|path| path.split('/').collect::<Vec<_>>().into_iter().rev())
    {
        match root.file_name().and_then(|name| name.to_str()) {
            Some(name) if name == segment => root = root.parent().unwrap_or(root),
            _ => return ets_dir,
        }
    }
    root
}

/// Version a dependency module's records carry.  The entry module is the exception: it
/// carries none, because the device addresses it without one (measured against a reference
/// package, where every `entry.*` record reads `&entry.…&` while `proxy_core.*` reads
/// `&proxy_core.…&1.0.0`).
pub(crate) const MODULE_VERSION: &str = "1.0.0";

/// Version of a module's records and import urls: empty for the entry module, which the
/// device addresses without a version, and the module's own version otherwise.
pub(crate) fn record_version(module: &str, entry_module: &str) -> &'static str {
    if module == entry_module {
        ""
    } else {
        MODULE_VERSION
    }
}

/// Addressable path of a file inside a module: `<module>/<root>/<relative path>`.
pub fn module_path(module: &str, rel: &str) -> String {
    // Build-generated modules (`build/...`) sit beside the source root, not inside it.
    if rel.starts_with("build/") {
        return format!("{module}/{rel}");
    }
    format!("{module}/{MODULE_SOURCE_ROOT}/{rel}")
}

/// Derive the record name from an expanded .ts path like the reference does:
/// `<module>/src/main/ets/<file>` minus the extension becomes
/// `src/src/main/ets/<file>` … in the reference's translated layout the module root
/// itself is `<module>/` so the record keeps `<module>/<root-relative>`.
/// For the flat hvolt layout we accept `<module>/<rel>` and normalize the
/// `src/main/ets/` segment away.
pub fn record_name_from_ts(ts_root: &Path, ts_path: &Path, entry_module: &str) -> String {
    let rel = ts_path
        .strip_prefix(ts_root)
        .unwrap_or(ts_path)
        .to_string_lossy()
        .replace('\\', "/");
    let rel = rel.trim_end_matches(".ts");
    let rel = rel.trim_end_matches(".js");
    // A package entry (`from '<pkg>'`) is addressed as `<pkg>.Index` — no source root.
    let rel = match rel.split_once('/') {
        // A member module's package entry is staged at the module root, so its record has no
        // source root (`&proxy_core/Index&1.0.0`). The entry module has no package entry: its
        // `entryPath` is the source root, so a file of its own keeps the full form.
        Some((module, entry)) if module != entry_module && module_entry(module) == entry => {
            module_entry_path(module)
        }
        Some((module, rest)) => module_path(module, rest),
        None => rel.to_string(),
    };
    let rel = rel.as_str();
    // The generated ResourceTable header lives at
    // `<module>/build/<mode>/generated/r/<mode>/ResourceTable.{js,ts}` and
    // the reference names its record `<module>/build/generated/r/ResourceTable` —
    // the build-mode segment is dropped from both occurrences.
    if let Some(i) = rel.find("/build/") {
        let tail = &rel[i + "/build/".len()..];
        if tail.starts_with("default/generated/r/default/") {
            let head = &rel[..i + "/build/".len()];
            return format!("{}generated/r/ResourceTable", head);
        }
    }
    rel.to_string()
}

/// Write filesInfo.txt for every .ts under `ts_root`.
pub fn write_files_info(
    ts_root: &Path,
    out: &Path,
    entry_module: &str,
    version: &str,
    entry_record: Option<&str>,
    packages: &[crate::build::deps::npm::Package],
    shared: &std::collections::BTreeSet<PathBuf>,
) -> Result<(), String> {
    let mut lines = String::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_sources(ts_root, &mut files)?;
    files.sort();
    for f in files {
        let rel = f.strip_prefix(ts_root).unwrap_or(&f).to_string_lossy();
        // package name = first path segment under the module root, like the reference's
        // `src|src|1.0.0|…` carrier.
        let pkg = rel.split(['/', '\\']).next().unwrap_or("src").to_string();
        // A third-party package carries its own record prefix and its own version; the
        // module-root convention above does not apply to it.
        //
        // A package's staged path is the ohpm layout, which spans several segments
        // (`oh_modules/.ohpm/@mcui+mccharts@2.8.9/oh_modules/@mcui/mccharts/…`), so it is
        // recognized by a prefix rather than by the first segment.
        let staged_package = packages.iter().find_map(|package| {
            rel.strip_prefix(&format!("{}/", package.staging_dir()))
                .map(|rest| (package, rest))
        });
        if let Some((package, rest)) = staged_package {
            let stem = rest.trim_end_matches(".ts").trim_end_matches(".js");
            let path = if stem.is_empty() {
                package.record_prefix_slash()
            } else {
                format!("{}/{}", package.record_prefix_slash(), stem)
            };
            let record = format!("&{path}&{}", package.version);
            lines.push_str(&files_info_line_v(
                &f,
                &record,
                entry_module,
                &package.name,
                &package.version,
                // The fourth field is package-relative, so it keeps the package's source
                // root (`browser/dist/index.js`), which the staged path also carries.
                &package.package_rel(rest),
                if rel.ends_with(".js") { "ts" } else { "ets" },
                false,
            ));
            continue;
        }
        // The record field carries an ohm url, not a bare path: that is what makes es2abc
        // keep the slash form and the `&<version>` suffix the device matches against.
        let path = record_name_from_ts(ts_root, &f, entry_module);
        // The entry record carries no version; module-internal records carry `&1.0.0`, and
        // import urls name them with that same suffix.
        // A record name always ends with `&<version>`; the entry module's version is empty,
        // which still leaves the `&` in place (`entry/.../EntryAbility&`).
        // A normalized-url record name is `&<pkg>.<path>&<version>`: the leading `&` is part
        // of the name, and the device looks the module up with it (`&entry/src/...&`).
        // The entry module's records carry no version; a member's own records carry its
        // version.  `version` is only meaningful for the app; a module-level record uses the
        // module convention, which is what the device matches against import urls.
        let module_version = record_version(&pkg, entry_module);
        let record = if Some(path.as_str()) == entry_record {
            format!("&{path}&")
        } else {
            format!("&{path}&{module_version}")
        };
        let _ = version;
        // Source language marker, sampled from the reference's filesInfo: expanded .ts
        // files are `ets` (they came from .ets), generated .js files are `ts`.
        let lang = if rel.ends_with(".js") { "ts" } else { "ets" };
        // The source carrier path drops the module-root segment (`src/…` →
        // `…`), while the record name keeps it.
        let source_rel = match rel.split_once('/') {
            Some((_, rest)) => rest.to_string(),
            None => rel.to_string(),
        };
        lines.push_str(&files_info_line(
            &f,
            &record,
            entry_module,
            &pkg,
            &source_rel,
            lang,
            shared.contains(&f),
        ));
    }
    std::fs::write(out, lines).map_err(|e| format!("write {}: {e}", out.display()))
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .flatten()
    {
        let p = entry.path();
        if p.is_dir() {
            collect_sources(&p, out)?;
        } else {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string());
            if name.as_deref().map(|n| n.ends_with(".ts")).unwrap_or(false)
                || name
                    .as_deref()
                    .map(|n| n.ends_with(".ets"))
                    .unwrap_or(false)
                || name.as_deref().map(|n| n.ends_with(".js")).unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    Ok(())
}

/// Run the es2abc merge. Returns the output path on success.
/// filesInfo/npmEntries land in the system temp dir, not next to the output —
/// the package layout must not contain them.
pub fn merge(input: &MergeInput<'_>) -> Result<PathBuf, String> {
    let work = std::env::temp_dir().join("hvolt-abc");
    std::fs::create_dir_all(&work).map_err(|e| format!("mkdir {}: {e}", work.display()))?;
    let files_info = work.join("filesInfo.txt");
    write_files_info(
        input.ts_root,
        &files_info,
        input.entry_module,
        input.version,
        input.entry_record,
        input.npm,
        input.shared,
    )?;
    // A byte-code HAR contributes its own abc as a merge input, with the record its package
    // entry is addressed by.
    if !input.abc_inputs.is_empty() {
        let mut extra = String::new();
        for (path, package) in input.abc_inputs.iter().zip(
            input
                .npm
                .iter()
                .filter(|p| p.kind == crate::build::deps::npm::Kind::ByteCode),
        ) {
            extra.push_str(&format!(
                "{};{}&{};esm;{}|{}|{}|;{};false;ets\n",
                path.display(),
                package.alias,
                package.version,
                package.alias,
                package.alias,
                package.version,
                package.alias
            ));
        }
        let mut text = std::fs::read_to_string(&files_info).map_err(|e| e.to_string())?;
        text.push_str(&extra);
        std::fs::write(&files_info, text).map_err(|e| format!("write filesInfo: {e}"))?;
    }
    let npm_entries = work.join("npmEntries.txt");
    std::fs::write(&npm_entries, NPM_ENTRIES).map_err(|e| format!("write npmEntries: {e}"))?;

    let mut cmd = Command::new(input.es2abc);
    cmd.arg("--merge-abc")
        .arg(format!("@{}", files_info.display()))
        .arg("--npm-module-entry-list")
        .arg(&npm_entries)
        .arg("--output")
        .arg(input.out_abc)
        .arg("--file-threads")
        .arg(input.file_threads.to_string())
        // The remaining flags mirror the reference's fast-build module mode
        // (`module_mode.js generateEs2AbcCmd`). Measured against asr-ondevice
        // on 2026-11-03: `--target-api-version 26 --enable-release-column
        // --enable-callable-name` reproduces the reference's modules.abc with a
        // byte-identical size and a semantically identical program (dump
        // differs only in internal literal ids); adding
        // `--opt-try-catch-func=false` on top breaks that match.
        .arg("--target-api-version")
        .arg(input.target_api_version.to_string())
        .arg("--enable-release-column")
        .arg("--enable-callable-name");
    if input.debug {
        cmd.arg("--debug-info");
    }
    if !input.abc_inputs.is_empty() {
        // Without this the compiler refuses a prebuilt abc as an input.
        cmd.arg("--enable-abc-input");
    }
    if let Some(ctx) = input.compile_context_info {
        cmd.arg("--compile-context-info").arg(ctx);
    }
    let output = cmd.output().map_err(|e| format!("run es2abc: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "es2abc merge failed: {}{}",
            stdout.trim_end(),
            stderr.trim_end()
        ));
    }
    Ok(input.out_abc.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record name is `<module>/<source root>/<relative path>` without the extension,
    /// so a file staged as `<module>/entryability/EntryAbility.ts` is addressable as the
    /// device composes it.
    #[test]
    fn record_name_keeps_the_module_source_root() {
        let root = Path::new("/m");
        let f = Path::new("/m/src/entryability/EntryAbility.ts");
        assert_eq!(
            record_name_from_ts(root, f, "entry"),
            module_path("src", "entryability/EntryAbility")
        );
        assert!(record_name_from_ts(root, f, "entry").ends_with("/entryability/EntryAbility"));
    }

    /// A member module's package entry is staged at the module root, so its record has no
    /// source root — the form the reference's own `filesInfo.txt` carries
    /// (`&proxy_core/Index&1.0.0` next to `&proxy_core/src/main/ets/fileUtils&1.0.0`).
    #[test]
    fn member_module_entry_record_has_no_source_root() {
        let root = Path::new("/m");
        let entry = Path::new("/m/proxy_core/Index.ts");
        assert_eq!(
            record_name_from_ts(root, entry, "entry"),
            "proxy_core/Index"
        );
        let member_file = Path::new("/m/proxy_core/Utils.ts");
        assert_eq!(
            record_name_from_ts(root, member_file, "entry"),
            module_path("proxy_core", "Utils")
        );
        // The entry module has no package entry: a file of its own keeps the source root even
        // when it is named `Index`.
        let own = Path::new("/m/entry/Index.ts");
        assert_eq!(
            record_name_from_ts(root, own, "entry"),
            module_path("entry", "Index")
        );
    }

    /// The generated ResourceTable record drops the build-mode segment
    /// (`build/default/generated/r/default/…` → `build/generated/r/…`).
    #[test]
    fn resource_table_record_drops_build_mode() {
        let root = Path::new("/m/src");
        let f = Path::new("/m/src/src/build/default/generated/r/default/ResourceTable.js");
        assert_eq!(
            record_name_from_ts(root, f, "entry"),
            module_path("src", "build/generated/r/ResourceTable")
        );
    }

    /// filesInfo line shape:
    /// `<ts>;&<record>&;esm;<carrier>|<pkg>|1.0.0|<rel>;<pkg>;<shared>;<lang>`.
    #[test]
    fn files_info_line_shape() {
        let line = files_info_line(
            Path::new("/m/a.ts"),
            "src/a",
            "src",
            "src",
            "src/main/ets/a.ts",
            "ts",
            false,
        );
        assert_eq!(
            line,
            "/m/a.ts;src/a;esm;src|src|1.0.0|src/main/ets/a.ts;src;false;ts\n"
        );
    }

    /// The shared-module flag is field 6 of the same line; the reference writes `true` for a
    /// source that declares `"use shared"` (measured: `fixtures/use-shared`, 2026-11-06).
    #[test]
    fn files_info_line_marks_a_shared_module() {
        let line = files_info_line(
            Path::new("/m/a.ts"),
            "src/a",
            "src",
            "src",
            "src/main/ets/a.ts",
            "ets",
            true,
        );
        assert_eq!(
            line,
            "/m/a.ts;src/a;esm;src|src|1.0.0|src/main/ets/a.ts;src;true;ets\n"
        );
    }
}
