// process stage: `gen_abc_plugin.js` — the es2abc merge input.
//
// The reference records one `ModuleInfo` per source file (its ohm-url, whether it is an ESM
// record, and its package) and feeds `filesInfo.txt` / `npmEntries.txt` to the es2abc merge;
// `initAbcEnv(...)` at `gen_abc_plugin.js` builds the environment, `GenAbcPlugin` walks it.
//
// hvolt performs the same merge from `crate::build::compile::abc_compile`, which writes both
// files and invokes es2abc. This module carries the record shapes so the two can be joined
// without moving that file; nothing here is called yet.
#![allow(dead_code)] // the merge lives in build/compile/abc_compile.rs today

/// One record of the merge: the file, the record name it is emitted under, its kind and the
/// package it belongs to.
pub(crate) struct ModuleInfo {
    /// `filesInfo.txt` column 1: the expanded `.ts` path.
    pub(crate) ts_path: std::path::PathBuf,
    /// `filesInfo.txt` column 2: the record name inside `modules.abc`.
    pub(crate) record_name: String,
    /// `is_esm_record`: the module kind column (`esm` for an ESM record).
    pub(crate) is_esm_record: bool,
    /// The owning package, or nothing for a file inside the project.
    pub(crate) package: Option<String>,
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

/// `initAbcEnv`: fill the merge environment for one invocation.
#[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
pub(crate) fn init_abc_env(_records: &[ModuleInfo], _entries: &[EntryInfo]) -> String {
    todo!()
}
