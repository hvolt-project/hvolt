// process stage: `process_custom_component.js` — a child custom component's construction.
//
// `processCustomComponent(exprStatement, statements, errors, name, …)`
// (`process_custom_component.js:67`) turns `Child({ a: this.a })` into the statements the
// runtime needs: the `new Child(this, params, undefined, elmtId, paramsLambda, {page, line,
// col})` construction (the arguments come from `addCustomComponentId`,
// `process_component_member.js:486`), the `let paramsLambda = () => { return {…}; }`
// declaration (`assignComponentParams`, `:401`), and the `Child.paramsGenerator_ =
// paramsLambda` assignment (`assignmentFunction`, `:454`). A `@Reusable` child is built by
// `createNewRecycleComponent` instead (`:451`), guarded by `isRecycle`/`isReuseInV2`.
//
// The parent's `@Link` member is the part that has to be read from the parent, not copied:
// `splitComponentParams` (`:434`) and `reWriteComponentParams` (`:413`) rewrite the argument
// to the parent's backstore (`this.__x`), which is what [`rewrite_bindings`] does here. The
// `__`-prefixed accessor is the same one `process_component_member.js` declares for the field.
//
// Statement placement stays in [`super::process_component_build`]: the reference places the
// construction from `processComponentChild`, and this module only decides its shape.

use crate::build::stage::process_component_build::UiNode;

pub(crate) const UPDATE_STATE_VARS_OF_CHIND_BY_ELMTID: &str = "updateStateVarsOfChildByElmtId";

/// The constructor form of a child's params object, for a call site outside a `build()` body.
///
/// A `builder:` argument in a `CustomDialogController` builds the same child a `build()` body
/// does, so it takes the same shape: the object the source wrote, with every `@Link` field
/// read from the parent's backstore.
pub(crate) fn construct_params(text: &str, link_fields: &[String]) -> String {
    rewrite_bindings(text, Binding::Backstore, link_fields)
}

/// The object `updateStateVarsOfChildByElmtId` forwards: the fields the child re-reads.
pub(crate) fn state_vars_object(
    node: &UiNode,
    meta: &crate::build::compile::component_index::Meta,
) -> String {
    if meta.state_vars.is_empty() {
        return "{}".to_string();
    }
    let Some(written) = node.create_args.as_deref() else {
        return "{}".to_string();
    };
    let written = written.trim();
    let wanted: Vec<String> = fields_of(written)
        .into_iter()
        .filter(|field| meta.state_vars.iter().any(|name| name == &field.key))
        .map(|field| {
            format!(
                "{}: {}",
                field.key,
                rewrite_tokens(&written[field.start..field.end], Binding::Accessor)
            )
        })
        .collect();
    format!("{{ {} }}", wanted.join(", "))
}

/// Which of the reference's two `@Link` spellings a params object is written in.
///
/// `createReference` (`process_component_class.js:453`) decides it, and `addDoubleUnderline`
/// (`:471`) applies it: the initializer of a property whose argument names a `@Link` field of
/// the enclosing struct becomes `this.<name>` when the caller passes the `i` flag
/// (`i || o` in `:472`), and `this.__<name>` when it does not. The two call sites pass
/// different flags, which is why the same member is written two ways in one construction:
///
/// * `componentParamDetachment` builds the constructor's arguments — the backstore form
///   (`this.__x`), because the child's `SynchedProperty…TwoWayPU` is handed the parent's
///   observed property object itself;
/// * `splitComponentParams` (`process_custom_component.js:434`), called from
///   `assignComponentParams` with `r=true`, reaches `createReference(e, [], t, r)` so the `i`
///   flag is set — the accessor form (`this.x`), because the `paramsLambda` body re-reads the
///   member when the child is re-bound.
///
/// A `@CustomDialog` construction calls `assignComponentParams(e)` with no flags
/// (`process_ui_syntax.js:259`), so both of its objects take the backstore form.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Binding {
    /// `this.__field`
    Backstore,
    /// `this.field`
    Accessor,
}

