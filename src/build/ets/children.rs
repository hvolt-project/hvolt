//! Turning a component's child block into the callback the runtime expects.
//!
//! ArkTS writes a component with its children in a block:
//!
//! ```text
//! Column() {
//!   Text('hi')
//! }
//! ```
//!
//! The compiled form passes the children as a trailing arrow function:
//!
//! ```text
//! Column(() => {
//!   Text('hi')
//! })
//! ```
//!
//! This rewrite produces the second shape so the source parses as TypeScript, where the first
//! is a syntax error. A name takes a child block when it is a built-in component, a `@Builder`
//! declared anywhere in the project, or a method of the struct being read.

use std::collections::HashSet;

use crate::build::lex::{matching_delimiter, tokenize, Kind};

/// Names this file binds by `import`.
///
/// A `@Builder` declared in another file is called with a child block exactly like a built-in
/// component, and `mask_builder_decorators` only sees declarations in this file — so
/// `toggleRowList({ …props }) { … }` keeps its child block, and the parser then rejects the file
/// with `Expected a semicolon` (`entry/components/Settings/Backgrounder.ets` imports it from
/// `../Common/Common`).
///
/// Every imported name is offered as a candidate, without resolving the import graph: the rewrite
/// only fires on a call **followed by a child block**, and `f() { … }` is not a legal TypeScript
/// expression, so a name that turns out not to be a builder cannot be rewritten by accident.
///
/// The local binding of `A as B` is both names' worth of coverage: whatever a call site writes
/// (`A(...)` or `B(...)`) is a name this returns.
pub(crate) fn imported_names(source: &str) -> Vec<String> {
    let tokens: Vec<_> = tokenize(source)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    let mut names: Vec<String> = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].kind != Kind::Ident || tokens[index].text != "import" {
            index += 1;
            continue;
        }
        index += 1;
        // `import type { … }` — the keyword is not a binding.
        if index < tokens.len() && tokens[index].kind == Kind::Ident && tokens[index].text == "type"
        {
            index += 1;
        }
        if index >= tokens.len() {
            break;
        }
        if tokens[index].kind == Kind::Ident {
            // `import X from '…'`
            names.push(tokens[index].text.to_string());
            index += 1;
        } else if tokens[index].is_punct('*') {
            // `import * as X from '…'`
            if index + 2 < tokens.len()
                && tokens[index + 1].kind == Kind::Ident
                && tokens[index + 1].text == "as"
            {
                names.push(tokens[index + 2].text.to_string());
            }
            index += 3;
        } else if tokens[index].is_punct('{') {
            index += 1;
            while index < tokens.len() && !tokens[index].is_punct('}') {
                if tokens[index].kind == Kind::Ident && tokens[index].text != "as" {
                    let name = tokens[index].text;
                    if !names.iter().any(|seen| seen == name) {
                        names.push(name.to_string());
                    }
                }
                index += 1;
            }
            index += 1;
        }
    }
    names
}

/// Names of the `struct` declarations this file writes.
///
/// A component declared in the same file can be called without `this.` — `Notice.ets` declares
/// `struct noticeInputRow` at its own top level and calls it bare, `noticeInputRow() { … }` — and
/// the mask only records `@Builder function` names, so without these the call keeps its child
/// block and the parser rejects the file.
pub(crate) fn declared_struct_names(source: &str) -> Vec<String> {
    let tokens: Vec<_> = tokenize(source)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    let mut names: Vec<String> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident || token.text != "struct" {
            continue;
        }
        if let Some(next) = tokens.get(index + 1) {
            if next.kind == Kind::Ident && !names.iter().any(|seen| seen == next.text) {
                names.push(next.text.to_string());
            }
        }
    }
    names
}

/// Rewrite every component call's child block into a trailing callback.
pub(crate) fn rewrite_builders(source: &str, builder_names: &[String]) -> String {
    rewrite_builders_with_locals(source, builder_names, &[])
}

