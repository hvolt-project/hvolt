// process stage: `utils.js` — the shared model and the helpers the stages share.
//
// The reference keeps the per-program registries in `utils.js` (`componentInfo.componentNames`,
// `storedFileInfo`) and the small predicates every stage calls (`hasDecorator`,
// `isSimpleType`'s declaration-side half). hvolt's equivalents are the struct description the
// parser produces (`ComponentInfo`, `FieldInfo`, `ComponentFile`) and the predicates below;
// they live here so no stage owns another stage's input.

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, ArrowFunctionExpression, CallExpression, Expression, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::build::compile::ets_source_map::{ComponentPositions, SourcePosition};
/// One class field of a component struct, with every decorator it carries.
pub(crate) struct FieldInfo {
    pub(crate) name: String,
    /// Position of the field's own name in the source (parser coordinates, one-based).
    pub(crate) position: SourcePosition,
    /// The same place as a byte offset, so a member can be slotted among the methods.
    pub(crate) offset: usize,
    /// `readonly name: T`: the modifier survives into the emitted declaration.
    pub(crate) is_readonly: bool,
    /// `static name: T`: a static member is not state. The reference pushes the declaration
    /// through untouched (`process_component_class.js::isStaticProperty`), so it keeps its
    /// modifiers and its initializer and never becomes a backstore or a `_Params` member.
    pub(crate) is_static: bool,
    /// The accessibility modifier the source wrote (`private` / `protected` / `public`), verbatim.
    ///
    /// V1 defaults to `private` when the source wrote none (`createPropertyDeclaration`:
    /// `concatenateDecoratorsAndModifiers(undefined, s || [private])`), V2 keeps exactly what the
    /// source wrote (`concatenateDecoratorsAndModifiers(s, getModifiers(e))`). Measured:
    /// `fixtures/_pending-member-modifiers`.
    pub(crate) accessibility: Option<String>,
    /// The comment block written immediately above the member, in source order.
    ///
    /// The reference keeps the whole block: `updatePropertyDeclaration` rewrites the node in
    /// place and the printer emits the leading trivia, so a two-line stack above a member comes
    /// out as two lines (measured: `fixtures/comment-leading`, markers `MEMBER_STATE` and
    /// `MEMBER_STATE_SECOND`). `compare.js` compares comments separately from the tree, so
    /// dropping them reports a difference even when every node matches.
    pub(crate) comment: Vec<String>,
    /// The column the member's comment block (or its decorator) starts at, zero-based.
    ///
    /// A block whose continuation lines carry no `*` keeps its own columns, so the lines move
    /// by `printer column - this` instead of collapsing (`ConfigInfo.ts` keeps `/*@State` and
    /// its second line in the same column; collapsing put the second line one deeper).
    pub(crate) comment_indent: usize,
    /// The comment written on the same line as the declaration, after it (`@State x = 0; // X`),
    /// in source order. The reference keeps it at the end of the emitted declaration line.
    pub(crate) trailing_comment: Vec<String>,
    /// The method named by `@Watch('name')`, when the field carries one.
    pub(crate) watch_method: Option<String>,
    /// Every decorator on the field, in source order (`State`, `Param`, `Once`, …).
    ///
    /// The V1 shape drops them (the backstore class carries the meaning), the V2 shape
    /// keeps them: `ViewV2` reads `@Local` / `@Param` / `@Once` / `@Event` / `@Provider` /
    /// `@Consumer` off the field at runtime (measured: component-v2 golden, 2026-11-05).
    pub(crate) decorators: Vec<String>,
    /// The decorators as written (`@Provide('theme')`), for the V2 declaration.
    pub(crate) decorator_texts: Vec<String>,
    /// First argument of `@Provide` / `@Consume` / `@StorageLink` / `@StorageProp` /
    /// `@LocalStorageLink` / `@LocalStorageProp`, verbatim (`'theme'` or `KEY`).
    pub(crate) key_expr: Option<String>,
    /// That argument's text when it is a string literal.
    pub(crate) key_value: Option<String>,
    /// Declared type text, verbatim (e.g. "number", "string", "audio.AudioCapturer | undefined").
    pub(crate) type_text: Option<String>,
    /// Written with a `?` (`x?: T`). The V2 declaration keeps it; the V1 `_Params` interface
    /// is optional for every member and the backstores carry the value.
    pub(crate) is_optional: bool,
    /// Initializer expression text, verbatim.
    pub(crate) initial: Option<String>,
}

/// The V1 decorators that lower a field to an observed property.
///
/// `INNER_COMPONENT_MEMBER_DECORATORS` minus `@Watch` / `@BuilderParam` / `@Require` /
/// `@Env`: those do not create a backstore of their own (process_component_member.js
/// `processStateDecorators`).
pub(crate) const V1_FIELD_DECORATORS: &[&str] = &[
    "State",
    "Prop",
    "Link",
    "Provide",
    "Consume",
    "ObjectLink",
    "StorageLink",
    "StorageProp",
    "LocalStorageLink",
    "LocalStorageProp",
];

impl FieldInfo {
    /// The V1 decorator that gives this field a backstore, if any.
    pub(crate) fn decorator(&self) -> Option<&str> {
        self.decorators
            .iter()
            .map(String::as_str)
            .find(|d| V1_FIELD_DECORATORS.contains(d))
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.decorators.iter().any(|d| d == name)
    }

