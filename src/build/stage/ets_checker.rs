// process stage: `ets_checker.js` — the ArkTS linter pass over the whole program.
//
// The reference builds a TypeScript program, runs `doArkTSLinter` over it and reports the
// `arkts-*` rules; `runArkTSLinter(es2abcPath, buildInfoWriteFile)` at `ets_checker.js:973`
// is the entry. hvolt compiles with oxc and es2abc and has no linter pass, so the stage is
// present with the reference's shape and no body.
#![allow(dead_code)] // not wired: hvolt has no ArkTS linter pass

/// One `arkts-*` diagnostic, in the shape the reference prints.
pub(crate) struct Diagnostic {
    /// The rule that fired (`arkts-no-any-unknown`).
    pub(crate) rule: String,
    pub(crate) message: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

/// `runArkTSLinter`: the linter's mode gate (`getArkTSLinterMode`) decides whether the pass
/// runs at all, so it takes the project's mode alongside the program.
#[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
pub(crate) fn run_arkts_linter(
    _program: &oxc_ast::ast::Program<'_>,
    _standard_mode: bool,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    todo!()
}
