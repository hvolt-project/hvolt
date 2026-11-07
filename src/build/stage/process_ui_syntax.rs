// process stage: `process_ui_syntax.js` — the file entry: the UI-syntax declarations, the
// assembly order and the `@CustomDialog` construction.
//
// `processUISyntax(sourceText, isEntry, …)` (`process_ui_syntax.js:52`) is the transformer the
// loader registers per file. It owns three things the other stages do not:
//
//   1. the declaration pass — `collectExtend` / `parseStylesNode` / `isExtendFunction` /
//      `globalBuilderParamAssignment` read the `@Extend` / `@Styles` / `@Builder` declarations
//      and their `.attribute()` bodies before anything parses them ([`mask`] below keeps that
//      shape);
//   2. the `@CustomDialog` construction — `createCustomDialogController` (`:219`) appends the
//      owning `this` to the controller's argument list, and
//      `createCustomComponentBuilderArrowFunction` (`:254`) builds the arrow function a
//      controller's `builder:` field receives:
//
//          () => {
//            let jsDialog = new <Cmp>(…);      // JS_DIALOG + createCustomComponentNewExpression
//            jsDialog.setController(<owner>);  // SET_CONTROLLER_METHOD
//            <ViewPU|ViewV2>.create(jsDialog); // createViewCreate
//            let paramsLambda = () => {…};     // assignComponentParams
//            jsDialog.paramsGenerator_ = paramsLambda; // assignmentFunction
//          }
//
//   3. the order the file's statements are emitted in.
//
// `process_component_member.js` holds the *class-side* half of the dialog rule
// ([`super::process_component_member::emit_controller_set`]): this stage builds the call site,
// that stage builds the setter the call site assigns through.

use crate::build::ets::build_body;
use crate::build::ets::{Builder, ExtendFn, StyleFn};
use crate::build::lex::{matching_delimiter, split_top_level_commas, tokenize, Kind};
use crate::build::stage::pre_define;

/// What one masking pass found in a file.
pub(crate) struct Masked {
    /// The source with every unparsable declaration replaced by spaces.
    pub(crate) source: String,
    /// `@Builder` function names: a call to one takes a child block.
    pub(crate) builders: Vec<String>,
    /// `@Builder` / `@LocalBuilder` methods declared inside a struct.
    pub(crate) locals: Vec<String>,
    pub(crate) extends: Vec<ExtendFn>,
    pub(crate) styles: Vec<StyleFn>,
    /// `@AnimatableExtend(Comp) function name(…)` declarations as `(component, property)`.
    /// The attribute list emits a call site for one of these as a bare
    /// `name(args, elmtId, isInitialRender, this)` call, never as `Comp.name(…)`.
    pub(crate) animatable_extends: Vec<(String, String)>,
}

/// Mask the decorators a parser rejects, collecting what the emitters need.
pub(crate) fn mask_builder_decorators(source: &str) -> Masked {
    mask(source)
}

/// As [`mask_builder_decorators`]; the name says the caller also wants the struct-local
/// builder names, which are always collected.
pub(crate) fn mask_builder_decorators_with_locals(source: &str) -> Masked {
    mask(source)
}

