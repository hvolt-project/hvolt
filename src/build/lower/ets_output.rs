//! The one printer.
//!
//! Both lowering paths — the plain one (a file without a component) and the component one
//! (`struct` → `ViewPU`/`ViewV2`) — produce a *program*, not text. This module is the only place
//! that turns a program into bytes: it configures oxc's codegen once, so the two paths cannot
//! drift in quoting, indentation or statement separation, and neither of them re-indents by
//! hand.
//!
//! A lowering assembles statements in the order the reference emits them; the layout of those
//! statements (braces, indentation, blank lines) is decided here and nowhere else.

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::Parser;
use oxc_span::SourceType;

/// Print a parsed program through the project's single codegen configuration.
///
/// Every statement a lowering produces goes through this function, so "how the output looks"
/// has exactly one definition. `source_text` is the text the program was parsed from: comments
/// are carried on the program by position, and the printer reads them from it.
pub(crate) fn print_program(program: &Program<'_>, source_text: &str) -> String {
    let printed = oxc_codegen::Codegen::new()
        .with_options(oxc_codegen::CodegenOptions {
            single_quote: true,
            ..Default::default()
        })
        .with_source_text(source_text)
        .build(program)
        .code;
    // ArkTS wants a decorated export written decorator-first (`@Entry export class`), while the
    // AST has `export` wrapping the decorated declaration and the printer emits the wrapper
    // first. Every decorated export in the project is written decorator-first, so the order is
    // restored by token position rather than by matching text.
    decorators_before_export(&printed)
}