/// A child component's parameters are read from the parent: `this.count` written as a
/// constructor argument is `this.__count`, and a `$field` binding follows the same rule.
pub(crate) fn rewrite_backstores(text: &str) -> String {
    rewrite_bindings(text, Binding::Backstore, &[])
}

/// One params object, or one expression, read in the form the target side needs.
///
/// A `@Link` member is the one member whose argument is not the expression the source wrote:
/// the two-way link reads and writes the parent's observed property, so the constructor takes
/// `this.__field` for `$field` and for `this.field` alike (measured 2026-11-06:
/// `fixtures/link-call-site` emits `{ linked: this.__linked }` for both spellings, and
/// `SynchedPropertySimpleTwoWayPU` throws `undefined is not callable` when it is handed the
/// value instead).
///
/// The rewrite replaces the argument of the members it has to re-point and keeps every other
/// byte as written: a params object carries the author's comments and line breaks, and
/// re-printing it would put a trailing `//` comment in front of the closing brace.
pub(crate) fn rewrite_bindings(text: &str, binding: Binding, link_fields: &[String]) -> String {
    if link_fields.is_empty() {
        return rewrite_tokens(text, binding);
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut links: Vec<(usize, usize)> = Vec::new();
    for field in fields_of(text) {
        if !link_fields.iter().any(|name| name == &field.key) {
            continue;
        }
        let Some(source) = link_source(&text[field.start..field.end], binding) else {
            continue;
        };
        edits.push((field.start, field.end, source));
        links.push((field.start, field.end));
    }
    for (start, end, replacement) in token_edits(text, binding) {
        // A link member's argument is already settled; the token form would target the same
        // span with the value's spelling.
        if links.iter().any(|(from, to)| start >= *from && end <= *to) {
            continue;
        }
        edits.push((start, end, replacement));
    }
    crate::build::ets_assemble::apply_replacements(text, &edits)
}

/// One field of a params object: its key, and the byte range of its argument.
pub(crate) struct FieldSpan {
    key: String,
    start: usize,
    end: usize,
}

/// The top-level fields of one params object in `text`.
///
/// A piece is a field only when it is written `key: value`: shorthand, spread and comment-only
/// pieces carry no argument to re-point and are skipped.
///
/// The object may arrive without its braces: `child_params` splits the written object into its
/// pieces and hands the inner list back to [`rewrite_bindings`], which then has to read the same
/// fields from it. Without this, a `@BuilderParam` child — the one shape that goes through that
/// split — silently lost the `@Link` rewrite (`renderType: this.renderType` stayed the accessor
/// where the reference writes `this.__renderType`, measured on `@mcui/mccharts` `Chart`).
pub(crate) fn fields_of(text: &str) -> Vec<FieldSpan> {
    let tokens = crate::build::lex::tokenize(text);
    let (inner_start, close) = match tokens.first().filter(|token| token.is_punct('{')) {
        Some(open) => {
            let Some(close) = crate::build::lex::matching_delimiter(text, open.start) else {
                return Vec::new();
            };
            (open.end, close)
        }
        // No braces to strip: the call already hands over the list itself.
        None => (0, text.len()),
    };
    let mut fields = Vec::new();
    let mut depth = 0i32;
    let mut start = inner_start;
    for token in tokens
        .iter()
        .filter(|token| token.start >= inner_start && token.end <= close)
    {
        if token.kind != crate::build::lex::Kind::Punct {
            continue;
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => {
                if let Some(field) = field_span(text, start, token.start) {
                    fields.push(field);
                }
                start = token.end;
            }
            _ => {}
        }
    }
    if let Some(field) = field_span(text, start, close) {
        fields.push(field);
    }
    fields
}

/// The key and argument range of the field written between `start` and `end`.
pub(crate) fn field_span(text: &str, start: usize, end: usize) -> Option<FieldSpan> {
    let mut tokens = crate::build::lex::tokenize(&text[start..end])
        .into_iter()
        .filter(|token| token.kind != crate::build::lex::Kind::Comment);
    let key = tokens.next()?;
    let is_key = key
        .text
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    if !is_key {
        return None;
    }
    let separator = tokens.next()?;
    if !separator.is_punct(':') {
        return None;
    }
    let mut argument_start = None;
    let mut argument_end = 0usize;
    for token in tokens {
        argument_start.get_or_insert(token.start);
        argument_end = token.end;
    }
    Some(FieldSpan {
        key: key.text.to_string(),
        start: argument_start? + start,
        end: argument_end + start,
    })
}

/// The parent property a `@Link` argument names, in the form the target side takes.
///
/// `createReference` (`process_component_class.js:456-462`) recognises three spellings of the
/// same argument and reduces each to the property name: `$x` (an identifier starting with `$`),
/// `this.$x` (`n.name.escapedText.toString().match(/^\$/)`, then `replace(/^\$/g, "")`), and
/// `this.x` when `x` is one of the enclosing struct's `linkCollection` members
/// (`isMatchInitExpression`, `:467`). The `$` is a marker on the reference, not part of the
/// member name, so `this.$x` names the same property `this.x` does (measured 2026-11-07:
/// `fixtures/link-dollar-arg` emits `{ total: this.__count }` for both spellings). Anything else —
/// `$r('app.string.x')`, an expression whose right-hand side is not an identifier — is not a
/// property reference, and `None` keeps it as written instead of inventing a backstore for it.
pub(crate) fn link_source(value: &str, binding: Binding) -> Option<String> {
    let name = match value.strip_prefix('$') {
        Some(rest) if !rest.starts_with('$') => rest,
        // `this.x` and `this.$x` differ only by the reference marker.
        _ => {
            let accessor = value.strip_prefix("this.")?;
            accessor.strip_prefix('$').unwrap_or(accessor)
        }
    };
    let is_property = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if !is_property {
        return None;
    }
    Some(match binding {
        Binding::Backstore => format!("this.__{name}"),
        Binding::Accessor => format!("this.{name}"),
    })
}

/// `$field` arguments as replacement edits.
///
/// Only a `$field` argument is rewritten: a plain `this.field` already reads what the child
/// needs — `@Prop` copies the value, so the accessor is the right expression, and the backstore
/// is only reached through a two-way binding.
pub(crate) fn token_edits(text: &str, binding: Binding) -> Vec<(usize, usize, String)> {
    let tokens = crate::build::lex::tokenize(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for token in &tokens {
        if token.kind != crate::build::lex::Kind::Ident {
            continue;
        }
        let Some(name) = token.text.strip_prefix('$') else {
            continue;
        };
        if name.starts_with('$') || !crate::build::ets_compile::is_v1_field(name) {
            continue;
        }
        let replacement = match binding {
            Binding::Backstore => format!("this.__{name}"),
            Binding::Accessor => format!("this.{name}"),
        };
        edits.push((token.start, token.end, replacement));
    }
    edits
}

pub(crate) fn rewrite_tokens(text: &str, binding: Binding) -> String {
    crate::build::ets_assemble::apply_replacements(text, &token_edits(text, binding))
}

/// A builder call whose only argument is an object literal takes the reference's proxy form.
///
/// The reference writes the object one level deeper: each top-level value becomes a thunk
/// (`a: 1` → `a: () => 1`), and a value that is itself an object literal is parenthesised
/// (`b: {…}` → `b: () => ({…})`) with the values inside it untouched
/// (`transDoubleDollarInCustomBuilder`; the shape is measured in `fixtures/builder-param-proxy`).
/// `builder` is the callee's bare name, so `this.memberContent(…)` passes `memberContent`.
///
/// Every shape the scan does not model returns `None` — a shorthand member, a spread, a
/// template literal, an unbalanced object — so the caller keeps the argument as written.
///
pub(crate) fn builder_param_proxy(builder: &str, arguments: &str) -> Option<String> {
    let text = arguments.trim();
    let (spans, object_start, object_end) = object_member_spans(text)?;
    if object_start != 0 || object_end != text.len() {
        return None;
    }
    // The members keep the separators the source wrote between them: the reference edits the
    // object in place, so a member that starts on its own line stays there and a member written
    // on the same line stays on it. Nothing about the layout is re-decided here.
    let mut body = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start, end) in spans {
        let member = text.get(start..end)?.trim();
        if member.is_empty() {
            continue;
        }
        body.push_str(text.get(cursor..start)?);
        body.push_str(&thunked_member(member)?);
        cursor = end;
    }
    body.push_str(text.get(cursor..)?);
    Some(format!("makeBuilderParameterProxy(\"{builder}\", {body})"))
}

/// The spans [`object_member_spans`] reports: every member's, then the object's own.
type ObjectMemberSpans = (Vec<(usize, usize)>, usize, usize);

/// The members of the one object literal a builder call passes, as spans into `text`.
///
/// `transDoubleDollarInCustomBuilder` reads the call's own arguments, and only a call whose
/// single argument is an object literal takes the proxy form; the parser is what says so.
/// `None` when the text is not that shape or a member is not a plain `key: value` property.
fn object_member_spans(text: &str) -> Option<ObjectMemberSpans> {
    const WRAPPER: &str = "f(";
    let wrapped = format!("{WRAPPER}{text})");
    let allocator = oxc_allocator::Allocator::default();
    let ret = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    let statement = ret.program.body.first()?;
    let oxc_ast::ast::Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let oxc_ast::ast::Expression::CallExpression(call) = &statement.expression else {
        return None;
    };
    let [oxc_ast::ast::Argument::ObjectExpression(object)] = call.arguments.as_slice() else {
        return None;
    };
    let shift = WRAPPER.len();
    let mut spans = Vec::new();
    for property in &object.properties {
        let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        spans.push((
            (property.span.start as usize).checked_sub(shift)?,
            (property.span.end as usize).checked_sub(shift)?,
        ));
    }
    Some((
        spans,
        (object.span.start as usize).checked_sub(shift)?,
        (object.span.end as usize).checked_sub(shift)?,
    ))
}

/// Rewrite one `key: value` member into `key: () => value`.
fn thunked_member(member: &str) -> Option<String> {
    let colon = *top_level_positions(member, ':')?.first()?;
    let key = member[..colon].trim();
    let value = member[colon + 1..].trim();
    if key.is_empty() || value.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    // A read of a state-decorated member inside the proxy object takes the backstore fallback:
    // the proxy may be invoked with another receiver, so the field goes through the accessor
    // the struct already holds (measured: `fixtures/builder-param-proxy`).
    if let Some(name) = value.strip_prefix("this.") {
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            if let Some(read) =
                crate::build::stage::process_component_build::observed_backstore_read(name)
            {
                return Some(format!("{key}: () => ({read})"));
            }
        }
    }
    if needs_parenthesised_value(value) {
        // An object literal has to be parenthesised, or the arrow would read the braces as its
        // body and the object would stop being the value. A resource call is parenthesised for
        // the same reason: the id substitution later in the build turns it into an object.
        Some(format!("{key}: () => ({value})"))
    } else {
        Some(format!("{key}: () => {value}"))
    }
}

