//! Enum members: the values TypeScript gives them, and the inlining a `const enum` gets.
//!
//! ArkTS (like TypeScript) lets a member fall back to the previous member's value, the first
//! one starting at zero: `enum Mode { Off, On }` means `Off = 0, On = 1`. The reference build
//! writes those values into the emitted source instead of leaving them implicit — the golden of
//! `fixtures/enum-initializers` shows `Off = 0, On = 1`, `Low = 3, Mid = 4, High = 5` and
//! `A = 0, B = 5, C = 6`.
//!
//! The reference's rule is the checker's constant value of each member
//! (`ts.getConstantValue(member)`), emitted as a numeric or string literal — the enum arm of
//! TypeScript's declaration emitter, which is the only place in the toolchain that performs
//! this rewrite (`bin/ark/build-mac/legacy_api8/src/index.js`, the `case 258` visitor that calls
//! `updateEnumDeclaration` over `mapDefined(members, …)`). A member whose value is not a
//! compile-time constant keeps no initializer.
//!
//! What this pass does, on the text the lowering already produced:
//!
//! - **values**: a member with no `= …` takes the previous member's value + 1, the first taking
//!   0; an explicit initializer is evaluated as a constant expression (literals, references to
//!   earlier members of the same enum, `+ - * / % ** << >> >>> & | ^ ~`, parentheses) and
//!   written back as that literal, which is what the reference emits (`A = 1, B = A * 2` becomes
//!   `A = 1, B = 2`; `H = 0x10` becomes `H = 16`). A member whose value is not a compile-time
//!   constant keeps its initializer as written and ends the implicit run;
//! - **inlining**: a `const enum` has every constant member value substituted at its
//!   `Name.Member` **value** uses, which is the reference's `const enum` erasure
//!   (`fixtures/const-enum-inline`, `fixtures/const-enum-import`). A plain `enum` is **not**
//!   inlined — only its declaration is rewritten.
//!
//! The member table of the file itself comes from the token scan below; the tables of the other
//! files come from [`set_const_enums`], which the build fills once with every source of the
//! project before it lowers one (`src/build/ets_assemble.rs`). A name the file declares wins
//! over the project entry of the same name, because module scope is what a use site means.
//!
//! Which uses count as *values* comes from the parsed program, not from the token stream: in a
//! type position TypeScript parses `K.A` as a qualified type name, never as a member
//! expression, so the reference keeps the member type there (`fixtures/const-enum-inline`:
//! `let typedValue: ConstMix.A = 0;`, `function takesMember(a: ConstMix.A)`,
//! `function returnsMember(): ConstMix.C`, `type MemberUnion = ConstMix.A | ConstMix.C` — the
//! value side of each is inlined, the type side is not). A use site the text cannot parse is
//! left as written rather than guessed at.
//!
//! The declaration scan runs over tokens, so `enum` inside a comment or a string is not a
//! declaration. The inliner does not resolve shadowing: a local binding named like the enum is
//! not distinguished from it.

use std::cell::RefCell;

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, MemberExpression};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::build::lex::{tokenize, Kind};

/// One enum member's compile-time value.
#[derive(Clone)]
enum Value {
    Number(f64),
    /// A string member, kept in its **source spelling** (quotes included): the reference
    /// re-emits it as a double-quoted literal, which is the same syntax tree, so keeping the
    /// original spelling avoids churn without changing the output.
    Text(String),
}

impl Value {
    /// The literal this value is written as.
    fn render(&self) -> String {
        match self {
            Value::Number(value) => number(*value),
            Value::Text(text) => text.clone(),
        }
    }
}

/// A `const enum`'s members, `name → literal`, for the use-site substitution.
#[derive(Clone, PartialEq, Eq)]
struct ConstEnum {
    name: String,
    members: Vec<(String, String)>,
}

/// One `const enum` of the project: its name and its `(member, literal)` pairs.
pub(crate) type ConstEnumTable = (String, Vec<(String, String)>);