    /// The key as a string literal, when the decorator carried a literal.
    pub(crate) fn key_literal(&self) -> Option<String> {
        self.key_value.as_ref().map(|value| {
            format!(
                "\"{}\"",
                crate::build::ets_compile::escape_string_literal(value)
            )
        })
    }

    pub(crate) fn is_immutable(&self) -> bool {
        // `immutableDecorators` in process_component_member.js: these get no setter.
        self.has("ObjectLink") || self.has("BuilderParam") || self.has("Env")
    }

    /// The `: T` a declaration carries, or nothing.
    ///
    /// A field written without a type annotation is declared without one: the reference copies
    /// the type node it found and writes none when there was none (`private uiContext;`,
    /// measured 2026-11-06 on clashbox's `HomePage`). Inventing `Object` changed the shape of
    /// every such member and of its `_Params` entry.
    pub(crate) fn type_annotation(&self) -> String {
        match self.type_text.as_deref() {
            Some(text) => format!(": {text}"),
            None => String::new(),
        }
    }
}

/// A method declaration without the decorators it opens with (`@Builder`, `@LocalBuilder`).
///
/// The method's span opens at its first decorator, and the node records every decorator's own
/// span, so the declaration begins where the last one ends. The old version re-tokenized the
/// text to find that boundary; the node already carries it.
pub(crate) fn strip_leading_decorators<'a>(
    prepared: &'a str,
    method: &oxc_ast::ast::MethodDefinition<'_>,
    end: usize,
) -> &'a str {
    let start = method
        .decorators
        .last()
        .map_or(method.span().start, |decorator| decorator.span.end)
        .max(method.span().start);
    prepared.get(start as usize..end).unwrap_or("")
}

/// A struct-level `@Builder` method: a component-producing body compiled like `build()`.
pub(crate) struct StructBuilderMethod {
    /// Declaration up to its opening brace, modifiers included (`private badge(text: string)`).
    pub(crate) signature: String,
    pub(crate) body: String,
    /// Where the declaration starts in the source, so the class can keep the struct's order.
    pub(crate) offset: usize,
    /// The comment block written above the declaration, in source order.
    ///
    /// Read while the source is at hand: the emitter prints it directly above the method
    /// (`fixtures/declaration-comments`: `METHOD_DECL_COMMENT` comes out on the line before
    /// `item(name: string, parent = null) {`, and a block of several lines keeps its order).
    pub(crate) comment: Vec<String>,
    /// 1-based line of the source file where `body` starts, so a child construction inside this
    /// method can carry the coordinate the reference puts in its constructor call.
    ///
    /// `build()` has had its own line since the beginning ([`ComponentInfo::build_body_line`]);
    /// this is the same fact for a `@Builder` method, and it is a different one — a struct's
    /// methods do not start on `build()`'s line (`Index.ets`'s `build()` body starts at 334 and
    /// `WelcomeSheet()`'s at 571, so the `CrashPage()` they both reach is reported 34 lines off
    /// before this is used).
    #[allow(dead_code)] // read path lands with the emitter; the field is populated now
    pub(crate) body_line: usize,
}

/// 1-based line where a method body starts: the line of its opening brace.
///
/// `body_start` is where the body span (`{ … }`) begins in `source`, and the count is the one
/// `build()`'s body line is measured with (`ets_component::read_component_file`).
///
/// The brace's line, not the first statement's: `parse_build_at` adds the node's own offset
/// from the start of the body text, which begins at the brace, so the two add up to the source
/// line the reference writes into the construction's `{ page, line, col }`.
pub(crate) fn body_start_line(source: &str, body_start: usize) -> usize {
    source[..body_start].matches('\n').count() + 1
}