/// Whether a thunked value has to be parenthesised to stay a value.
fn needs_parenthesised_value(value: &str) -> bool {
    value.starts_with('{')
        || value.starts_with("$r(")
        || value.starts_with("$rawfile(")
        || value.starts_with("$media(")
}

/// Offsets of `needle` at the text's own level, skipping strings and bracketed spans.
///
/// `None` when the scan cannot be trusted: an unterminated string, a template literal (its
/// `${…}` would need a second scanner), or brackets that do not balance.
fn top_level_positions(text: &str, needle: char) -> Option<Vec<usize>> {
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut i = 0usize;
    while let Some(c) = text[i..].chars().next() {
        match c {
            '\'' | '"' => i = skip_string(text, i + 1, c)?,
            // A comment is prose: a comma, a brace or a backtick written there is not structure.
            '/' if text[i..].starts_with("//") => {
                i = text[i..].find('\n').map_or(text.len(), |end| i + end + 1);
            }
            '/' if text[i..].starts_with("/*") => {
                let end = text[i + 2..].find("*/")?;
                i += 2 + end + 2;
            }
            '`' => return None,
            '(' | '[' | '{' => {
                depth += 1;
                i += 1;
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                i += 1;
            }
            _ if c == needle && depth == 0 => {
                found.push(i);
                i += 1;
            }
            _ => i += c.len_utf8(),
        }
    }
    if depth != 0 {
        return None;
    }
    Some(found)
}

