//! `@Sendable`: moving the decorator out of the parser's way and putting it back.
//!
//! `@Sendable` is an ArkTS decorator with no TypeScript spelling, so it cannot reach the
//! parser. It is blanked before parsing and re-inserted as the `"use sendable"` directive at
//! the head of the body it decorated, which is the form the byte-code compiler reads.

use crate::build::lex::{matching_delimiter, tokenize, Kind};

/// Blank every `@Sendable`, returning the decorated class and function names.
pub(crate) fn mask_sendable(source: &str) -> (String, Vec<String>, Vec<String>) {
    let tokens = tokenize(source);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut classes = Vec::new();
    let mut functions = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if !tokens[index].is_punct('@') {
            index += 1;
            continue;
        }
        let Some(name) = tokens.get(index + 1) else {
            break;
        };
        if name.kind != Kind::Ident || name.text != "Sendable" {
            index += 2;
            continue;
        }
        edits.push((
            tokens[index].start,
            name.end,
            " ".repeat(name.end - tokens[index].start),
        ));
        // The declaration it decorates decides where the directive goes.
        let Some(keyword) = tokens.get(index + 2) else {
            break;
        };
        let kind = keyword.text;
        if kind == "class" || kind == "function" {
            if let Some(declared) = tokens.get(index + 3) {
                if declared.kind == Kind::Ident {
                    if kind == "class" {
                        classes.push(declared.text.to_string());
                    } else {
                        functions.push(declared.text.to_string());
                    }
                }
            }
        }
        index += 2;
    }
    (
        crate::build::ets_assemble::apply_replacements(source, &edits),
        classes,
        functions,
    )
}

/// Insert `"use sendable";` at the head of every masked class and function body.
pub(crate) fn inject_sendable(source: &str, classes: &[String], functions: &[String]) -> String {
    if classes.is_empty() && functions.is_empty() {
        return source.to_string();
    }
    let tokens = tokenize(source);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        let keyword = &tokens[index];
        let name = &tokens[index + 1];
        if keyword.kind != Kind::Ident || name.kind != Kind::Ident {
            index += 1;
            continue;
        }
        let wanted = match keyword.text {
            "class" => classes,
            "function" => functions,
            _ => {
                index += 1;
                continue;
            }
        };
        if !wanted.iter().any(|candidate| candidate == name.text) {
            index += 1;
            continue;
        }
        let Some(open) = tokens.iter().skip(index + 2).find(|t| t.is_punct('{')) else {
            index += 1;
            continue;
        };
        // Only the body that belongs to this declaration, not one nested inside a signature.
        let Some(close) = matching_delimiter(source, open.start) else {
            index += 1;
            continue;
        };
        // A class carries the directive inside a constructor the compiler generates when the
        // source declares none; a function carries it as its first statement.
        let (at, text) = if keyword.text == "class" {
            match constructor_body(source, &tokens, index + 2) {
                Some(at) => (at, " \"use sendable\";".to_string()),
                None => {
                    // `addConstructorForSendableClass` (`process_sendable.js`): the synthesized
                    // constructor forwards to the base when the class extends one
                    // (`n = heritageClauses.some(ExtendsKeyword)`) — a subclass constructor
                    // without `super(...)` throws at run time, so the shape is not cosmetic.
                    let inherited = tokens
                        .iter()
                        .skip(index)
                        .take_while(|t| !t.is_punct('{'))
                        .any(|t| t.kind == Kind::Ident && t.text == "extends");
                    let text = if inherited {
                        " constructor(...args) { \"use sendable\"; super(...args); }"
                    } else {
                        " constructor() { \"use sendable\"; }"
                    };
                    (open.start + 1, text.to_string())
                }
            }
        } else {
            (open.start + 1, " \"use sendable\";".to_string())
        };
        edits.push((at, at, text));
        if keyword.text == "class" {
            optional_members(&tokens, open.start, close, &mut edits);
        }
        index += 2;
    }
    crate::build::ets_assemble::apply_replacements(source, &edits)
}

