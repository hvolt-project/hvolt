// process stage: `declaration_merger.js` — the merged `.d.ets` output.
//
// The reference's `DeclarationMerger` accumulates the type declarations of every source file
// and writes one declaration file, deduplicating the declarations a shared or re-exported
// module brings in twice. hvolt emits `.ts` and `modules.abc` and no declaration file, so the
// merger is kept with its behaviour spelled out and covered by tests rather than wired into the
// package.
#![allow(dead_code)] // not wired: hvolt writes no .d.ets

use std::collections::BTreeSet;

/// `DeclarationMerger`: one merger per emitted declaration file.
#[derive(Default)]
pub(crate) struct DeclarationMerger {
    /// The declarations already merged, so a repeated one is dropped instead of emitted twice.
    pub(crate) merged: BTreeSet<String>,
    /// The declarations in the order they were first added, which is the order they are emitted.
    order: Vec<String>,
}

impl DeclarationMerger {
    /// `addDeclaration(filePath, content, isEntry)`: merge one file's declarations.
    ///
    /// A declaration a second file brings in with the same text is dropped (that is the
    /// shared-module case); one that differs keeps both, because the two files disagree and the
    /// merger is not the place to pick a winner.
    pub(crate) fn add_declaration(&mut self, _file_path: &std::path::Path, content: &str) {
        for declaration in top_level_declarations(content) {
            if self.merged.insert(declaration.clone()) {
                self.order.push(declaration);
            }
        }
    }

    /// `emitDeclarations()`: the merged file, or nothing when it would be empty.
    pub(crate) fn emit_declarations(&self) -> Option<String> {
        if self.order.is_empty() {
            return None;
        }
        let mut text = String::new();
        for declaration in &self.order {
            text.push_str(declaration);
            text.push('\n');
        }
        Some(text)
    }
}

/// The `export declare`/`declare` declarations of one file, each with its own text.
///
/// A declaration runs from its keyword line to the closing brace at column 0 (or the end of its
/// one-line form). Nested declarations belong to their parent and are not split out.
fn top_level_declarations(content: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let starts_declaration = depth == 0
            && (trimmed.starts_with("declare ")
                || trimmed.starts_with("export declare ")
                || trimmed.starts_with("export interface ")
                || trimmed.starts_with("export type "));
        if starts_declaration && !current.is_empty() {
            found.push(current.trim_end().to_string());
            current.clear();
        }
        if starts_declaration || !current.is_empty() {
            current.push_str(line);
            current.push('\n');
            depth += line.matches('{').count();
            depth = depth.saturating_sub(line.matches('}').count());
            if depth == 0 && (line.trim_end().ends_with(';') || line.trim_end().ends_with('}')) {
                found.push(current.trim_end().to_string());
                current.clear();
            }
        }
    }
    if !current.trim().is_empty() {
        found.push(current.trim_end().to_string());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// One file's declaration is emitted as written.
    #[test]
    fn one_declaration_is_kept() {
        let mut merger = DeclarationMerger::default();
        merger.add_declaration(
            Path::new("a.ets"),
            "export declare interface Shape {\n  w: number;\n}\n",
        );
        assert_eq!(
            merger.emit_declarations().as_deref(),
            Some("export declare interface Shape {\n  w: number;\n}\n")
        );
    }

    /// The same declaration from two files is emitted once (the shared-module case).
    #[test]
    fn repeated_declaration_is_merged() {
        let mut merger = DeclarationMerger::default();
        let text = "export declare interface Shape {\n  w: number;\n}\n";
        merger.add_declaration(Path::new("a.ets"), text);
        merger.add_declaration(Path::new("b.ets"), text);
        assert_eq!(
            merger
                .emit_declarations()
                .unwrap()
                .matches("interface Shape")
                .count(),
            1
        );
    }

    /// Two different declarations both survive, in the order they were added.
    #[test]
    fn declarations_keep_their_order() {
        let mut merger = DeclarationMerger::default();
        merger.add_declaration(
            Path::new("a.ets"),
            "export declare interface A {\n  a: number;\n}\n",
        );
        merger.add_declaration(
            Path::new("b.ets"),
            "export declare interface B {\n  b: number;\n}\n",
        );
        let text = merger.emit_declarations().unwrap();
        assert!(text.find("interface A").unwrap() < text.find("interface B").unwrap());
    }

    /// A file with no declarations contributes nothing, and an empty merger emits nothing.
    #[test]
    fn nothing_to_merge() {
        let mut merger = DeclarationMerger::default();
        merger.add_declaration(
            Path::new("a.ets"),
            "@Entry\n@Component\nstruct A {\n  build() {}\n}\n",
        );
        assert_eq!(merger.emit_declarations(), None);
    }
}
