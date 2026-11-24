// process stage: `gen_abc_plugin.js` — the es2abc merge input.
//
// The reference records one `ModuleInfo` per source file and feeds `filesInfo.txt` /
// `npmEntries.txt` to the es2abc merge. hvolt builds the same table here and
// `crate::build::compile::abc_compile` renders it with [`render_files_info`]; the row format is
// the reference's (`path;record;esm;carrier|package|version|source;package;shared;lang`), with
// the fields the merge actually needs named instead of positional.
#![allow(dead_code)] // `AbcEnv` and `EntryInfo` carry the reference's shapes; the merge reads them

/// One row of `filesInfo.txt`.
pub(crate) struct ModuleInfo {
    /// Column 1: the expanded `.ts` path.
    pub(crate) ts_path: std::path::PathBuf,
    /// Column 2: the record name inside `modules.abc`.
    pub(crate) record_name: String,
    /// Column 3: the module carrier (`esm`).
    pub(crate) carrier: String,
    /// Column 4, before the first `|`: the package the file belongs to.
    pub(crate) package_name: String,
    /// Column 4, second field: the package version.
    pub(crate) version: String,
    /// Column 4, third field: the package-relative source path.
    pub(crate) source_rel: String,
    /// Column 5: the language marker (`ets` for an expanded `.ets`, `ts` for a `.js`).
    pub(crate) lang: String,
    /// Column 6: whether the record belongs to a shared package.
    pub(crate) shared: bool,
}

/// One `--npm-module-entry-list` entry: a virtual module name and the native stub it maps to.
pub(crate) struct EntryInfo {
    pub(crate) module_name: String,
    pub(crate) entry_name: String,
}

/// `initAbcEnv(...)`: the input root, the output `modules.abc`, and the es2abc binary.
pub(crate) struct AbcEnv<'a> {
    pub(crate) ts_root: &'a std::path::Path,
    pub(crate) output: &'a std::path::Path,
    pub(crate) es2abc: &'a std::path::Path,
}

/// `initAbcEnv`: the environment one merge runs in.
pub(crate) fn init_abc_env<'a>(
    ts_root: &'a std::path::Path,
    output: &'a std::path::Path,
    es2abc: &'a std::path::Path,
) -> AbcEnv<'a> {
    AbcEnv {
        ts_root,
        output,
        es2abc,
    }
}

/// The `filesInfo.txt` text for one merge: one row per record, in the order given.
///
/// The row format is the reference's — `path;record;esm;a|b|c;package;shared;lang` — which is
/// what es2abc parses; `fixtures/*/golden/modules.abc` are the merged outputs this table
/// produces.
pub(crate) fn render_files_info(records: &[ModuleInfo]) -> String {
    let mut text = String::new();
    for record in records {
        text.push_str(&format!(
            "{};{};esm;{}|{}|{}|{};{};{};{}\n",
            record.ts_path.display(),
            record.record_name,
            record.carrier,
            record.package_name,
            record.version,
            record.source_rel,
            record.package_name,
            record.shared,
            record.lang
        ));
    }
    text
}