thread_local! {
    /// `const enum` tables gathered from every source of the project, so a use site can see an
    /// enum another file declares (`fixtures/const-enum-import`). The build lowers one file at
    /// a time, so a thread-local is the narrowest place to carry it here — the same choice the
    /// `@AnimatableExtend` table makes.
    static PROJECT_CONST_ENUMS: RefCell<Vec<ConstEnumTable>> =
        const { RefCell::new(Vec::new()) };
}

/// Publish the project's `const enum` tables for the files lowered after this call.
pub(crate) fn set_const_enums(entries: &[ConstEnumTable]) {
    PROJECT_CONST_ENUMS.with(|cell| {
        let mut table = cell.borrow_mut();
        table.clear();
        table.extend_from_slice(entries);
    });
}

fn project_const_enums() -> Vec<ConstEnumTable> {
    PROJECT_CONST_ENUMS.with(|cell| cell.borrow().clone())
}

/// Every `const enum` the given sources declare, as `name → [(member, literal)]`.
///
/// A name declared twice with different members is dropped: at a use site there is no way to
/// tell which declaration is meant, and a wrong literal is worse than none.
pub(crate) fn collect_const_enums(sources: &[String]) -> Vec<ConstEnumTable> {
    let mut table: Vec<ConstEnumTable> = Vec::new();
    let mut ambiguous: Vec<String> = Vec::new();
    for source in sources {
        for (name, members) in const_enums_in(source) {
            if ambiguous.contains(&name) {
                continue;
            }
            match table.iter().position(|(known, _)| *known == name) {
                None => table.push((name, members)),
                Some(at) if table[at].1 == members => {}
                Some(_) => {
                    table.retain(|(known, _)| *known != name);
                    ambiguous.push(name);
                }
            }
        }
    }
    table
}

/// Give every enum member the value TypeScript gives it, and inline a `const enum`'s uses.
pub(crate) fn fill_enum_initializers(text: &str) -> String {
    let tokens: Vec<crate::build::lex::Token<'_>> = tokenize(text)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    let (local, mut edits) = scan_enums(&tokens);
    // The file's own declarations win over the project table: a use site in this file resolves
    // in module scope first.
    let mut tables = local;
    for (name, members) in project_const_enums() {
        if !tables.iter().any(|table| table.name == name) {
            tables.push(ConstEnum { name, members });
        }
    }
    if !tables.is_empty() {
        edits.extend(value_uses(text, &tables));
    }
    apply_edits(text, edits)
}

/// The `const enum` declarations of one token stream: the tables that can be inlined, and the
/// edits that write each member's constant value into the declaration.
fn scan_enums(
    tokens: &[crate::build::lex::Token<'_>],
) -> (Vec<ConstEnum>, Vec<(usize, usize, String)>) {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut consts: Vec<ConstEnum> = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].kind != Kind::Ident || tokens[index].text != "enum" {
            index += 1;
            continue;
        }
        let Some(name_token) = tokens.get(index + 1) else {
            break;
        };
        let named = matches!(name_token.kind, Kind::Ident | Kind::Str);
        let opens = tokens
            .get(index + 2)
            .is_some_and(|token| token.is_punct('{'));
        if !(named && opens) {
            index += 1;
            continue;
        }
        let Some(close) = matching_brace(tokens, index + 2) else {
            break;
        };
        // `const enum`: the declaration is erased at the use sites, the declaration itself
        // stays. `declare const enum` is treated the same way; no fixture exercises it.
        let is_const =
            index > 0 && tokens[index - 1].kind == Kind::Ident && tokens[index - 1].text == "const";
        let enum_name = name_token.text.to_string();

        let mut values: Vec<(String, Value)> = Vec::new();
        let mut complete = true;
        let mut next: Option<f64> = Some(0.0);
        let mut start = index + 3;
        while start < close {
            let end = member_end(tokens, start, close);
            if start < end {
                let name_token = &tokens[start];
                if name_token.kind == Kind::Ident || name_token.kind == Kind::Str {
                    let equals = (start + 1..end).find(|&at| tokens[at].is_punct('='));
                    let value = match equals {
                        None => next.map(Value::Number),
                        Some(at) => evaluate(tokens, at + 1, end, &values),
                    };
                    if let Some(value) = &value {
                        let canonical = value.render();
                        match equals {
                            // An implicit member gets the value written after its name.
                            None => edits.push((
                                name_token.end,
                                name_token.end,
                                format!(" = {canonical}"),
                            )),
                            // An explicit initializer is rewritten only when the source does not
                            // already spell the same literal (`0x10` → `16`, `A * 2` → `2`).
                            Some(at) if !spells_the_literal(tokens, at + 1, end, &canonical) => {
                                edits.push((
                                    tokens[at].end,
                                    tokens[end - 1].end,
                                    format!(" {canonical}"),
                                ));
                            }
                            Some(_) => {}
                        }
                        next = match value {
                            Value::Number(number) => Some(number + 1.0),
                            Value::Text(_) => None,
                        };
                    } else {
                        next = None;
                    }
                    match value {
                        Some(value) => values.push((name_token.text.to_string(), value)),
                        None => complete = false,
                    }
                } else {
                    next = None;
                    complete = false;
                }
            }
            start = end + 1;
        }

        if is_const && complete && !values.is_empty() {
            consts.push(ConstEnum {
                name: enum_name,
                members: values
                    .iter()
                    .map(|(name, value)| (name.clone(), value.render()))
                    .collect(),
            });
        }
        index = close + 1;
    }
    (consts, edits)
}