/// `transformOptionalMemberForSendable` (`process_sendable.js`): in a `@Sendable` class, a
/// property declared `name?: T` becomes `name: T | undefined = undefined`, and one that already
/// writes an initializer keeps it (`init?: string = 'i'` → `init: string | undefined = 'i'`).
///
/// The reference builds the member from the AST; here the member is edited in place: the `?` is
/// removed, ` | undefined` is written before the initializer (or before the terminator), and
/// `= undefined` is added when the source wrote none. A member whose type is a function type is
/// left alone — the shape needs parentheses this text pass does not decide, and the ArkTS linter
/// rejects a function-typed sendable property anyway.
fn optional_members(
    tokens: &[crate::build::lex::Token<'_>],
    body_open: usize,
    body_close: usize,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate() {
        if token.start > body_close {
            break;
        }
        if token.start < body_open {
            continue;
        }
        if token.is_punct('{') {
            depth += 1;
            continue;
        }
        if token.is_punct('}') {
            depth -= 1;
            continue;
        }
        // Members sit one level inside the class body.
        if depth != 1 || token.kind != Kind::Ident {
            continue;
        }
        let question = tokens.get(index + 1);
        let colon = tokens.get(index + 2);
        if !question.is_some_and(|t| t.is_punct('?')) || !colon.is_some_and(|t| t.is_punct(':')) {
            continue;
        }
        let question = tokens[index + 1];
        let Some((insert_at, suffix)) = member_terminator(tokens, index + 3, body_close) else {
            continue;
        };
        edits.push((question.start, question.end, String::new()));
        edits.push((insert_at, insert_at, suffix.to_string()));
    }
}

/// Where a member's type ends and what has to be written there: ` | undefined = undefined` for
/// a member with no initializer, ` | undefined` for one that writes its own.
fn member_terminator(
    tokens: &[crate::build::lex::Token<'_>],
    from: usize,
    body_close: usize,
) -> Option<(usize, &'static str)> {
    let mut depth = 0i32;
    let mut arrow_type = false;
    let mut previous: Option<&crate::build::lex::Token<'_>> = None;
    for (offset, token) in tokens.iter().enumerate().skip(from) {
        if token.start >= body_close {
            // The member runs to the closing brace: the source wrote no `;`.
            let at = previous.map_or(token.start, |t| t.end);
            return (!arrow_type).then_some((at, " | undefined = undefined"));
        }
        if token.kind == Kind::Punct {
            if token.is_punct('=') && tokens.get(offset + 1).is_some_and(|t| t.is_punct('>')) {
                // A function type: ` | undefined` would bind to its return type without the
                // parentheses this pass does not decide, so the member is left as written.
                arrow_type = true;
            }
            for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
                if token.is_punct(open) {
                    depth += 1;
                } else if token.is_punct(close) {
                    depth -= 1;
                }
            }
            // The type ends before the separator, so the write goes at its last token.
            let end = previous.map_or(token.start, |t| t.end);
            if depth == 0 && token.is_punct(';') {
                return (!arrow_type).then_some((end, " | undefined = undefined"));
            }
            if depth == 0 && token.is_punct('=') && !arrow_type {
                // A lone `=` starts the initializer; `==` and `=>` are neither.
                let next = tokens.get(offset + 1);
                if !next.is_some_and(|t| t.is_punct('=')) {
                    return Some((end, " | undefined"));
                }
            }
        }
        previous = Some(token);
    }
    None
}

/// The opening brace of the class body's constructor, when it declares one.
fn constructor_body(
    source: &str,
    tokens: &[crate::build::lex::Token<'_>],
    from: usize,
) -> Option<usize> {
    let class_open = tokens.iter().skip(from).find(|t| t.is_punct('{'))?;
    let class_end = matching_delimiter(source, class_open.start)?;
    for (index, token) in tokens.iter().enumerate().skip(from) {
        if token.start >= class_end {
            break;
        }
        if token.kind != Kind::Ident || token.text != "constructor" {
            continue;
        }
        let open = tokens.iter().skip(index + 1).find(|t| t.is_punct('{'))?;
        if open.start < class_end {
            return Some(open.start + 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decorator is blanked and the name recorded, and the directive comes back inside
    /// the body.
    #[test]
    fn a_sendable_class_gets_the_directive_inside_its_body() {
        let src = "@Sendable\nclass Box {\n  value: number = 0;\n}\n";
        let (masked, classes, functions) = mask_sendable(src);
        assert!(!masked.contains("@Sendable"), "{masked}");
        assert_eq!(classes, vec!["Box".to_string()]);
        assert!(functions.is_empty());
        let restored = inject_sendable(&masked, &classes, &functions);
        assert!(
            restored.contains("constructor() { \"use sendable\"; }"),
            "a class without a constructor gets one: {restored}"
        );
    }

    /// A `@Sendable` class member written `name?: T` gets `| undefined` and an initializer.
    #[test]
    fn a_sendable_optional_member_gets_undefined_and_an_initializer() {
        let src = "@Sendable\nclass Box {\n  value?: number;\n}\n";
        let (masked, classes, functions) = mask_sendable(src);
        let out = inject_sendable(&masked, &classes, &functions);
        assert!(
            out.contains("value: number | undefined = undefined"),
            "{out}"
        );
    }

    /// A member that writes its own initializer keeps it.
    #[test]
    fn a_sendable_optional_member_keeps_its_initializer() {
        let src = "@Sendable\nclass Box {\n  init?: string = 'i';\n}\n";
        let (masked, classes, functions) = mask_sendable(src);
        let out = inject_sendable(&masked, &classes, &functions);
        assert!(out.contains("init: string | undefined = 'i'"), "{out}");
    }

    /// A member without the question token is untouched.
    #[test]
    fn a_required_member_is_untouched() {
        let src = "@Sendable\nclass Box {\n  plain: boolean = false;\n}\n";
        let (masked, classes, functions) = mask_sendable(src);
        let out = inject_sendable(&masked, &classes, &functions);
        assert!(out.contains("plain: boolean = false;"), "{out}");
    }

    /// A class that is not `@Sendable` keeps its optional member as written.
    #[test]
    fn an_optional_member_outside_a_sendable_class_is_untouched() {
        let src = "@Sendable\nclass Box {\n  value: number = 0;\n}\nclass Plain {\n  other?: number;\n}\n";
        let (masked, classes, functions) = mask_sendable(src);
        let out = inject_sendable(&masked, &classes, &functions);
        assert!(out.contains("other?: number;"), "{out}");
    }

    /// A decorated function gets the directive as its first statement.
    #[test]
    fn a_sendable_function_gets_the_directive_first() {
        let src = "@Sendable\nfunction run() {\n  return 1;\n}\n";
        let (masked, classes, functions) = mask_sendable(src);
        assert_eq!(functions, vec!["run".to_string()]);
        let restored = inject_sendable(&masked, &classes, &functions);
        assert!(
            restored.contains("function run() { \"use sendable\";"),
            "{restored}"
        );
    }
}