/// Print a list of statements in the order a lowering emits them.
///
/// Each statement is printed by [`print_program`], so the caller never lays text out itself.
/// A statement that carries a comment keeps its own text: a comment is attached text rather
/// than syntax, and the printer re-emits none of the comments written inside a clause (the
/// reference keeps them, and a fixture compares them), so the choice is between the comment and
/// the printer's layout — the comment wins.
pub(crate) fn print_statements(items: &[String]) -> String {
    let mut out = String::new();
    for item in items {
        let printed = if keeps_its_text(item) {
            item.clone()
        } else {
            print_module(item).unwrap_or_else(|| item.clone())
        };
        let trimmed = printed.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

/// Whether the printer would lose something this statement carries.
///
/// A comment is attached text the printer re-emits nothing of, and a constructor's type
/// arguments are not printed at all: `oxc_codegen`'s `NewExpression` generator writes
/// `new` + callee + arguments (`oxc_codegen-0.75.1/src/gen.rs:2152`) and never the
/// `type_arguments`, so `new Map<string, string>([])` comes back as `new Map([])`. The
/// reference keeps them (`entry/components/Home/FavoriteConfiguration.ts` re-emits
/// `this.createStorageLink('favoriteProfiles', new Map<string, string>([]), …)`), so a
/// statement carrying either keeps the text it was given.
fn keeps_its_text(text: &str) -> bool {
    carries_comments(text)
        || !constructor_type_arguments(text).is_empty()
        || has_collapsed_spelling(text)
}

/// Whether the statement writes something `oxc_codegen` collapses to the shorter spelling.
///
/// Two of them, both kept by the reference:
///   - an object property written `key: key`. `oxc_codegen`'s object-property generator re-derives
///     `shorthand` from the AST and never reads the node's own flag
///     (`oxc_codegen-0.75.1/src/gen.rs:1581`), and no `CodegenOptions` field turns that off. A
///     statement that writes one keeps the text it was given (measured 2026-11-07:
///     `fixtures/plain-ts-shorthand`, `fixtures/common-vpn-service`).
///   - an import or export binding written `Foo as Foo`. The generators compare the two names and
///     print the alias only when they differ (`oxc_codegen-0.75.1/src/gen.rs:883` for the import
///     specifier, `:1039` for the export specifier), so `import { ProxyItem as ProxyItem }` comes
///     back as `import { ProxyItem }`. The reference rebuilds its clauses from the source's own
///     specifier nodes (`transformTypeExportImportAndConstEnumInTypeScript`), so the alias
///     survives an `import type` split (`entry/components/Proxy/ProxyNodeItem.ts`).
///
/// Both checks read the tree in one parse: the span is the evidence for the alias, because the
/// parser clones one node when the source wrote none and keeps two when it did.
fn has_collapsed_spelling(text: &str) -> bool {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked {
        return false;
    }
    use oxc_ast_visit::Visit as _;
    let mut visitor = CollapsedSpelling { found: false };
    visitor.visit_program(&ret.program);
    visitor.found
}

/// Finds a spelling [`has_collapsed_spelling`] describes.
struct CollapsedSpelling {
    found: bool,
}

impl<'a> oxc_ast_visit::Visit<'a> for CollapsedSpelling {
    fn visit_object_property(&mut self, property: &oxc_ast::ast::ObjectProperty<'a>) {
        if let oxc_ast::ast::PropertyKey::StaticIdentifier(key) = &property.key {
            if let oxc_ast::ast::Expression::Identifier(ident) = &property.value {
                if key.name == ident.name {
                    self.found = true;
                }
            }
        }
        oxc_ast_visit::walk::walk_object_property(self, property);
    }

    fn visit_import_specifier(&mut self, specifier: &oxc_ast::ast::ImportSpecifier<'a>) {
        use oxc_span::GetSpan;
        if specifier.imported.span() != specifier.local.span {
            self.found = true;
        }
    }

    fn visit_export_specifier(&mut self, specifier: &oxc_ast::ast::ExportSpecifier<'a>) {
        use oxc_span::GetSpan;
        if specifier.local.span() != specifier.exported.span() {
            self.found = true;
        }
    }
}

/// Whether a statement already contains a comment.
fn carries_comments(text: &str) -> bool {
    crate::build::lex::tokenize(text)
        .iter()
        .any(|t| t.kind == crate::build::lex::Kind::Comment)
}

/// Parse `text` as a TypeScript module and print it through [`print_program`].
///
/// Returns `None` when the text is not something the parser accepts: a caller with a fallback
/// keeps it, instead of emitting a file the byte-code compiler will reject.
pub(crate) fn print_module(text: &str) -> Option<String> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    Some(print_program(&ret.program, text))
}

/// The text of a declaration together with the comments written directly above it.
///
/// Comments are tokens, so the text a declaration owns is found by walking back over comment
/// tokens whose end is separated from the declaration by whitespace only. No line arithmetic is
/// involved, and a comment sharing a line with the declaration is included.
pub(crate) fn with_leading_comments(text: &str, start: usize, end: usize) -> String {
    let end = end.min(text.len());
    let mut from = start.min(text.len());
    let tokens = crate::build::lex::tokenize(text);
    while let Some(previous) = tokens.iter().rev().find(|t| t.end <= from) {
        if previous.kind != crate::build::lex::Kind::Comment {
            break;
        }
        if !text[previous.end..from].chars().all(char::is_whitespace) {
            break;
        }
        from = previous.start;
    }
    text[from..end].to_string()
}

/// The spans of `new Foo<…>` type arguments, so a caller can leave them alone.
///
/// A construction may name a dotted type (`new a.b.Foo<…>`) and the arguments may nest
/// (`new Map<string, Array<number>>`), so the closing bracket is found by depth. A `<` that
/// does not close before the argument list starts something else — a comparison, say — and is
/// not shielded.
pub(crate) fn constructor_type_arguments(source: &str) -> Vec<(usize, usize)> {
    use crate::build::lex::{tokenize, Kind};
    let tokens: Vec<crate::build::lex::Token<'_>> = tokenize(source)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    let mut spans = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        // `Repeat<Item>(source)`: the type argument is part of what the reference emits — the
        // call is printed as written, and the runtime reads the item type from it
        // (process_component_build.js `recurseRepeatExpression` keeps `typeArguments`).
        if tokens[index].kind == Kind::Ident && tokens[index].text == "Repeat" {
            if let Some(next) = tokens.get(index + 1) {
                if next.is_punct('<') {
                    if let Some(close) = closing_angle(&tokens, index + 1) {
                        spans.push((next.start, close + 1));
                    }
                }
            }
            index += 1;
            continue;
        }
        if !(tokens[index].kind == Kind::Ident && tokens[index].text == "new") {
            index += 1;
            continue;
        }
        let mut cursor = index + 1;
        let mut named = false;
        while cursor < tokens.len() {
            if tokens[cursor].kind == Kind::Ident {
                named = true;
                cursor += 1;
            } else if named && tokens[cursor].is_punct('.') {
                cursor += 1;
            } else {
                break;
            }
        }
        if named && cursor < tokens.len() && tokens[cursor].is_punct('<') {
            if let Some(close) = closing_angle(&tokens, cursor) {
                let followed_by_arguments = tokens
                    .iter()
                    .find(|token| token.start > close)
                    .is_some_and(|token| token.is_punct('('));
                if followed_by_arguments {
                    spans.push((tokens[cursor].start, close + 1));
                }
            }
        }
        index = cursor.max(index + 1);
    }
    spans
}

/// The start offset of the `>` that closes the `<` at `open`.
fn closing_angle(tokens: &[crate::build::lex::Token<'_>], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for index in open..tokens.len() {
        let token = &tokens[index];
        if token.is_punct('<') {
            depth += 1;
            continue;
        }
        if token.is_punct('>') {
            if index > 0 && tokens[index - 1].is_punct('=') {
                continue;
            }
            depth -= 1;
            if depth == 0 {
                return Some(token.start);
            }
            continue;
        }
        if matches!(token.text, ";" | ")" | "}" | "]" | "{" | "(" | "[") {
            return None;
        }
    }
    None
}