/// As [`rewrite_builders`], with the struct's own method names known.
///
/// A struct-level `@Builder` method is called like a component — `this.badge('x')` followed by
/// a child block is legal ArkTS — and its name is only in scope inside the struct, so the caller
/// passes it in separately from the file-level builder table.
pub(crate) fn rewrite_builders_with_locals(
    source: &str,
    builder_names: &[String],
    locals: &[String],
) -> String {
    rewrite_builders_with_shifts(source, builder_names, locals).0
}

/// As [`rewrite_builders_with_locals`], reporting where the rewrite pushed the text right.
///
/// Each entry is `(offset in the rewritten text, bytes added)`. The rewrite inserts the callback
/// the parser needs (`Foo() { … }` becomes `Foo(() => { … })`), and a position measured on its
/// output is that much too far right wherever the insertion precedes it on the same line — the
/// source's own column is what a `{ page, line, col }` object carries.
pub(crate) fn rewrite_builders_with_shifts(
    source: &str,
    builder_names: &[String],
    locals: &[String],
) -> (String, Vec<(usize, usize)>) {
    let mut takes_children: HashSet<&str> = HashSet::new();
    for name in crate::build::ets_compile::builtin_component_names() {
        takes_children.insert(name.as_str());
    }
    for name in builder_names.iter().chain(locals.iter()) {
        takes_children.insert(name.as_str());
    }
    // A keyword followed by `(…) { … }` is a statement, never a component construction.
    for keyword in [
        "if", "for", "while", "switch", "catch", "with", "function", "return", "else",
    ] {
        takes_children.remove(keyword);
    }

    let tokens = tokenize(source);
    // A candidate name written at class-body level is a method *declaration*
    // (`build() { … }`), not a construction; the same shape inside a body is a call.
    let class_bodies = class_body_ranges(source, &tokens);
    let depth_at = brace_depths(&tokens);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident || !takes_children.contains(token.text) {
            continue;
        }
        // A member call (`obj.Text(…)`) is not a component construction.
        if index > 0 && tokens[index - 1].is_punct('.') {
            continue;
        }
        // A declaration names a function or a class, not a construction: `function badge(…) {`
        // and `class Badge {` have the same shape as a call with a child block.
        if index > 0
            && tokens[index - 1].kind == Kind::Ident
            && matches!(tokens[index - 1].text, "function" | "class")
        {
            continue;
        }
        let inside_declaration = class_bodies.iter().any(|(start, end, depth)| {
            token.start > *start && token.start < *end && depth_at[index] == *depth + 1
        });
        if inside_declaration {
            continue;
        }
        // The argument list follows the name. A comment between the two does not break the
        // call, so it is stepped over rather than read as the argument list.
        let Some(open) = next_code_token(&tokens, index + 1) else {
            continue;
        };
        if !tokens[open].is_punct('(') {
            continue;
        }
        let Some(close_at) = matching_delimiter(source, tokens[open].start) else {
            continue;
        };
        // The delimiter search reports a byte offset; the `)` token starts exactly there.
        let Some(close) = tokens[open..]
            .iter()
            .position(|t| t.start == close_at)
            .map(|offset| open + offset)
        else {
            continue;
        };
        // The child block follows the argument list. Comments may sit in between — ArkTS
        // routinely puts one there (`}) // 子组件多行布局`, the block on the next line) — so
        // the same rule applies: skip comments, then demand the `{`. Reading the literal next
        // token instead made the comment look like the block, and the component was silently
        // left in its illegal ArkTS shape.
        let Some(block) = next_code_token(&tokens, close + 1) else {
            continue;
        };
        // Anything but a comment in between means the call is over and the block belongs to
        // something else: `Foo(a) bar {` is not a component construction.
        if !tokens[block].is_punct('{') {
            continue;
        }
        let Some(block_end) = matching_delimiter(source, tokens[block].start) else {
            continue;
        };
        // `Foo()` loses its empty argument list and gains the callback; `Foo(a, b)` keeps its
        // arguments and gains one more.
        let open_start = tokens[open].start;
        let close_start = tokens[close].start;
        if close == open + 1 {
            edits.push((open_start + 1, close_start + 1, "() =>".to_string()));
        } else {
            edits.push((close_start, close_start + 1, ", () =>".to_string()));
        }
        edits.push((block_end + 1, block_end + 1, ")".to_string()));
    }
    crate::build::ets_assemble::apply_replacements_tracked(source, &edits)
}