pub(crate) struct ComponentInfo {
    pub(crate) name: String,
    /// Position of the struct's name in the source (parser coordinates, one-based).
    pub(crate) declaration: SourcePosition,
    /// Where the declaration starts in the source, decorators included. The emitted file keeps
    /// every statement at its own position, so a class is emitted where its struct stood.
    pub(crate) declaration_offset: usize,
    /// Position of the `build` keyword, when the struct declares `build()`.
    pub(crate) build_position: Option<SourcePosition>,
    pub(crate) is_entry: bool,
    /// `@CustomDialog`: the class carries the controller setter its call site assigns through.
    pub(crate) is_dialog: bool,
    /// `@ComponentV2` (or any V2-only decorator) lowers this struct to `ViewV2`; a V1 struct
    /// in the same file keeps `ViewPU`.
    pub(crate) is_v2: bool,
    /// `@Reusable`: the loader only emits `resetStateVarsOnReuse` for these.
    pub(crate) is_reusable: bool,
    /// `export struct X`: the generated class has to stay exported, or every importer of the
    /// component fails at link time.
    pub(crate) is_exported: bool,
    pub(crate) fields: Vec<FieldInfo>,
    /// `@Computed` getter names (V2). `resetStateVarsOnReuse` calls `resetComputed` for each.
    pub(crate) computed: Vec<String>,
    /// Whether the struct declares a `@Monitor` method (V2). Reuse resets the monitors.
    pub(crate) has_monitor: bool,
    /// Top-level statements that are not imports and not the component class
    /// (consts, interfaces, helper classes), in source order.
    pub(crate) top_level: Vec<String>,
    /// User methods from the struct body (everything except `build`), verbatim.
    pub(crate) methods: Vec<String>,
    /// Where each of `methods` starts in the source, parallel to it.
    pub(crate) method_offsets: Vec<usize>,
    /// Where `build` starts in the source (the order slot `initialRender` takes).
    pub(crate) build_offset: usize,
    /// The statements of `build()` (without the braces), if present.
    pub(crate) build_body: Option<String>,
    /// The comment written above `build()`. The reference prints it ahead of `initialRender()`.
    /// The comment block written above `build()`, in source order.
    ///
    /// The whole block, not just its first line: the reference prints every line of it
    /// (`Index.ts` keeps `// TODO 6.0(20)光效` and `// @Provide lightSourceType …` above the
    /// same `initialRender`), and taking only the first dropped the rest.
    pub(crate) build_comment: Vec<String>,
    /// 1-based line in the source file where `build_body` starts, so a child component can carry
    /// the source coordinate the reference puts in its constructor call.
    pub(crate) build_body_line: usize,
    /// `@Builder` functions (top-level declarations), with their source bodies.
    pub(crate) builders: Vec<crate::build::ets_compile::Builder>,
    /// Struct-level `@Builder` method names (call sites become bound calls).
    pub(crate) struct_builders: Vec<String>,
    /// The `@Builder` methods themselves, emitted with their bodies transformed.
    pub(crate) struct_builder_methods: Vec<StructBuilderMethod>,
    /// `@Extend` functions: emitted once, called from the chain by name.
    pub(crate) extends: Vec<crate::build::ets_compile::ExtendFn>,
    /// `@Styles` functions: inlined at each call site.
    pub(crate) styles: Vec<crate::build::ets_compile::StyleFn>,
}

impl ComponentInfo {
    /// The source positions the emitted source map needs for this struct.
    pub(crate) fn source_positions(&self) -> ComponentPositions {
        let mut calls = Vec::new();
        if let Some(body) = &self.build_body {
            let (prepared_body, _builders) = prepare_build_body(body, self);
            let types = collect_array_types(self);
            if let Ok(items) = crate::build::ets_compile::parse_build_at(
                &prepared_body,
                &types,
                self.build_body_line,
            ) {
                crate::build::compile::ets_source_map::collect_calls(&items, &mut calls);
            }
        }
        ComponentPositions {
            name: self.name.clone(),
            declaration: self.declaration,
            build: self.build_position,
            fields: self
                .fields
                .iter()
                .map(|field| (field.name.clone(), field.position))
                .collect(),
            calls,
        }
    }
}

