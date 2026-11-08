//! Making ArkTS source legible to a TypeScript parser.
//!
//! The `struct` → `class` rewrite happens before the parser runs: it is a text edit whose
//! result is what the rest of the lowering parses and slices. A call's type arguments are not
//! a text edit: the reference's `.ts` is es2abc's own input and keeps
//! `AppStorage.get<boolean>('crash')`, so no pass here touches them.

use crate::build::lex::{tokenize, Kind};

/// Rewrite the ArkTS-only declaration keyword `struct` into `class`.
///
/// The replacement keeps the source length: a node span measured on the result is the same
/// offset as on the input, so every later slice of the prepared text stays aligned with the
/// source it came from. `class ` is one character longer than `struct` is not a problem — the
/// padding is the space that separates the keyword from the name either way.
pub(crate) fn preprocess(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut prev = 0usize;
    for token in tokenize(source) {
        // A `struct` inside a comment or a string is data, and the scanner reports those as
        // their own token kinds, so only a real identifier is rewritten.
        if token.kind == Kind::Ident && token.text == "struct" {
            out.push_str(&source[prev..token.start]);
            out.push_str("class");
            prev = token.end;
        }
    }
    out.push_str(&source[prev..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `struct` becomes `class` and nothing else moves.
    #[test]
    fn a_struct_declaration_becomes_a_class() {
        let src = "@Component\nstruct Card {\n  build() {}\n}\n";
        let out = preprocess(src);
        assert!(out.contains("class Card"), "{out}");
        assert_eq!(
            out.len(),
            src.len() - 1,
            "`class` is one byte shorter than `struct`"
        );
    }

    /// A `struct` written inside a string or a comment is not a declaration.
    #[test]
    fn struct_in_a_literal_or_comment_is_left_alone() {
        let src = "const a = 'struct X';\n// struct Y\n";
        assert_eq!(preprocess(src), src);
    }
}