fn mask(source: &str) -> Masked {
    let tokens = tokenize(source);
    let class_bodies = class_body_ranges(source, &tokens);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut builders = Vec::new();
    let mut locals = Vec::new();
    let mut extends = Vec::new();
    let mut styles = Vec::new();
    let mut animatable_extends: Vec<(String, String)> = Vec::new();

    let mut index = 0usize;
    while index < tokens.len() {
        if !tokens[index].is_punct('@') {
            index += 1;
            continue;
        }
        let Some(name) = tokens.get(index + 1) else {
            break;
        };
        if name.kind != Kind::Ident {
            index += 1;
            continue;
        }
        // The decorator, with its argument list when it has one.
        let mut decorator_end = name.end;
        let mut argument: Option<&str> = None;
        let mut after = index + 2;
        if let Some(open) = tokens.get(after).filter(|t| t.is_punct('(')) {
            if let Some(close) = matching_delimiter(source, open.start) {
                argument = Some(source[open.end..close].trim());
                decorator_end = close + 1;
                after = tokens
                    .iter()
                    .position(|t| t.start > close)
                    .unwrap_or(tokens.len());
            }
        }
        match name.text {
            "Extend" | "Styles" => {
                let Some(function) = tokens.get(after) else {
                    index = after;
                    continue;
                };
                if function.kind != Kind::Ident || function.text != "function" {
                    // A `@Styles`/`@Extend` *method* of a struct: the body is the same
                    // attribute chain, so it is unparsable for the same reason and is
                    // blanked the same way.
                    if let Some((member_end, member_name, member_params, member_body)) =
                        read_member(source, &tokens, after)
                    {
                        let chain = parse_attribute_chain(member_body);
                        if name.text == "Extend" {
                            extends.push(ExtendFn {
                                name: member_name,
                                comp: argument.unwrap_or_default().to_string(),
                                params_text: member_params,
                                body: chain,
                                // Both read the source here; the reader that parses the
                                // widened text translates `order` and keeps `source_order`
                                // for the comment scan (`ets_component::read_component_file`).
                                order: tokens[index].start,
                                source_order: tokens[index].start,
                            });
                        } else {
                            // A member `@Styles` belongs to its struct and must not answer
                            // another component's call of the same name
                            // (measured: fixtures/reusable-guard-boundary, `StylesScopeProbe`
                            // expands its own `cardStyle` 11/12, not `StyleProbe`'s 40/20/5).
                            styles.push(StyleFn {
                                comp: class_bodies
                                    .iter()
                                    .find(|(_, open, end)| {
                                        tokens[index].start > *open && tokens[index].start < *end
                                    })
                                    .map(|(name, _, _)| name.clone())
                                    .unwrap_or_default(),
                                name: member_name,
                                body: chain,
                            });
                        }
                        edits.push((
                            tokens[index].start,
                            member_end,
                            blank_of(source, tokens[index].start, member_end),
                        ));
                    }
                    index = after;
                    continue;
                }
                let Some((declaration_end, function_name, params_text, body)) =
                    read_function(source, &tokens, after)
                else {
                    index = after;
                    continue;
                };
                let chain = parse_attribute_chain(body);
                if name.text == "Extend" {
                    extends.push(ExtendFn {
                        name: function_name,
                        comp: argument.unwrap_or_default().to_string(),
                        params_text,
                        body: chain,
                        // See the member case above: the reader translates `order` into the
                        // coordinates of the text the parser reads.
                        order: tokens[index].start,
                        source_order: tokens[index].start,
                    });
                } else {
                    // A top-level `@Styles function` is visible to every component in the
                    // file, so it is filed under no owner (measured: `GlobalOnlyProbe`
                    // expands the global body 98/99).
                    styles.push(StyleFn {
                        comp: String::new(),
                        name: function_name,
                        body: chain,
                    });
                }
                edits.push((
                    tokens[index].start,
                    declaration_end,
                    blank_of(source, tokens[index].start, declaration_end),
                ));
                index = after;
                while index < tokens.len() && tokens[index].start < declaration_end {
                    index += 1;
                }
            }
            "Builder" | "LocalBuilder" => {
                // Only the decorator is blanked: a `@Builder` function's body is ordinary
                // statements for the parser (its child blocks were rewritten already), and the
                // component reader needs the declaration to record the body it emits.
                // A struct member keeps its decorator for the same reader and is only named
                // here.
                let inside_class = class_bodies.iter().any(|(_, start, end)| {
                    tokens[index].start > *start && tokens[index].start < *end
                });
                if inside_class {
                    if let Some(member) = struct_member_name(&tokens, after) {
                        if !locals.contains(&member) {
                            locals.push(member);
                        }
                    }
                } else if let Some(function) = tokens.get(after) {
                    // `@Builder function name` and `@Builder export function name`.
                    let name_at = if function.kind == Kind::Ident && function.text == "function" {
                        after + 1
                    } else if function.kind == Kind::Ident && function.text == "export" {
                        after + 2
                    } else {
                        after + 1
                    };
                    if let Some(name) = tokens.get(name_at) {
                        if name.kind == Kind::Ident && !builders.contains(&name.text.to_string()) {
                            builders.push(name.text.to_string());
                        }
                    }
                    edits.push((
                        tokens[index].start,
                        decorator_end,
                        blank_of(source, tokens[index].start, decorator_end),
                    ));
                } else {
                    edits.push((
                        tokens[index].start,
                        decorator_end,
                        blank_of(source, tokens[index].start, decorator_end),
                    ));
                }
                index = after;
            }
            "AnimatableExtend" => {
                // `processAnimatableExtend` keeps the function name, appends `elmtId`,
                // `isInitialRender` and `parent`, and builds the body from the attribute
                // statement. The declaration cannot reach a parser as written (its body is a
                // leading-dot chain), so the whole declaration is replaced by the generated
                // function: the text that follows is ordinary TypeScript, and the statement
                // printer carries it through.
                match read_declaration(source, &tokens, after) {
                    Some((exported, declaration_end, function_name, params_text, body))
                        if argument.is_some_and(|comp| !comp.is_empty()) =>
                    {
                        let comp = argument.unwrap_or_default();
                        if !animatable_extends.iter().any(|(component, property)| {
                            component == comp && property == &function_name
                        }) {
                            animatable_extends.push((comp.to_string(), function_name.clone()));
                        }
                        edits.push((
                            tokens[index].start,
                            declaration_end,
                            emit_animatable_extend(
                                exported,
                                comp,
                                &function_name,
                                &params_text,
                                body,
                            ),
                        ));
                    }
                    _ => {}
                }
                index = after;
            }
            "Concurrent" => {
                // `@Concurrent` marks a function the task pool may run elsewhere; it changes
                // neither the declaration nor a call, and TypeScript accepts no decorator on a
                // function declaration at all. So only the decorator is blanked, in every
                // position it can take: a top-level `function`, a struct/class method, and an
                // arrow-function variable. The declaration that follows is untouched and the
                // emitter carries it over as written.
                edits.push((
                    tokens[index].start,
                    decorator_end,
                    blank_of(source, tokens[index].start, decorator_end),
                ));
                // `processConcurrent` (`process_ui_syntax.js`) prepends the `"use concurrent"`
                // directive to a function *declaration*'s body. A struct method or an
                // arrow-function variable takes no directive: the reference reaches them
                // through paths that never call `processConcurrent`.
                let inside_class = class_bodies.iter().any(|(_, start, end)| {
                    tokens[index].start > *start && tokens[index].start < *end
                });
                if !inside_class {
                    if let Some(brace) = function_body_brace(source, &tokens, after) {
                        edits.push((
                            tokens[brace].start + 1,
                            tokens[brace].start + 1,
                            format!("\n    \"{}\";", pre_define::CONCURRENT_DIRECTIVE),
                        ));
                    }
                }
                index = after;
            }
            _ => {
                index = after;
            }
        }
    }
    // The same walk also rewrites two call shapes `processUISyntax` handles statement by
    // statement: a bare `animateTo(…)` / `animateToImmediately(…)` call, and the script URL of
    // a `new <namespace>.Worker(…)`.
    edits.extend(call_site_rewrites(source, &tokens));
    Masked {
        source: crate::build::ets_assemble::apply_replacements(source, &edits),
        builders,
        locals,
        extends,
        styles,
        animatable_extends,
    }
}