/// The index of the first token at or after `from` that is not a comment.
///
/// The child-block rewrite has to look past comments, never treat one as syntax: a comment is
/// not a `{`, so a positional lookup that stops on it concludes the call has no child block
/// and leaves the source unparseable.
fn next_code_token(tokens: &[crate::build::lex::Token<'_>], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|&index| tokens[index].kind != Kind::Comment)
}

/// The byte ranges of every `class { … }` body, with the depth its members sit at.
fn class_body_ranges(
    source: &str,
    tokens: &[crate::build::lex::Token<'_>],
) -> Vec<(usize, usize, i32)> {
    let depths = brace_depths(tokens);
    let mut ranges = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident || token.text != "class" {
            continue;
        }
        let Some(open) = tokens
            .iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, t)| t.is_punct('{'))
        else {
            continue;
        };
        let Some(end) = crate::build::lex::matching_delimiter(source, open.1.start) else {
            continue;
        };
        ranges.push((open.1.start, end, depths[open.0]));
    }
    ranges
}

/// The brace depth each token sits at, before the token itself is counted.
fn brace_depths(tokens: &[crate::build::lex::Token<'_>]) -> Vec<i32> {
    let mut depths = Vec::with_capacity(tokens.len());
    let mut depth = 0i32;
    for token in tokens {
        if token.is_punct('}') {
            depth -= 1;
        }
        depths.push(depth);
        if token.is_punct('{') {
            depth += 1;
        }
    }
    depths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A call to an `@Builder` this file imports keeps its child block rewritten.
    ///
    /// The name is only in the import statement — `entry/components/Settings/Backgrounder.ets`
    /// calls `toggleRowList({ … }) { … }` with `toggleRowList` imported from `../Common/Common` —
    /// and before this the block survived and the parser rejected the file with
    /// `Expected a semicolon`.
    #[test]
    fn an_imported_builder_keeps_its_child_block_rewritten() {
        let src = "import { toggleRowList } from './Common'\nRow() {\n  toggleRowList({ title: 'x' }) {\n    Text('y')\n  }\n}\n";
        let names = imported_names(src);
        assert_eq!(names, vec!["toggleRowList"]);
        let out = rewrite_builders(src, &names);
        assert!(
            out.contains("toggleRowList({ title: 'x' }, () => {"),
            "{out}"
        );
    }

    /// A bare call to a `struct` declared in the same file keeps its child block rewritten.
    ///
    /// `Notice.ets` declares `struct noticeInputRow` at its own top level and calls it without
    /// `this.`; the mask only records `@Builder function` names, so the declaration itself is
    /// where the name has to come from.
    #[test]
    fn a_bare_call_to_a_declared_struct_keeps_its_child_block_rewritten() {
        let src = "@Component\nstruct noticeInputRow {\n  build() {\n    Column() {\n      noticeInputRow() {\n        Text('x')\n      }\n    }\n  }\n}\n";
        let names = declared_struct_names(src);
        assert_eq!(names, vec!["noticeInputRow"]);
        let out = rewrite_builders(src, &names);
        assert!(out.contains("noticeInputRow(() => {"), "{out}");
    }

    /// Default, namespace and `as` bindings are all names a call site can write.
    #[test]
    fn every_import_shape_names_a_binding() {
        let src = "import Default from './a'\nimport * as ns from './b'\nimport { A, B as C } from './c'\nimport type { T } from './d'\n";
        let mut names = imported_names(src);
        names.sort();
        assert_eq!(names, vec!["A", "B", "C", "Default", "T", "ns"]);
    }

    /// A name is not a builder just because it is imported: a call without a child block stays.
    #[test]
    fn an_imported_name_without_a_child_block_is_left_alone() {
        let src = "import { helper } from './h'\nconst x = helper(1);\n";
        let out = rewrite_builders(src, &imported_names(src));
        assert!(out.contains("helper(1)"), "{out}");
        assert!(!out.contains("() => {"), "{out}");
    }

    /// The child block becomes a trailing callback; an empty argument list disappears.
    #[test]
    fn a_builtin_component_takes_its_children_as_a_callback() {
        let out = rewrite_builders("Column() {\n  Text('hi')\n}\n", &[]);
        assert!(out.contains("Column(() => {"), "{out}");
        assert!(out.contains("Text('hi')"), "{out}");
        assert!(out.ends_with("})\n"), "{out}");
    }

    /// A call that already has arguments keeps them and gains the callback last.
    #[test]
    fn an_argument_list_is_kept_and_the_callback_is_appended() {
        let out = rewrite_builders("ForEach(a, gen) {\n  Text('x')\n}\n", &[]);
        assert_eq!(
            out, "ForEach(a, gen) {\n  Text('x')\n}\n",
            "ForEach is not a component"
        );
        let out = rewrite_builders("Column(space) {\n  Text('x')\n}\n", &[]);
        assert!(out.contains("Column(space, () => {"), "{out}");
    }

    /// The rewrite reports where it widened a line: the callback it inserts before the block
    /// pushes every position after it to the right, and a `{ page, line, col }` object carries
    /// the source's column.
    #[test]
    fn an_inserted_callback_reports_where_the_line_grew() {
        let (out, grown) = rewrite_builders_with_shifts("ListItem() { Text('x') }\n", &[], &[]);
        assert_eq!(out, "ListItem(() => { Text('x') })\n");
        assert_eq!(grown.len(), 2, "{grown:?}");
        let (offset, added) = grown[0];
        assert_eq!(added, 4, "`)` became `() =>`");
        assert!(out[offset..].starts_with("() =>"), "{out}");
        let (offset, _added) = grown[1];
        assert!(out[offset..].starts_with(')'), "{out}");
    }

    /// A comment between the argument list and the block does not hide the block.
    ///
    /// Measured on `clashbox`'s `CardEdit.ets`: `Flex({ … }) // 子组件多行布局` with the block
    /// on the next line was left untouched, so the parser met `Flex(…)` followed by a bare
    /// `{ … }` block and then `.width(…)` and reported "Unexpected token".
    #[test]
    fn a_comment_between_the_arguments_and_the_block_is_transparent() {
        let src = "Flex({ wrap: W }) // 子组件多行布局\n{\n  Row() {\n    Text('x')\n  }\n}\n.width('100%')\n";
        let out = rewrite_builders(src, &[]);
        assert_eq!(
            out,
            "Flex({ wrap: W }, () => // 子组件多行布局\n{\n  Row(() => {\n    Text('x')\n  })\n})\n.width('100%')\n"
        );
    }

    /// A comment between the name and the argument list is transparent as well.
    #[test]
    fn a_comment_between_the_name_and_the_arguments_is_transparent() {
        let out = rewrite_builders("Column /* 布局 */ () {\n  Text('hi')\n}\n", &[]);
        assert_eq!(out, "Column /* 布局 */ (() => {\n  Text('hi')\n})\n");
    }

    /// A token that is not a comment still ends the call: the block is not its child block.
    #[test]
    fn a_statement_between_the_arguments_and_the_block_keeps_both() {
        let src = "Column() ; {\n}\n";
        assert_eq!(rewrite_builders(src, &[]), src);
    }

    /// A call the table does not name keeps its shape.
    #[test]
    fn an_unknown_call_is_left_alone() {
        let src = "compute(1) {\n  2\n}\n";
        assert_eq!(rewrite_builders(src, &[]), src);
    }

    /// A member call is a method, not a component construction.
    #[test]
    fn a_member_call_is_left_alone() {
        let src = "obj.Text() {\n  Text('x')\n}\n";
        assert_eq!(rewrite_builders(src, &[]), src);
    }
}