/// Whether `source` declares `name` as a `const enum`.
///
/// A `const enum`'s members are erased at their use sites, so importing one is a type-space
/// import: the reference writes `import type { ImportedKind } from './Shared'`
/// (`fixtures/const-enum-import`) and the target keeps no runtime binding to read.
pub(crate) fn declares_const_enum(source: &str, name: &str) -> bool {
    const_enums_in(source)
        .iter()
        .any(|(declared, _)| declared == name)
}

/// The `const enum` tables one file declares, for the project-wide collection.
fn const_enums_in(source: &str) -> Vec<ConstEnumTable> {
    let tokens: Vec<crate::build::lex::Token<'_>> = tokenize(source)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    let (tables, _edits) = scan_enums(&tokens);
    tables
        .into_iter()
        .map(|table| (table.name, table.members))
        .collect()
}

/// The `Name.Member` **value** uses of the given tables, as replacement edits.
///
/// Only value positions are collected. TypeScript parses `K.A` in a type position as a
/// qualified type name, never as a member expression, which is why the walk below can decide
/// this without a type checker (`fixtures/const-enum-inline`).
fn value_uses(text: &str, tables: &[ConstEnum]) -> Vec<(usize, usize, String)> {
    struct Uses<'t> {
        tables: &'t [ConstEnum],
        hits: Vec<(usize, usize, String)>,
    }

    impl<'a> Visit<'a> for Uses<'_> {
        fn visit_member_expression(&mut self, it: &MemberExpression<'a>) {
            if let MemberExpression::StaticMemberExpression(member) = it {
                if !member.optional {
                    if let Expression::Identifier(object) = &member.object {
                        let literal = self
                            .tables
                            .iter()
                            .find(|table| table.name == object.name.as_str())
                            .and_then(|table| {
                                table
                                    .members
                                    .iter()
                                    .find(|(name, _)| name == member.property.name.as_str())
                            });
                        if let Some((_, literal)) = literal {
                            self.hits.push((
                                member.span.start as usize,
                                member.span.end as usize,
                                literal.clone(),
                            ));
                        }
                    }
                }
            }
            walk::walk_member_expression(self, it);
        }
    }

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, text, SourceType::ts()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        // The lowering hands this pass valid TypeScript; if it does not parse, leave every use
        // as written rather than substitute at a guessed position.
        return Vec::new();
    }
    let mut visitor = Uses {
        tables,
        hits: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    let mut hits = visitor.hits;
    // `K.A.toString()`: substituting would leave `0.toString`, which is not valid TypeScript.
    // The enum member is a value there, but the rewrite has to add parentheses the reference
    // does not; leaving it is the smaller error.
    hits.retain(|(_, end, _)| !text[*end..].trim_start().starts_with('.'));
    hits
}