pub(crate) fn decorator_name(expr: &oxc_ast::ast::Expression<'_>) -> Option<String> {
    match expr {
        oxc_ast::ast::Expression::Identifier(id) => Some(id.name.to_string()),
        oxc_ast::ast::Expression::CallExpression(call) => {
            if let oxc_ast::ast::Expression::Identifier(callee) = &call.callee {
                Some(callee.name.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// An import statement: `import …`, `import type …`, or `import x = require(…)`.
///
/// The statement node decides it, so a leading comment that spells `import` is not mistaken for
/// one and a re-export stays an ordinary top-level statement.
pub(crate) fn is_import_stmt(statement: &Statement<'_>) -> bool {
    matches!(
        statement,
        Statement::ImportDeclaration(_) | Statement::TSImportEqualsDeclaration(_)
    )
}

pub(crate) fn take_with_comments(prepared: &str, start: usize, end: usize) -> String {
    let text = crate::build::lower::ets_output::with_leading_comments(prepared, start, end);
    // `with_leading_comments` reaches back over every comment, including one written after the
    // previous statement on its own line. That one belongs to the previous declaration (the
    // reference keeps it on the declaration's line), so it is not part of this node's text
    // (measured: `fixtures/trailing-comment`, `TRAILING_*` markers).
    let from = end.saturating_sub(text.len());
    let mut at = from;
    for token in crate::build::lex::tokenize(&prepared[from..end]) {
        if token.kind != crate::build::lex::Kind::Comment {
            break;
        }
        let comment_at = from + token.start;
        let line_start = prepared[..comment_at]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        if !prepared[line_start..comment_at].trim().is_empty() {
            at = from + token.end;
            // Skip the line break that ends the comment.
            at = prepared[at..end]
                .find('\n')
                .map_or(at, |offset| at + offset + 1);
            continue;
        }
        at = from + token.start;
        break;
    }
    // The slice begins at the comment token, which drops the indentation written in front of
    // it while the block's continuation lines keep theirs: one comment arrived as `/**` at
    // column zero with ` * …` at three (`fixtures/indent-unit`). Start at the line instead —
    // but only when nothing but whitespace stands between the two, so a comment trailing code
    // on the same line keeps the current behaviour.
    let line_start = prepared[..at].rfind('\n').map_or(0, |newline| newline + 1);
    let start = if prepared[line_start..at].trim().is_empty() {
        line_start
    } else {
        at
    };
    reindent_to_printer(prepared[start..end].to_string())
}

/// The printer's indentation unit: every statement it re-emits sits at four spaces per level.
const PRINTER_INDENT: usize = 4;

/// A member or statement moved from the source's indent unit to the printer's four spaces.
///
/// A text that carries a comment keeps its own spelling, so it also keeps the source's
/// indentation — two spaces per level in this corpus — while everything the printer re-emits
/// sits at four. One file then held both units (`fixtures/indent-unit`; the comment classifier's
/// `indent-only` files).
///
/// Rules, all measured on `fixtures/indent-unit`:
///
/// * a code line moves from `level * unit` to `level * 4`;
/// * a block comment's continuation is the level's indent plus one, so it moves from
///   `level * unit + 1` to `level * 4 + 1` (`[2, 3, 3]` becomes `[4, 5, 5]`; scaling by two
///   would give `[4, 6, 6]`);
/// * a leading comment block belongs to the declaration that follows it, so it takes that
///   declaration's level however the source indented it;
/// * a line inside a template literal is content, not layout: not one space of it moves;
/// * a line whose indent is not a multiple of the unit, and a text with no code line or no
///   indent at all (a top-level statement reaches the printer already at column zero), are
///   left as written rather than guessed.
pub(crate) fn reindent_to_printer(text: String) -> String {
    let unit = source_indent_unit(&text);
    // A text whose first code line sits at column zero (a top-level `class` / `function`) still
    // has indented members inside it, so the conversion runs; only a text with no indented code
    // line at all is left alone.
    let base = first_code_indent(&text);
    if unit == 0 || unit == PRINTER_INDENT {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut in_block_comment = false;
    let mut in_template = false;
    let mut seen_code = false;
    for (index, line) in text.lines().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let leading = line.len() - line.trim_start().len();
        let body = &line[leading..];
        if in_template {
            out.push_str(line);
            if closes_template(body) {
                in_template = false;
            }
            continue;
        }
        let continuation = in_block_comment || body.starts_with('*');
        let opening = body.starts_with("/*");
        let anchored = base > 0 && !seen_code && (continuation || opening);
        let level = if anchored {
            Some(base / unit)
        } else if continuation {
            (leading > 1).then(|| (leading - 1) / unit)
        } else if leading == 0 || leading % unit != 0 {
            None
        } else {
            Some(leading / unit)
        };
        match level {
            Some(level) => {
                let width = if continuation {
                    level * PRINTER_INDENT + 1
                } else {
                    level * PRINTER_INDENT
                };
                for _ in 0..width {
                    out.push(' ');
                }
            }
            None => {
                for _ in 0..leading {
                    out.push(' ');
                }
            }
        }
        out.push_str(body);
        if !body.is_empty() && !opening && !continuation {
            seen_code = true;
        }
        if opening && !body.contains("*/") {
            in_block_comment = true;
        } else if in_block_comment && body.contains("*/") {
            in_block_comment = false;
        }
        if opens_template(body) {
            in_template = true;
        }
    }
    out
}

/// The source's indentation unit: the smallest positive indent among the text's code lines.
///
/// Block comment lines are skipped — a continuation sits at the level's indent plus one, so a
/// comment at the top level would otherwise report `1` as the unit.
fn source_indent_unit(text: &str) -> usize {
    let mut unit = 0usize;
    // A block of commented-out code is not code: its lines sit at whatever indent the old
    // implementation had (`ConfigInfo.ets` keeps one at a single space), which would halve the
    // unit and double every level (`fixtures/indent-unit` shares the same tracking). The two
    // state transitions match `reindent_to_printer`'s own loop exactly.
    let mut in_block_comment = false;
    for line in text.lines() {
        let leading = line.len() - line.trim_start().len();
        let body = line.trim_start();
        if in_block_comment {
            if body.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if body.starts_with("/*") {
            if !body.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if leading == 0 || body.is_empty() || body.starts_with('*') {
            continue;
        }
        if unit == 0 || leading < unit {
            unit = leading;
        }
    }
    unit
}

/// The indent of the text's first code line, which a leading comment block inherits.
fn first_code_indent(text: &str) -> usize {
    // A block of commented-out code is not code either: `ConfigInfo.ets` keeps a whole old
    // implementation inside `/* … */` after a member's comment, and its lines sit at four
    // spaces while the member itself sits at two. Reading one of those as the first code line
    // anchored the comment one level too deep (8 where the reference writes 4).
    let mut in_block_comment = false;
    for line in text.lines() {
        let leading = line.len() - line.trim_start().len();
        let body = line.trim_start();
        if in_block_comment {
            if body.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if body.starts_with("/*") {
            if !body.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if body.is_empty() || body.starts_with('*') {
            continue;
        }
        return leading;
    }
    0
}

/// Whether this line's backticks leave a template literal open.
fn opens_template(body: &str) -> bool {
    body.bytes().filter(|byte| *byte == b'`').count() % 2 == 1
}

/// Whether this line closes the template literal the previous lines opened.
fn closes_template(body: &str) -> bool {
    opens_template(body)
}

/// Where a class statement's text begins: at its first decorator, when it has one.
///
/// ArkTS writes a class decorator ahead of the keyword it decorates (`@Observed export class`),
/// and the parser starts the exported statement at `export` — the decorators sit outside its
/// span. Slicing from there dropped them, so a `@Observed` class arrived without the marker the
/// runtime observation machinery keys on. The class node keeps the decorators, so their own
/// span gives the text back; a class without decorators keeps the statement's start.
pub(crate) fn class_decorator_start(
    class: &oxc_ast::ast::Class<'_>,
    statement_start: usize,
) -> usize {
    class
        .decorators
        .first()
        .map(|decorator| decorator.span.start as usize)
        .filter(|start| *start < statement_start)
        .unwrap_or(statement_start)
}

/// Element type of `xxx[]` / `Array<xxx>` / `xxx`, as written.
pub(crate) fn element_type(ty: &str) -> Option<String> {
    // `T[]` and `Array<T>`, both read from the token stream: the brackets and the angle
    // brackets are punctuation tokens, so the element type is the text between them.
    let tokens = crate::build::lex::tokenize(ty);
    let significant: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind != crate::build::lex::Kind::Comment)
        .collect();
    if significant.len() >= 2
        && significant[significant.len() - 2].is_punct('[')
        && significant[significant.len() - 1].is_punct(']')
    {
        let end = significant[significant.len() - 2].start;
        return Some(ty[..end].trim().to_string());
    }
    let is_array = significant
        .first()
        .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "Array");
    if is_array && significant.len() >= 3 && significant[1].is_punct('<') {
        let close = crate::build::lex::matching_delimiter(ty, significant[1].start)?;
        return Some(ty[significant[1].end..close].trim().to_string());
    }
    None
}

/// Map variable name → element type for ForEach arrays: top-level
/// `const MODELS: ModelSpec[] = …` and component fields (`{name}: T[]`).
/// The reference's checker supplies these to the key function; here they come from the
/// written annotations instead of from inference.
pub(crate) fn collect_array_types(
    comp: &ComponentInfo,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for stmt in &comp.top_level {
        let text = stmt.trim_start();
        // `const name: Type = …` read as tokens.
        let tokens: Vec<_> = crate::build::lex::tokenize(text).into_iter().collect();
        let mut index = 0usize;
        if tokens
            .get(index)
            .is_none_or(|t| t.kind != crate::build::lex::Kind::Ident || t.text != "const")
        {
            continue;
        }
        index += 1;
        let Some(name_token) = tokens.get(index) else {
            continue;
        };
        if name_token.kind != crate::build::lex::Kind::Ident {
            continue;
        }
        index += 1;
        if tokens.get(index).is_none_or(|t| !t.is_punct(':')) {
            continue;
        }
        index += 1;
        let type_start = tokens.get(index).map(|t| t.start);
        let mut equals = None;
        for (offset, token) in tokens.iter().enumerate().skip(index) {
            if token.is_punct('=') {
                equals = Some(offset);
                break;
            }
        }
        let (Some(type_start), Some(equals)) = (type_start, equals) else {
            continue;
        };
        let name = name_token.text;
        let ty = text[type_start..tokens[equals].start].trim();
        if let Some(elem) = element_type(ty) {
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                map.insert(name.to_string(), elem);
            }
        }
    }
    for f in &comp.fields {
        if let Some(ty) = &f.type_text {
            if let Some(elem) = element_type(ty) {
                map.insert(f.name.clone(), elem);
            }
        }
    }
    map
}

/// Rewrite `this.badge(...)` into `this.badge.bind(this)(...)` and pull each
/// such call out of the parsed body as a verbatim statement (the call-on-call
/// shape is unparseable by the view walker). Returns the remaining body and
/// the raw statement texts, in source order.
/// The body a component emits, plus the builder table it needs.
///
/// A statement-level `this.name(…)` on a struct `@Builder` becomes a bare `name(…)` so the
/// walker emits `this.name(…)` at that exact position; a call inside an argument list keeps
/// its `this.` form and is left to normal expression handling.
pub(crate) fn prepare_build_body(
    body: &str,
    comp: &ComponentInfo,
) -> (String, Vec<crate::build::ets_compile::Builder>) {
    let mut prepared = String::new();
    // The bound form is written only outside an arrow the source wrote: the reference binds a
    // statement at the top of `build()`, and leaves the same statement inside
    // `builder: () => { … }` as written (measured: `fixtures/builder-arrow-call`). A loop's item
    // generator is not one of those — it is a component block the reference expands statement by
    // statement — which is what [`user_arrow_body_ranges`] reads from the nodes.
    let arrow_bodies = user_arrow_body_ranges(body);
    let mut line_starts: Vec<usize> = vec![0];
    for (index, byte) in body.bytes().enumerate() {
        if byte == b'\n' && index + 1 < body.len() {
            line_starts.push(index + 1);
        }
    }
    // The statement's argument may span several lines (an object literal the source wrapped),
    // so the call is measured on the whole body rather than on one line.
    let mut copied = 0usize;
    for (index, &start) in line_starts.iter().enumerate() {
        if start < copied
            || arrow_bodies
                .iter()
                .any(|(from, to)| start >= *from && start < *to)
        {
            continue;
        }
        let line_end = line_starts.get(index + 1).copied().unwrap_or(body.len());
        let line = &body[start..line_end];
        let trimmed = line.trim_start();
        let Some(bname) = comp
            .struct_builders
            .iter()
            .map(String::as_str)
            .find(|name| trimmed.starts_with(&format!("this.{name}(")))
        else {
            continue;
        };
        let indent = &line[..line.len() - trimmed.len()];
        let paren = start + indent.len() + "this.".len() + bname.len();
        let Some(close) = matching_paren(&body[paren..]) else {
            continue;
        };
        let inner = &body[paren + 1..paren + close];
        let arguments =
            crate::build::stage::process_custom_component::builder_param_proxy(bname, inner)
                .unwrap_or_else(|| inner.to_string());
        prepared.push_str(&body[copied..start]);
        prepared.push_str(indent);
        prepared.push_str(&format!("this.{bname}.bind(this)({arguments})"));
        let after = paren + close + 1;
        // The walker prints statements with their terminator; a statement the source left open
        // takes one too.
        let tail_end = line_starts
            .iter()
            .copied()
            .find(|&next| next > after)
            .unwrap_or(body.len());
        if !body[after..tail_end].trim().starts_with(';') && body[after..tail_end].trim().is_empty()
        {
            prepared.push(';');
        }
        copied = after;
    }
    prepared.push_str(&body[copied..]);
    let mut builders = comp.builders.clone();
    for other in &comp.struct_builder_methods {
        let name = other
            .signature
            .split('(')
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .last()
            .unwrap_or_default()
            .to_string();
        if !name.is_empty() && !builders.iter().any(|b| b.name == name) {
            builders.push(crate::build::ets_compile::Builder {
                name,
                params: Vec::new(),
                body: String::new(),
                is_export: false,
                order: 0,
                comment: Vec::new(),
            });
        }
    }
    (prepared, builders)
}

/// The offset of the `)` that closes the `(` at offset 0 of `text`, skipping quoted text.
fn matching_paren(text: &str) -> Option<usize> {
    if !text.starts_with('(') {
        return None;
    }
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut i = 0usize;
    while let Some(c) = text[i..].chars().next() {
        if let Some(open) = quote {
            if c == '\\' {
                i += 1;
                i += text[i..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            if c == open {
                quote = None;
            }
            i += c.len_utf8();
            continue;
        }
        match c {
            '\'' | '"' | '`' => quote = Some(c),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += c.len_utf8();
    }
    None
}

/// The byte ranges of the arrow bodies whose statements the reference copies instead of
/// expanding — every arrow the source wrote that is not a component block.
///
/// The distinction is a property of the node, not of a line. A block the reference reaches
/// through a component is expanded statement by statement (`processComponentChild`,
/// `process_component_build.js`), and the loop item generator is walked the same way; every
/// other arrow it meets is an ordinary argument it copies. In the body this pass receives, the
/// walker's own shape makes the two nodes tell themselves apart:
///
/// * a child block is the trailing arrow argument of a component construction
///   (`Column(() => { … })`, `Column({ space: 4 }, () => { … })`);
/// * a loop's item generator is `ForEach` / `LazyForEach`'s second argument, and the
///   `Repeat` chain's `.each(…)` / `.template(…)` callback — `ProxyPage.ts:824` carries
///   `this.TabBuilder.bind(this)(group.name, index);` because of exactly this rule, and the
///   same shape appears in `Xb_TabComponent.ts:96`, `CardEdit.ts:143`, `PXSecScreen.ts:294`
///   and `Resources.ts:1322`.
///
/// Everything else — a `builder:` property value, an `onClick` callback, a `bindSheet` argument
/// — is returned here, so `prepare_build_body` leaves the statements inside it as written
/// (`fixtures/builder-arrow-call`: the call inside `builder: () => { … }` keeps its plain form).
fn user_arrow_body_ranges(body: &str) -> Vec<(usize, usize)> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, body, SourceType::ts()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        // `parse_build_at` parses the same text right after this pass and reports the failure
        // there; with no node to read there is no arrow to tell apart either.
        return Vec::new();
    }
    let mut collector = ArrowBodies::default();
    collector.visit_program(&parsed.program);
    let mut bodies = collector.all;
    bodies.retain(|span| !collector.expanded.contains(span));
    bodies
}

/// Every arrow body in a build body, and the ones the reference expands as a component block.
#[derive(Default)]
struct ArrowBodies {
    all: Vec<(usize, usize)>,
    expanded: Vec<(usize, usize)>,
}

impl<'a> Visit<'a> for ArrowBodies {
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        // `() => expr` has no statement list: there is nothing to rewrite inside it.
        if !arrow.expression {
            let span = arrow.body.span;
            self.all.push((span.start as usize, span.end as usize));
        }
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        // The decision is made where the argument position is known, not from the callee's
        // spelling alone.
        let generator = is_loop_call(call) || is_repeat_generator(call);
        let child_block = is_component_construction(call);
        for (index, argument) in call.arguments.iter().enumerate() {
            let Argument::ArrowFunctionExpression(arrow) = argument else {
                continue;
            };
            if arrow.expression {
                continue;
            }
            let expanded = if generator {
                // `ForEach(source, (item) => { … }, key)`: only the generator is expanded.
                index == 1
            } else if child_block {
                // A component's child block is written without parameters; the trailing arrow
                // of a construction that carries a callback keeps it (`@BuilderParam` sites are
                // read the same way the source wrote them).
                index + 1 == call.arguments.len() && arrow.params.items.is_empty()
            } else {
                false
            };
            if expanded {
                let span = arrow.body.span;
                self.expanded.push((span.start as usize, span.end as usize));
            }
        }
        walk::walk_call_expression(self, call);
    }
}

/// `ForEach(source, item => …)` / `LazyForEach(…)`: the loop whose item generator the reference
/// renders as a component block.
fn is_loop_call(call: &CallExpression<'_>) -> bool {
    matches!(callee_identifier(call), Some("ForEach" | "LazyForEach"))
}

/// `Repeat(source).each(source, (item) => …)` / `.template(…)`: the same role, one link later.
///
/// The link is on the `Repeat` chain, so the receiver is unwrapped until the name is known:
/// `Repeat(…)` directly, or a chain of attribute calls standing on it.
fn is_repeat_generator(call: &CallExpression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    if member.property.name != "each" && member.property.name != "template" {
        return false;
    }
    let mut object = &member.object;
    loop {
        match object {
            Expression::CallExpression(inner) => {
                if callee_identifier(inner) == Some("Repeat") {
                    return true;
                }
                object = &inner.callee;
            }
            Expression::StaticMemberExpression(inner) => object = &inner.object,
            _ => return false,
        }
    }
}

/// Whether the call constructs a component: its callee is a bare name, which the walker prints
/// with a capital (`Column(…)`, `ListItemGroup(…)`). A member call is an attribute or a plain
/// function of the file and never takes a child block.
fn is_component_construction(call: &CallExpression<'_>) -> bool {
    callee_identifier(call).is_some_and(|name| name.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// The name a call invokes, when the callee is a bare identifier.
fn callee_identifier<'a>(call: &'a CallExpression<'a>) -> Option<&'a str> {
    match &call.callee {
        Expression::Identifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

/// Parse the `@Entry @Component struct` blocks and their fields.
/// Struct-level `@Builder` method names, scanned from the raw source: their
/// decorators are masked before oxc parses, so the names must be collected here
/// (a `@Builder`-masked method looks like an ordinary method to the parser).
pub(crate) fn struct_builder_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for (i, _) in source.match_indices("@Builder") {
        let after = &source[i + "@Builder".len()..];
        let head = after.trim_start();
        let head = head.strip_prefix("private").unwrap_or(head).trim_start();
        let head = head.strip_prefix("public").unwrap_or(head).trim_start();
        let name_end = head
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        if name_end > 0 {
            names.push(head[..name_end].to_string());
        }
    }
    names
}

/// Names declared as methods inside a `struct` body.
///
/// A method and a component call can share a name (`Title(t: ResourceStr) { … }` next to
/// `this.Title(x)`), and only the declaration must keep its parameter list. Declarations
/// sit one level inside the struct, so the indent is the signal used here.
/// Decorators that only exist in the V2 protocol. Their presence lowers the struct to
/// `ViewV2` even when `@ComponentV2` is not spelled out.
pub(crate) const V2_DECORATORS: &[&str] = &[
    "ComponentV2",
    "Local",
    "Param",
    "Once",
    "Event",
    "Provider",
    "Consumer",
    "Monitor",
    "Computed",
    "ObservedV2",
    "Trace",
    "LocalBuilder",
];

/// The protocol a component lowers to. The file-level `protocol` is the default the caller
/// selected; a struct whose own decorators say otherwise overrides it.
pub(crate) fn protocol_for(comp: &ComponentInfo, default: Protocol) -> Protocol {
    if comp.is_v2 == default.v2 {
        default
    } else if comp.is_v2 {
        V2_PROTOCOL
    } else {
        V1_PROTOCOL
    }
}

pub(crate) fn struct_method_names(source: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut in_struct = false;
    let mut depth = 0i32;
    for line in source.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        // `struct X`, `export struct X` and `export default struct X` all declare one; missing
        // the exported spellings leaves every method of that struct uncollected, and the
        // builder rewriter then treats their declarations as calls.
        // `struct X` / `export struct X` / `export default struct X`, read from the first
        // tokens so a comment above the line does not change the answer.
        let declares_struct = {
            let tokens: Vec<_> = crate::build::lex::tokenize(trimmed)
                .into_iter()
                .filter(|t| t.kind != crate::build::lex::Kind::Comment)
                .collect();
            let mut at = 0usize;
            if tokens
                .get(at)
                .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "export")
            {
                at += 1;
            }
            if tokens
                .get(at)
                .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "default")
            {
                at += 1;
            }
            tokens
                .get(at)
                .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "struct")
        };
        if !in_struct && declares_struct && line.contains('{') {
            in_struct = true;
            depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
            continue;
        }
        if !in_struct {
            continue;
        }
        if indent == 2 {
            let name_end = trimmed
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .count();
            if name_end > 0 {
                let rest = &trimmed[name_end..];
                if rest.starts_with('(') {
                    // Every struct member that takes a parameter list counts: the rewriter
                    // needs to tell a declaration from a call, and a `@LocalBuilder` method
                    // may start with either case (`dialogContent` is lowercase).
                    let name = &trimmed[..name_end];
                    if !names.contains(&name.to_string()) {
                        names.push(name.to_string());
                    }
                }
            }
        }
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if depth <= 0 {
            in_struct = false;
        }
    }
    names
}

/// One component file, read once.
///
/// The parser sees the prepared text — masked decorators, `struct` → `class`, child blocks
/// rewritten — so every node span indexes into [`ComponentFile::prepared`]. Reading the file
/// once means the import list and the components describe the same parse; a caller never
/// re-scans the source line by line to find what the parser already knows.
pub(crate) struct ComponentFile {
    /// Every name the file reads as a value, so the kit expansion drops a binding that only
    /// appears in a type — decided on the tree, not by scanning the text.
    pub(crate) value_used: std::collections::HashSet<String>,
    /// Every `import` declaration in source order, taken by node span so a clause spread over
    /// several lines (or carrying a comment) comes out whole.
    pub(crate) imports: Vec<String>,
    pub(crate) components: Vec<ComponentInfo>,
    /// Top-level statements with the source offset they start at, `@Builder` declarations
    /// excluded. A file with no struct still needs them in order around its helpers.
    pub(crate) top_level_ordered: Vec<(usize, String)>,
    /// The file's `@Builder function` table, whether or not it declares a struct.
    pub(crate) builders: Vec<crate::build::ets::Builder>,
    pub(crate) extends: Vec<crate::build::ets::ExtendFn>,
    /// The file's `@AnimatableExtend` declarations as `(component, property)`.
    ///
    /// A call site for one of these is not `{Comp}.{attr}(…)`: the reference appends
    /// `elmtId`, `isInitialRender` and `this` and emits a bare `{property}(…)` call
    /// (`process_component_build.js::isExtendFunctionNode`), so the attribute list needs to know
    /// which `(component, name)` pairs came from an animatable extend rather than a plain
    /// `@Extend`.
    ///
    /// The attribute list is reached without the file, so the same table is published to it by
    /// `process_ui_syntax::set_animatable_extends` when this struct is built
    /// (`read_component_file`); this copy keeps the declarations on the file for callers that
    /// already hold it.
    #[allow(dead_code)] // read path is the published thread-local; see the doc above
    pub(crate) animatable_extends: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
pub(crate) struct Protocol {
    /// V2 (`@ComponentV2`) semantics.
    pub(crate) v2: bool,
}

pub(crate) const V1_PROTOCOL: Protocol = Protocol { v2: false };
pub(crate) const V2_PROTOCOL: Protocol = Protocol { v2: true };

#[cfg(test)]
mod tests {
    use crate::build::lower::ets_transpile::{transpile, TranspileOptions};

    /// Lower one source file with no project context (the kit config dirs and the project
    /// builder table are only read by the `@kit.*` and cross-file passes).
    fn lower(source: &str) -> String {
        let opts = TranspileOptions {
            kit_config_dirs: &[],
            project_builders: &[],
            route: None,
        };
        transpile(source, &opts).expect("the fixture lowers")
    }

    /// A `@Builder` call written inside a `ForEach` item generator takes the bound form.
    ///
    /// The generator's body is a component block, not a callback: the reference walks it with
    /// the same statement pass `build()` gets, so `this.tab(…)` becomes
    /// `this.tab.bind(this)(…)` and never a component frame (measured on the packed reference:
    /// `ProxyPage.ts:824`, `Xb_TabComponent.ts:96`, `CardEdit.ts:143`, `PXSecScreen.ts:294`).
    #[test]
    fn a_builder_call_in_a_loop_item_body_is_bound() {
        let out = lower(
            r#"
@Entry
@Component
struct Page {
  @Builder
  tab(name: string, index: number) {
    Text(name)
  }

  build() {
    Column() {
      List() {
        ForEach(this.names, (name: string, index: number) => {
          this.tab(name, index)
        })
      }
    }
  }
}
"#,
        );
        assert!(out.contains("this.tab.bind(this)(name, index)"), "{out}");
        assert!(!out.contains("tab.observeComponentCreation2"), "{out}");
    }

    /// A `@Builder` method's body line is its own, not `build()`'s.
    ///
    /// Each field feeds the `{ page, line, col }` a child construction writes, so the two must
    /// differ whenever the methods start on different lines.
    #[test]
    fn a_builder_method_carries_its_own_body_line() {
        let source = r#"
@Entry
@Component
struct Page {
  build() {
    Column() {
      Text('x')
    }
  }

  @Builder
  sheet() {
    if (this.show) {
      Text('y')
    }
  }
}
"#;
        let components = crate::build::lower::ets_component::parse_components(source, &[])
            .expect("the fixture parses");
        let page = &components[0];
        assert_eq!(page.build_body_line, 5);
        assert_eq!(page.struct_builder_methods.len(), 1);
        assert_eq!(page.struct_builder_methods[0].body_line, 12);
    }

    /// The same call inside a callback the source wrote stays as written.
    ///
    /// `fixtures/builder-arrow-call` measures the rule for a `builder:` property; an `onClick`
    /// is the other shape that must not be bound, because its body is an argument the reference
    /// copies rather than a block it expands.
    #[test]
    fn a_builder_call_inside_a_callback_is_left_as_written() {
        let out = lower(
            r#"
@Entry
@Component
struct Page {
  @Builder
  tab(name: string) {
    Text(name)
  }

  build() {
    Column() {
      Text('host')
        .onClick(() => {
          this.tab('clicked')
        })
    }
  }
}
"#,
        );
        assert!(out.contains("this.tab('clicked')"), "{out}");
        assert!(!out.contains("this.tab.bind(this)('clicked')"), "{out}");
    }
}
