// process stage: `declaration_merger.js` — the merged `.d.ets` output.
//
// The reference's `DeclarationMerger` accumulates the type declarations of every source file
// and writes one declaration file, deduplicating the declarations a shared or re-exported
// module brings in twice. hvolt emits `.ts` and `modules.abc` and no declaration file, so the
// class is present with the reference's shape and no body.
#![allow(dead_code)] // not wired: hvolt writes no .d.ets

use std::collections::BTreeSet;

/// `DeclarationMerger`: one merger per emitted declaration file.
#[derive(Default)]
pub(crate) struct DeclarationMerger {
    /// The declarations already merged, so a repeated one is dropped instead of emitted twice.
    pub(crate) merged: BTreeSet<String>,
}

impl DeclarationMerger {
    /// `addDeclaration(filePath, content, isEntry)`: merge one file's declarations.
    #[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
    pub(crate) fn add_declaration(&mut self, _file_path: &std::path::Path, _content: &str) {
        todo!()
    }

    /// `emitDeclarations()`: the merged file, or nothing when it would be empty.
    #[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
    pub(crate) fn emit_declarations(&self) -> Option<String> {
        todo!()
    }
}