/// Write the edits into the text, from the back, skipping any that overlap one already applied.
fn apply_edits(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    if edits.is_empty() {
        return text.to_string();
    }
    edits.sort_by_key(|(start, end, _)| (std::cmp::Reverse(*start), std::cmp::Reverse(*end)));
    let mut out = text.to_string();
    let mut previous_start = usize::MAX;
    for (start, end, replacement) in edits {
        if end > previous_start {
            continue;
        }
        out.replace_range(start..end, &replacement);
        previous_start = start;
    }
    out
}

/// Whether `from..to` already spells `canonical` as one literal.
fn spells_the_literal(
    tokens: &[crate::build::lex::Token<'_>],
    from: usize,
    to: usize,
    canonical: &str,
) -> bool {
    matches!(
        tokens.get(from..to),
        Some([one]) if (one.kind == Kind::Number || one.kind == Kind::Str) && one.text == canonical
    )
}

/// The index one past the member that starts at `from`, i.e. its comma or the closing brace.
fn member_end(tokens: &[crate::build::lex::Token<'_>], from: usize, close: usize) -> usize {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().take(close + 1).skip(from) {
        if token.is_punct('(') || token.is_punct('[') || token.is_punct('{') {
            depth += 1;
        } else if token.is_punct(')') || token.is_punct(']') || token.is_punct('}') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && token.is_punct(',') {
            return index;
        }
    }
    close
}

/// The constant value of the initializer tokens `from..to`.
fn evaluate(
    tokens: &[crate::build::lex::Token<'_>],
    from: usize,
    to: usize,
    prior: &[(String, Value)],
) -> Option<Value> {
    let mut at = from;
    let value = expression(tokens, &mut at, to, prior, 0)?;
    // Trailing tokens mean this is not an expression we understand: leave it as written.
    (at == to).then_some(value)
}

/// Precedence climbing over the constant binary operators.
fn expression(
    tokens: &[crate::build::lex::Token<'_>],
    at: &mut usize,
    to: usize,
    prior: &[(String, Value)],
    minimum: u8,
) -> Option<Value> {
    let mut left = unary(tokens, at, to, prior)?;
    while let Some((operator, precedence, width)) = binary(tokens, *at, to) {
        if precedence < minimum {
            break;
        }
        *at += width;
        // `**` is right-associative; every other operator here is left-associative.
        let next_minimum = if operator == "**" {
            precedence
        } else {
            precedence + 1
        };
        let right = expression(tokens, at, to, prior, next_minimum)?;
        left = apply(&operator, left, right)?;
    }
    Some(left)
}

fn unary(
    tokens: &[crate::build::lex::Token<'_>],
    at: &mut usize,
    to: usize,
    prior: &[(String, Value)],
) -> Option<Value> {
    let token = tokens.get(*at).filter(|_| *at < to)?;
    if token.is_punct('-') {
        *at += 1;
        return match unary(tokens, at, to, prior)? {
            Value::Number(value) => Some(Value::Number(-value)),
            Value::Text(_) => None,
        };
    }
    if token.is_punct('+') {
        *at += 1;
        return unary(tokens, at, to, prior);
    }
    if token.is_punct('~') {
        *at += 1;
        return match unary(tokens, at, to, prior)? {
            // JavaScript's bitwise operators work on 32-bit integers, and the reference
            // evaluates an initializer as JavaScript: `~1` is `-2`, `1 << 31` is negative.
            Value::Number(value) => Some(Value::Number(!(value as i32) as f64)),
            Value::Text(_) => None,
        };
    }
    primary(tokens, at, to, prior)
}

fn primary(
    tokens: &[crate::build::lex::Token<'_>],
    at: &mut usize,
    to: usize,
    prior: &[(String, Value)],
) -> Option<Value> {
    let token = tokens.get(*at).filter(|_| *at < to)?;
    match token.kind {
        Kind::Number => {
            *at += 1;
            parse_number(token.text).map(Value::Number)
        }
        Kind::Str => {
            *at += 1;
            Some(Value::Text(token.text.to_string()))
        }
        Kind::Ident => {
            // A reference to a member declared earlier in the same enum.
            let value = prior.iter().find(|(name, _)| name == token.text)?;
            *at += 1;
            Some(value.1.clone())
        }
        _ if token.is_punct('(') => {
            *at += 1;
            let value = expression(tokens, at, to, prior, 0)?;
            let closes = tokens.get(*at).is_some_and(|token| token.is_punct(')'));
            if !closes {
                return None;
            }
            *at += 1;
            Some(value)
        }
        _ => None,
    }
}

/// The binary operator at `at`, with its precedence and how many tokens it spans.
fn binary(
    tokens: &[crate::build::lex::Token<'_>],
    at: usize,
    to: usize,
) -> Option<(String, u8, usize)> {
    let token = tokens.get(at).filter(|_| at < to)?;
    if token.kind != Kind::Punct {
        return None;
    }
    // Multi-character operators arrive as adjacent single-character punctuation.
    let adjacent = |offset: usize, text: &str| {
        tokens
            .get(at + offset)
            .filter(|_| at + offset < to)
            .is_some_and(|next| next.text == text && next.start == token.start + offset)
    };
    match token.text {
        "*" if adjacent(1, "*") => Some(("**".to_string(), 14, 2)),
        "*" => Some(("*".to_string(), 13, 1)),
        "/" => Some(("/".to_string(), 13, 1)),
        "%" => Some(("%".to_string(), 13, 1)),
        "+" => Some(("+".to_string(), 12, 1)),
        "-" => Some(("-".to_string(), 12, 1)),
        "<" if adjacent(1, "<") => Some(("<<".to_string(), 11, 2)),
        ">" if adjacent(1, ">") && adjacent(2, ">") => Some((">>>".to_string(), 11, 3)),
        ">" if adjacent(1, ">") => Some((">>".to_string(), 11, 2)),
        "&" => Some(("&".to_string(), 10, 1)),
        "^" => Some(("^".to_string(), 9, 1)),
        "|" => Some(("|".to_string(), 8, 1)),
        _ => None,
    }
}

fn apply(operator: &str, left: Value, right: Value) -> Option<Value> {
    let (Value::Number(left), Value::Number(right)) = (left, right) else {
        // TypeScript allows no operator but nothing at all on string members.
        return None;
    };
    let value = match operator {
        "+" => left + right,
        "-" => left - right,
        "*" => left * right,
        "/" => {
            if right == 0.0 {
                return None;
            }
            left / right
        }
        "%" => {
            if right == 0.0 {
                return None;
            }
            left % right
        }
        "**" => left.powf(right),
        // The bitwise family is int32 in JavaScript, shift counts included.
        "<<" => ((left as i32) << (right as u32 & 31)) as f64,
        ">>" => ((left as i32) >> (right as u32 & 31)) as f64,
        ">>>" => ((left as i32) as u32 >> (right as u32 & 31)) as f64,
        "&" => ((left as i32) & (right as i32)) as f64,
        "|" => ((left as i32) | (right as i32)) as f64,
        "^" => ((left as i32) ^ (right as i32)) as f64,
        _ => return None,
    };
    // `10 ** 400` is `Infinity` in JavaScript, and `Infinity` is not a literal this pass may
    // write: leave the source initializer alone instead.
    value.is_finite().then_some(Value::Number(value))
}

/// The index of the `}` closing the `{` at `open`.
fn matching_brace(tokens: &[crate::build::lex::Token<'_>], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.is_punct('{') {
            depth += 1;
        } else if token.is_punct('}') {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// A numeric literal, including the bases and the `_` separator TypeScript accepts.
fn parse_number(text: &str) -> Option<f64> {
    let digits = text.replace('_', "");
    let radix = |prefix: &str, base: u32| {
        digits
            .strip_prefix(prefix)
            .and_then(|rest| i64::from_str_radix(rest, base).ok())
            .map(|value| value as f64)
    };
    if let Some(value) = radix("0x", 16).or_else(|| radix("0X", 16)) {
        return Some(value);
    }
    if let Some(value) = radix("0b", 2).or_else(|| radix("0B", 2)) {
        return Some(value);
    }
    if let Some(value) = radix("0o", 8).or_else(|| radix("0O", 8)) {
        return Some(value);
    }
    if digits.ends_with('n') {
        return None;
    }
    digits.parse::<f64>().ok()
}

/// A value as TypeScript writes it: `0`, `1`, `-1`, `2.5`.
fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_a_run_from_zero() {
        let src = "enum Mode {\n  Off,\n  On\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "enum Mode {\n  Off = 0,\n  On = 1\n}\n"
        );
    }

    #[test]
    fn fills_from_the_explicit_value() {
        let src = "enum Level {\n  Low = 3,\n  Mid,\n  High\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "enum Level {\n  Low = 3,\n  Mid = 4,\n  High = 5\n}\n"
        );
    }

    #[test]
    fn fills_after_an_explicit_member_in_the_middle() {
        let src = "enum Mixed {\n  A,\n  B = 5,\n  C\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "enum Mixed {\n  A = 0,\n  B = 5,\n  C = 6\n}\n"
        );
    }

    #[test]
    fn leaves_string_members_alone() {
        let src = "enum Named {\n  X = 'x',\n  Y = 'y'\n}\n";
        assert_eq!(fill_enum_initializers(src), src);
    }

    #[test]
    fn an_expression_stops_the_run() {
        let src = "enum E {\n  A = compute(),\n  B\n}\n";
        assert_eq!(fill_enum_initializers(src), src);
    }

    #[test]
    fn counts_bases_and_signs() {
        let src = "enum E {\n  A = 0x10,\n  B,\n  C = -1,\n  D\n}\n";
        // The reference writes the constant, not the source spelling.
        assert_eq!(
            fill_enum_initializers(src),
            "enum E {\n  A = 16,\n  B = 17,\n  C = -1,\n  D = 0\n}\n"
        );
    }

    #[test]
    fn folds_an_expression_initializer() {
        // `fixtures/const-enum-inline`: `A = 1, B = A * 2` is emitted as `A = 1, B = 2`.
        let src = "enum E {\n  A = 1,\n  B = A * 2\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "enum E {\n  A = 1,\n  B = 2\n}\n"
        );
    }

    #[test]
    fn inlines_a_const_enum_member() {
        let src = "const enum K {\n  A = 1\n}\nlet x = K.A;\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum K {\n  A = 1\n}\nlet x = 1;\n"
        );
    }

    #[test]
    fn inlines_an_implicit_const_enum_member() {
        let src = "const enum K {\n  A,\n  B = 5,\n  C\n}\nlet x = K.C;\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum K {\n  A = 0,\n  B = 5,\n  C = 6\n}\nlet x = 6;\n"
        );
    }

    #[test]
    fn leaves_a_plain_enum_member_alone() {
        // Only the declaration is rewritten; the use site stays (`fixtures/const-enum-inline`).
        let src = "enum K {\n  A = 1\n}\nlet x = K.A;\n";
        assert_eq!(fill_enum_initializers(src), src);
    }

    #[test]
    fn a_const_enum_with_a_computed_member_is_not_inlined() {
        let src = "const enum K {\n  A = compute()\n}\nlet x = K.A;\n";
        assert_eq!(fill_enum_initializers(src), src);
    }

    #[test]
    fn evaluates_with_javascript_bitwise_semantics() {
        // int32, not int64: `1 << 31` is negative in JavaScript. No golden covers this yet;
        // the reference evaluates an initializer as JavaScript, so the rule is the language's.
        let src = "enum E {\n  A = 1 << 31,\n  B = ~1,\n  C = 0x10 | 1\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "enum E {\n  A = -2147483648,\n  B = -2,\n  C = 17\n}\n"
        );
    }

    #[test]
    fn a_value_that_is_not_finite_keeps_the_source() {
        // `10 ** 400` is `Infinity`, which is not a literal this pass may write.
        let src = "enum E {\n  A = 10 ** 400\n}\n";
        assert_eq!(fill_enum_initializers(src), src);
    }

    #[test]
    fn a_member_type_in_a_parameter_is_kept() {
        // `fixtures/const-enum-inline`: `function takesMember(a: ConstMix.A)` keeps the type.
        let src = "const enum K {\n  A,\n  B = 5,\n  C\n}\nfunction takesMember(a: K.A): number {\n  return a;\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum K {\n  A = 0,\n  B = 5,\n  C = 6\n}\nfunction takesMember(a: K.A): number {\n  return a;\n}\n"
        );
    }

    #[test]
    fn a_member_type_in_a_return_annotation_is_kept() {
        // `function returnsMember(): ConstMix.C { return ConstMix.C }` prints `return 6;`.
        let src =
            "const enum K {\n  A,\n  C = 6\n}\nfunction returnsMember(): K.C {\n  return K.C;\n}\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum K {\n  A = 0,\n  C = 6\n}\nfunction returnsMember(): K.C {\n  return 6;\n}\n"
        );
    }

    #[test]
    fn a_member_type_in_a_union_is_kept() {
        // `type MemberUnion = ConstMix.A | ConstMix.C` keeps both member types.
        let src = "const enum K {\n  A,\n  C = 6\n}\ntype U = K.A | K.C;\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum K {\n  A = 0,\n  C = 6\n}\ntype U = K.A | K.C;\n"
        );
    }

    #[test]
    fn a_project_table_inlines_a_use_in_another_file() {
        // `fixtures/const-enum-import`: the table comes from the declaration's file.
        let table = [("K".to_string(), vec![("A".to_string(), "1".to_string())])];
        set_const_enums(&table);
        let out = fill_enum_initializers("let x = K.A;\n");
        set_const_enums(&[]);
        assert_eq!(out, "let x = 1;\n");
    }

    #[test]
    fn a_file_declaration_wins_over_the_project_table() {
        // Module scope is what a use site means: the file's own `K` is the one `K.A` reads.
        let table = [("K".to_string(), vec![("A".to_string(), "9".to_string())])];
        set_const_enums(&table);
        let out = fill_enum_initializers("const enum K {\n  A = 1\n}\nlet x = K.A;\n");
        set_const_enums(&[]);
        assert_eq!(out, "const enum K {\n  A = 1\n}\nlet x = 1;\n");
    }

    #[test]
    fn a_conflicting_project_name_is_dropped() {
        // Two files declare `K` with different members: no site can know which one it means,
        // and a wrong literal is worse than none.
        let first = "const enum K {\n  A = 1\n}\n".to_string();
        let second = "const enum K {\n  A = 2\n}\n".to_string();
        assert!(collect_const_enums(&[first, second]).is_empty());
    }

    #[test]
    fn an_identical_project_declaration_is_kept_once() {
        let first = "const enum K {\n  A = 1\n}\n".to_string();
        let second = "const enum K {\n  A = 1\n}\n".to_string();
        assert_eq!(
            collect_const_enums(&[first, second]),
            vec![("K".to_string(), vec![("A".to_string(), "1".to_string())])]
        );
    }

    #[test]
    fn a_plain_enum_is_not_collected() {
        let source = "enum K {\n  A = 1\n}\n".to_string();
        assert!(collect_const_enums(&[source]).is_empty());
    }

    #[test]
    fn a_member_used_as_a_type_keeps_the_member_type() {
        // `fixtures/const-enum-inline`: `let typedValue: ConstMix.A = ConstMix.A;` is emitted
        // as `let typedValue: ConstMix.A = 0;` — the value is inlined, the type is not.
        let src = "const enum K {\n  A,\n  B = 5,\n  C\n}\nlet typedValue: K.A = K.A;\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum K {\n  A = 0,\n  B = 5,\n  C = 6\n}\nlet typedValue: K.A = 0;\n"
        );
    }

    #[test]
    fn a_string_member_is_inlined_with_its_spelling() {
        let src = "const enum S {\n  X = 'x'\n}\nlet x = S.X;\n";
        assert_eq!(
            fill_enum_initializers(src),
            "const enum S {\n  X = 'x'\n}\nlet x = 'x';\n"
        );
    }
}