/// Collapse layout and quoting so a test can assert on what the emitted program says rather
/// than on how the printer laid it out. The printer owns whitespace and quote style; the
/// reference-golden comparison is AST-level for the same reason.
#[cfg(test)]
pub(crate) fn flattened(text: &str) -> String {
    text.replace('"', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Move `@Decorator` in front of the `export` that wraps it.
///
/// The AST has the decorator on the declaration and `export` wrapping it; the printer emits
/// the wrapper first, which ArkTS rejects (`'interface' keyword expected`). The order is fixed
/// here by token positions, so a decorator argument containing `)` or a string cannot move the
/// wrong range.
fn decorators_before_export(text: &str) -> String {
    use crate::build::lex::{matching_delimiter, tokenize, Kind};

    let tokens = tokenize(text);
    let mut moves: Vec<(usize, usize, &str)> = Vec::new();
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        let is_export = tokens[index].kind == Kind::Ident && tokens[index].text == "export";
        if !is_export {
            index += 1;
            continue;
        }
        // `export @A @B class X` — collect the decorators up to the declaration keyword.
        let mut cursor = index + 1;
        let mut decorators: Vec<(usize, usize)> = Vec::new();
        while cursor + 1 < tokens.len() {
            if !tokens[cursor].is_punct('@') || tokens[cursor + 1].kind != Kind::Ident {
                break;
            }
            decorators.push((tokens[cursor].start, tokens[cursor + 1].end));
            cursor += 2;
            // An optional argument list belongs to the decorator.
            if cursor < tokens.len() && tokens[cursor].is_punct('(') {
                if let Some(close) = matching_delimiter(text, tokens[cursor].start) {
                    if let Some(last) = decorators.last_mut() {
                        last.1 = close + 1;
                    }
                    cursor = tokens
                        .iter()
                        .position(|t| t.start > close)
                        .unwrap_or(tokens.len());
                }
            }
        }
        let Some(declaration) = tokens.get(cursor) else {
            index += 1;
            continue;
        };
        let is_declaration = declaration.kind == Kind::Ident
            && matches!(declaration.text, "class" | "function" | "const" | "struct");
        if decorators.is_empty() || !is_declaration {
            index = cursor.max(index + 1);
            continue;
        }
        let (first_start, _) = decorators[0];
        let (_, last_end) = *decorators.last().unwrap();
        moves.push((
            tokens[index].start,
            last_end,
            text[first_start..last_end].trim(),
        ));
        index = cursor;
    }
    if moves.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut prev = 0usize;
    for (export_start, decorator_end, decorators) in &moves {
        out.push_str(&text[prev..*export_start]);
        out.push_str(decorators);
        out.push('\n');
        out.push_str("export ");
        prev = *decorator_end;
        // The decorator range and the keyword may be separated by a space the printer wrote;
        // it belongs to neither token, so it is not emitted twice.
        while text[prev..].starts_with(' ') {
            prev += 1;
        }
    }
    out.push_str(&text[prev..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The printer is the only place quoting is decided: a double-quoted source comes back
    /// single-quoted, which is what the reference emits.
    #[test]
    fn the_printer_owns_the_quoting() {
        let out = print_module("const a = \"x\";\n").expect("parse");
        assert_eq!(out, "const a = 'x';\n");
    }

    /// A decorated export is printed decorator-first, the only order ArkTS accepts.
    #[test]
    fn a_decorated_export_is_printed_decorator_first() {
        let out = print_module("@ObservedV2\nexport class C {\n  @Trace a: number = 0;\n}\n")
            .expect("parse");
        assert!(
            out.starts_with("@ObservedV2\nexport class C"),
            "expected decorator-first order, got:\n{out}"
        );
    }

    /// Text the parser rejects is refused rather than printed, so a caller can keep its
    /// fallback instead of emitting an unparsable file.
    #[test]
    fn unparsable_text_is_refused() {
        assert!(print_module("@Styles function card() { .width(10) }").is_none());
    }

    /// A template literal keeps its `${}` contents: the printer does not touch string bodies.
    #[test]
    fn template_contents_are_untouched() {
        let out = print_module("const t = `${a + b}`;\n").expect("parse");
        assert!(out.contains("${a + b}"), "{out}");
    }

    /// A binding written `Foo as Foo` keeps the statement's own text: `oxc_codegen` prints the
    /// alias only when the names differ, and the reference keeps the source's `propertyName`
    /// (`import type { ProxyItem as ProxyItem }` in `ProxyNodeItem.ts`).
    #[test]
    fn a_bare_alias_keeps_the_statement_text() {
        assert!(keeps_its_text(
            "import { ProxyItem as ProxyItem, ProxyData } from './ProxyData';"
        ));
        assert!(!keeps_its_text(
            "import { ProxyItem, ProxyData } from './ProxyData';"
        ));
        assert!(keeps_its_text("export { A as A } from './a';"));
    }
}