// The `@AnimatableExtend` table of the file currently being lowered.
//
// `process_component_build::emit_attribute_list` receives the `@Extend` table through its
// callers but never the file itself, and the attribute list is the one place that has to tell
// an animatable call site from an instance attribute write. The build lowers one file at a
// time and the table is published when that file's `ComponentFile` is built
// (`read_component_file`), immediately before the file is emitted, so a thread-local is the
// narrowest place to carry it without threading a fourth table through every emit function.
thread_local! {
    static ANIMATABLE_EXTENDS: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Publish one file's `@AnimatableExtend` declarations as `(component, property)` pairs.
pub(crate) fn set_animatable_extends(entries: &[(String, String)]) {
    ANIMATABLE_EXTENDS.with(|cell| {
        let mut table = cell.borrow_mut();
        table.clear();
        table.extend_from_slice(entries);
    });
}

/// Whether `(component, property)` came from an `@AnimatableExtend` declaration.
pub(crate) fn is_animatable_extend(component: &str, property: &str) -> bool {
    ANIMATABLE_EXTENDS.with(|cell| {
        cell.borrow()
            .iter()
            .any(|(comp, prop)| comp == component && prop == property)
    })
}

/// The byte ranges of every `class { … }` body, outermost only.
fn class_body_ranges(
    source: &str,
    tokens: &[crate::build::lex::Token<'_>],
) -> Vec<(String, usize, usize)> {
    let mut ranges = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        // The masking pass runs before `struct` is spelled `class`, so both declare a body.
        if token.kind != Kind::Ident || !matches!(token.text, "class" | "struct") {
            continue;
        }
        // The name is the identifier right after the keyword (`struct Foo` / `struct Foo
        // extends Bar`); it is what a `@Styles` member is filed under.
        let name = tokens
            .get(index + 1)
            .filter(|next| next.kind == Kind::Ident)
            .map(|next| next.text.to_string())
            .unwrap_or_default();
        let Some(open) = tokens.iter().skip(index + 1).find(|t| t.is_punct('{')) else {
            continue;
        };
        let Some(end) = matching_delimiter(source, open.start) else {
            continue;
        };
        ranges.push((name, open.start, end));
    }
    ranges
}

/// The name of the member a pair of decorators belongs to, skipping further decorators.
fn struct_member_name(tokens: &[crate::build::lex::Token<'_>], mut index: usize) -> Option<String> {
    while index < tokens.len() {
        let token = &tokens[index];
        if token.is_punct('@') {
            index += 2;
            if tokens.get(index).is_some_and(|t| t.is_punct('(')) {
                index += 1;
                let mut depth = 1;
                while index < tokens.len() && depth > 0 {
                    if tokens[index].is_punct('(') {
                        depth += 1;
                    } else if tokens[index].is_punct(')') {
                        depth -= 1;
                    }
                    index += 1;
                }
            }
            continue;
        }
        if matches!(
            token.text,
            "private" | "public" | "protected" | "readonly" | "static"
        ) {
            index += 1;
            continue;
        }
        return (token.kind == Kind::Ident).then(|| token.text.to_string());
    }
    None
}

/// The `function` declaration after a decorator, with the `export` modifier it may carry.
fn read_declaration<'a>(
    source: &'a str,
    tokens: &[crate::build::lex::Token<'a>],
    mut at: usize,
) -> Option<(bool, usize, String, String, &'a str)> {
    let mut exported = false;
    loop {
        let token = tokens.get(at)?;
        if token.kind != Kind::Ident {
            return None;
        }
        match token.text {
            "function" => break,
            "export" => {
                exported = true;
                at += 1;
            }
            "async" | "default" => at += 1,
            _ => return None,
        }
    }
    let (end, name, params_text, body) = read_function(source, tokens, at)?;
    Some((exported, end, name, params_text, body))
}

/// The `function` declaration that starts at `at`, read as a whole.
fn read_function<'a>(
    source: &'a str,
    tokens: &[crate::build::lex::Token<'a>],
    at: usize,
) -> Option<(usize, String, String, &'a str)> {
    let function = tokens.get(at)?;
    let name = tokens.get(at + 1)?;
    if name.kind != Kind::Ident {
        return None;
    }
    let open = tokens.iter().skip(at + 2).find(|t| t.is_punct('('))?;
    let close = matching_delimiter(source, open.start)?;
    let brace = tokens.iter().find(|t| t.start > close)?;
    if !brace.is_punct('{') {
        return None;
    }
    let body_end = matching_delimiter(source, brace.start)?;
    let params_text = source[open.end..close].trim().to_string();
    let body = &source[brace.end..body_end];
    (function.kind == Kind::Ident).then(|| (body_end + 1, name.text.to_string(), params_text, body))
}

/// The method declaration that starts at `at`, read as a whole.
fn read_member<'a>(
    source: &'a str,
    tokens: &[crate::build::lex::Token<'a>],
    at: usize,
) -> Option<(usize, String, String, &'a str)> {
    let name = tokens.get(at)?;
    if name.kind != Kind::Ident {
        return None;
    }
    let open = tokens.iter().skip(at + 1).find(|t| t.is_punct('('))?;
    let close = matching_delimiter(source, open.start)?;
    let brace = tokens.iter().find(|t| t.start > close)?;
    if !brace.is_punct('{') {
        return None;
    }
    let body_end = matching_delimiter(source, brace.start)?;
    Some((
        body_end + 1,
        name.text.to_string(),
        source[open.end..close].trim().to_string(),
        &source[brace.end..body_end],
    ))
}

/// `.fontSize(12).fontFamily('mono')` → `[("fontSize", ["12"]), ("fontFamily", ["'mono'"])]`.
fn parse_attribute_chain(body: &str) -> Vec<(String, Vec<String>)> {
    let tokens = tokenize(body);
    let mut chain = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if !tokens[index].is_punct('.') {
            index += 1;
            continue;
        }
        let Some(name) = tokens.get(index + 1) else {
            break;
        };
        if name.kind != Kind::Ident {
            index += 1;
            continue;
        }
        let Some(open) = tokens.get(index + 2).filter(|t| t.is_punct('(')) else {
            index += 1;
            continue;
        };
        let Some(close) = matching_delimiter(body, open.start) else {
            break;
        };
        let args = split_top_level_commas(&body[open.end..close]);
        chain.push((
            name.text.to_string(),
            args.into_iter()
                .map(|arg| arg.trim().to_string())
                .filter(|arg| !arg.is_empty())
                .collect(),
        ));
        index = tokens
            .iter()
            .position(|t| t.start > close)
            .unwrap_or(tokens.len());
    }
    chain
}

/// The blank that stands in for a masked declaration between `start` and `end`.
///
/// Spaces of the same width hold every offset where it was, and the line breaks stay line
/// breaks: the masked text is the coordinate system the rest of the lowering reads positions
/// from (`build_body_line`, `build_body::parse`'s node lines, `line_column`), so a declaration
/// masked over several lines must not shift the lines below it. A `@Extend(Row)` written with
/// its attribute chain on eight lines erased seven line breaks, and every later position in
/// clashbox's `entry/components/Common/Common.ets` came out seven lines short (measured
/// 2026-11-07).
fn blank_of(source: &str, start: usize, end: usize) -> String {
    let end = end.min(source.len());
    let start = start.min(end);
    // Byte for byte, so a multi-byte character in the masked declaration keeps its width: the
    // replacement is read by offsets that were measured on the source.
    let bytes: Vec<u8> = source.as_bytes()[start..end]
        .iter()
        .map(|byte| match byte {
            b'\n' | b'\r' => *byte,
            _ => b' ',
        })
        .collect();
    String::from_utf8(bytes).expect("a blank of spaces and line breaks")
}

/// The `{` token that opens the body of the `function` declaration after a decorator.
///
/// `processConcurrent` runs only on a `FunctionDeclaration`; the modifiers sit between the
/// decorator and the keyword (`export`), and the body is the first `{` after the parameter
/// list. A decorator followed by a method, a variable or a class name returns `None`.
fn function_body_brace(
    source: &str,
    tokens: &[crate::build::lex::Token<'_>],
    mut at: usize,
) -> Option<usize> {
    while let Some(token) = tokens.get(at) {
        if token.kind == Kind::Ident {
            match token.text {
                "function" => break,
                "export" | "default" | "async" => {
                    at += 1;
                    continue;
                }
                _ => return None,
            }
        }
        return None;
    }
    let open = tokens.iter().skip(at).find(|t| t.is_punct('('))?;
    let close = matching_delimiter(source, open.start)?;
    let brace = tokens
        .iter()
        .position(|t| t.start > close && t.is_punct('{'))?;
    (brace > at).then_some(brace)
}

/// The call-site rewrites `processUISyntax` applies while it walks a file.
///
/// * `processAnimateToOrImmediately`: a bare `animateTo(…)` / `animateToImmediately(…)` call
///   becomes `Context.animateTo(…)`. A member call (`this.animateTo(…)`) and a declaration are
///   left alone: only a `CallExpression` whose callee is an identifier is rewritten.
/// * `processWorker`: the script URL of `new <namespace>.Worker('…')` loses its `.ts` suffix.
fn call_site_rewrites(
    source: &str,
    tokens: &[crate::build::lex::Token<'_>],
) -> Vec<(usize, usize, String)> {
    let mut edits = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident {
            continue;
        }
        if pre_define::ATTRIBUTE_ANIMATETO_SET.contains(&token.text) {
            let previous = index.checked_sub(1).map(|i| &tokens[i]);
            let member = previous.is_some_and(|p| p.is_punct('.'));
            let declaration =
                previous.is_some_and(|p| p.kind == Kind::Ident && p.text == "function");
            let Some(open) = tokens.get(index + 1).filter(|t| t.is_punct('(')) else {
                continue;
            };
            if member || declaration {
                continue;
            }
            let Some(close) = matching_delimiter(source, open.start) else {
                continue;
            };
            // A method or function named `animateTo` ends its signature with `{` or `:`; only
            // a call expression takes `&`/`;`/`)`/… after its argument list.
            let after = tokens.iter().find(|t| t.start > close);
            let signature = after.is_some_and(|t| t.is_punct('{') || t.is_punct(':'));
            if signature {
                continue;
            }
            edits.push((
                token.start,
                token.end,
                format!("{}.{}", pre_define::GLOBAL_CONTEXT, token.text),
            ));
        }
        if token.text == "new" {
            let previous = index.checked_sub(1).map(|i| &tokens[i]);
            if previous.is_some_and(|p| p.is_punct('.')) {
                continue;
            }
            if let Some(edit) = worker_script_url(tokens, index) {
                edits.push(edit);
            }
        }
    }
    edits
}

/// The string argument of `new <namespace>.Worker('…')`, with its `.ts` suffix rewritten.
fn worker_script_url(
    tokens: &[crate::build::lex::Token<'_>],
    at: usize,
) -> Option<(usize, usize, String)> {
    let mut index = at + 1;
    while let Some(token) = tokens.get(index) {
        if token.is_punct('(') {
            let name = tokens.get(index.checked_sub(1)?)?;
            let dot = tokens.get(index.checked_sub(2)?)?;
            if name.kind != Kind::Ident
                || name.text != pre_define::WORKER_OBJECT
                || !dot.is_punct('.')
            {
                return None;
            }
            let argument = tokens.get(index + 1)?;
            if argument.kind != Kind::Str {
                return None;
            }
            let raw = argument.text;
            let quote = raw.chars().next()?;
            let quote_len = quote.len_utf8();
            if raw.len() < 3 || !raw.ends_with(quote) {
                return None;
            }
            let inner = &raw[quote_len..raw.len() - quote_len];
            let stem = inner.strip_suffix(".ts")?;
            return Some((
                argument.start,
                argument.end,
                format!("{quote}{stem}.js{quote}"),
            ));
        }
        // An operator before the constructor call means this `new` owns a different shape.
        if token.is_punct(';') || token.is_punct('=') || token.is_punct(',') {
            return None;
        }
        index += 1;
    }
    None
}

/// `@Extend(Text) function mono(size) { .fontFamily('mono') }`
/// → `function __Text__mono(size) { Text.fontFamily('mono'); }`
pub(crate) fn emit_extend(extend: &ExtendFn) -> String {
    let mut out = format!(
        "function __{}__{}({}): void {{\n",
        extend.comp, extend.name, extend.params_text
    );
    for (attribute, args) in &extend.body {
        out.push_str(&format!(
            "    {}.{}({});\n",
            extend.comp,
            attribute,
            args.join(", ")
        ));
    }
    out.push_str("}\n");
    out
}

/// `@AnimatableExtend(Text) function animWidth(width: number) { .width(width) }`
/// → `function animWidth(width: number, elmtId, isInitialRender, parent): void { … }`
///
/// `processAnimatableExtend` (`process_ui_syntax.js`) keeps the name and appends the three
/// parameters, then writes `createAnimatableBody`: the first-render branch registers the
/// property through `<Comp>.createAnimatableProperty(…)` — its callback records the frame and
/// writes the attribute statements — and the else branch calls
/// `<Comp>.updateAnimatableProperty(…)`. The attribute statements are repeated after the
/// registration because `createAnimatableBody` appends `...a` to the then-block.
fn emit_animatable_extend(
    exported: bool,
    comp: &str,
    name: &str,
    params_text: &str,
    body: &str,
) -> String {
    let params_text = params_text.trim();
    let names: Vec<String> = crate::build::lex::split_top_level_commas(params_text)
        .iter()
        .filter_map(|param| parameter_name(param))
        .collect();
    let chain = parse_attribute_chain(body);
    let mut out = String::new();
    if exported {
        out.push_str("export ");
    }
    let params = if params_text.is_empty() {
        format!(
            "{elmt_id}, {is_initial_render}, {parent}",
            elmt_id = pre_define::ELMTID,
            is_initial_render = pre_define::ISINITIALRENDER,
            parent = pre_define::COMPONENT_CONSTRUCTOR_PARENT,
        )
    } else {
        format!(
            "{params_text}, {elmt_id}, {is_initial_render}, {parent}",
            elmt_id = pre_define::ELMTID,
            is_initial_render = pre_define::ISINITIALRENDER,
            parent = pre_define::COMPONENT_CONSTRUCTOR_PARENT,
        )
    };
    out.push_str(&format!("function {name}({params}): void {{\n"));
    if chain.is_empty() {
        out.push_str("}\n");
        return out;
    }
    let attribute_writes = |indent: &str| {
        let mut text = String::new();
        for (attribute, args) in &chain {
            text.push_str(&format!(
                "{indent}{comp}.{attribute}({});\n",
                args.join(", ")
            ));
        }
        text
    };
    out.push_str(&format!("    if ({}) {{\n", pre_define::ISINITIALRENDER));
    out.push_str(&format!(
        "        {comp}.{create}(\"{name}\"",
        create = pre_define::CREATE_ANIMATABLE_PROPERTY,
    ));
    for param in &names {
        out.push_str(&format!(", {param}"));
    }
    out.push_str(&format!(", ({params_text}) => {{\n"));
    out.push_str(&format!(
        "            {}.{}({});\n",
        pre_define::VIEW_STACK_PROCESSOR,
        pre_define::STARTGETACCESSRECORDINGFOR,
        pre_define::ELMTID
    ));
    out.push_str(&format!(
        "            {}.{}(\"{comp}\", {});\n",
        pre_define::VIEW_STACK_PROCESSOR,
        pre_define::GET_AND_PUSH_FRAME_NODE,
        pre_define::ELMTID
    ));
    out.push_str(&attribute_writes("            "));
    out.push_str(&format!(
        "            {}.{}();\n",
        pre_define::VIEW_STACK_PROCESSOR,
        pre_define::STOPGETACCESSRECORDING
    ));
    out.push_str(&format!(
        "            {}.{}({});\n",
        pre_define::COMPONENT_CONSTRUCTOR_PARENT,
        pre_define::FINISH_UPDATE_FUNC,
        pre_define::ELMTID
    ));
    out.push_str("        });\n");
    out.push_str(&attribute_writes("        "));
    out.push_str("    }\n    else {\n");
    out.push_str(&format!(
        "        {comp}.{update}(\"{name}\"",
        update = pre_define::UPDATE_ANIMATABLE_PROPERTY,
    ));
    for param in &names {
        out.push_str(&format!(", {param}"));
    }
    out.push_str(");\n    }\n}\n");
    out
}

/// `@Builder function badge(x) { Text(x) }`
/// → `function badge(x) { this.observeComponentCreation2(…); … }`
///
/// The body is emitted against the file's own tables. `EXTEND_ATTRIBUTE` and
/// `GLOBAL_STYLE_FUNCTION` are filled per file (`isExtendFunctionNode`,
/// `process_component_build.js`), and a `@Builder function` is a free function: it sees every
/// `@Extend` the file declares and the `@Styles function`s no struct owns, even though it
/// belongs to none of them. Without the tables a `.fancy(…)` written in a builder body falls
/// through to a plain attribute write (`Text.fancy(…)`) instead of the generated helper
/// (`__Text__fancy(…)`), which is what the reference emits (measured: `@mcui/mccharts`
/// `pageView/Tooltip.ts`, whose `globalBuilder` calls the file's `@Extend(Text) fancy`).
pub(crate) fn emit_builder(
    builder: &Builder,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) -> Result<String, String> {
    let mut out = String::new();
    // The comment block above the declaration is printed over the emitted signature, ahead of
    // its `export` (measured: `fixtures/declaration-comments`).
    for comment in &builder.comment {
        out.push_str(comment);
        out.push('\n');
    }
    if builder.is_export {
        out.push_str("export ");
    }
    // A builder renders into its caller's view stack, so the reference takes the parent as a
    // trailing parameter and hands it to every child construction inside the body
    // (`parent ? parent : this`). The declared parameter names stay as written — `ConnectBuilder`
    // is emitted as `export function ConnectBuilder(name: string, param: Object, parent = null)`
    // from `export function ConnectBuilder(name: string, param: Object)`.
    let declared = params_text(&builder.params);
    let params = if declared.is_empty() {
        "parent = null".to_string()
    } else {
        format!("{declared}, parent = null")
    };
    out.push_str(&format!("function {}({params}) {{\n", builder.name));
    let types = std::collections::HashMap::new();
    let body = builder.body.clone();
    // `parent` is a parameter here, not the `const parent` a `@LocalBuilder` declares, so only
    // the receiver changes.
    let _parent = crate::build::stage::process_component_build::GlobalBuilderScope::enter();
    match build_body::parse(&body, &types, 1) {
        Ok(items) => {
            let mut body = String::new();
            build_body::emit(&items, &mut body, "    ", builders, extends, styles);
            out.push_str(&capture_single_parameter(builder, &body));
        }
        Err(_) => {
            // A body the walker cannot read is emitted as written: a wrong shape here would be
            // worse than an unrendered block.
            for line in builder.body.lines() {
                out.push_str("    ");
                out.push_str(line.trim_start());
                out.push('\n');
            }
        }
    }
    out.push_str("}\n");
    Ok(out)
}

/// `parseGlobalBuilderParams` (process_component_build.js) records a `firstParam` only when the
/// builder declares **one** parameter, and that parameter is what the body carries across a
/// re-render:
///
/// ```js
/// const __text__ = text;
/// (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, text = __text__) => { … });
/// ```
///
/// `forkBuilderParamNode` writes the `const`, and `createComponentCreationArrowParams` /
/// `createCustomComponentNewExpression` add the third arrow parameter. A builder with two or
/// more parameters gets neither (`ConnectBuilder(name: string, param: Object)`), which is why
/// only the single-parameter case renames anything.
/// `parseGlobalBuilderParams` reads the parameter's *name node*, not its written text: a
/// `text?: string` parameter still names `text`, and the capture below has to be spelled
/// `const __text__ = text;`. The annotation, the optional marker and a default value are not
/// part of the name.
fn parameter_name(param: &str) -> Option<String> {
    let head = param.split([':', '=']).next()?.trim();
    let head = head
        .trim_start_matches("...")
        .trim()
        .trim_end_matches('?')
        .trim();
    (!head.is_empty()
        && head
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$'))
    .then(|| head.to_string())
}

fn capture_single_parameter(builder: &Builder, body: &str) -> String {
    // `parseGlobalBuilderParams` requires exactly one parameter (`1 === e.length`); a builder
    // with two keeps both names as written and gets no capture.
    if builder.params.len() != 1 {
        return body.to_string();
    }
    let Some(name) = builder
        .params
        .as_slice()
        .first()
        .and_then(|param| parameter_name(param))
    else {
        return body.to_string();
    };
    let mut out = format!("    const __{name}__ = {name};\n");
    // `createItemCreationArgs` appends the captured name to the `itemCreation(…)` call a deep
    // render starts with, the same way `createComponentCreationArrowParams` appends it to the
    // arrow parameters.
    out.push_str(
        &body
            .replace(
                "(elmtId, isInitialRender) =>",
                &format!("(elmtId, isInitialRender, {name} = __{name}__) =>"),
            )
            .replace(
                "itemCreation(elmtId, isInitialRender);",
                &format!("itemCreation(elmtId, isInitialRender, {name});"),
            ),
    );
    out
}

fn params_text(params: &[String]) -> String {
    params.join(", ")
}

// ── declaration masking ────────────────────────────────────────────────────────────
// Blanking the declarations no JavaScript parser accepts.
//
// Four ArkTS constructs are not TypeScript at all: a `@Styles` or `@Extend` function whose
// body is an attribute chain (`.width(10)`), a `@Builder` function whose body is a UI block,
// the `@Builder` decorator on a struct method, and `@Concurrent`, which TypeScript allows on
// no function declaration at all. What cannot reach the parser is replaced by spaces of the
// same length: every span measured afterwards still points at the text it came from, and the
// declarations are re-emitted from the tables built here. `@Concurrent` is the one whose
// declaration survives — only the decorator is blanked, because the marker changes neither
// the declaration nor a call site.

#[cfg(test)]
mod mask_tests {
    use super::*;

    /// A `@Styles` function is blanked and its chain is kept for inlining.
    #[test]
    fn a_styles_function_is_blanked_and_its_chain_kept() {
        let src = "@Styles\nfunction card() {\n  .width('100%')\n}\nconst x = 1;\n";
        let masked = mask_builder_decorators(src);
        assert!(!masked.source.contains("@Styles"), "{}", masked.source);
        assert!(!masked.source.contains(".width"), "{}", masked.source);
        assert!(masked.source.contains("const x = 1;"), "{}", masked.source);
        assert_eq!(masked.styles.len(), 1);
        assert_eq!(masked.styles[0].name, "card");
        assert_eq!(masked.styles[0].body[0].0, "width");
        assert_eq!(masked.styles[0].body[0].1, vec!["'100%'".to_string()]);
        assert_eq!(masked.source.len(), src.len(), "the mask keeps the length");
    }

    /// A declaration masked over several lines keeps the lines it spanned: the masked text is
    /// where the lowering reads positions from, so a shorter line count would move every
    /// position below it (`Common.ets`'s eight-line `@Extend(Row)` cost seven lines).
    #[test]
    fn a_masked_declaration_keeps_its_line_breaks() {
        let src = "@Extend(Row)\nfunction RowStyle() {\n  .width('100%')\n  .borderRadius(20)\n}\n\n@Component\nstruct Card {\n  build() {\n    Text('x')\n  }\n}\n";
        let masked = mask_builder_decorators(src);
        assert_eq!(masked.source.len(), src.len(), "the mask keeps the length");
        assert_eq!(
            masked.source.matches('\n').count(),
            src.matches('\n').count(),
            "the mask keeps the line breaks: {}",
            masked.source
        );
        assert!(
            masked
                .source
                .ends_with("struct Card {\n  build() {\n    Text('x')\n  }\n}\n"),
            "{}",
            masked.source
        );
    }

    /// An `@Extend` function records the component it extends and its parameter list.
    #[test]
    fn an_extend_function_records_its_component() {
        let src = "@Extend(Text)\nfunction mono(size: number) {\n  .fontFamily('mono')\n}\n";
        let masked = mask_builder_decorators(src);
        assert_eq!(masked.extends.len(), 1);
        assert_eq!(masked.extends[0].comp, "Text");
        assert_eq!(masked.extends[0].name, "mono");
        assert_eq!(masked.extends[0].params_text, "size: number");
    }

    /// `@AnimatableExtend` is replaced by the function `processAnimatableExtend` builds: the
    /// name stays, three parameters are appended, and the body branches on `isInitialRender`.
    #[test]
    fn an_animatable_extend_declaration_is_rewritten() {
        let src =
            "@AnimatableExtend(Text)\nfunction animWidth(width: number) {\n  .width(width)\n}\n";
        let masked = mask_builder_decorators(src);
        assert!(
            !masked.source.contains("@AnimatableExtend"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains(
                "function animWidth(width: number, elmtId, isInitialRender, parent): void {"
            ),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains(
                "Text.createAnimatableProperty(\"animWidth\", width, (width: number) => {"
            ),
            "{}",
            masked.source
        );
        assert!(
            masked
                .source
                .contains("ViewStackProcessor.StartGetAccessRecordingFor(elmtId);"),
            "{}",
            masked.source
        );
        assert!(
            masked
                .source
                .contains("ViewStackProcessor.GetAndPushFrameNode(\"Text\", elmtId);"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("Text.width(width);"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("parent.finishUpdateFunc(elmtId);"),
            "{}",
            masked.source
        );
        assert!(
            masked
                .source
                .contains("Text.updateAnimatableProperty(\"animWidth\", width);"),
            "{}",
            masked.source
        );
        // The declaration is emitted as text, so it must not also enter the `@Extend` table.
        assert!(masked.extends.is_empty(), "{}", masked.source);
        assert_eq!(
            masked.animatable_extends,
            vec![("Text".to_string(), "animWidth".to_string())],
            "{}",
            masked.source
        );
    }

    /// A top-level `@Builder` function is blanked and its name recorded.
    #[test]
    fn a_builder_function_is_blanked_and_named() {
        let src = "@Builder\nfunction badge() {\n  Text('x')\n}\nlet y = 2;\n";
        let masked = mask_builder_decorators(src);
        assert_eq!(masked.builders, vec!["badge".to_string()]);
        assert!(!masked.source.contains("@Builder"), "{}", masked.source);
        assert!(masked.source.contains("let y = 2;"), "{}", masked.source);
    }

    /// A `@Builder` struct method loses its decorator but keeps its body, and its name becomes
    /// a local builder name.
    #[test]
    fn a_builder_method_keeps_its_body() {
        let src = "class Card {\n  @Builder\n  badge(t: string) {\n    Text(t)\n  }\n}\n";
        let masked = mask_builder_decorators(src);
        assert_eq!(masked.locals, vec!["badge".to_string()]);
        assert!(masked.source.contains("Text(t)"), "{}", masked.source);
        assert!(
            masked.source.contains("@Builder"),
            "a member decorator stays for the member reader: {}",
            masked.source
        );
    }

    /// `@Concurrent` on a top-level function is blanked and the body gains the directive
    /// `processConcurrent` prepends; the declaration itself stays whole.
    #[test]
    fn a_concurrent_function_keeps_its_declaration() {
        let src = "@Concurrent\nfunction parseYAMLInBackground(content: string): boolean {\n  return true;\n}\nconst after = 1;\n";
        let masked = mask_builder_decorators(src);
        assert!(!masked.source.contains("@Concurrent"), "{}", masked.source);
        assert!(
            masked
                .source
                .contains("function parseYAMLInBackground(content: string): boolean {"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("\"use concurrent\";"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("const after = 1;"),
            "{}",
            masked.source
        );
    }

    /// The same decorator on a struct/class method is blanked, and the method keeps its name
    /// and body: blanking stops at the decorator's own end.
    #[test]
    fn a_concurrent_method_keeps_its_name() {
        let src = "class Worker {\n  @Concurrent\n  run(task: number) {\n    return task;\n  }\n  other() {}\n}\n";
        let masked = mask_builder_decorators(src);
        assert!(!masked.source.contains("@Concurrent"), "{}", masked.source);
        assert!(
            masked.source.contains("run(task: number)"),
            "{}",
            masked.source
        );
        assert!(masked.source.contains("other() {}"), "{}", masked.source);
        assert_eq!(masked.source.len(), src.len(), "the mask keeps the length");
    }

    /// The same decorator on an arrow-function variable is blanked and the initializer stays.
    #[test]
    fn a_concurrent_arrow_variable_keeps_its_initializer() {
        let src = "@Concurrent\nconst heavy = (n: number): number => {\n  return n * 2;\n};\n";
        let masked = mask_builder_decorators(src);
        assert!(!masked.source.contains("@Concurrent"), "{}", masked.source);
        assert!(
            masked
                .source
                .contains("const heavy = (n: number): number => {"),
            "{}",
            masked.source
        );
        assert_eq!(masked.source.len(), src.len(), "the mask keeps the length");
    }

    /// The decorator's argument list, when a source writes one, is blanked with it.
    #[test]
    fn a_concurrent_decorator_with_arguments_is_blanked_as_a_whole() {
        let src = "@Concurrent()\nfunction work() {}\n";
        let masked = mask_builder_decorators(src);
        assert!(!masked.source.contains("@Concurrent"), "{}", masked.source);
        assert!(
            masked.source.contains("function work() {"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("\"use concurrent\";"),
            "{}",
            masked.source
        );
    }

    /// The modifiers between the decorator and `function` do not hide the declaration: an
    /// `export`ed `@Concurrent` function still takes the directive.
    #[test]
    fn a_concurrent_exported_function_keeps_its_modifier_and_takes_the_directive() {
        let src = "@Concurrent\nexport function work() {\n  return 1;\n}\n";
        let masked = mask_builder_decorators(src);
        assert!(
            masked.source.contains("export function work() {"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("\"use concurrent\";"),
            "{}",
            masked.source
        );
    }

    /// `processAnimateToOrImmediately`: a bare call takes the `Context.` receiver.
    #[test]
    fn an_animate_to_call_takes_the_global_context() {
        let src = "function run() {\n  animateTo({ duration: 1 }, () => {});\n  animateToImmediately({ duration: 1 }, () => {});\n  this.animateTo({ duration: 1 }, () => {});\n}\n";
        let masked = mask_builder_decorators(src);
        assert!(
            masked.source.contains("Context.animateTo({ duration: 1 }"),
            "{}",
            masked.source
        );
        assert!(
            masked
                .source
                .contains("Context.animateToImmediately({ duration: 1 }"),
            "{}",
            masked.source
        );
        assert!(
            masked.source.contains("this.animateTo({ duration: 1 }"),
            "a member call keeps its receiver: {}",
            masked.source
        );
    }

    /// `processWorker`: the script URL loses its `.ts` suffix; a bare `new Worker(…)` and a
    /// string that is not the script URL stay as written.
    #[test]
    fn a_worker_script_url_loses_its_type_suffix() {
        let src = "const a = new worker.Worker('entry/src/workers/task.ts');\nconst b = new Worker('entry/src/workers/task.ts');\nconst c = 'entry/src/workers/task.ts';\n";
        let masked = mask_builder_decorators(src);
        assert!(
            masked
                .source
                .contains("new worker.Worker('entry/src/workers/task.js')"),
            "{}",
            masked.source
        );
        assert!(
            masked
                .source
                .contains("new Worker('entry/src/workers/task.ts')"),
            "only a namespaced construction is rewritten: {}",
            masked.source
        );
        assert!(
            masked
                .source
                .contains("const c = 'entry/src/workers/task.ts';"),
            "{}",
            masked.source
        );
    }
}

// ── builder helper emission ──────────────────────────────────────────────────────────
// Emitting the declarations a `build()` body can call.
//
// Three kinds share one table because they are all callable from a build body:
//
// * `@Extend(Comp)` becomes a free function `__<Comp>__<name>(…)` whose body writes the
// attributes onto the current component;
// * `@Styles` emits nothing at all: every call site is inlined with that site's component;
// * `@Builder` becomes a function whose body is the same view protocol `build()` uses.

#[cfg(test)]
mod builder_tests {
    use super::*;

    /// An `@Extend` helper writes its chain onto the component it extends.
    #[test]
    fn an_extend_helper_writes_its_chain() {
        let out = emit_extend(&ExtendFn {
            name: "mono".to_string(),
            comp: "Text".to_string(),
            params_text: "size: number".to_string(),
            body: vec![("fontFamily".to_string(), vec!["'mono'".to_string()])],
            order: 0,
            source_order: 0,
        });
        assert!(
            out.contains("function __Text__mono(size: number): void {"),
            "{out}"
        );
        assert!(out.contains("Text.fontFamily('mono');"), "{out}");
    }

    /// A `@Builder` function emits the view protocol, with the parent passed in last.
    #[test]
    fn a_builder_function_emits_the_view_protocol() {
        let out = emit_builder(
            &Builder {
                name: "badge".to_string(),
                params: vec!["t: string".to_string()],
                body: "Text(t)".to_string(),
                is_export: false,
                order: 0,
                comment: Vec::new(),
            },
            &[],
            &[],
            &[],
        )
        .expect("emit");
        assert!(
            out.contains("function badge(t: string, parent = null) {"),
            "{out}"
        );
        assert!(out.contains("Text.create(t);"), "{out}");
    }

    /// An optional parameter keeps its written type but is still captured by name.
    #[test]
    fn an_optional_builder_parameter_is_captured_by_its_name() {
        let out = emit_builder(
            &Builder {
                name: "tag".to_string(),
                params: vec!["text?: string".to_string()],
                body: "Text(text)".to_string(),
                is_export: false,
                order: 0,
                comment: Vec::new(),
            },
            &[],
            &[],
            &[],
        )
        .expect("emit");
        assert!(
            out.contains("function tag(text?: string, parent = null) {"),
            "{out}"
        );
        assert!(out.contains("const __text__ = text;"), "{out}");
        assert!(
            out.contains("(elmtId, isInitialRender, text = __text__) =>"),
            "{out}"
        );
    }
}

/// The arrow function a `CustomDialogController`'s `builder:` field receives.
///
/// `createCustomComponentBuilderArrowFunction` (`process_ui_syntax.js:254`) emits exactly five
/// statements, in this order, and they are the body below:
///
/// ```text
/// () => {
///   let jsDialog = new <Cmp>(…);                  // JS_DIALOG + createCustomComponentNewExpression
///   jsDialog.setController(<owner>);              // SET_CONTROLLER_METHOD
///   <ViewPU>.create(jsDialog);                    // createViewCreate
///   let paramsLambda = () => { … };               // assignComponentParams
///   jsDialog.paramsGenerator_ = paramsLambda;     // assignmentFunction
/// }
/// ```
///
/// `createViewCreate` (`process_component_member.js:477`) names
/// `BASE_COMPONENT_NAME_PU` (`ViewPU`) whenever `partialUpdateMode` is on, whatever protocol the
/// dialog's own struct lowers to: the arrow is built by the *call site*, not by the dialog
/// class, so a `@ComponentV2` dialog is still handed to `ViewPU.create`.
///
/// The owner (`this.<field>`) is the controller's own field, which is the property the
/// controller construction is assigned to; ``owner`` carries it.
/// The controller expression `setController(…)` is handed: the field the construction is
/// stored in, and the comment block that field keeps between `this.` and its name.
pub(crate) struct ControllerOwner<'a> {
    pub(crate) name: &'a str,
    pub(crate) comments: &'a [String],
}

pub(crate) fn custom_dialog_builder(
    component: &str,
    params: &str,
    controller: ControllerOwner<'_>,
    page: &str,
    line: usize,
    column: usize,
    indent: &str,
) -> String {
    let body = format!("{indent}    ");
    let inner = format!("{body}    ");
    // A plain controller member keeps its comment block between `this.` and the name: the
    // reference's printer emits the field's leading trivia where it appends the owner
    // (measured: `fixtures/comment-leading`, `MEMBER_PLAIN`; `fixtures/configuration-page`,
    // `/* 自定义弹框 */` + `// 配置是否保存弹框 Controller定义`).
    let mut owner = String::from("this.");
    for comment in controller.comments {
        owner.push_str(&format!("\n{body}{comment}"));
    }
    if !controller.comments.is_empty() {
        owner.push_str(&format!("\n{body}"));
    }
    owner.push_str(controller.name);
    format!(
        "() => {{\n\
         {body}let {js_dialog} = new {component}(this, {params}, {undefined}, -1, () => {{ }}, {{ page: \"{page}\", line: {line}, col: {column} }});\n\
         {body}{js_dialog}.{set_controller}({owner});\n\
         {body}{view}.{create}({js_dialog});\n\
         {body}let {params_lambda} = () => {{\n\
         {inner}return {params};\n\
         {body}}};\n\
         {body}{js_dialog}.{params_generator} = {params_lambda};\n\
         {indent}}}",
        js_dialog = pre_define::JS_DIALOG,
        undefined = pre_define::COMPONENT_CONSTRUCTOR_UNDEFINED,
        set_controller = pre_define::SET_CONTROLLER_METHOD,
        view = pre_define::BASE_COMPONENT_NAME_PU,
        create = pre_define::COMPONENT_CREATE_FUNCTION,
        params_lambda = pre_define::COMPONENT_PARAMS_LAMBDA_FUNCTION,
        params_generator = pre_define::COMPONENT_PARAMS_FUNCTION,
    )
}

// `createNavigationInit` (`process_ui_syntax.js`) registers the global `@Builder` functions a
// project's `routerMap` names. The map is per module and the lowering runs per file, so the
// pipeline publishes it here and each file looks itself up.

/// One `NavigationBuilderRegister` argument pair, by the name the router map gave it.
#[derive(Clone, Debug)]
pub(crate) struct RouterEntry {
    pub(crate) name: String,
    pub(crate) build_function: String,
}

static ROUTER_MAP: std::sync::OnceLock<
    std::sync::RwLock<std::collections::BTreeMap<String, Vec<RouterEntry>>>,
> = std::sync::OnceLock::new();

fn router_map() -> &'static std::sync::RwLock<std::collections::BTreeMap<String, Vec<RouterEntry>>>
{
    ROUTER_MAP.get_or_init(|| std::sync::RwLock::new(std::collections::BTreeMap::new()))
}

/// Publish one module's `routerMap`, keyed by `<module>/src/main/ets/<page>.ets`.
pub(crate) fn set_router_map(entries: &[crate::build::deps::pipeline::RouterMapEntry]) {
    let Ok(mut map) = router_map().write() else {
        return;
    };
    for entry in entries {
        let slot = map.entry(entry.page_source_file.clone()).or_default();
        if !slot.iter().any(|existing| {
            existing.name == entry.name && existing.build_function == entry.build_function
        }) {
            slot.push(RouterEntry {
                name: entry.name.clone(),
                build_function: entry.build_function.clone(),
            });
        }
    }
}

/// The `NavigationBuilderRegister` block a file's end carries, or `None` when it has nothing
/// to register.
///
/// The reference registers only the pairs whose `buildFunction` is a global `@Builder`
/// function (`GLOBAL_CUSTOM_BUILDER_METHOD`), and writes them into one IIFE per file, after
/// every declaration the file emits.
pub(crate) fn navigation_init(page: &str, declared: &[&str]) -> Option<String> {
    let map = router_map().read().ok()?;
    let entries = map.get(page)?;
    let calls: Vec<String> = entries
        .iter()
        .filter(|entry| declared.contains(&entry.build_function.as_str()))
        .map(|entry| {
            format!(
                "        NavigationBuilderRegister(\"{}\", wrapBuilder({}));",
                entry.name, entry.build_function
            )
        })
        .collect();
    if calls.is_empty() {
        return None;
    }
    Some(format!(
        "(function () {{\n    if (typeof NavigationBuilderRegister === \"function\") {{\n{}\n    }}\n}})();",
        calls.join("\n")
    ))
}