/// The offset just past the quote that closes the string opening before `from`.
fn skip_string(text: &str, from: usize, quote: char) -> Option<usize> {
    let mut i = from;
    while let Some(c) = text[i..].chars().next() {
        if c == '\\' {
            i += 1;
            i += text[i..].chars().next()?.len_utf8();
            continue;
        }
        if c == quote {
            return Some(i + 1);
        }
        i += c.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{builder_param_proxy, link_source, rewrite_bindings, Binding};

    /// The inner field list of a params object, handed over without its braces, still takes the
    /// `@Link` rewrite: `child_params` splits the written object that way for a `@BuilderParam`
    /// child, and the reference re-points the member there as well (`process_component_class.js`
    /// `createReference`).
    #[test]
    fn a_brace_less_field_list_keeps_the_link_rewrite() {
        let link = vec!["renderType".to_string()];
        assert_eq!(
            rewrite_bindings("renderType: this.renderType", Binding::Backstore, &link),
            "renderType: this.__renderType"
        );
        assert_eq!(
            rewrite_bindings(
                "customTooltip: this.customTooltip, renderType: this.renderType",
                Binding::Backstore,
                &link
            ),
            "customTooltip: this.customTooltip, renderType: this.__renderType"
        );
    }

    /// The params lambda reads the same member through its accessor, braces or not.
    #[test]
    fn a_brace_less_field_list_keeps_the_accessor_form() {
        let link = vec!["renderType".to_string()];
        assert_eq!(
            rewrite_bindings("renderType: this.renderType", Binding::Accessor, &link),
            "renderType: this.renderType"
        );
    }

    /// A value that is not a `key: value` piece has no field to re-point, with or without braces.
    #[test]
    fn a_bare_expression_is_not_a_field_list() {
        let link = vec!["renderType".to_string()];
        assert_eq!(
            rewrite_bindings("someParams", Binding::Backstore, &link),
            "someParams"
        );
        assert_eq!(
            rewrite_bindings("cond ? a : b", Binding::Backstore, &link),
            "cond ? a : b"
        );
    }

    /// `createReference` reduces all three spellings of the argument to the member name, so the
    /// `$` in `this.$x` is a marker that never reaches the emitted accessor.
    #[test]
    fn the_three_link_spellings_name_the_same_property() {
        assert_eq!(
            link_source("$count", Binding::Backstore),
            Some("this.__count".to_string())
        );
        assert_eq!(
            link_source("this.$count", Binding::Backstore),
            Some("this.__count".to_string())
        );
        assert_eq!(
            link_source("this.count", Binding::Accessor),
            Some("this.count".to_string())
        );
        assert_eq!(link_source("$r('app.string.x')", Binding::Backstore), None);
        assert_eq!(link_source("$$count", Binding::Backstore), None);
    }

    /// A lone object literal becomes the proxy call, and every top-level value is thunked.
    #[test]
    fn a_lone_object_literal_takes_the_proxy_form() {
        assert_eq!(
            builder_param_proxy("globalContent", "{ a: 1, b: null }").as_deref(),
            Some(r#"makeBuilderParameterProxy("globalContent", { a: () => 1, b: () => null })"#)
        );
    }

    /// A value that is itself an object literal is parenthesised, and the values inside it are
    /// not thunked a second time.
    #[test]
    fn a_nested_object_literal_is_parenthesised() {
        assert_eq!(
            builder_param_proxy("globalContent", "{ a: 1, b: { a: 2 } }").as_deref(),
            Some(
                r#"makeBuilderParameterProxy("globalContent", { a: () => 1, b: () => ({ a: 2 }) })"#
            )
        );
    }

    /// An arrow written as a value is thunked as a whole, so the emitted value takes one more
    /// level: `run: () => {}` reads `run: () => () => {}`.
    #[test]
    fn an_arrow_value_is_thunked_as_a_whole() {
        assert_eq!(
            builder_param_proxy("memberContent", "{ run: () => {} }").as_deref(),
            Some(r#"makeBuilderParameterProxy("memberContent", { run: () => () => {} })"#)
        );
    }

    /// A comma inside a string is part of the value, not a member separator.
    #[test]
    fn a_comma_inside_a_string_does_not_split() {
        assert_eq!(
            builder_param_proxy("b", "{ a: 'x,y', c: 2 }").as_deref(),
            Some(r#"makeBuilderParameterProxy("b", { a: () => 'x,y', c: () => 2 })"#)
        );
    }

    /// A comma inside a nested call or array is below the object's own level, so it does not
    /// split either.
    #[test]
    fn a_comma_inside_a_nested_call_does_not_split() {
        assert_eq!(
            builder_param_proxy("b", "{ a: f(1, 2), c: [3, 4] }").as_deref(),
            Some(r#"makeBuilderParameterProxy("b", { a: () => f(1, 2), c: () => [3, 4] })"#)
        );
    }

    /// Anything that is not a lone object literal keeps the call as written: the reference
    /// forwards a constant, a reference and a literal untouched.
    #[test]
    fn a_non_object_argument_is_left_alone() {
        assert_eq!(builder_param_proxy("globalContent", "SHARED_CFG"), None);
        assert_eq!(builder_param_proxy("globalContent", "this.shared"), None);
        assert_eq!(builder_param_proxy("globalContent", "2"), None);
        assert_eq!(builder_param_proxy("globalContent", "[1, 2]"), None);
    }

    /// The members keep the separators the source wrote: `transDoubleDollarInCustomBuilder`
    /// edits the object in place, so a multi-line object stays multi-line and a single-line one
    /// stays on its line.
    #[test]
    fn proxy_members_keep_the_source_line_breaks() {
        assert_eq!(
            builder_param_proxy("b", "{\n  a: 1,\n  b: 2\n}").as_deref(),
            Some("makeBuilderParameterProxy(\"b\", {\n  a: () => 1,\n  b: () => 2\n})")
        );
        assert_eq!(
            builder_param_proxy("b", "{ a: 1, b: 2 }").as_deref(),
            Some("makeBuilderParameterProxy(\"b\", { a: () => 1, b: () => 2 })")
        );
    }

    /// An empty object literal is still an object literal, so it takes the proxy with no member.
    #[test]
    fn an_empty_object_literal_still_takes_the_proxy() {
        assert_eq!(
            builder_param_proxy("b", "{}").as_deref(),
            Some(r#"makeBuilderParameterProxy("b", {})"#)
        );
    }

    /// A shorthand member has no value to thunk, and a spread is not a `key: value` pair, so the
    /// scan gives up and the caller keeps the argument as written.
    #[test]
    fn a_shorthand_member_is_left_alone() {
        assert_eq!(builder_param_proxy("b", "{ a }"), None);
        assert_eq!(builder_param_proxy("b", "{ ...rest }"), None);
        assert_eq!(builder_param_proxy("b", "{ a: `t` }"), None);
    }
}
