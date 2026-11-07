// process stage: `process_component_build.js` — a `build()` body becomes the runtime's view
// protocol.
//
// The reference walks the body's statements (`processComponentChild`,
// `process_component_build.js:150`) and dispatches on the component's kind
// (`getComponentType`): an inner component goes through `processInnerComponent` (`:310`) →
// `processNormalComponent` (`:320`) / `processItemComponent` (`:434`) /
// `processTabAndNav` (`:504`), a custom component goes to
// `process_custom_component.js`, `if` to `processIfStatement` (`:643`), `ForEach` to
// `processForEachComponentNew` (`:555`), `Repeat` to `processRepeatComponent` (`:354`).
//
// `createComponent(node, method, …)` (`:706`) is the one place the emitted method name is
// decided:
//
//   - the innermost identifier is `Button` and the method is not `pop` → the method becomes
//     `COMPONENT_CREATE_LABEL_FUNCTION` (`createWithLabel`), or stays
//     `COMPONENT_CREATE_CHILD_FUNCTION` (`createWithChild`) when that is what was asked for;
//   - `needPop` comes from `NEEDPOP_COMPONENT`, `isContainerComponent` from
//     `BUILDIN_CONTAINER_COMPONENT` (`checkContainer`, `:731`), and `pop()` is appended for
//     either (`processNormalComponent`, `:326`).
//
// `processNormalComponentBlock` (`:330`) is the block form: a `Button` with a block has the
// `create` statement it just emitted *replaced* by the `createWithChild` one — at `-2` when the
// preview/debug-line build added a statement, at `-1` otherwise. hvolt's equivalent decision is
// the one made in this module, so the two readings are compared rule by rule rather than by
// output.

//! Reading a `build()` body and rendering it as the runtime's view protocol.
//!
//! `build()` is a declarative tree: every call is a component, every child block is its
//! content, and `if` / `ForEach` / `LazyForEach` are control flow the runtime re-evaluates.
//! The compiled form is a flat sequence of `observeComponentCreation2` calls with explicit
//! `create()` / `pop()` pairs, which is what [`emit`] writes.
//!
//! The tree is read from the parsed body rather than from its text: a call's arguments and its
//! child block are AST nodes, so a nested block cannot be mistaken for a sibling one.

use std::collections::HashMap;

use crate::build::stage::pre_define;
use crate::build::stage::process_component_class::{
    FileFacts, BUILDER_PARENT, BUILDER_PARENT_EXPR, CONTEXT_STACK_POP, CONTEXT_STACK_PUSH,
};
use crate::build::stage::process_custom_component::{
    rewrite_backstores, rewrite_bindings, state_vars_object, Binding,
    UPDATE_STATE_VARS_OF_CHIND_BY_ELMTID,
};
use crate::build::stage::utils::{
    collect_array_types, prepare_build_body, ComponentInfo, StructBuilderMethod,
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, ArrowFunctionExpression, CallExpression, Expression, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::build::ets::{Builder, ExtendFn, StyleFn};

thread_local! {
    /// Whether the body being emitted belongs to a `@LocalBuilder`.
    ///
    /// The reference carries the same fact down as the `isBuilder` argument of
    /// `createComponentCreationStatement` / `createCustomComponent` and turns it into
    /// `createConditionParent(isBuilder)` (process_component_build.js), so a child
    /// construction inside the body is handed `parent ? parent : this`
    /// (process_custom_component.js `createCustomComponentNewExpression`) instead of `this`.
    static IN_LOCAL_BUILDER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn in_local_builder() -> bool {
    IN_LOCAL_BUILDER.with(std::cell::Cell::get)
}

thread_local! {
    /// Whether the body being emitted belongs to a top-level `@Builder function`.
    ///
    /// `createComponentCreationStatement` takes its receiver from `isBuilder`, and a global
    /// builder's receiver is the `parent` argument it was handed:
    /// `(parent ? parent : this).observeComponentCreation2(…)` (measured: `PipManager.ts`,
    /// where every frame of `PipBackgroundBuilder` is written that way). A `@LocalBuilder`
    /// method keeps `this`.
    static IN_GLOBAL_BUILDER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The receiver an `observeComponentCreation2` frame is written on.
fn frame_receiver() -> &'static str {
    if IN_GLOBAL_BUILDER.with(std::cell::Cell::get) {
        // Parenthesised: this is a receiver, and `parent ? parent : this.x()` would parse as
        // `parent ? parent : (this.x())`.
        "(parent ? parent : this)"
    } else {
        "this"
    }
}

/// Holds the global-builder flags for one function body: both the frame receiver and the parent
/// a child construction is handed come from the same `isBuilder` argument.
pub(crate) struct GlobalBuilderScope(bool, bool);

impl GlobalBuilderScope {
    pub(crate) fn enter() -> Self {
        Self(
            IN_GLOBAL_BUILDER.with(|flag| flag.replace(true)),
            IN_LOCAL_BUILDER.with(|flag| flag.replace(true)),
        )
    }
}

impl Drop for GlobalBuilderScope {
    fn drop(&mut self) {
        IN_GLOBAL_BUILDER.with(|flag| flag.set(self.0));
        IN_LOCAL_BUILDER.with(|flag| flag.set(self.1));
    }
}

/// Holds the flag for the span of one `@LocalBuilder` body, restoring the previous value so a
/// nested ordinary builder keeps its own receiver.
///
/// A top-level `@Builder function` takes the same receiver without the `const parent` line: its
/// `parent` is a parameter.
pub(crate) struct LocalBuilderScope(bool);

impl LocalBuilderScope {
    pub(crate) fn enter() -> Self {
        Self(IN_LOCAL_BUILDER.with(|flag| flag.replace(true)))
    }
}

impl Drop for LocalBuilderScope {
    fn drop(&mut self) {
        IN_LOCAL_BUILDER.with(|flag| flag.set(self.0));
    }
}

/// One item of a `build()` body.
pub(crate) enum UiItem {
    /// A component construction with its attributes and children.
    Node(UiNode),
    /// `if` / `else if` / `else`, in source order.
    If(Vec<UiBranch>),
    /// `ForEach(source, item => …, key => …)`.
    ForEach(UiLoop),
    /// `LazyForEach(source, item => …, key => …)`.
    LazyForEach(UiLoop),
    /// `Repeat<T>(source).each(…)`: one frame, with the callback body expanded like any other
    /// build body (process_component_build.js `processRepeatComponent`).
    Repeat(UiRepeat),
    /// Anything else, kept as written (a plain statement inside a block).
    Raw(String),
}

/// `Repeat<T>(source).each(…)` — `processRepeatComponent` (process_component_build.js).
///
/// The expression is printed as written, with two edits the reference makes: the `Repeat` call
/// gains a trailing `this` (`recurseRepeatExpression` appends it to the argument list), and
/// `.render(isInitialRender)` is appended to the chain. The callback of `.each` (and of
/// `.template`) is the one part read as UI: its body is expanded into frames exactly like a
/// `ForEach` item body, which is what `processRepeatPropWithChild` does.
pub(crate) struct UiRepeat {
    /// `Repeat<T>(source, this)`, ready to print.
    pub call: String,
    /// The chained calls after it, in source order.
    pub links: Vec<UiRepeatLink>,
}

/// One link of a `Repeat` chain.
pub(crate) struct UiRepeatLink {
    pub name: String,
    /// `.each` / `.template`: the callback's parameter list as written, and its body.
    pub generator: Option<(String, Vec<UiItem>)>,
    /// Every other link: its arguments as written (`None` when it has none).
    pub args: Option<String>,
}

/// A component construction.
pub(crate) struct UiNode {
    pub component: String,
    /// Position of the component name in the source file (one-based).
    pub line: usize,
    pub col: usize,
    /// The arguments of `Component.create(…)`, as written. `None` when the call has none.
    pub create_args: Option<String>,
    /// The construction carries a trailing block: `Button('x') { Text('y') }`. The reference
    /// tests the same thing (`isBlock(u.etsComponentNode.body)`,
    /// `process_component_build.js:324`), which is why an empty block still counts.
    pub has_block: bool,
    /// Attribute calls on the node, in chain order.
    pub attributes: Vec<UiAttribute>,
    pub children: Vec<UiItem>,
    /// The comments written directly above the construction, in source order.
    ///
    /// The reference prints the whole block at each of the two positions a component's comment
    /// gets (`fixtures/comment-leading`: `BLOCK_ONE`/`BLOCK_TWO`, `BLOCK_A`/`BLOCK_B` and the
    /// three-line `TRIO_*` block all come out twice, one line each, in source order).
    pub comment: Vec<String>,
}

/// One attribute call: `.fontSize(16)` on the component the chain starts from.
pub(crate) struct UiAttribute {
    pub name: String,
    /// The argument list, as written; empty when the call takes none.
    pub args: String,
}

/// One branch of an `if`.
pub(crate) struct UiBranch {
    /// The comments written after the whole `if` inside its enclosing block — only the last
    /// branch carries them, and only the synthesized `else` reads them
    /// (`fixtures/comment-leading`, marker `IF_TAIL`).
    pub tail: Vec<String>,
    /// The comment written above this branch's `if` / `else if` / `else`, when there is one.
    ///
    /// Every branch carries its own block. The first one is printed between `If.create();` and
    /// the `if` (`fixtures/comment-leading`, marker `IF_START`); the others are printed on their
    /// own lines directly above `else if (…)` / `else {` (markers `BRANCH_ELSEIF`,
    /// `BRANCH_ELSE`, `BETWEEN_BRANCH`, measured 2026-11-07), which is a different position.
    pub comment: Vec<String>,
    /// The comment written after the `else` keyword on its own line
    /// (`else // X` + `\n {`), kept so the synthesized `else {` can print it back
    /// (measured: `fixtures/keyword-trailing-comment`, marker `ELSE_TRAILING`).
    pub keyword_trailing: Vec<String>,
    /// `None` for the final `else`.
    pub condition: Option<String>,
    pub body: Vec<UiItem>,
}

/// One iteration construct.
pub(crate) struct UiLoop {
    /// The comment written above the loop, when there is one.
    ///
    /// The reference prints it twice for a `ForEach` — at the start of the frame and above the
    /// `pop()` (`fixtures/comment-leading`, marker `FOREACH_START`; the same shape appears in
    /// `fixtures/configuration-page/golden/Index.ts:2013,2031`) — and once for a `LazyForEach`,
    /// above its `pop()` only (marker `LAZY_START`, measured 2026-11-07).
    pub comment: Vec<String>,
    /// The array expression, as written.
    pub source: String,
    /// The name the generator binds the item to.
    pub item: String,
    /// The key generator, as written, and whether it reads the index.
    pub key: Option<(String, bool)>,
    /// Whether the item generator reads the index.
    pub item_uses_index: bool,
    /// The attribute calls written on the loop itself (`ForEach(…).onMove(…)`).
    ///
    /// The reference keeps them inside the loop's frame, after `create()` and before the item
    /// generator (`HomePage.ts`: `ForEach.create(); ForEach.onMove(…); const
    /// forEachItemGenFunction = …`), so they belong to the loop, not to a component named after
    /// it.
    pub attributes: Vec<UiAttribute>,
    /// The source's own second parameter, as written (`i: number`).
    ///
    /// The reference copies that parameter node into the generator it emits
    /// (`process_component_build.js`: `getParameters` pushes `e.parameters[1]` unchanged after
    /// the `_item` it always writes), so the name — and whether it carries a type at all — is
    /// the file's, not ours.
    pub index_parameter: Option<String>,
    pub body: Vec<UiItem>,
}

/// Read a `build()` body into a tree.
///
/// `start_line` is the one-based line of `body` in its file, so every node can report the
/// coordinate the reference writes into the source map.
pub(crate) fn parse(
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Result<Vec<UiItem>, String> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, body, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        let first = ret
            .errors
            .first()
            .map(|e| format!("{e:?}"))
            .unwrap_or_else(|| "parser panicked".to_string());
        return Err(first);
    }
    let mut items = Vec::new();
    for statement in &ret.program.body {
        items.push(read_statement(statement, body, array_types, start_line));
    }
    // The comments written after the last statement of the body belong to no statement; the
    // synthesized `else` of a bare `if` standing last prints them
    // (`fixtures/comment-leading`, marker `IF_TAIL`; `fixtures/configuration-page`,
    // `// 悬浮新增配置按钮 END`).
    if let (Some(last), Some(UiItem::If(branches))) = (ret.program.body.last(), items.last_mut()) {
        let tail = comments_between(body, last.span().end as usize, body.len());
        if let Some(branch) = branches.last_mut() {
            branch.tail = tail;
        }
    }
    Ok(items)
}

fn read_statement(
    statement: &Statement<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> UiItem {
    match statement {
        Statement::IfStatement(node) => {
            let mut branches = Vec::new();
            let mut current = Some(node);
            // The comment above the `if` belongs to the statement, not to its first branch's
            // body; it is carried on the first branch because that is where the emitter reads
            // it back (the source map walks the bodies and does not care).
            let comment = crate::build::ets::comments::leading_comments_before(
                body,
                statement.span().start as usize,
            );
            while let Some(branch) = current.take() {
                let branch_comment = if branches.is_empty() {
                    comment.clone()
                } else {
                    // An `else if (…)` alternate starts at its `if` and a final `else { … }`
                    // starts at its `{`; the `else` keyword — and the comment written above it
                    // — sits before the statement in both cases.
                    else_branch_comments(body, branch.span().start as usize)
                };
                branches.push(UiBranch {
                    tail: Vec::new(),
                    keyword_trailing: Vec::new(),
                    comment: branch_comment,
                    condition: Some(branch.test.span().source_text(body).trim().to_string()),
                    body: read_block(&branch.consequent, body, array_types, start_line),
                });
                current = match &branch.alternate {
                    Some(Statement::IfStatement(nested)) => Some(nested),
                    Some(other) => {
                        branches.push(UiBranch {
                            tail: Vec::new(),
                            keyword_trailing: else_keyword_trailing_comment(
                                body,
                                other.span().start as usize,
                            ),
                            comment: else_branch_comments(body, other.span().start as usize),
                            condition: None,
                            body: read_statement_block(other, body, array_types, start_line),
                        });
                        None
                    }
                    None => None,
                };
            }
            UiItem::If(branches)
        }
        Statement::ExpressionStatement(expr) => {
            let item = read_expression(&expr.expression, body, array_types, start_line);
            // A statement kept as written carries the comment above it too: the source wrote it
            // over the statement, not over the node before.
            match item {
                UiItem::Raw(text) => {
                    let comments = crate::build::ets::comments::leading_comments_before(
                        body,
                        statement.span().start as usize,
                    );
                    if comments.is_empty() {
                        UiItem::Raw(text)
                    } else {
                        UiItem::Raw(format!("{}\n{text}", comments.join("\n")))
                    }
                }
                other => other,
            }
        }
        Statement::BlockStatement(_) => UiItem::Raw(statement.span().source_text(body).to_string()),
        other => UiItem::Raw(other.span().source_text(body).to_string()),
    }
}

/// The statements of a branch body, in order.
fn read_block(
    statement: &Statement<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Vec<UiItem> {
    read_statement_block(statement, body, array_types, start_line)
}

fn read_statement_block(
    statement: &Statement<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Vec<UiItem> {
    let mut items = Vec::new();
    match statement {
        Statement::BlockStatement(block) => {
            for inner in &block.body {
                items.push(read_statement(inner, body, array_types, start_line));
            }
            if let (Some(last), Some(UiItem::If(branches))) = (block.body.last(), items.last_mut())
            {
                let tail =
                    comments_between(body, last.span().end as usize, block.span.end as usize);
                if let Some(branch) = branches.last_mut() {
                    branch.tail = tail;
                }
            }
        }
        other => items.push(read_statement(other, body, array_types, start_line)),
    }
    items
}

/// The whole-line `//` comments written between two offsets.
fn comments_between(body: &str, from: usize, to: usize) -> Vec<String> {
    body.get(from..to)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("//"))
        .map(str::to_string)
        .collect()
}

/// One expression statement: a component chain, a loop, or a plain statement.
fn read_expression(
    expression: &Expression<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> UiItem {
    let Expression::CallExpression(call) = expression else {
        return UiItem::Raw(expression.span().source_text(body).to_string());
    };
    if let Some(repeat_) = read_repeat(call, body, array_types, start_line) {
        return UiItem::Repeat(repeat_);
    }
    // The chain is resolved before the name is judged: a loop may carry attribute calls of its
    // own (`ForEach(this.list, (item, index) => {…}, (item) => item.key).onMove(…)`), and those
    // belong to the loop's frame. Reading only a bare call as a loop turned such a statement
    // into a component named `ForEach` whose `create(…)` swallowed the item generator.
    let Some((component, root, attributes)) = component_chain(call, body) else {
        return UiItem::Raw(expression.span().source_text(body).to_string());
    };
    match component.as_str() {
        "ForEach" => return read_loop(root, attributes, body, array_types, start_line, false),
        "LazyForEach" => return read_loop(root, attributes, body, array_types, start_line, true),
        _ => {}
    }
    // The child block belongs to the call that constructs the component, not to the last
    // attribute in its chain.
    let children = child_block(root, body, array_types, start_line);
    // `isBlock(body)`: the trailing argument is an arrow with a block body, not an expression.
    let has_block = matches!(
        root.arguments.last(),
        Some(Argument::ArrowFunctionExpression(arrow)) if !arrow.expression
    );
    let create_args = create_arguments(root, body);
    let (line, column) = position_of(root, body, start_line);
    let comment =
        crate::build::ets::comments::leading_comments_before(body, root.span.start as usize);
    UiItem::Node(UiNode {
        component,
        line,
        col: column,
        create_args,
        has_block,
        attributes,
        children,
        comment,
    })
}

/// The component name a chain starts from, plus its attribute calls.
fn component_chain<'a, 'b>(
    call: &'b CallExpression<'a>,
    body: &str,
) -> Option<(String, &'b CallExpression<'a>, Vec<UiAttribute>)> {
    // Walk down the member-call chain until the callee is a plain identifier.
    let mut attributes = Vec::new();
    let mut current: &CallExpression<'a> = call;
    loop {
        match &current.callee {
            Expression::Identifier(_) => {
                let name = callee_name(current)?.to_string();
                attributes.reverse();
                return Some((name, current, attributes));
            }
            Expression::StaticMemberExpression(member) => {
                // `Text.fontSize(16)` in a chain reads as a member call on the previous link.
                attributes.push(UiAttribute {
                    name: member.property.name.to_string(),
                    args: call_arguments(current, body),
                });
                match &member.object {
                    Expression::CallExpression(inner) => current = inner,
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
}

/// The source text of a call's arguments.
fn call_arguments(call: &CallExpression<'_>, body: &str) -> String {
    call.arguments
        .iter()
        .map(|argument| argument.span().source_text(body).trim().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The name a call invokes, when it invokes an identifier.
fn callee_name<'a>(call: &'a CallExpression<'a>) -> Option<&'a str> {
    match &call.callee {
        Expression::Identifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

/// The `create(…)` arguments: everything the root call passes except the child callback.
fn create_arguments(root: &CallExpression<'_>, body: &str) -> Option<String> {
    let mut parts = Vec::new();
    for (index, argument) in root.arguments.iter().enumerate() {
        let is_last = index + 1 == root.arguments.len();
        if is_last && matches!(argument, Argument::ArrowFunctionExpression(_)) {
            continue;
        }
        parts.push(strip_trailing_line_comments(
            argument.span().source_text(body).trim(),
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// A copied argument without the comments that share a line with code before them.
///
/// The reference rebuilds the object it forwards and drops a property's trailing comment
/// (`QRCodeParam: { // 二维码参数` comes out as `QRCodeParam: {`), while a comment written on
/// its own line above a property is kept (`fixtures/configuration-page`, where `// 排序按钮`
/// survives and the two trailing ones do not).
fn strip_trailing_line_comments(text: &str) -> String {
    strip_line_comments(text, |_| true)
}

/// Removes the `//` comments that share a line with code, when `strip` accepts their start.
///
/// The space in front of a removed comment goes with it.
fn strip_line_comments(text: &str, strip: impl Fn(usize) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for token in crate::build::lex::tokenize(text) {
        if token.kind != crate::build::lex::Kind::Comment
            || !text[token.start..token.end].starts_with("//")
        {
            continue;
        }
        let line_start = text[..token.start]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        if text[line_start..token.start].trim().is_empty() || !strip(token.start) {
            continue;
        }
        out.push_str(&text[cursor..token.start]);
        // The space before the comment goes with it.
        while out.ends_with(' ') {
            out.pop();
        }
        cursor = token.end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// An attribute call's arguments reprinted at the call's column.
///
/// The reference rebuilds the multi-line objects it copies: the object's members sit four
/// spaces past the attribute call, the lines nested inside them keep their offset from the
/// first member (a block comment's `*` continuation moves with it), and a `//` written after
/// the member's code is dropped while another member follows. A comment on the last member
/// line survives, and so does one inside a nested arrow body (measured on
/// `fixtures/configuration-page`: `// SheetSize.LARGE` after the first member is gone,
/// `// 注册回调，防止反向回弹` after the last one stays).
///
/// A one-line argument is returned untouched. That is the shape that carries the property-line
/// comments the reference keeps — 56 across 32 files of the `clashbox-flat` copy.
fn reprint_attribute_arguments(arguments: &str, call_indent: &str) -> String {
    let text = arguments.trim();
    if !text.starts_with('{') || !text.contains('\n') {
        return text.to_string();
    }
    let text = strip_object_member_comments(text);
    let protected = template_content_lines(&text);
    // The members start at the first line below the opening one.
    let base = text
        .lines()
        .enumerate()
        .skip(1)
        .find(|(index, line)| {
            !line.trim().is_empty() && !protected.get(*index).copied().unwrap_or(false)
        })
        .map_or(0, |(_, line)| line.len() - line.trim_start().len());
    let member = call_indent.len() + 4;
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.lines().enumerate() {
        if index == 0 {
            out.push_str(line);
            continue;
        }
        out.push('\n');
        // A template literal's text is data, not layout: its lines keep every space.
        if protected.get(index).copied().unwrap_or(false) {
            out.push_str(line);
            continue;
        }
        let leading = line.len() - line.trim_start().len();
        // A closing delimiter sits further left in the source than the members do; the
        // reference prints it at the call's column, not at the member column.
        let width = if leading < base {
            call_indent.len()
        } else {
            member + (leading - base)
        };
        out.extend(std::iter::repeat_n(' ', width));
        out.push_str(line.trim_start());
    }
    out
}

/// The line indices whose content lies inside a multi-line template literal.
///
/// A template's text is data: a reprint that moved its lines would change a string value.
fn template_content_lines(text: &str) -> Vec<bool> {
    let mut lines = vec![false; text.lines().count()];
    for token in crate::build::lex::tokenize(text) {
        if token.kind != crate::build::lex::Kind::Template {
            continue;
        }
        let template = &text[token.start..token.end];
        if !template.contains('\n') {
            continue;
        }
        let first = text[..token.start].matches('\n').count();
        let last = first + template.matches('\n').count();
        for inside in lines.iter_mut().take(last + 1).skip(first + 1) {
            *inside = true;
        }
    }
    lines
}

/// The multi-line object argument without the `//` comments its rebuilt members drop.
fn strip_object_member_comments(text: &str) -> String {
    let tokens = crate::build::lex::tokenize(text);
    let mut stale = Vec::new();
    for (opening, last_member) in object_member_spans(&tokens) {
        let mut depth = 1i32;
        for token in tokens.iter().filter(|token| token.start > opening) {
            match token.kind {
                crate::build::lex::Kind::Comment => {
                    // The reference's rebuilt member owns a trailing comment as trivia; the
                    // last member's own comment belongs to the closing brace and is
                    // reprinted. A comment inside a nested body is that body's, not the
                    // object's.
                    if depth == 1
                        && token.start < last_member
                        && text[token.start..token.end].starts_with("//")
                    {
                        stale.push(token.start);
                    }
                }
                crate::build::lex::Kind::Punct => match token.text {
                    "{" | "[" | "(" => depth += 1,
                    "}" | "]" | ")" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    if stale.is_empty() {
        return text.to_string();
    }
    strip_line_comments(text, |start| stale.contains(&start))
}

/// The opening brace and last-member start of every object literal the arguments pass at the
/// top level.
fn object_member_spans(tokens: &[crate::build::lex::Token<'_>]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut depth = 0i32;
    let mut opening = None;
    for token in tokens {
        if token.kind != crate::build::lex::Kind::Punct {
            continue;
        }
        match token.text {
            "{" => {
                if depth == 0 {
                    opening = Some(token.start);
                }
                depth += 1;
            }
            "}" => {
                depth -= 1;
                if depth == 0 {
                    if let Some(open) = opening.take() {
                        if let Some(member) = last_member_start(tokens, open) {
                            spans.push((open, member));
                        }
                    }
                }
            }
            "[" | "(" => depth += 1,
            "]" | ")" => depth -= 1,
            _ => {}
        }
    }
    spans
}

/// The start of the last member inside the object literal at `opening`.
fn last_member_start(tokens: &[crate::build::lex::Token<'_>], opening: usize) -> Option<usize> {
    let mut depth = 1i32;
    let mut awaiting_member = true;
    let mut last_member = None;
    for token in tokens.iter().filter(|token| token.start > opening) {
        if token.kind == crate::build::lex::Kind::Comment {
            continue;
        }
        if token.kind != crate::build::lex::Kind::Punct {
            if awaiting_member && depth == 1 {
                last_member = Some(token.start);
                awaiting_member = false;
            }
            continue;
        }
        match token.text {
            "{" | "[" | "(" => {
                if awaiting_member && depth == 1 {
                    last_member = Some(token.start);
                    awaiting_member = false;
                }
                depth += 1;
            }
            "}" | "]" | ")" => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            "," if depth == 1 => awaiting_member = true,
            _ => {
                if awaiting_member && depth == 1 {
                    last_member = Some(token.start);
                    awaiting_member = false;
                }
            }
        }
    }
    last_member
}

/// The children a call passes as its trailing arrow function.
fn child_block(
    call: &CallExpression<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Vec<UiItem> {
    let Some(Argument::ArrowFunctionExpression(arrow)) = call.arguments.last() else {
        return Vec::new();
    };
    read_arrow(arrow, body, array_types, start_line)
}

fn read_arrow(
    arrow: &ArrowFunctionExpression<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Vec<UiItem> {
    let mut items = Vec::new();
    if arrow.expression {
        return items;
    }
    for statement in &arrow.body.statements {
        items.push(read_statement(statement, body, array_types, start_line));
    }
    // A comment written after the last child of the block belongs to no child; the synthesized
    // `else` of a bare `if` standing last prints it (`fixtures/comment-leading`, marker
    // `IF_TAIL`; `fixtures/configuration-page`, `// 悬浮新增配置按钮 END`).
    if let (Some(last), Some(UiItem::If(branches))) =
        (arrow.body.statements.last(), items.last_mut())
    {
        let tail = comments_between(body, last.span().end as usize, arrow.body.span.end as usize);
        if let Some(branch) = branches.last_mut() {
            branch.tail = tail;
        }
    }
    items
}

/// `ForEach` / `LazyForEach`: source array, item generator, optional key generator.
fn read_loop(
    call: &CallExpression<'_>,
    attributes: Vec<UiAttribute>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
    lazy: bool,
) -> UiItem {
    let source = call
        .arguments
        .first()
        .map(|argument| argument.span().source_text(body).trim().to_string())
        .unwrap_or_default();
    let generator = call.arguments.iter().find_map(|argument| match argument {
        Argument::ArrowFunctionExpression(arrow) => Some(arrow),
        _ => None,
    });
    let Some(generator) = generator else {
        return UiItem::Raw(call.span.source_text(body).to_string());
    };
    let item = binding_name(generator, body);
    let item_uses_index = generator.params.items.len() > 1;
    let index_parameter = generator
        .params
        .items
        .get(1)
        .map(|parameter| parameter.span().source_text(body).trim().to_string());
    let item_type = binding_type(generator, body).or_else(|| array_types.get(&source).cloned());
    let _ = &item_type; // the generator keeps the source's own annotation
                        // The key generator is the third argument as written, which is not always an arrow:
                        // clashbox picks between two generators with a conditional
                        // (`Xb_SettingsCommon.ets:110`). `getForEachParameters`
                        // (process_component_build.js) reads `e[2]` whole and asks only whether it declares a
                        // second parameter, so an expression that is not a bare arrow travels verbatim and
                        // reports both flags false.
    let key = call.arguments.get(2).map(|argument| {
        let uses_index = matches!(
            argument,
            Argument::ArrowFunctionExpression(arrow) if arrow.params.items.len() > 1
        );
        (
            argument.span().source_text(body).trim().to_string(),
            uses_index,
        )
    });
    let body_items = read_arrow(generator, body, array_types, start_line);
    let loop_ = UiLoop {
        comment: crate::build::ets::comments::leading_comments_before(
            body,
            call.span.start as usize,
        ),
        source,
        item,
        key,
        item_uses_index,
        attributes,
        index_parameter,
        body: body_items,
    };
    if lazy {
        UiItem::LazyForEach(loop_)
    } else {
        UiItem::ForEach(loop_)
    }
}

/// The name an arrow function binds its first parameter to.
fn binding_name(arrow: &ArrowFunctionExpression<'_>, body: &str) -> String {
    arrow
        .params
        .items
        .first()
        .map(|param| match &param.pattern.kind {
            oxc_ast::ast::BindingPatternKind::BindingIdentifier(id) => id.name.to_string(),
            _ => param.pattern.span().source_text(body).trim().to_string(),
        })
        .unwrap_or_else(|| "_item".to_string())
}

/// The declared type of an arrow function's first parameter, when it has one.
fn binding_type(arrow: &ArrowFunctionExpression<'_>, body: &str) -> Option<String> {
    let param = arrow.params.items.first()?;
    let annotation = param.pattern.type_annotation.as_ref()?;
    let text = annotation.span().source_text(body).trim();
    // oxc's annotation span starts at the ':' itself.
    Some(
        text.strip_prefix(':')
            .map(|rest| rest.trim().to_string())
            .unwrap_or_else(|| text.to_string()),
    )
}

/// The one-based line and column of a node, given the body's own starting line.
///
/// The body text is the rewritten one, so a node written inside its parent's block on the
/// parent's line is as far right as the child-block rewrite pushed it (`Foo() { X }` is read as
/// `Foo(() => { X })`). The line is unaffected — the rewrite inserts no line break — and the
/// widening the file reported is taken back from the column.
fn position_of(node: &CallExpression<'_>, body: &str, start_line: usize) -> (usize, usize) {
    let offset = node.span.start as usize;
    let line = start_line + body[..offset.min(body.len())].matches('\n').count();
    let column = offset
        - body[..offset.min(body.len())]
            .rfind('\n')
            .map_or(0, |at| at + 1);
    let column = column + 1;
    (
        line,
        column.saturating_sub(crate::build::ets_compile::column_shift(line, column)),
    )
}

/// `Repeat<T>(source).each(…)` — `processRepeatComponent` (process_component_build.js).
///
/// `recurseRepeatExpression` walks the chain outward-in: the `Repeat` call keeps its type
/// arguments and gains a trailing `this`, every other link keeps its arguments, and the two
/// callback links (`.each`, `.template`) have their body read as a build body.
fn read_repeat(
    call: &CallExpression<'_>,
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Option<UiRepeat> {
    let mut calls: Vec<&CallExpression<'_>> = Vec::new();
    let mut current: &CallExpression<'_> = call;
    loop {
        match &current.callee {
            // `Repeat<T>(source)`: the root of the chain.
            Expression::Identifier(id) if id.name == "Repeat" => break,
            Expression::StaticMemberExpression(member) => {
                calls.push(current);
                match &member.object {
                    Expression::CallExpression(inner) => current = inner,
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    // `Repeat<X>(source)` → `Repeat<X>(source, this)`. A call with no argument at all becomes
    // `Repeat(this)`: the reference always appends the receiver.
    let head_end = current
        .arguments
        .first()
        .map(|argument| argument.span().start as usize)
        .unwrap_or(current.span.end as usize);
    let head = body[current.span.start as usize..head_end]
        .trim()
        .to_string();
    let call_text = match current.arguments.last() {
        Some(last) => format!("{head}{}, this)", last.span().source_text(body).trim()),
        None => format!("{head}this)"),
    };
    calls.reverse();
    let mut links = Vec::new();
    for link in calls {
        let name = match &link.callee {
            Expression::StaticMemberExpression(member) => member.property.name.to_string(),
            _ => return None,
        };
        let generator = if name == "each" || name == "template" {
            match link.arguments.last() {
                Some(Argument::ArrowFunctionExpression(arrow)) if !arrow.expression => {
                    let params = arrow.params.span().source_text(body).trim().to_string();
                    Some((params, read_arrow(arrow, body, array_types, start_line)))
                }
                _ => None,
            }
        } else {
            None
        };
        let args = if generator.is_some() {
            None
        } else {
            let text = call_arguments(link, body);
            (!text.trim().is_empty()).then_some(text)
        };
        links.push(UiRepeatLink {
            name,
            generator,
            args,
        });
    }
    Some(UiRepeat {
        call: call_text,
        links,
    })
}

/// Render one `Repeat` chain: a single frame, whose body is the chain itself with
/// `.render(isInitialRender)` appended.
fn emit_repeat(
    repeat_: &UiRepeat,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    let inner = format!("{indent}    ");
    out.push_str(&format!("{inner}{}", repeat_.call));
    for link in &repeat_.links {
        match &link.generator {
            Some((params, items)) => {
                out.push_str(&format!(".{}({} => {{\n", link.name, params));
                let body_indent = format!("{inner}    ");
                emit(items, out, &body_indent, builders, extends, styles);
                out.push_str(&format!("{inner}}})"));
            }
            None => match &link.args {
                Some(args) => out.push_str(&format!(".{}({})", link.name, args)),
                None => out.push_str(&format!(".{}()", link.name)),
            },
        }
    }
    out.push_str(".render(isInitialRender);\n");
    out.push_str(&format!("{indent}}}, Repeat);\n"));
}

/// Render a tree as the runtime's view protocol.
pub(crate) fn emit(
    items: &[UiItem],
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    for (index, item) in items.iter().enumerate() {
        match item {
            UiItem::Node(node) => emit_node(node, out, indent, builders, extends, styles),
            UiItem::If(branches) => emit_if(
                branches,
                next_statement_comments(items, index),
                out,
                indent,
                builders,
                extends,
                styles,
            ),
            UiItem::ForEach(loop_) => emit_for_each(loop_, out, indent, builders, extends, styles),
            UiItem::Repeat(repeat_) => emit_repeat(repeat_, out, indent, builders, extends, styles),
            UiItem::LazyForEach(loop_) => {
                emit_lazy_for_each(loop_, out, indent, builders, extends, styles)
            }
            UiItem::Raw(text) => {
                out.push_str(indent);
                out.push_str(text.trim());
                out.push('\n');
            }
        }
    }
}

fn emit_node(
    node: &UiNode,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    let name = &node.component;
    // `.animation(…)` opens an animation context for everything the component writes, and the
    // context closes after the children are done.
    let animation = node
        .attributes
        .iter()
        .find(|attribute| attribute.name == "animation");
    // A `@Builder function` declared in another file is called with the component bound to it;
    // it builds no view of its own, so no frame is opened.
    if crate::build::compile::component_index::is_builder(name) {
        // The comment a builder call carries travels with it, as it does for a component: the
        // statement is the node the source wrote it above.
        push_comments(out, &node.comment, indent);
        // A lone object literal argument travels as the reference's proxy call
        // (`makeBuilderParameterProxy`), one level of thunking deep.
        let args = node.create_args.clone().unwrap_or_default();
        let args = crate::build::stage::process_custom_component::builder_param_proxy(name, &args)
            .unwrap_or(args);
        out.push_str(&format!("{indent}{name}.bind(this)({args});\n"));
        return;
    }
    // A name the project declares is a child component: its call site builds an instance and
    // hands it to the view stack, instead of calling a `create()` on a built-in.
    if let Some(meta) = crate::build::compile::component_index::meta_of(name) {
        emit_child(node, meta, out, indent, builders, extends, styles);
        return;
    }
    if name == LIST_ITEM && is_lazy_list_item(node) {
        emit_list_item(node, out, indent, builders, extends, styles);
        return;
    }
    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    push_comments(out, &node.comment, &format!("{indent}    "));
    // `processTabAndNav` (process_component_build.js): a component from
    // `TabContentAndNavDestination` takes its children as the argument of `create`, not as
    // sibling nodes — `X.create(() => { …children… })` — and its attributes follow the call.
    let inline_children = crate::build::stage::pre_define::TAB_CONTENT_AND_NAV_DESTINATION
        .contains(&name.as_str())
        && !node.children.is_empty();
    // A `NavDestination` whose body holds no statement takes the render arrow all the same:
    // `processTabAndNav`'s `d.statements.length` branch is not what decides the arrow — the
    // `navigationCreateParam` of the other branch writes `() => { }` for a destination that has
    // no children to supply one (`i = false`, so the arrow is pushed).
    let empty_destination =
        name == pre_define::COMPONENT_NAV_DESTINATION && node.children.is_empty();
    if inline_children {
        out.push_str(&format!("{indent}    {name}.create(() => {{\n"));
        let inner = format!("{indent}            ");
        emit(&node.children, out, &inner, builders, extends, styles);
        // `navigationCreateParam` (`process_component_build.js`) gives a `NavDestination`'s
        // `create` call its page coordinate; a `TabContent` takes nothing
        // (`navigationOrNavDestinationCreateContent` is reached only for the two names in
        // `CREATE_ROUTER_COMPONENT_COLLECT`).
        out.push_str(&format!(
            "{indent}    }}{});\n",
            navigation_route(name, false)
        ));
    } else if empty_destination {
        out.push_str(&format!(
            "{indent}    {name}.create(() => {{ }}{});\n",
            navigation_route(name, false)
        ));
    } else {
        let create = create_method(name, node.has_block);
        match &node.create_args {
            // A `Navigation` written with its own stack reports that the caller owns it; one
            // written without an argument starts one (`new NavPathStack()`) and reports that
            // the component does.
            Some(args) => out.push_str(&format!(
                "{indent}    {name}.{create}({}{});\n",
                bind_builder_method_calls(
                    &rewrite_builder_properties(
                        // The object literal is one argument: only its property values can carry a
                        // binding here, never the whole-argument form. There is no attribute name
                        // in this position, so the WITHOUTKEY form cannot apply.
                        &expand_dollar_bindings(&rewrite_backstores(args), name, "", false),
                        "",
                        builders,
                        false
                    ),
                    &BuilderScope {
                        builders,
                        locals: None,
                    }
                ),
                navigation_route(name, true)
            )),
            None if name == pre_define::COMPONENT_NAVIGATION => out.push_str(&format!(
                // `navigationCreateParam` pushes `new NavPathStack()` and clears its
                // `isUserCreateStack` flag in the same branch, so the two travel together.
                "{indent}    {name}.{create}(new NavPathStack(){route});\n",
                route = navigation_route(name, false)
            )),
            None => out.push_str(&format!("{indent}    {name}.{create}();\n")),
        }
    }
    // The context opens before the first write it wraps, so an `animation` written first has
    // nothing to open for: the reference emits only the close there (measured:
    // `fixtures/animation-span`, case FIRST — one `animation(null)`, no `animation({…})`).
    let animation_opens = node
        .attributes
        .iter()
        .position(|attribute| attribute.name == "animation")
        .is_some_and(|position| position > 0);
    if let (true, Some(animation)) = (animation_opens, &animation) {
        out.push_str(&format!(
            "{indent}    globalThis.Context.animation({});\n",
            rewrite_arguments(&animation.args, name, "animation", builders)
        ));
    }
    let animation_closed = emit_attributes(
        node,
        name,
        out,
        indent,
        builders,
        extends,
        styles,
        animation.is_some(),
    );
    // The context closes inside the creation callback, right after the attribute writes —
    // unless the chain closed it at the `animation` attribute's own position.
    if animation.is_some() && !animation_closed {
        out.push_str(&format!(
            "{indent}    globalThis.Context.animation(null);\n"
        ));
    }
    out.push_str(&format!("{indent}}}, {name});\n"));
    if !inline_children {
        let child_indent = format!("{indent}    ");
        emit(
            &node.children,
            out,
            &child_indent,
            builders,
            extends,
            styles,
        );
    }
    // The reference pops only where a child stack exists: a non-atomic component always has
    // one, an atomic one never does, and `Blank`/`Search` are the two atomic components that
    // still do. The comment's second position is the `pop()` itself, so a component that is
    // never popped gets no copy there (measured: `fixtures/proxy-node-item`, whose `Radio`
    // frame keeps four copies and none before the following `Row.pop()`).
    let pops = !crate::build::ets::types::is_atomic_component(name)
        || matches!(name.as_str(), "Blank" | "Search");
    if pops {
        push_comments(out, &node.comment, indent);
        out.push_str(&format!("{indent}{name}.pop();\n"));
    }
}

/// The call site of a component the project declares.
///
/// The produced shape is the reference's: an instance is built once, the view stack is told to
/// take it, and a re-render only forwards the state the parent owns. A `@BuilderParam` field
/// receives the call site's child block as an arrow function inside the params object.
/// `new Name(` with the component's comment block printed between `new` and the name.
///
/// The reference's printer emits the leading trivia where it appends the construction, so the
/// block lands inside the expression (measured: `fixtures/configuration-page`,
/// `let componentCall = new ` / `// 文件导入的` / `ImportConfigFromFile(this, …)`; the same shape
/// is the probe `CHILD_CALL` in `fixtures/comment-leading`).
fn child_construction_head(node: &UiNode, name: &str, indent: &str) -> String {
    let mut head = String::from("new ");
    for comment in &node.comment {
        head.push_str(&format!("\n{indent}            {comment}"));
    }
    if !node.comment.is_empty() {
        head.push_str(&format!("\n{indent}            "));
    }
    head.push_str(name);
    head.push('(');
    head
}

fn emit_child(
    node: &UiNode,
    meta: &crate::build::compile::component_index::Meta,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    let name = &node.component;
    // A `@Reusable` child goes through the recycle entry point: the view stack keeps one
    // instance alive and hands a recycled node back to the creation callback.
    if meta.is_reusable {
        let (construct_params, lambda_params) =
            child_params(node, meta, indent, builders, extends, styles);
        emit_recyclable_child(
            node,
            meta,
            out,
            indent,
            &construct_params,
            &lambda_params,
            builders,
        );
        return;
    }
    // `@ReusableV2` takes the V2 recycle entry point instead of the whole observe/create frame:
    // `createCustomComponent` picks `REUSE_OR_CREATE_METHOD` over `OBSERVECOMPONENTCREATION2`
    // when the child is a reuse-V2 component, and `generateReuseOrCreateArgArr` then supplies
    // the four properties. Measured: `fixtures/_pending-v2-reuse` golden.
    if meta.is_reusable_v2 {
        emit_reuse_or_create_child(node, out, indent);
        return;
    }
    let page = crate::build::ets_compile::source_page().unwrap_or_default();
    let (construct_params, lambda_params) =
        child_params(node, meta, indent, builders, extends, styles);
    let view_class = match meta.protocol {
        crate::build::compile::component_index::Protocol::V2 => "ViewV2",
        crate::build::compile::component_index::Protocol::V1 => "ViewPU",
    };
    // Inside a `@LocalBuilder` body the construction attaches to the component the builder was
    // bound to, not to the class it was written in.
    let parent = if in_local_builder() {
        BUILDER_PARENT_EXPR
    } else {
        "this"
    };
    // An attribute chain on the construction (`Child({…}).transition(…)`) is not a property of
    // the child: the reference opens a `__Common__` frame around it
    // (process_custom_component.js `processCustomComponent`, guarded by
    // `isPropertyAccessExpression(y.parent)`), writes the chain onto that frame, and pops it
    // after the construction.
    let common = !node.attributes.is_empty();
    if common {
        emit_common_frame(node, out, indent, builders);
    }
    out.push_str(&format!("{indent}{{\n"));
    out.push_str(&format!(
        "{indent}    {}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    out.push_str(&format!("{indent}        if (isInitialRender) {{\n"));
    out.push_str(&format!(
        "{indent}            let componentCall = {head}{parent}, {construct_params}, undefined, elmtId, () => {{ }}, {{ page: \"{page}\", line: {}, col: {} }});\n",
        node.line,
        node.col,
        head = child_construction_head(node, name, indent),
    ));
    out.push_str(&format!(
        "{indent}            {view_class}.create(componentCall);\n"
    ));
    out.push_str(&format!(
        "{indent}            let paramsLambda = () => {{\n"
    ));
    out.push_str(&format!(
        "{indent}                return {lambda_params};\n"
    ));
    out.push_str(&format!("{indent}            }};\n"));
    out.push_str(&format!(
        "{indent}            componentCall.paramsGenerator_ = paramsLambda;\n"
    ));
    out.push_str(&format!("{indent}        }}\n"));
    out.push_str(&format!("{indent}        else {{\n"));
    // The update call takes the same receiver the construction did:
    // `createComponentCreationStatement` writes `a ? (parent ? parent : this) : this` for it
    // (process_custom_component.js), where `a` is the `isBuilder` flag the builder body set.
    out.push_str(&format!(
        "{indent}            {}.{}(elmtId, {});\n",
        frame_receiver(),
        UPDATE_STATE_VARS_OF_CHIND_BY_ELMTID,
        state_vars_object(node, meta)
    ));
    out.push_str(&format!("{indent}        }}\n"));
    // The frame's name is the innermost identifier: a component reached through a namespace
    // import constructs as `new Names.Card(…)` but registers the frame as `{ name: "Card" }`
    // (`process_component_class.js`: `isAsPageImport ? e.getText() : e.name.escapedText` decides
    // the lookup key, while the emitted name comes from the declaration).
    let frame_name = name.rsplit('.').next().unwrap_or(name.as_str());
    out.push_str(&format!("{indent}    }}, {{ name: \"{frame_name}\" }});\n"));
    // A component with a `@BuilderParam` takes its children through that field; one without
    // takes them as ordinary child nodes after the creation call.
    if meta.builder_param.is_none() {
        let inner = format!("{indent}    ");
        emit(&node.children, out, &inner, builders, extends, styles);
    }
    out.push_str(&format!("{indent}}}\n"));
    if common {
        out.push_str(&format!("{indent}__Common__.pop();\n"));
    }
}

/// The `__Common__` frame an attribute chain on a child construction opens.
///
/// `process_custom_component.js` `processCustomComponent` seeds the statement list with
/// `__Common__.create()`, hands the chain to `bindComponentAttr` with `__Common__` as the
/// receiver, and emits the `observeComponentCreation2` frame only when that produced more than
/// the seed (`a = 1 < T.length || 0 < O.length`).
fn emit_common_frame(node: &UiNode, out: &mut String, indent: &str, builders: &[Builder]) {
    let name = &node.component;
    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    out.push_str(&format!("{indent}    __Common__.create();\n"));
    for attribute in &node.attributes {
        if attribute.name == "animation" {
            continue;
        }
        // `bindComponentAttr` runs the same attribute paths on this receiver a built-in gets,
        // so `.gesture(G)` opens the gesture frame instead of calling `__Common__.gesture`.
        if attribute.name == "gesture" {
            emit_gesture(&attribute.args, out, indent, builders);
            continue;
        }
        out.push_str(&format!(
            "{indent}    __Common__.{}({});\n",
            attribute.name,
            rewrite_arguments(&attribute.args, name, &attribute.name, builders)
        ));
    }
    out.push_str(&format!("{indent}}}, __Common__);\n"));
}

/// The call site of a `@Reusable` component.
///
/// The recycle container is opened first, then the reuse callback gets either the recycled node
/// or a fresh instance, and the container is closed after the callback. The instance's params
/// are read the same way an ordinary child's are.
fn emit_recyclable_child(
    node: &UiNode,
    meta: &crate::build::compile::component_index::Meta,
    out: &mut String,
    indent: &str,
    construct_params: &str,
    lambda_params: &str,
    builders: &[Builder],
) {
    let name = &node.component;
    let page = crate::build::ets_compile::source_page().unwrap_or_default();
    let view_class = match meta.protocol {
        crate::build::compile::component_index::Protocol::V2 => "ViewV2",
        crate::build::compile::component_index::Protocol::V1 => "ViewPU",
    };
    let parent = if in_local_builder() {
        BUILDER_PARENT_EXPR
    } else {
        "this"
    };
    // The attribute chain of a reusable child opens its `__Common__` frame ahead of the recycle
    // container, and closes it before the container is popped
    // (measured: `ProxyGroupItem.ts`, the `__Recycle__` shape).
    let common = !node.attributes.is_empty();
    if common {
        emit_common_frame(node, out, indent, builders);
    }
    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    out.push_str(&format!("{indent}    __Recycle__.create();\n"));
    out.push_str(&format!("{indent}}}, __Recycle__);\n"));
    out.push_str(&format!("{indent}{{\n"));
    out.push_str(&format!(
        "{indent}    this.observeRecycleComponentCreation(\"{name}\", (elmtId, isInitialRender, recycleNode = null) => {{\n"
    ));
    out.push_str(&format!(
        "{indent}        ViewStackProcessor.StartGetAccessRecordingFor(elmtId);\n"
    ));
    out.push_str(&format!("{indent}        if (isInitialRender) {{\n"));
    out.push_str(&format!(
        "{indent}            let componentCall = recycleNode ? recycleNode : {head}{parent}, {construct_params}, undefined, elmtId, () => {{ }}, {{ page: \"{page}\", line: {}, col: {} }});\n",
        node.line,
        node.col,
        head = child_construction_head(node, name, indent),
    ));
    out.push_str(&format!(
        "{indent}            {view_class}.createRecycle(componentCall, recycleNode !== null, \"{name}\", () => {{\n"
    ));
    out.push_str(&format!(
        "{indent}                if (recycleNode && typeof recycleNode.aboutToReuseInternal === \"function\") {{\n"
    ));
    out.push_str(&format!(
        "{indent}                    recycleNode.aboutToReuseInternal();\n"
    ));
    out.push_str(&format!("{indent}                }}\n"));
    out.push_str(&format!("{indent}                else {{\n"));
    out.push_str(&format!(
        "{indent}                    if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === \"function\") {{\n"
    ));
    out.push_str(&format!(
        "{indent}                        recycleNode.aboutToReuse({lambda_params});\n"
    ));
    out.push_str(&format!("{indent}                    }}\n"));
    out.push_str(&format!(
        "{indent}                    recycleNode.rerender();\n"
    ));
    out.push_str(&format!("{indent}                }}\n"));
    out.push_str(&format!("{indent}            }});\n"));
    out.push_str(&format!(
        "{indent}            let paramsLambda = () => {{\n"
    ));
    out.push_str(&format!(
        "{indent}                return {lambda_params};\n"
    ));
    out.push_str(&format!("{indent}            }};\n"));
    out.push_str(&format!(
        "{indent}            componentCall.paramsGenerator_ = paramsLambda;\n"
    ));
    out.push_str(&format!("{indent}        }}\n"));
    out.push_str(&format!("{indent}        else {{\n"));
    // The update call takes the same receiver the construction did:
    // `createComponentCreationStatement` writes `a ? (parent ? parent : this) : this` for it
    // (process_custom_component.js), where `a` is the `isBuilder` flag the builder body set.
    out.push_str(&format!(
        "{indent}            {}.{}(elmtId, {});\n",
        frame_receiver(),
        UPDATE_STATE_VARS_OF_CHIND_BY_ELMTID,
        state_vars_object(node, meta)
    ));
    out.push_str(&format!("{indent}        }}\n"));
    out.push_str(&format!(
        "{indent}        ViewStackProcessor.StopGetAccessRecording();\n"
    ));
    out.push_str(&format!("{indent}    }});\n"));
    out.push_str(&format!("{indent}}}\n"));
    if common {
        out.push_str(&format!("{indent}__Common__.pop();\n"));
    }
    out.push_str(&format!("{indent}__Recycle__.pop();\n"));
}

// The argument passes run in this order, and the order is load-bearing:
//
//   rewrite_backstores        the field name is fixed first (`this.count` → `this.__count`);
//   expand_dollar_bindings    `$$` expands into a `changeEvent` body that writes the final
//                             field name, so it must run after the backstore rewrite;
//   rewrite_builder_properties the `builder:` property form;
//   as_bound_builder          the whole-argument form, `{ builder: () => … }`.
//
// The two builder passes rewrite calls into `this.X.call(this, …)`; running them earlier would
// let the other passes walk over that generated text a second time.

/// Attribute arguments carry two ArkUI spellings the runtime does not accept.
///
/// * `$$expr` is a two-way binding: it becomes the value/change-event pair the runtime reads.
/// * `this.buildSomething()` names a builder: it becomes the `{ builder: … }` wrapper, and the
///   call keeps its receiver through `.call(this)`.
fn rewrite_arguments(args: &str, component: &str, attribute: &str, builders: &[Builder]) -> String {
    if args.trim().is_empty() {
        return String::new();
    }
    // `processDragStartBuilder`: a drag callback's `return` value is a builder, and it takes the
    // builder-value spelling before the per-argument passes see the text.
    let dragged = DRAG_START_ATTRIBUTES
        .contains(&attribute)
        .then(|| rewrite_drag_start_returns(args, attribute, builders))
        .flatten();
    let args = dragged.as_deref().unwrap_or(args);
    let scope = BuilderScope {
        builders,
        locals: None,
    };
    let arguments = crate::build::lex::split_top_level_commas(args);
    // `classifyArgumentsNum` only takes an attribute's two-way argument when the call has two or
    // more arguments and it is the first one (`STYLE_ADD_DOUBLE_DOLLAR.has(r)&&2<=e.length`
    // reading `e[0]`, and the same shape for the `!!` set); that fact travels with the argument.
    let argument_count = arguments.len();
    let rewritten: Vec<String> = arguments
        .into_iter()
        .enumerate()
        .map(|(index, argument)| {
            let text = rewrite_builder_properties(
                &expand_dollar_bindings(
                    argument.trim(),
                    component,
                    attribute,
                    argument_count >= 2 && index == 0,
                ),
                attribute,
                builders,
                attribute == "bindPopup",
            );
            // `isDoubleDollarToChange` (process_component_build.js) matches
            // `/^(?!\$\$\.)\$\$/`: a `$$` that names a two-way binding, never a read through
            // something already called `$$` (a builder's `$$` parameter, `$$.tooltipInfo`).
            if let Some(rest) = text.strip_prefix("$$").filter(|rest| !rest.starts_with('.')) {
                let expression = rest.trim().to_string();
                return format!(
                    "{{ value: {expression}, changeEvent: newValue => {{ {expression} = newValue; }} }}"
                );
            }
            // A builder method called where a builder is expected keeps its receiver as a
            // partial application: `header: this.headerB(row.name)` is emitted as
            // `this.headerB.bind(this, row.name)`, and the argument-less form as
            // `this.headerB.bind(this)`. Only attributes that are not the `builder:` property
            // take this spelling — that one goes through `parseBuilderNode` and keeps the shape
            // that function writes (measured: `fixtures/property-access-this`).
            if !bind_object_property(attribute) {
                if let Some(bound) = bound_builder_call(&text, &scope) {
                    return bound;
                }
            }
            // `parseBuilderNode` (`process_component_build.js`) decides every remaining builder
            // value from its node: a call becomes the arrow that keeps `this`, a reference the
            // `.bind(this)` spelling.
            if bind_object_property(attribute) {
                if let Some(rewritten) = builder_argument_value(&text, attribute, builders) {
                    return rewritten;
                }
            }
            text
        })
        .collect();
    rewritten.join(", ")
}

// ---------------------------------------------------------------------------
// Builder spellings, decided on the AST
// ---------------------------------------------------------------------------
//
// `process_component_build.js` never asks what an expression's text looks like: it asks what
// node it is. A statement is a builder call when `getComponentType` sees a `CallExpression`
// whose callee names a builder; a value is a builder when `isBuilderChangeNode` sees a call, a
// `this.<name>` access, an identifier in `builderLikeCollection`, or a conditional whose arms
// are builders; a drag callback's `return` goes through the same decision (`checkStatement` →
// `processInitializer` → `parseBuilderNode`).
//
// The passes below read the same nodes. Each one keeps the layout by replacing source spans,
// so a node's position in the emitted source stays where the lowering put it.

/// The attributes whose callback returns a builder (`BIND_DRAG_SET`, `pre_define.js`).
const DRAG_START_ATTRIBUTES: &[&str] = &["onDragStart", "onItemDragStart"];

/// The names one emission may spell as a builder: the struct's `@Builder` methods and
/// `@BuilderParam` fields (`builders`) plus the file's `@LocalBuilder` functions (`locals`).
///
/// `storedFileInfo.builderLikeCollection` (`process_component_class.js`) holds the same three
/// sets, and `isBuilderChangeNode` / `getComponentType` read every decision from it.
struct BuilderScope<'a> {
    builders: &'a [Builder],
    /// The file's `@LocalBuilder` names, when the emission knows them.
    locals: Option<&'a std::collections::BTreeSet<String>>,
}

impl BuilderScope<'_> {
    fn names(&self, name: &str) -> bool {
        in_builder_scope(name, self.builders)
            || self.locals.is_some_and(|locals| locals.contains(name))
    }
}

/// Read `text` as one expression and hand the node to `f` together with the byte offset the
/// parse added. The text is written inside parentheses so that a leading `{` stays an object
/// literal rather than becoming a block.
fn with_expression<T>(
    text: &str,
    f: impl for<'a, 'b> FnOnce(&'a Expression<'b>, usize) -> T,
) -> Option<T> {
    let wrapped = format!("({text})");
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &wrapped, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    let statement = ret.program.body.first()?;
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::ParenthesizedExpression(paren) = &statement.expression else {
        return None;
    };
    Some(f(&paren.expression, 1))
}

/// Run `f` over a statement list, or hand back `None` when the text does not parse.
fn with_statements<T>(
    body: &str,
    f: impl for<'a, 'b> FnOnce(&'a [Statement<'b>]) -> T,
) -> Option<T> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, body, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    Some(f(&ret.program.body))
}

/// The source of one node, as written.
fn node_text(node: &impl GetSpan, source: &str, shift: usize) -> String {
    let start = node.span().start as usize;
    let end = node.span().end as usize;
    source
        .get(start - shift..end - shift)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The arguments of a call, as written, without the parentheses.
fn arguments_text(call: &CallExpression<'_>, source: &str, shift: usize) -> String {
    let (Some(first), Some(last)) = (call.arguments.first(), call.arguments.last()) else {
        return String::new();
    };
    let start = first.span().start as usize;
    let end = last.span().end as usize;
    source
        .get(start - shift..end - shift)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// `this.<name>` naming a builder: the node facts `isPropertyAccessExpressionNode` joins.
fn member_builder_name<'a>(
    member: &'a oxc_ast::ast::StaticMemberExpression<'a>,
    scope: &BuilderScope<'_>,
) -> Option<&'a str> {
    if !matches!(member.object, Expression::ThisExpression(_)) {
        return None;
    }
    let name = member.property.name.as_str();
    scope.names(name).then_some(name)
}

/// `this.<name>` naming a builder, from the expression form.
fn this_builder_name<'a>(
    expression: &'a Expression<'a>,
    scope: &BuilderScope<'_>,
) -> Option<&'a str> {
    match expression {
        Expression::StaticMemberExpression(member) => member_builder_name(member, scope),
        _ => None,
    }
}

/// The callee a builder call names, as it is written (`this.X` or `X`), plus the bare name.
fn builder_callee<'a>(
    callee: &'a Expression<'a>,
    scope: &BuilderScope<'_>,
) -> Option<(String, &'a str)> {
    if let Some(name) = this_builder_name(callee, scope) {
        return Some((format!("this.{name}"), name));
    }
    let Expression::Identifier(identifier) = callee else {
        return None;
    };
    let name = identifier.name.as_str();
    scope.names(name).then(|| (name.to_string(), name))
}

/// `this.<builder>(args)` written where a partial application is expected: the bound spelling
/// with the call's arguments as the binding's parameters, `this.X.bind(this, args)`.
///
/// `checkArguments` (`process_component_build.js`) hands every argument of a
/// `CUSTOM_BUILDER_CONSTRUCTORS` create call to `parseBuilderNode`; the node is a call whose
/// callee is a property access on `this`, and the arguments become the binding's parameters
/// (measured: `fixtures/property-access-this`, `header` and `footer` on `ListItemGroup`).
fn bound_builder_call(text: &str, scope: &BuilderScope<'_>) -> Option<String> {
    with_expression(text, |expression, shift| {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let name = this_builder_name(&call.callee, scope)?;
        let arguments = arguments_text(call, text, shift);
        Some(if arguments.is_empty() {
            format!("this.{name}.bind(this)")
        } else {
            format!("this.{name}.bind(this, {arguments})")
        })
    })
    .flatten()
}

/// Bind every builder-method call written inside an attribute object.
///
/// Built-in components take their attributes as one object literal, so the per-attribute
/// rewriting in [`rewrite_arguments`] never sees `header:` / `footer:`; `transformBuilder`
/// (`process_component_build.js`) walks the object's members instead. `builder:` members are
/// already in their final spelling by the time this runs, so neither `this.X.bind(this)` nor
/// `this.X.call(this, …)` is matched again.
fn bind_builder_method_calls(text: &str, scope: &BuilderScope<'_>) -> String {
    let mut collector = BuilderCallCollector {
        source: text,
        scope,
        edits: Vec::new(),
    };
    let wrapped = format!("f({text});");
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &wrapped, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return text.to_string();
    }
    collector.visit_program(&ret.program);
    let edits: Vec<(usize, usize, String)> = collector
        .edits
        .into_iter()
        .map(|(start, end, replacement)| {
            (
                start - EXPRESSION_WRAPPER.len(),
                end - EXPRESSION_WRAPPER.len(),
                replacement,
            )
        })
        .collect();
    crate::build::ets_assemble::apply_replacements(text, &edits)
}

/// The bytes [`bind_builder_method_calls`] puts in front of the arguments it parses.
const EXPRESSION_WRAPPER: &str = "f(";

/// Span collector for [`bind_builder_method_calls`].
struct BuilderCallCollector<'a, 'b> {
    source: &'b str,
    scope: &'b BuilderScope<'a>,
    edits: Vec<(usize, usize, String)>,
}

impl<'a> oxc_ast_visit::Visit<'a> for BuilderCallCollector<'_, '_> {
    /// An arrow's body is a closure the reference leaves as written, so the walk stops here.
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(name) = this_builder_name(&call.callee, self.scope) {
            let arguments = arguments_text(call, self.source, EXPRESSION_WRAPPER.len());
            let replacement = if arguments.is_empty() {
                format!("this.{name}.bind(this)")
            } else {
                format!("this.{name}.bind(this, {arguments})")
            };
            self.edits.push((
                call.span.start as usize,
                call.span.end as usize,
                replacement,
            ));
            return;
        }
        walk::walk_call_expression(self, call);
    }
}

/// One arrow whose body the reference renders as UI.
///
/// `processComponentChild` walks the statements of exactly these bodies — a component
/// construction's child block (`processNormalComponent` / `processCustomComponent`), a
/// `ForEach` / `LazyForEach` item generator (`processForEachBlock`), and a
/// `Repeat(…).each` / `.template` callback (`processRepeatPropWithChild`). An arrow written as
/// an ordinary callback (`.onClick(() => …)`, `builder: () => …`) is a closure whose body the
/// reference does not render, so its statements keep the spelling the source gave them.
fn ui_body_arrows<'a>(
    call: &'a CallExpression<'a>,
    scope: &BuilderScope<'_>,
) -> Vec<&'a ArrowFunctionExpression<'a>> {
    let mut arrows = Vec::new();
    // `Repeat(…).each(…)` / `.template(…)`: the generator rides on the chain link itself.
    let mut link = Some(call);
    while let Some(current) = link {
        let Expression::StaticMemberExpression(member) = &current.callee else {
            break;
        };
        if matches!(member.property.name.as_str(), "each" | "template") {
            if let Some(arrow) = current.arguments.iter().find_map(block_arrow) {
                arrows.push(arrow);
            }
        }
        link = match &member.object {
            Expression::CallExpression(inner) => Some(inner),
            _ => None,
        };
    }
    // The child block is the root call's trailing arrow; a `ForEach`'s item generator is its
    // second argument, and its key generator is not a UI body.
    let root = root_call(call);
    if let Expression::Identifier(identifier) = &root.callee {
        if matches!(identifier.name.as_str(), "ForEach" | "LazyForEach") {
            if let Some(arrow) = root.arguments.get(1).and_then(block_arrow) {
                arrows.push(arrow);
            }
            return arrows;
        }
        if !is_component_name(identifier.name.as_str()) && !scope.names(identifier.name.as_str()) {
            return arrows;
        }
    } else if this_builder_name(&root.callee, scope).is_none() {
        return arrows;
    }
    if let Some(arrow) = root.arguments.last().and_then(block_arrow) {
        arrows.push(arrow);
    }
    arrows
}

/// The innermost call of a member chain (`Column(…)` in `Column(…).width(100)`).
fn root_call<'a>(call: &'a CallExpression<'a>) -> &'a CallExpression<'a> {
    let mut current = call;
    loop {
        let Expression::StaticMemberExpression(member) = &current.callee else {
            return current;
        };
        match &member.object {
            Expression::CallExpression(inner) => current = inner,
            _ => return current,
        }
    }
}

/// A component construction is written with a capitalized name (`getComponentType` reaches the
/// same fact through `componentCollection.customComponents` / `INNER_COMPONENT_NAMES`).
fn is_component_name(name: &str) -> bool {
    name.starts_with(|first: char| first.is_ascii_uppercase())
        || crate::build::compile::component_index::is_builder(name)
}

/// The arrow argument whose body is a block, when the argument is one.
fn block_arrow<'a, 'b>(argument: &'a Argument<'b>) -> Option<&'a ArrowFunctionExpression<'b>> {
    match argument {
        Argument::ArrowFunctionExpression(arrow) if !arrow.expression => Some(arrow),
        _ => None,
    }
}

/// Bind every statement-level builder call in one UI body: `this.X(args)` becomes
/// `this.X.bind(this)(args)`.
///
/// `transferBuilderCall` (`process_component_build.js`) is reached from `processComponentChild`
/// for `ComponentType.customBuilderMethod` / `builderParamMethod`, which is why the walk follows
/// the UI-body arrows [`ui_body_arrows`] reports and nothing else.
fn bind_ui_builder_statements(body: &str, scope: &BuilderScope<'_>) -> String {
    let mut edits = Vec::new();
    with_statements(body, |statements| {
        bind_ui_statements(statements, body, scope, &mut edits)
    });
    crate::build::ets_assemble::apply_replacements(body, &edits)
}

/// The statements of one UI body.
fn bind_ui_statements(
    statements: &[Statement<'_>],
    body: &str,
    scope: &BuilderScope<'_>,
    edits: &mut Vec<(usize, usize, String)>,
) {
    for statement in statements {
        match statement {
            Statement::ExpressionStatement(expression) => {
                bind_ui_expression(&expression.expression, body, scope, edits);
            }
            // `processComponentChild` reaches an `if` branch from the same statement walk
            // (`processIfStatement`), and a bare block keeps its statements.
            Statement::IfStatement(node) => {
                bind_ui_statement(&node.consequent, body, scope, edits);
                if let Some(alternate) = &node.alternate {
                    bind_ui_statement(alternate, body, scope, edits);
                }
            }
            Statement::BlockStatement(block) => bind_ui_statements(&block.body, body, scope, edits),
            _ => {}
        }
    }
}

/// One statement of a UI body, which may itself be a block or an `if`.
fn bind_ui_statement(
    statement: &Statement<'_>,
    body: &str,
    scope: &BuilderScope<'_>,
    edits: &mut Vec<(usize, usize, String)>,
) {
    bind_ui_statements(std::slice::from_ref(statement), body, scope, edits);
}

/// One statement expression: a builder call takes the bound form, a construction opens its UI
/// bodies.
fn bind_ui_expression(
    expression: &Expression<'_>,
    body: &str,
    scope: &BuilderScope<'_>,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let Expression::CallExpression(call) = expression else {
        return;
    };
    if let Some(name) = this_builder_name(&call.callee, scope) {
        let insert = call.callee.span().end as usize;
        edits.push((insert, insert, ".bind(this)".to_string()));
        // `transDoubleDollarInCustomBuilder`: a call whose only argument is an object literal
        // travels as the reference's proxy call.
        if call.arguments.len() == 1 {
            if let Argument::ObjectExpression(object) = &call.arguments[0] {
                let start = object.span.start as usize;
                let end = object.span.end as usize;
                if let Some(proxy) =
                    crate::build::stage::process_custom_component::builder_param_proxy(
                        name,
                        body.get(start..end).unwrap_or_default(),
                    )
                {
                    edits.push((start, end, proxy));
                }
            }
        }
        return;
    }
    for arrow in ui_body_arrows(call, scope) {
        bind_ui_statements(&arrow.body.statements, body, scope, edits);
    }
}

/// The builder-value spelling of one expression: `{ builder: … }` around a reference, or the
/// arrow that calls the builder with `this` fixed.
///
/// `parseBuilderNode` (`process_component_build.js`) is the single place this decision is made
/// for an attribute value; `checkStatement` reaches it for a drag callback's `return` value.
fn builder_value(
    expression: &Expression<'_>,
    source: &str,
    attribute: &str,
    scope: &BuilderScope<'_>,
    shift: usize,
) -> Option<String> {
    let without_key = custom_builder_properties_without_key(attribute);
    match expression {
        Expression::CallExpression(call) => {
            let (callee, name) = builder_callee(&call.callee, scope)?;
            let arguments = forwarded_builder_arguments(name, call, source, shift);
            let call_text = format!("{callee}.call(this{arguments})");
            Some(if without_key {
                format!("() => {{ {call_text} }}")
            } else {
                format!("{{ builder: () => {{ {call_text} }} }}")
            })
        }
        Expression::StaticMemberExpression(_) => {
            let name = this_builder_name(expression, scope)?;
            Some(if without_key {
                format!("this.{name}.bind(this)")
            } else {
                format!("{{ builder: this.{name}.bind(this) }}")
            })
        }
        Expression::Identifier(identifier) if scope.names(identifier.name.as_str()) => {
            let name = identifier.name.as_str();
            Some(if without_key {
                format!("{name}.bind(this)")
            } else {
                format!("{{ builder: {name}.bind(this) }}")
            })
        }
        _ => None,
    }
}

/// The arguments a builder call forwards: a lone object literal travels as the reference's
/// proxy call, everything else as written.
fn forwarded_builder_arguments(
    name: &str,
    call: &CallExpression<'_>,
    source: &str,
    shift: usize,
) -> String {
    if call.arguments.len() == 1 {
        if let Argument::ObjectExpression(object) = &call.arguments[0] {
            let start = object.span.start as usize;
            let end = object.span.end as usize;
            let text = source
                .get(start - shift..end - shift)
                .unwrap_or_default()
                .trim();
            if let Some(proxy) =
                crate::build::stage::process_custom_component::builder_param_proxy(name, text)
            {
                return format!(", {proxy}");
            }
        }
    }
    let arguments = arguments_text(call, source, shift);
    if arguments.is_empty() {
        String::new()
    } else {
        format!(", {arguments}")
    }
}

/// `onDragStart` / `onItemDragStart`: the callback's `return` value is a builder.
///
/// `processDragStartBuilder` (`process_component_build.js`) walks the arrow's statements and
/// hands every returned expression to `parseBuilderNode` — `checkStatement` → `processInitializer`
/// — so a returned builder call becomes `{ builder: () => { this.X.call(this, …) } }`.
fn rewrite_drag_start_returns(args: &str, attribute: &str, builders: &[Builder]) -> Option<String> {
    let scope = BuilderScope {
        builders,
        locals: None,
    };
    let shift = EXPRESSION_WRAPPER.len();
    let wrapped = format!("{EXPRESSION_WRAPPER}{args})");
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &wrapped, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    let statement = ret.program.body.first()?;
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::CallExpression(call) = &statement.expression else {
        return None;
    };
    let arrow = block_arrow(call.arguments.first()?)?;
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for statement in &arrow.body.statements {
        let Statement::ReturnStatement(statement) = statement else {
            continue;
        };
        let Some(argument) = &statement.argument else {
            continue;
        };
        if let Some(replacement) = builder_value(argument, args, attribute, &scope, shift) {
            edits.push((
                argument.span().start as usize - shift,
                argument.span().end as usize - shift,
                replacement,
            ));
        }
    }
    if edits.is_empty() {
        return None;
    }
    Some(crate::build::ets_assemble::apply_replacements(args, &edits))
}

/// `$$expr` written as a property value: `{ value: expr, changeEvent: newValue => { expr = newValue; } }`.
///
/// `processDollarEtsComponent` (`process_component_build.js`) applies this to the properties in
/// `PROPERTIES_ADD_DOUBLE_DOLLAR` — `Toggle.isOn`, `Radio.checked`, `Slider.value`, … — so a
/// `Toggle({ isOn: $$this.uiConfig.isVibrate })` reaches the runtime with both halves of the
/// two-way binding. The `$$` spelling only ever names this binding (`isDoubleDollarToChange`
/// rejects `$$.`), so the value position alone decides; nothing else in an argument list starts
/// with `$$`.
pub(crate) fn expand_dollar_bindings(
    text: &str,
    component: &str,
    attribute: &str,
    binding_argument: bool,
) -> String {
    let text = expand_dollar_values(text, component);
    expand_exclamation_values(&text, component, attribute, binding_argument)
}

/// The `$$` half of [`expand_dollar_bindings`].
fn expand_dollar_values(text: &str, component: &str) -> String {
    if !text.contains("$$") {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'$' {
            let previous = text[..i].trim_end().chars().next_back();
            let value_position = matches!(
                previous,
                Some(':') | Some(',') | Some('(') | Some('{') | Some('[')
            );
            let names_binding = !text[i + 2..].starts_with('.');
            let property = property_name_before(text, i);
            let allowed = property
                .as_deref()
                .is_some_and(|name| double_dollar_property(component, name));
            if value_position && names_binding && allowed {
                let mut depth = 0i32;
                let mut end = text.len();
                for (offset, ch) in text[i + 2..].char_indices() {
                    match ch {
                        '(' | '{' | '[' => depth += 1,
                        ')' | '}' | ']' if depth == 0 => {
                            end = i + 2 + offset;
                            break;
                        }
                        ')' | '}' | ']' => depth -= 1,
                        ',' if depth == 0 => {
                            end = i + 2 + offset;
                            break;
                        }
                        _ => {}
                    }
                }
                let expression = text[i + 2..end].trim();
                if !expression.is_empty() && !expression.contains("=>") {
                    out.push_str(&text[copied..i]);
                    out.push_str(&format!(
                        "{{ value: {expression}, changeEvent: newValue => {{ {expression} = newValue; }} }}"
                    ));
                    copied = end;
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

/// The postfix `!!` half of [`expand_dollar_bindings`].
///
/// `generateObjectForExclamation` (`process_component_build.js`) writes a different pair from
/// the `$$` one — `{ value: e, $value: newValue => { e = newValue; } }`, with `$value` where
/// `$$` has `changeEvent` — and `isDoubleExclamationToChange` gates it on two independent
/// things: the attribute is in `STYLE_ADD_DOUBLE_EXCLAMATION` and the call has two or more
/// arguments, or the component/property pair is in `PROPERTIES_ADD_DOUBLE_EXCLAMATION`
/// (measured 2026-11-07: `fixtures/double-exclamation` emits both forms, and leaves
/// `this.value!! as number` and `cond ? this.x!! : null` untouched because neither argument is
/// itself the double non-null expression).
fn expand_exclamation_values(
    text: &str,
    component: &str,
    attribute: &str,
    binding_argument: bool,
) -> String {
    if !text.contains("!!") {
        return text.to_string();
    }
    // The whole argument is the binding: `.bindPopup(this.isShowMenu!!, { … })`.
    if binding_argument
        && crate::build::stage::pre_define::EXCLAMATION_ATTRIBUTES.contains(&attribute)
    {
        if let Some(expression) = double_non_null(text) {
            return exclamation_object(expression);
        }
        return text.to_string();
    }
    // A property value of the child's params object: `Slider({ value: this.value!! })`. The
    // `!!` has to end the value, and the value is the text between the property's `:` and it.
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    let bytes = text.as_bytes();
    while i + 1 < bytes.len() {
        if bytes[i] != b'!' || bytes[i + 1] != b'!' {
            i += 1;
            continue;
        }
        let Some((from, to)) = property_name_before(text, i)
            .as_deref()
            .filter(|name| exclamation_property(component, name))
            .and_then(|_| value_before(text, i))
            .filter(|_| ends_the_value(text, i + 2))
        else {
            i += 1;
            continue;
        };
        // A property keeps its own key: `value: x!!` becomes `value: x, $value: …`, where the
        // whole-argument form replaces the argument itself (measured: `fixtures/double-exclamation`
        // emits `Slider.create({ value: this.value, $value: newValue => { … } })`).
        let (_, writer_key, parameter) = crate::build::stage::pre_define::EXCLAMATION_OBJECT_KEYS;
        let expression = &text[from..to];
        out.push_str(&text[copied..from]);
        out.push_str(expression);
        out.push_str(&format!(
            ", {writer_key}: {parameter} => {{ {expression} = {parameter}; }}"
        ));
        copied = i + 2;
        i += 2;
    }
    out.push_str(&text[copied..]);
    out
}

/// `{ value: e, $value: newValue => { e = newValue; } }` — `generateObjectForExclamation`.
fn exclamation_object(expression: &str) -> String {
    let (value_key, writer_key, parameter) =
        crate::build::stage::pre_define::EXCLAMATION_OBJECT_KEYS;
    format!(
        "{{ {value_key}: {expression}, {writer_key}: {parameter} => {{ {expression} = {parameter}; }} }}"
    )
}

/// The expression of a value written `expr!!`, when the argument is exactly that.
fn double_non_null(text: &str) -> Option<&str> {
    let inner = text.trim().strip_suffix("!!")?;
    let inner = inner.trim_end();
    // `updateArgumentForExclamation` only unwraps when the inner expression is not itself a
    // non-null assertion, so `x!!!` keeps one `!` and is not a binding.
    if inner.is_empty() || inner.ends_with('!') {
        return None;
    }
    Some(inner)
}

/// The byte range of the value that ends with the `!!` at `at`, when it follows a property's
/// colon. A value inside a conditional or an `as` cast does not qualify: the reference matches
/// the argument node itself, not an expression that contains one.
fn value_before(text: &str, at: usize) -> Option<(usize, usize)> {
    let mut depth = 0i32;
    let mut colon = None;
    for (offset, ch) in text[..at].char_indices().rev() {
        match ch {
            ')' | ']' | '}' => depth += 1,
            '(' | '[' | '{' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            ':' if depth == 0 => {
                colon = Some(offset + ch.len_utf8());
                break;
            }
            ',' | ';' if depth == 0 => break,
            _ => {}
        }
    }
    let start = colon?;
    let value = text[start..at].trim();
    if value.is_empty() {
        return None;
    }
    let leading = text[start..at].len() - text[start..at].trim_start().len();
    Some((start + leading, start + leading + value.len()))
}

/// Whether the value ending at `end` is the whole piece: the next token closes it.
fn ends_the_value(text: &str, end: usize) -> bool {
    let rest = text[end..].trim_start();
    rest.is_empty()
        || rest.starts_with(',')
        || rest.starts_with('}')
        || rest.starts_with(')')
        || rest.starts_with(']')
}

/// Whether `component`'s `property` is in `PROPERTIES_ADD_DOUBLE_EXCLAMATION`.
fn exclamation_property(component: &str, property: &str) -> bool {
    crate::build::stage::pre_define::EXCLAMATION_PROPERTIES
        .iter()
        .any(|(name, properties)| *name == component && properties.contains(&property))
}

/// A `builder:` property whose value is a builder: `this.X()` becomes `this.X.bind(this)` and
/// `this.X(args)` becomes `() => { this.X.call(this, args) }`.
///
/// `bindComponentAttr` (`process_component_build.js`) rewrites the property, not the whole
/// argument, so an object literal that carries a `builder:` key keeps its other keys untouched
/// (`Refresh.create({ refreshing: …, builder: this.LoadingBuilder.bind(this) })`).
///
/// Two more shapes reach this property through `parseBuilderNode`, so they are written exactly
/// as the same expression would be written as a builder argument ([`parse_builder_node`]): a
/// builder named without a call (`{ builder: this.actions }` → `{ builder: { builder:
/// this.actions.bind(this) } }`) and a conditional, whose arms are rewritten one by one
/// (measured: `fixtures/conditional-builder`, the `popup-*` rows).
pub(crate) fn rewrite_builder_properties(
    text: &str,
    attribute: &str,
    builders: &[Builder],
    wrap_global_builder: bool,
) -> String {
    let mut out = String::new();
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = text[from..].find("builder:") {
        let at = from + found;
        let after = at + "builder:".len();
        let rest = text[after..].trim_start();
        let start = text.len() - rest.len();
        let end = property_value_end(text, start);
        let value = text[start..end].trim_end();
        // A conditional value takes the arm rewrite the whole-argument form takes.
        if let Some(rewritten) = rewrite_conditional_builder(value, attribute, builders) {
            out.push_str(&text[copied..start]);
            out.push_str(&rewritten);
            copied = start + value.len();
            from = copied;
            continue;
        }
        // A builder named without a call is a builder reference all the same.
        if named_builder(value).is_some_and(|name| in_builder_scope(name, builders)) {
            if let Some(rewritten) = parse_builder_node(value, attribute, builders) {
                out.push_str(&text[copied..start]);
                out.push_str(&rewritten);
                copied = start + value.len();
                from = copied;
                continue;
            }
        }
        // The value must be a `this.<name>(` call, or there is nothing to rewrite.
        let Some(call) = rest.strip_prefix("this.") else {
            // A global `@Builder` call. Only `bindPopup`'s option object wraps it: its
            // `builder:` member has to be a `CustomBuilder`, which the reference builds as a
            // `{ builder: … }` object (measured: `fixtures/bind-popup-global-builder`).
            if wrap_global_builder {
                if let Some((value_end, replacement)) = wrap_global_builder_value(text, rest) {
                    out.push_str(&text[copied..after]);
                    out.push_str(&replacement);
                    copied = value_end;
                    from = value_end;
                    continue;
                }
            }
            // Every other member position takes the reference's partial application:
            // `processObjectPropertyBuilder` sends a builder call to
            // `transformBuilderCallExpression`, which keeps the callee as written and binds
            // `this` together with the arguments `transDoubleDollarInCustomBuilder` forwards
            // (a lone object literal arrives as `makeBuilderParameterProxy(…)`).
            if let Some(value_end) = start.checked_add(value.len()) {
                if let Some(replacement) = bound_global_builder_member(value) {
                    out.push_str(&text[copied..start]);
                    out.push_str(&replacement);
                    copied = value_end;
                    from = value_end;
                    continue;
                }
            }
            from = after;
            continue;
        };
        let Some(paren) = call.find('(') else {
            from = after;
            continue;
        };
        let name = &call[..paren];
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            from = after;
            continue;
        }
        let Some(close) = call[paren..].find(')') else {
            from = after;
            continue;
        };
        let arguments = call[paren + 1..paren + close].trim();
        if arguments.contains(')') || arguments.contains("=>") {
            from = after;
            continue;
        }
        let value_end = start + "this.".len() + paren + close + 1;
        let replacement = if arguments.is_empty() {
            format!("this.{name}.bind(this)")
        } else {
            format!("() => {{ this.{name}.call(this, {arguments}) }}")
        };
        out.push_str(&text[copied..start]);
        out.push_str(&replacement);
        copied = value_end;
        from = value_end;
    }
    out.push_str(&text[copied..]);
    // The sheet options' `title` slot takes a `CustomBuilder`, and a builder reference written
    // there is bound the same way a `builder:` member is.
    rewrite_title_builders(&out, builders)
}

/// A global `@Builder` call written as an object member's value: `X({…})` → `X.bind(this, …)`.
///
/// `transformBuilderCallExpression` (`process_component_build.js`) is reached from
/// `processObjectPropertyBuilder` for the member keys that take a builder (`builder`, `title`,
/// `header`, `footer`, `start`, `end`, `preview`, `indicatorBuilder`). It keeps the callee as
/// written — a bare name stays bare — and binds `this` together with the call's own arguments,
/// which `transDoubleDollarInCustomBuilder` first turns into the proxy call when the only
/// argument is an object literal.
fn bound_global_builder_member(value: &str) -> Option<String> {
    let value = value.trim();
    let open = value.find('(')?;
    let name = value[..open].trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        || !crate::build::compile::component_index::is_builder(name)
    {
        return None;
    }
    let close = crate::build::lex::matching_delimiter(value, open)?;
    if close + 1 != value.len() {
        return None;
    }
    let arguments = value[open + 1..close].trim();
    if arguments.is_empty() {
        return Some(format!("{name}.bind(this)"));
    }
    let forwarded =
        crate::build::stage::process_custom_component::builder_param_proxy(name, arguments)
            .unwrap_or_else(|| arguments.to_string());
    Some(format!("{name}.bind(this, {forwarded})"))
}

/// A `title:` member whose value names a builder takes the `.bind(this)` form.
///
/// Only a builder reference is rewritten: an inline arrow, a string, and an options object
/// (`title: { title: $r(…) }`) are values that have no receiver to bind
/// (measured: `fixtures/bind-sheet-title`).
fn rewrite_title_builders(text: &str, builders: &[Builder]) -> String {
    let mut out = String::new();
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = text[from..].find("title:") {
        let at = from + found;
        let after = at + "title:".len();
        let rest = text[after..].trim_start();
        let start = text.len() - rest.len();
        let end = property_value_end(text, start);
        let value = text[start..end].trim_end();
        if named_builder(value).is_some_and(|name| in_builder_scope(name, builders)) {
            out.push_str(&text[copied..start]);
            out.push_str(&format!("{value}.bind(this)"));
            copied = start + value.len();
            from = copied;
            continue;
        }
        from = after;
    }
    out.push_str(&text[copied..]);
    out
}

/// The end of the property value that begins at `start`: the next top-level `,`, or the `}`
/// closing the object, whichever comes first.
///
/// The scan skips quoted text, so a value carrying a comma or a brace inside a literal is not
/// cut in half.
fn property_value_end(text: &str, start: usize) -> usize {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ',' if depth == 0 => return start + offset,
            ')' | ']' | '}' if depth == 0 => return start + offset,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    text.len()
}

/// Rewrite a global `@Builder` call standing where a builder value is expected, into
/// `{ builder: () => { X.call(this, …) } }`.
///
/// `processBindPopupBuilderProperty` (`process_component_build.js`) reaches the member through
/// `processInitializer` → `parseBuilderNode`, and `PopupOptions.builder` is a `CustomBuilder`, so
/// the reference emits the value as a `{ builder: … }` object. The arrow calls the global
/// builder with `this` as the receiver and forwards the call's own arguments unchanged
/// (`getParsedBuilderAttrArgumentWithParams`: `[createThis(), ...e.arguments]`), so a `this` the
/// source wrote as an argument stays one (measured: `fixtures/bind-popup-global-builder`, the
/// `arrow-arg` / `self-arg` / `two-args` rows).
///
/// Returns the byte offset just past the call and the replacement text, or `None` when the value
/// is not a call to a declared global `@Builder` function, or an argument cannot be forwarded as
/// written ([`forwardable_builder_argument`]).
fn wrap_global_builder_value(text: &str, rest: &str) -> Option<(usize, String)> {
    let name_len = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
        .count();
    if name_len == 0 {
        return None;
    }
    let name = &rest[..name_len];
    if !crate::build::compile::component_index::is_builder(name) {
        return None;
    }
    let tail = &rest[name_len..];
    let paren = tail.find('(')?;
    // Balanced scan, not the first `)`: an argument can be an arrow whose body contains
    // parentheses (`PopupBuilder(this, x, () => { … })`), and cutting at the first one splices
    // the replacement into the middle of that arrow.
    let open = text.len() - rest.len() + name_len + paren;
    let close = crate::build::lex::matching_delimiter(text, open)?;
    let arguments = text[open + 1..close].trim();
    if !arguments.is_empty()
        && !crate::build::lex::split_top_level_commas(arguments)
            .iter()
            .all(|argument| forwardable_builder_argument(argument))
    {
        return None;
    }
    let call = if arguments.is_empty() {
        format!("{name}.call(this)")
    } else {
        format!("{name}.call(this, {arguments})")
    };
    Some((close + 1, format!("{{ builder: () => {{ {call}; }} }}")))
}

/// Whether one argument of a global `@Builder` call can be forwarded as written.
///
/// The reference forwards the argument list node by node, so whatever the AST already holds
/// travels unchanged. hvolt only has text, so it takes the shapes that need no rewriting of
/// their own: a value expression — literal, identifier, `this`, member/element access — or a
/// function / arrow expression, whose body this pass does not look into. A nested call, an
/// object literal, an assignment or a statement would have to be processed first, so those are
/// left as written. A *lone* object literal is not forwarded by the reference either: it becomes
/// a `__BuilderParamProxy(…)` call (`transDoubleDollarInCustomBuilder`), which hvolt does not
/// emit.
fn forwardable_builder_argument(argument: &str) -> bool {
    let argument = argument.trim();
    if argument.is_empty() {
        return false;
    }
    if is_function_expression(argument) {
        return true;
    }
    !argument.contains(['(', ')', '{', '}', ';', '='])
}

/// Whether `argument` is a function or arrow expression: a value whose body this pass leaves
/// alone, so the parentheses and braces inside it are not a reason to give up.
fn is_function_expression(argument: &str) -> bool {
    if argument.starts_with("function") {
        return true;
    }
    // `(a, b) => …` / `a => …`: the arrow has to sit at the argument's own level, not inside a
    // nested call or literal.
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut previous = '\0';
    for ch in argument.chars() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            previous = ch;
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '>' if depth == 0 && previous == '=' => return true,
            _ => {}
        }
        previous = ch;
    }
    false
}

/// Whether `component`'s `attribute` takes a builder reference (`CUSTOM_BUILDER_PROPERTIES`).
fn bind_object_property(attribute: &str) -> bool {
    crate::build::stage::pre_define::CUSTOM_BUILDER_PROPERTIES.contains(&attribute)
}

/// The name a builder reference is written with: `this.TabBar` names `TabBar`, and a bare
/// `TabBar` names itself. Anything else — `this.a.b`, `Color.Red`, a literal, a call — names
/// nothing here.
fn named_builder(text: &str) -> Option<&str> {
    let name = text.strip_prefix("this.").unwrap_or(text);
    let single = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    single.then_some(name)
}

/// Whether `name` is a builder the current struct can name.
///
/// `storedFileInfo.builderLikeCollection` (`process_component_class.js`) holds the current
/// struct's `@Builder` methods and `@BuilderParam` fields plus the global builder table, and
/// `parseBuilderNode` (`process_component_build.js`) rewrites an argument only when the name is
/// in it. The walked `builders` list carries the first two (a `@BuilderParam` field is added by
/// [`with_builder_params`]) and the third, so the two readings agree.
fn in_builder_scope(name: &str, builders: &[Builder]) -> bool {
    builders.iter().any(|builder| builder.name == name)
        || crate::build::compile::component_index::is_builder(name)
}

/// Register the struct's `@BuilderParam` fields as builder names.
///
/// A `@BuilderParam` field holds a builder, so `this.footer` standing where a builder reference
/// is expected has to take the `.bind(this)` form; the reference reaches the same names through
/// `builderParamObjectCollection` (`process_component_class.js`).
fn with_builder_params(mut builders: Vec<Builder>, comp: &ComponentInfo) -> Vec<Builder> {
    // `builderLikeCollection` holds the struct's `@Builder` methods as well as its
    // `@BuilderParam` fields; a method declared in this struct is a builder the call site can
    // name, and its name has to reach [`rewrite_arguments`] for the `.bind(this, …)` form
    // (measured: `fixtures/property-access-this`, `header: this.headerB(row.name)`).
    for method in &comp.struct_builder_methods {
        let name = method
            .signature
            .split('(')
            .next()
            .map(str::trim)
            .and_then(|head| head.split_whitespace().last())
            .unwrap_or_default()
            .to_string();
        if name.is_empty() || builders.iter().any(|builder| builder.name == name) {
            continue;
        }
        builders.push(Builder {
            name,
            params: Vec::new(),
            body: String::new(),
            is_export: false,
            order: 0,
            comment: Vec::new(),
        });
    }
    for field in &comp.fields {
        if !field.has("BuilderParam") || builders.iter().any(|builder| builder.name == field.name) {
            continue;
        }
        builders.push(Builder {
            name: field.name.clone(),
            params: Vec::new(),
            body: String::new(),
            is_export: false,
            order: 0,
            comment: Vec::new(),
        });
    }
    builders
}

/// Whether `attribute`'s builder argument is written without the `builder:` key
/// (`CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY`, `component_map.js`).
fn custom_builder_properties_without_key(attribute: &str) -> bool {
    crate::build::stage::pre_define::CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY.contains(&attribute)
}

/// The property name a value is being assigned to (`isOn: $$…` → `isOn`).
fn property_name_before(text: &str, at: usize) -> Option<String> {
    let colon = text[..at].rfind(':')?;
    let head = text[..colon].trim_end();
    let name: String = head
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Whether `component`'s `property` is in `PROPERTIES_ADD_DOUBLE_DOLLAR`.
fn double_dollar_property(component: &str, property: &str) -> bool {
    crate::build::stage::pre_define::DOUBLE_DOLLAR_PROPERTIES
        .iter()
        .any(|(name, properties)| *name == component && properties.contains(&property))
}

/// The trailing object a navigation component's `create` call carries.
///
/// `navigationOrNavDestinationCreateContent` (`process_component_build.js`) writes
/// `{ moduleName, pagePath }` from `projectConfig.moduleName` and the source file relative to
/// the project root without its extension. This build records that coordinate for the file
/// (`ets_compile::source_page`, `<module>/src/main/ets/<page>.ets`), so `pagePath` is the same
/// string with its extension stripped; `moduleName` is the module the task assembles, which is
/// not the module that owns the source file. A file with no coordinate — an unnamed source the
/// build never routed — names nothing and gets no argument.
///
/// A `NavDestination` writes the two keys and nothing else. A `Navigation` adds
/// `isUserCreateStack`: `true` when the source supplied the stack, `false` when
/// `navigationCreateParam` started one for it. Any other component takes no such argument.
fn navigation_route(name: &str, caller_owns_stack: bool) -> String {
    if name != pre_define::COMPONENT_NAVIGATION && name != pre_define::COMPONENT_NAV_DESTINATION {
        return String::new();
    }
    let Some(source) = crate::build::ets_compile::source_page() else {
        return String::new();
    };
    let page_path = source.strip_suffix(".ets").unwrap_or(&source);
    let page_module = page_path.split('/').next().unwrap_or("");
    if page_module.is_empty() {
        return String::new();
    }
    // `moduleName` is `projectConfig.moduleName`: the module this CompileArkTS task assembles,
    // not the module that owns the source file (measured on clashbox, whose `xb_components`
    // sources register under `entry`). The page's own module is the fallback for a single-file
    // CLI lowering — `compile-ets` has no assemble context — not a second normal path.
    let module = crate::build::compile::abc_compile::assemble_module()
        .unwrap_or_else(|| page_path.split('/').next().unwrap_or(""));
    // The reference writes `compileHar ? "" : relative(projectRoot, resourceFileName)`. Measured
    // on clashbox: the module's sources reach the reference as an entry dependency, where
    // `compileHar` is false, so the page path is written — keying off the module's own `type`
    // produced `differing` 123 → 123 with token 4,654 → 4,664 (worse), and the flag was
    // withdrawn from this path. It stays in the transpile context for a build that does compile
    // a HAR on its own.
    route_object(name, page_path, module, caller_owns_stack)
}

/// The route argument itself, with the two module names kept apart.
///
/// Split from [`navigation_route`] so the emitted shape is testable without a build:
/// `page_path` is the recorded coordinate (the page's own module first), `module_name` the
/// module the task assembles.
fn route_object(name: &str, page_path: &str, module_name: &str, caller_owns_stack: bool) -> String {
    if name == pre_define::COMPONENT_NAVIGATION {
        return format!(
            ", {{ moduleName: \"{module_name}\", pagePath: \"{page_path}\", isUserCreateStack: {caller_owns_stack} }}"
        );
    }
    format!(", {{ moduleName: \"{module_name}\", pagePath: \"{page_path}\" }}")
}

/// The method a component construction is emitted through.
///
/// Two branches of the reference decide it, and they are the only two:
///
/// * `createComponent` (`process_component_build.js`) — when the innermost identifier of the
///   chain is `COMPONENT_BUTTON` and the method is not `COMPONENT_POP_FUNCTION`, the method
///   becomes `COMPONENT_CREATE_LABEL_FUNCTION` (`createWithLabel`) instead of
///   `COMPONENT_CREATE_FUNCTION` (`create`). It keeps `COMPONENT_CREATE_CHILD_FUNCTION`
///   (`createWithChild`) when that is the method it was asked for. The decision reads the
///   component name and nothing else: the arguments — a label, an object literal, an
///   identifier, or none — do not take part in it;
/// * `processNormalComponentBlock` — when the construction carries a block, the `create`
///   statement is replaced by the `createWithChild` one, because that variant opens the child
///   frame the block's statements are emitted into. The replacement is the same
///   `createComponent` call, so `Button` and `createWithChild` compose: a `Button` with a block
///   is `createWithChild`, a `Button` without one is `createWithLabel`.
///
/// Every other component keeps `create`; a `Button` never emits it.
fn create_method(name: &str, has_block: bool) -> &'static str {
    match (name, has_block) {
        (pre_define::COMPONENT_BUTTON, true) => pre_define::COMPONENT_CREATE_CHILD_FUNCTION,
        (pre_define::COMPONENT_BUTTON, false) => pre_define::COMPONENT_CREATE_LABEL_FUNCTION,
        _ => pre_define::COMPONENT_CREATE_FUNCTION,
    }
}

/// `.gesture(G)` wraps the gesture's own chain in the gesture context the runtime reads.
fn emit_gesture(args: &str, out: &mut String, indent: &str, builders: &[Builder]) {
    let allocator = Allocator::default();
    let source = format!("{args};");
    let ret = Parser::new(&allocator, &source, SourceType::ts()).parse();
    let Some(Statement::ExpressionStatement(expression)) = ret.program.body.first() else {
        return;
    };
    let Expression::CallExpression(call) = &expression.expression else {
        return;
    };
    let Some((root, root_call, attributes)) = component_chain(call, &source) else {
        return;
    };
    out.push_str(&format!(
        "{indent}    globalThis.Gesture.create(GesturePriority.Low);\n"
    ));
    match create_arguments(root_call, &source) {
        Some(arguments) => out.push_str(&format!(
            "{indent}    {root}.create({});\n",
            rewrite_arguments(&arguments, &root, "create", builders)
        )),
        None => out.push_str(&format!("{indent}    {root}.create();\n")),
    }
    for attribute in &attributes {
        out.push_str(&format!(
            "{indent}    {root}.{}({});\n",
            attribute.name,
            rewrite_arguments(&attribute.args, &root, &attribute.name, builders)
        ));
    }
    out.push_str(&format!("{indent}    {root}.pop();\n"));
    out.push_str(&format!("{indent}    globalThis.Gesture.pop();\n"));
}

/// The retake id of a node: the string literal it hands to `.id(…)`.
///
/// `ifRetakeId` (`process_component_build.js`) wraps a child's statements in
/// `if (!If.canRetake('<id>')) { … }`, and the id is the `id` attribute's literal. The guard is
/// only produced for a **direct child of an `if` branch** — a node nested deeper inside the
/// branch keeps its plain frame (`ConfigInfo.ets` writes `.id('text_input_auto_update_time')`
/// under a nested `if` and the reference emits no guard for it).
fn retake_id(node: &UiNode) -> Option<String> {
    let attribute = node.attributes.iter().find(|a| a.name == "id")?;
    let text = attribute.args.trim();
    let inner = text
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            text.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })?;
    if inner.is_empty() || inner.contains('\'') || inner.contains('\\') || inner.contains('\n') {
        return None;
    }
    Some(inner.to_string())
}

fn emit_if(
    branches: &[UiBranch],
    next_comments: &[String],
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    out.push_str(&format!("{indent}    If.create();\n"));
    // The comment above the `if` lands between `If.create();` and the condition, not above the
    // `create` call (`fixtures/comment-leading`, marker `IF_START`).
    push_comments(
        out,
        branches.first().map_or(&[], |branch| &branch.comment),
        &format!("{indent}    "),
    );
    for (index, branch) in branches.iter().enumerate() {
        if index > 0 {
            // Unlike the `if`, an `else if` / `else` takes its comment on its own line directly
            // above the branch (`fixtures/comment-leading`, markers `BRANCH_ELSEIF`,
            // `BRANCH_ELSE`, `BETWEEN_BRANCH`).
            push_comments(out, &branch.comment, &format!("{indent}    "));
        }
        match &branch.condition {
            Some(condition) if index == 0 => {
                out.push_str(&format!("{indent}    if ({condition}) {{\n"));
            }
            Some(condition) => {
                out.push_str(&format!("{indent}    else if ({condition}) {{\n"));
            }
            None => {
                let trailing = branch.keyword_trailing.join(" ");
                if trailing.is_empty() {
                    out.push_str(&format!("{indent}    else {{\n"));
                } else {
                    // A line comment runs to the end of its line, so the brace opens on the
                    // next one, one space deeper — the shape the reference prints
                    // (`else //分割线` then ` {`).
                    out.push_str(&format!("{indent}    else {trailing}\n{indent}     {{\n"));
                }
            }
        }
        // A branch the source wrote updates through the receiver its frame took:
        // `createIfBranchFunc` (`process_component_build.js`) writes
        // `isBuilder ? parentConditionalExpression() : createThis()`, the same `isBuilder` flag
        // `frame_receiver()` reads. The empty `else` synthesised further down stays on `this`:
        // that one is built by `processElseStatement`, which writes a bare `createThis()` and
        // never reads the flag (measured: clashbox `Common.ts` emits `(parent ? parent : this)`
        // at `:469/533/561` and `this` at `:495/548/595` — the synthesized elses of the same
        // builder).
        out.push_str(&format!(
            "{indent}        {}.ifElseBranchUpdateFunction({index}, () => {{\n",
            frame_receiver()
        ));
        let inner = format!("{indent}            ");
        // A branch's direct children are the statements `ifElseBranchUpdateFunction` runs: the
        // ones carrying a literal `.id(…)` are guarded by `If.canRetake` so a re-run of the
        // branch does not take the element id twice.
        for item in &branch.body {
            match item {
                UiItem::Node(node) => match retake_id(node) {
                    Some(id) => {
                        let deeper = format!("{inner}    ");
                        let mut frame = String::new();
                        emit_node(node, &mut frame, &deeper, builders, extends, styles);
                        out.push_str(&format!("{inner}if (!If.canRetake('{id}')) {{\n"));
                        out.push_str(&frame);
                        out.push_str(&format!("{inner}}}\n"));
                    }
                    None => emit_node(node, out, &inner, builders, extends, styles),
                },
                other => emit(
                    std::slice::from_ref(other),
                    out,
                    &inner,
                    builders,
                    extends,
                    styles,
                ),
            }
        }
        out.push_str(&format!("{indent}        }});\n"));
        out.push_str(&format!("{indent}    }}\n"));
    }
    // An `if` written without an `else` still gets one: the branch function is keyed by
    // branch id, and the compiler always emits the pair.
    if branches
        .last()
        .is_some_and(|branch| branch.condition.is_some())
    {
        let index = branches.len();
        // A comment written after the whole `if` — including one at the end of the enclosing
        // block, where no next statement exists — lands above the synthesized `else`
        // (measured: `fixtures/comment-leading`, markers `AFTER_BLOCK` and `IF_TAIL`).
        let above_else = if next_comments.is_empty() {
            branches
                .last()
                .map_or(&[][..], |branch| branch.tail.as_slice())
        } else {
            next_comments
        };
        push_comments(out, above_else, &format!("{indent}    "));
        out.push_str(&format!("{indent}    else {{\n"));
        out.push_str(&format!(
            "{indent}        this.ifElseBranchUpdateFunction({index}, () => {{\n"
        ));
        out.push_str(&format!("{indent}        }});\n"));
        out.push_str(&format!("{indent}    }}\n"));
    }
    out.push_str(&format!("{indent}}}, If);\n"));
    out.push_str(&format!("{indent}If.pop();\n"));
}

/// The comment the statement *after* `items[index]` carries.
///
/// The reference prints it once more above the empty `else` it synthesises for a bare `if`: that
/// `else` is created with the position the `if` ends at, so the printer hands it the trivia that
/// follows the block (`fixtures/comment-leading`, marker `AFTER_BLOCK` — three copies in the
/// golden: above the `else`, and the two the next component's own frame writes).
fn next_statement_comments(items: &[UiItem], index: usize) -> &[String] {
    match items.get(index + 1) {
        Some(UiItem::Node(node)) => &node.comment,
        Some(UiItem::If(branches)) => branches.first().map_or(&[], |branch| &branch.comment),
        Some(UiItem::ForEach(loop_) | UiItem::LazyForEach(loop_)) => &loop_.comment,
        _ => &[],
    }
}

/// The comments written above the `else` that introduces the branch starting at `at`.
///
/// The alternate of an `else if` is an `IfStatement` whose span starts at the nested `if`, and
/// the alternate of a plain `else` is the branch body, whose span starts at its `{`. The `else`
/// keyword is in neither span, so it is looked up before the statement. A comment written
/// between the closing `}` and the `else` (`} // BETWEEN_BRANCH \n else {`) belongs here too:
/// whitespace and whole comments are skipped on the way back.
fn else_branch_comments(text: &str, at: usize) -> Vec<String> {
    let start = if text[at..].starts_with("else") {
        at
    } else {
        else_keyword_before(text, at).unwrap_or(at)
    };
    crate::build::ets::comments::leading_comments_before(text, start)
}

/// The comment written on the `else` keyword's own line, after it (`else // X` + newline).
///
/// This is the other half of [`else_branch_comments`]: that one reads the block above the
/// keyword, this one reads what follows it on the same line. The reference prints it back
/// between `else` and `{` (measured: `fixtures/keyword-trailing-comment`, marker
/// `ELSE_TRAILING`, and `fixtures/proxy-group-item`'s `else //分割线`).
fn else_keyword_trailing_comment(text: &str, at: usize) -> Vec<String> {
    // The keyword is the last `else` before the branch body: `else_keyword_before` walks back
    // over the comments above the keyword too, so it stops at the previous statement when a
    // block comment sits between (`fixtures/proxy-group-item`'s `else //分割线`, preceded by a
    // commented-out `MenuItem(…)` block).
    let Some(start) = text[..at.min(text.len())].rfind("else") else {
        return Vec::new();
    };
    let after = start + "else".len();
    let line_end = text[after..]
        .find('\n')
        .map_or(text.len(), |offset| after + offset);
    let mut found = Vec::new();
    for token in crate::build::lex::tokenize(&text[after..line_end]) {
        if token.kind == crate::build::lex::Kind::Comment {
            found.push(
                text[after + token.start..after + token.end]
                    .trim()
                    .to_string(),
            );
        }
    }
    found
}

/// The offset of the `else` keyword that ends just before `at`, skipping the whitespace and the
/// comments between the two.
fn else_keyword_before(text: &str, at: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = at.min(text.len());
    loop {
        while i > 0 && matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'\r') {
            i -= 1;
        }
        // Byte comparisons, not string ones: `i` may sit right after a multi-byte character
        // (a Chinese comment line), and slicing the last two bytes of it would panic.
        if i >= 2 && &bytes[i - 2..i] == b"*/" {
            i = text[..i - 2].rfind("/*")?;
            continue;
        }
        let line_start = text[..i].rfind('\n').map_or(0, |p| p + 1);
        if let Some(offset) = text[line_start..i].find("//") {
            i = line_start + offset;
            continue;
        }
        break;
    }
    (i >= 4 && &bytes[i - 4..i] == b"else").then_some(i - 4)
}

/// Writes each comment on its own line at `prefix`.
fn push_comments(out: &mut String, comments: &[String], prefix: &str) {
    for comment in comments {
        out.push_str(prefix);
        // A multi-line block is reindented to the target column: the reference prints the first
        // line at `prefix` and every continuation at `prefix` + one space, so `/**` at four
        // spaces puts `* …` at five (`fixtures/proxy-group-item`, `/** 速度与延迟区别弹窗 */`
        // blocks). The source's own continuation indent is not what lands in the output.
        let mut lines = comment.lines();
        if let Some(first) = lines.next() {
            out.push_str(first.trim_end());
            for line in lines {
                out.push('\n');
                out.push_str(prefix);
                out.push(' ');
                out.push_str(line.trim_start());
            }
        }
        out.push('\n');
    }
}

/// The item generator's parameter list: `_item`, then the source's own index parameter.
///
/// `getParameters` in `process_component_build.js` builds exactly this list — the item is
/// always renamed `_item`, and the second parameter is the node the source wrote (in clashbox:
/// `_item, i: number`, `_item, gIndex: number`, `_item, gi: number`, and five untyped
/// `_item, index`). A source that declares no second parameter gets none.
fn generator_parameters(loop_: &UiLoop) -> String {
    match &loop_.index_parameter {
        Some(parameter) => format!("_item, {parameter}"),
        None => "_item".to_string(),
    }
}

fn emit_for_each(
    loop_: &UiLoop,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    out.push_str(&format!(
        "{indent}{}.observeComponentCreation2((elmtId, isInitialRender) => {{\n",
        frame_receiver()
    ));
    push_comments(out, &loop_.comment, &format!("{indent}    "));
    out.push_str(&format!("{indent}    ForEach.create();\n"));
    // The attributes written on the loop itself come next, before the item generator — the
    // reference puts `ForEach.onMove(…)` there (`HomePage.ts`, `ConfigurationPage.ts`).
    let _ = emit_attribute_list(
        &loop_.attributes,
        &[],
        "ForEach",
        out,
        indent,
        builders,
        extends,
        styles,
        false,
    );
    // The generator's own parameters follow the source: a generator that reads the index is
    // emitted with one.
    let parameters = generator_parameters(loop_);
    out.push_str(&format!(
        "{indent}    const forEachItemGenFunction = ({parameters}) => {{\n"
    ));
    out.push_str(&format!("{indent}        const {} = _item;\n", loop_.item));
    let inner = format!("{indent}        ");
    emit(&loop_.body, out, &inner, builders, extends, styles);
    out.push_str(&format!("{indent}    }};\n"));
    // No key generator: the compiler passes three arguments. With one, the two flags say which
    // side reads the index, and they follow the key generator's own parameters.
    match &loop_.key {
        None if !loop_.item_uses_index => out.push_str(&format!(
            "{indent}    {receiver}.forEachUpdateFunction(elmtId, {}, forEachItemGenFunction);\n",
            loop_.source,
            receiver = frame_receiver()
        )),
        // A generator that reads the index needs the two flags, and a call without a key
        // generator writes `undefined` in its place.
        None => out.push_str(&format!(
            "{indent}    {receiver}.forEachUpdateFunction(elmtId, {}, forEachItemGenFunction, undefined, true, false);\n",
            loop_.source,
            receiver = frame_receiver()
        )),
        Some((key, key_uses_index)) => {
            // The two flags say which generator reads the index: the item generator first, the
            // key generator second (measured: fixtures/foreach and fixtures/foreach-forms).
            let flags = format!("{}, {}", loop_.item_uses_index, key_uses_index);
            out.push_str(&format!(
                "{indent}    {receiver}.forEachUpdateFunction(elmtId, {}, forEachItemGenFunction, {}, {flags});\n",
                loop_.source,
                key,
                receiver = frame_receiver()
            ));
        }
    }
    out.push_str(&format!("{indent}}}, ForEach);\n"));
    push_comments(out, &loop_.comment, indent);
    out.push_str(&format!("{indent}ForEach.pop();\n"));
}

fn emit_lazy_for_each(
    loop_: &UiLoop,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    // The item's root node is constructed once, outside the deep render; only its children are
    // expanded inside.
    let root_node = loop_.body.iter().find_map(|item| match item {
        UiItem::Node(node) => Some(node),
        _ => None,
    });
    let root = root_node
        .map(|node| node.component.clone())
        .unwrap_or_default();
    // `checkLazyCreate` (process_component_build.js) asks the generator's own root node
    // (`t.name === LIST_ITEM`). A body with no component node is never asked, so it takes the
    // ordinary shape: defaulting the root to a `ListItem` gave such a body a deep render it never
    // had (measured on clashbox `ProxyPage.ts`: 9361 → 9299 disassembly lines, and
    // `itemCreation2` / `observeComponentCreation2` / `pop` / `create` all drop).
    let deep_render = root_node.is_some_and(|node| needs_deep_render(&node.component));
    out.push_str(&format!("{indent}{{\n"));
    let parameters = generator_parameters(loop_);
    out.push_str(&format!(
        "{indent}    const __lazyForEachItemGenFunction = ({parameters}) => {{\n"
    ));
    out.push_str(&format!("{indent}        const {} = _item;\n", loop_.item));
    // The `itemCreation`/`observedDeepRender` indirection exists because `ListItem.create` takes
    // the render function as its first argument; `checkLazyCreate`
    // (process_component_build.js) says exactly that: `t.name === LIST_ITEM`, minus a
    // `ListItem('false')` argument, a `Repeat` template, and a bare-expression LazyForEach
    // child. Every other root — a user component (`ProxyPage.ts`), or another built-in such as
    // `ListItemGroup` (`ProxyArrangement.ts`) — is built with the ordinary call shape. The
    // packed reference confirms the set: every `create(deepRenderFunction, true)` in it is a
    // `ListItem`.
    if !deep_render {
        let inner = format!("{indent}        ");
        emit(&loop_.body, out, &inner, builders, extends, styles);
        out.push_str(&format!("{indent}    }};\n"));
        emit_lazy_for_each_tail(loop_, out, indent, extends, styles, builders);
        out.push_str(&format!("{indent}}}\n"));
        return;
    }
    out.push_str(&format!("{indent}        {{\n"));
    out.push_str(&format!(
        "{indent}            const itemCreation2 = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!(
        "{indent}                {root}.create(() => {{ }}, false);\n"
    ));
    // The item's own attribute writes belong to `itemCreation2`, next to the `create` it
    // annotates — `createItemBlock` (process_component_build.js) builds that block from both.
    // Emitting only the `create` dropped every write on the item's root: 351 tokens in
    // `ProxyGroupItem.ts`, and the `ListItem.transition(…)` calls it carries.
    if let Some(node) = root_node {
        let attribute_indent = format!("{indent}                ");
        let _ = emit_attributes(
            node,
            &root,
            out,
            &attribute_indent,
            builders,
            extends,
            styles,
            false,
        );
    }
    out.push_str(&format!("{indent}            }};\n"));
    out.push_str(&format!(
        "{indent}            const observedDeepRender = () => {{\n"
    ));
    out.push_str(&format!(
        "{indent}                this.observeComponentCreation2(itemCreation2, {root});\n"
    ));
    let inner = format!("{indent}                ");
    match root_node {
        Some(node) => emit(&node.children, out, &inner, builders, extends, styles),
        None => emit(&loop_.body, out, &inner, builders, extends, styles),
    }
    out.push_str(&format!("{indent}                {root}.pop();\n"));
    out.push_str(&format!("{indent}            }};\n"));
    out.push_str(&format!("{indent}            observedDeepRender();\n"));
    out.push_str(&format!("{indent}        }}\n"));
    out.push_str(&format!("{indent}    }};\n"));
    emit_lazy_for_each_tail(loop_, out, indent, extends, styles, builders);
    out.push_str(&format!("{indent}}}\n"));
}

/// The statements after the item generator: the key function, and the `LazyForEach` frame.
fn emit_lazy_for_each_tail(
    loop_: &UiLoop,
    out: &mut String,
    indent: &str,
    extends: &[ExtendFn],
    styles: &[StyleFn],
    builders: &[Builder],
) {
    // A key generator is optional: without one the compiler passes four arguments and emits no
    // id function at all.
    match &loop_.key {
        Some((written, _)) => {
            out.push_str(&format!(
                "{indent}    const __lazyForEachItemIdFunc = {written};\n"
            ));
            out.push_str(&format!(
                "{indent}    LazyForEach.create(\"1\", this, {}, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);\n",
                loop_.source
            ));
        }
        None => out.push_str(&format!(
            "{indent}    LazyForEach.create(\"1\", this, {}, __lazyForEachItemGenFunction);\n",
            loop_.source
        )),
    }
    // An attribute written on the `LazyForEach` call itself belongs to its frame, the way a
    // `ForEach`'s does (`ForEach(…).onMove(…)`). No file in the corpus chains one, so this
    // position is the `ForEach` one by symmetry rather than measured.
    let _ = emit_attribute_list(
        &loop_.attributes,
        &[],
        "LazyForEach",
        out,
        indent,
        builders,
        extends,
        styles,
        false,
    );
    // A `LazyForEach` has no creation frame of its own, and the reference keeps only the copy
    // that precedes its `pop()` (measured: `fixtures/comment-leading`, marker `LAZY_START`,
    // one occurrence in the golden).
    push_comments(out, &loop_.comment, &format!("{indent}    "));
    out.push_str(&format!("{indent}    LazyForEach.pop();\n"));
}

/// The attribute writes of one component, in source order.
///
/// A `@Reusable` view writes them only on the first render: the recycled instance keeps them,
/// and rewriting them on every update costs a diff.
#[allow(clippy::too_many_arguments)] // Each parameter is a distinct table the list needs.
fn emit_attributes(
    node: &UiNode,
    name: &str,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
    animation_open: bool,
) -> bool {
    emit_attribute_list(
        &node.attributes,
        &node.comment,
        name,
        out,
        indent,
        builders,
        extends,
        styles,
        animation_open,
    )
}

/// `reuseOrCreateNewComponent` for a `@ReusableV2` child (`process_struct_componentV2.js`
/// `createReusableV2Object` + `generateReuseOrCreateArgArr`).
///
/// The frame is one call and nothing else: no `componentCall`, no `paramsLambda`, no
/// `ViewV2.create`, and no `updateStateVarsOfChildByElmtId` branch — the runtime's
/// `reuseOrCreateNewComponent` decides between reusing a pooled instance and constructing one.
///
/// - `componentClass` is the identifier the call site names, not a string;
/// - `getParams` returns the call site's own object literal, re-wrapped in parentheses so the
///   arrow body parses as an expression rather than a block;
/// - `getReuseId` is the API-26 conditional; without an attribute-level reuse id both arms are
///   the child's name (measured: the fixture's golden);
/// - `extraInfo` is the same page/line/col object the ordinary frame carries.
fn emit_reuse_or_create_child(node: &UiNode, out: &mut String, indent: &str) {
    let page = crate::build::ets_compile::source_page().unwrap_or_default();
    let component = &node.component;
    let short = component.rsplit('.').next().unwrap_or(component.as_str());
    let written = node.create_args.as_deref().unwrap_or("{}");
    let inner = written
        .trim()
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap_or("")
        .trim();
    out.push_str(&format!("{indent}{{\n"));
    out.push_str(&format!(
        "{indent}    {}.reuseOrCreateNewComponent({{\n",
        frame_receiver()
    ));
    out.push_str(&format!("{indent}        componentClass: {component},\n"));
    if inner.is_empty() {
        out.push_str(&format!("{indent}        getParams: () => ({{}}),\n"));
    } else {
        out.push_str(&format!(
            "{indent}        getParams: () => ({{{inner}}}),\n"
        ));
    }
    out.push_str(&format!(
        "{indent}        getReuseId: getUINativeModule().common.getApiTargetVersion() >= 26 ? () => \"{short}\" : () => \"{short}\",\n"
    ));
    out.push_str(&format!(
        "{indent}        extraInfo: {{ page: \"{page}\", line: {}, col: {} }}\n",
        node.line, node.col
    ));
    out.push_str(&format!("{indent}    }});\n"));
    out.push_str(&format!("{indent}}}\n"));
}

/// The `parseBuilderNode` rendering of one builder expression: the shape a builder-property
/// argument takes, and the shape one arm of a conditional written there takes.
///
/// This is the single place the decision is made: a whole attribute argument
/// ([`rewrite_arguments`]) and a `builder:` / `title:` member value
/// ([`rewrite_builder_properties`]) both end here. `attribute` decides the
/// `CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY` form and `builders` is the name table a bare
/// reference has to be in.
///
/// `processConditionalBuilder` sends each arm of a conditional here on its own, so a
/// `cond ? Xb_Menu(…) : null` argument becomes `cond ? { builder: () => { Xb_Menu.call(this, …) } }
/// : null` (measured: `fixtures/conditional-builder`). An arm that is not a builder stays as
/// written, and so does an arm that is itself a conditional (the reference does not recurse
/// there).
///
/// Returns `None` for anything that is not a builder — `null`, `undefined`, a plain value — so
/// the callers leave those alone.
fn parse_builder_node(text: &str, attribute: &str, builders: &[Builder]) -> Option<String> {
    builder_argument_value(text, attribute, builders)
}

/// [`parse_builder_node`] for a value whose text is the whole expression.
fn builder_argument_value(text: &str, attribute: &str, builders: &[Builder]) -> Option<String> {
    let scope = BuilderScope {
        builders,
        locals: None,
    };
    with_expression(text, |expression, shift| {
        conditional_value(expression, text, attribute, &scope, shift)
            .or_else(|| builder_value(expression, text, attribute, &scope, shift))
    })
    .flatten()
}

/// `processConditionalBuilder`: each arm goes through `parseBuilderNode` on its own.
///
/// An arm that is itself a conditional is not a builder node, so it stays as written — the
/// reference does not recurse there (measured: `cond ? MyMenu() : cond ? OtherMenu() : null`
/// wraps the first arm only).
fn conditional_value(
    expression: &Expression<'_>,
    source: &str,
    attribute: &str,
    scope: &BuilderScope<'_>,
    shift: usize,
) -> Option<String> {
    let Expression::ConditionalExpression(conditional) = expression else {
        return None;
    };
    let mut changed = false;
    let mut arm = |node: &Expression<'_>| -> String {
        match builder_value(node, source, attribute, scope, shift) {
            Some(value) => {
                changed = true;
                value
            }
            None => node_text(node, source, shift),
        }
    };
    let when_true = arm(&conditional.consequent);
    let when_false = arm(&conditional.alternate);
    let test = node_text(&conditional.test, source, shift);
    changed.then(|| format!("{test} ? {when_true} : {when_false}"))
}

/// [`conditional_value`] for a value whose text is the whole expression.
fn rewrite_conditional_builder(
    text: &str,
    attribute: &str,
    builders: &[Builder],
) -> Option<String> {
    let scope = BuilderScope {
        builders,
        locals: None,
    };
    with_expression(text, |expression, shift| {
        conditional_value(expression, text, attribute, &scope, shift)
    })
    .flatten()
}

/// The attribute writes of one component or loop.
///
/// A `@Reusable` view keeps its attribute writes on the recycled instance, so the reference
/// writes only the *regular* ones inside `if (isInitialRender)` and leaves the rest to run on
/// every render (SPEC: `fixtures/reusable-guard-boundary/SPEC.md`). Every other component writes
/// them all, in source order.
#[allow(clippy::too_many_arguments)] // Each parameter is a distinct table the list needs.
fn emit_attribute_list(
    attributes: &[UiAttribute],
    comments: &[String],
    name: &str,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
    animation_open: bool,
) -> bool {
    if crate::build::ets_compile::is_reusable_component() {
        emit_guarded_attribute_writes(
            attributes, comments, name, out, indent, builders, extends, styles,
        );
        // The guarded frame writes every attribute inside its own `if`, so the close stays
        // where the caller puts it.
        false
    } else {
        emit_attribute_writes_in_order(
            attributes,
            comments,
            name,
            out,
            indent,
            builders,
            extends,
            styles,
            animation_open,
        )
    }
}

/// The attribute writes of a component that is not `@Reusable`: every write, in source order,
/// with no guard.
#[allow(clippy::too_many_arguments)] // Each parameter is a distinct table the list needs.
fn emit_attribute_writes_in_order(
    attributes: &[UiAttribute],
    comments: &[String],
    name: &str,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
    animation_open: bool,
) -> bool {
    let attribute_indent = indent.to_string();
    // `Context.animation(…)` wraps only the writes that follow it in the chain, so the close
    // belongs at the `animation` attribute's own position, not after the last write
    // (measured on clashbox's `FloatBar.ets`: the reference closes before `.shadow`, we
    // wrapped the rest).
    let mut animation_closed = false;
    for attribute in attributes {
        if attribute.name == "animation" {
            if animation_open && !animation_closed {
                out.push_str(&format!(
                    "{indent}    globalThis.Context.animation(null);\n"
                ));
                animation_closed = true;
            }
            continue;
        }
        // A `gesture(…)` call is not an attribute write: the comment is not repeated above it
        // (measured: `fixtures/node-comment`, whose golden keeps three copies and none before
        // `globalThis.Gesture.create(…)`).
        if attribute.name == "gesture" {
            emit_gesture(&attribute.args, out, indent, builders);
            continue;
        }
        // An `@Extend` / `@AnimatableExtend` call is not an attribute write either: the comment is
        // not repeated above it (measured: `fixtures/comment-leading`, marker `EXTEND_CALL` — the
        // golden keeps three copies for a `Text` with `.fontSize(9).loudExtend()` and none above
        // `__Text__loudExtend();`).
        let extension_call = extends
            .iter()
            .any(|candidate| candidate.name == attribute.name && candidate.comp == *name)
            || crate::build::stage::process_ui_syntax::is_animatable_extend(name, &attribute.name);
        if !extension_call {
            // The comment above a component is repeated before every write the frame emits, not
            // only the first (measured: `fixtures/comment-leading`, markers `ATTR_ONE` 3 copies,
            // `ATTR_TWO` 4, `ATTR_FOUR` 6 — one before `create`, one per attribute write, one
            // before `pop`).
            push_comments(out, comments, &format!("{indent}    "));
        }
        // An `@Extend` helper is a free function; a `@Styles` function is inlined at its call
        // site; anything else is an ordinary attribute write on the component.
        if let Some(extend) = extends
            .iter()
            .find(|candidate| candidate.name == attribute.name && candidate.comp == *name)
        {
            out.push_str(&format!(
                "{indent}    __{}__{}({});\n",
                extend.comp, extend.name, attribute.args
            ));
            continue;
        }
        if let Some(style) = styles
            .iter()
            .find(|candidate| candidate.name == attribute.name)
        {
            emit_style_body(style, name, out, indent, builders);
            continue;
        }
        // `isExtendFunctionNode` (`process_component_build.js`): an `@AnimatableExtend`
        // attribute is called without the component receiver, and the three coordinates are
        // appended to the declared arguments.
        if crate::build::stage::process_ui_syntax::is_animatable_extend(name, &attribute.name) {
            emit_animatable_call(attribute, &format!("{indent}    "), out);
            continue;
        }
        out.push_str(&format!(
            "{attribute_indent}    {name}.{}({});\n",
            attribute.name,
            reprint_attribute_arguments(
                &rewrite_arguments(&attribute.args, name, &attribute.name, builders),
                &format!("{attribute_indent}    "),
            )
        ));
    }
    animation_closed
}

/// One `@Styles` call's body, written where the call stood.
///
/// The reference hands the body to `bindComponentAttr` a second time (`traverseStateStylesAttr`
/// and the `STYLES_ATTRIBUTE` branch), so the body is an attribute chain of its own: a
/// `.animation(…)` inside it opens a context before the body's writes when it is not the body's
/// first attribute, and closes at the attribute's own position. The enclosing component's frame
/// is not consulted, because the animation belongs to the body, not to the call site (measured:
/// clashbox `entry/components/Common/Common.ts:701-703`, where the body
/// `.padding(FONT_SIZE_16).animation({ duration: ANIMATION_DURATION_300, curve: Curve.Ease })`
/// emits `globalThis.Context.animation({…})`, then `Column.padding(FONT_SIZE_16)`, then
/// `globalThis.Context.animation(null)` — the open hoisted above the body's first write and the
/// close after its last one, which is the same rule `fixtures/animation-span` records for a
/// component's own chain).
fn emit_style_body(
    style: &StyleFn,
    name: &str,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
) {
    let animation_at = style
        .body
        .iter()
        .position(|(attribute, _)| attribute == "animation");
    if let Some(at) = animation_at.filter(|at| *at > 0) {
        let (_, args) = &style.body[at];
        out.push_str(&format!(
            "{indent}    globalThis.Context.animation({});\n",
            rewrite_arguments(&args.join(", "), name, "animation", builders)
        ));
    }
    for (index, (attribute_name, args)) in style.body.iter().enumerate() {
        if Some(index) == animation_at {
            out.push_str(&format!(
                "{indent}    globalThis.Context.animation(null);\n"
            ));
            continue;
        }
        out.push_str(&format!(
            "{indent}    {name}.{attribute_name}({});\n",
            args.join(", ")
        ));
    }
}

/// The attribute writes of a `@Reusable` view, split into the ones that run on every render and
/// the ones the first-render guard keeps (SPEC §4).
///
/// Four rules, each measured (`fixtures/proxy-node-item`, `fixtures/reusable-guard-boundary`):
///
/// 1. the dynamic writes come first, outside the guard, in source order;
/// 2. the guard is emitted only when the static set is non-empty — a frame whose writes are all
///    dynamic has no `if (isInitialRender)` at all;
/// 3. an `@Styles` call's body is classified write by write, its static writes *unshifted* into
///    the static set and its dynamic writes pushed into the dynamic one;
/// 4. `@Extend` / `@AnimatableExtend` calls are free functions: they stay above the guard and are
///    not part of either set.
#[allow(clippy::too_many_arguments)] // Each parameter is a distinct table the list needs.
fn emit_guarded_attribute_writes(
    attributes: &[UiAttribute],
    comments: &[String],
    name: &str,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    let outside = format!("{indent}    ");
    let inside = format!("{indent}        ");
    // An `@Extend` helper is a free function, so the reference emits it **before** the guard and
    // leaves the guard to the attribute writes (`fixtures/proxy-node-item/golden/Index.ts:218`).
    for attribute in attributes {
        if let Some(extend) = extends
            .iter()
            .find(|candidate| candidate.name == attribute.name && candidate.comp == *name)
        {
            out.push_str(&format!(
                "{indent}    __{}__{}({});\n",
                extend.comp, extend.name, attribute.args
            ));
        }
    }
    // An `@AnimatableExtend` call keeps the same position for the same reason.
    for attribute in attributes {
        if crate::build::stage::process_ui_syntax::is_animatable_extend(name, &attribute.name) {
            emit_animatable_call(attribute, &outside, out);
        }
    }
    let ctx = classify_ctx();
    let default_ctx = ClassifyCtx::default();
    let ctx = ctx.as_deref().unwrap_or(&default_ctx);
    let mut dynamic: Vec<String> = Vec::new();
    let mut statics: Vec<String> = Vec::new();
    for attribute in attributes {
        if attribute.name == "animation" {
            continue;
        }
        if attribute.name == "gesture" {
            let mut line = String::new();
            emit_gesture(&attribute.args, &mut line, &outside, builders);
            dynamic.push(line);
            continue;
        }
        // Already written above the guard.
        if extends
            .iter()
            .any(|candidate| candidate.name == attribute.name && candidate.comp == *name)
        {
            continue;
        }
        if let Some(style) = styles
            .iter()
            .find(|candidate| candidate.name == attribute.name)
        {
            for (attribute_name, args) in &style.body {
                let arguments = args.join(", ");
                let static_write = attribute_is_static(&arguments, ctx);
                let prefix = if static_write { &inside } else { &outside };
                let line = format!("{prefix}{name}.{attribute_name}({arguments});\n");
                let line = with_leading_comment(line, prefix, comments);
                if static_write {
                    // An `@Styles` body's writes land at the head of the static set, in body
                    // order, which is what the reference's emission order amounts to (SPEC §5).
                    statics.insert(0, line);
                } else {
                    dynamic.push(line);
                }
            }
            continue;
        }
        if crate::build::stage::process_ui_syntax::is_animatable_extend(name, &attribute.name) {
            continue;
        }
        let static_write = attribute_is_static(&attribute.args, ctx);
        let prefix = if static_write { &inside } else { &outside };
        let line = format!(
            "{prefix}{name}.{}({});\n",
            attribute.name,
            reprint_attribute_arguments(
                &rewrite_arguments(&attribute.args, name, &attribute.name, builders),
                prefix,
            )
        );
        let line = with_leading_comment(line, prefix, comments);
        if static_write {
            statics.push(line);
        } else {
            dynamic.push(line);
        }
    }
    // A frame whose every attribute is an `@Extend` / `@AnimatableExtend` call produces no write
    // line, and the comment still has to be written somewhere.
    if dynamic.is_empty() && statics.is_empty() && !comments.is_empty() {
        push_comments(out, comments, &format!("{indent}    "));
    }
    for line in dynamic {
        out.push_str(&line);
    }
    if !statics.is_empty() {
        out.push_str(&format!("{indent}    if (isInitialRender) {{\n"));
        for line in statics {
            out.push_str(&line);
        }
        out.push_str(&format!("{indent}    }}\n"));
    }
}

/// Puts the component's comment ahead of an attribute write the frame emits — every write, not
/// only the first (measured: `fixtures/comment-leading`, markers `ATTR_ONE`..`ATTR_REUSABLE`).
fn with_leading_comment(line: String, prefix: &str, comments: &[String]) -> String {
    if comments.is_empty() {
        return line;
    }
    let mut out = String::new();
    push_comments(&mut out, comments, prefix);
    out.push_str(&line);
    out
}

/// Whether one attribute write belongs inside `if (isInitialRender)`.
///
/// `isRegularAttrNode` (`process_component_build.js`) is the reference's judgement, and it is a
/// *shape* test, not a constness test:
///
/// * a literal — string, number, `true`, `false`, an arrow function — is regular;
/// * an `X.Y` value is regular only when `X` names an enum (the reference asks the type checker
///   whether the value's type is an enum; a class's static member, a namespace export and a module
///   variable are not);
/// * `this.<member>` is regular when the member is one of the struct's undecorated fields;
/// * an object literal is regular when every member value is; an array literal when every element
///   is;
/// * everything else — a call, a unary / binary / conditional / `as` expression, a bare
///   identifier, a template literal — is dynamic.
///
/// A call with no argument is regular (`[].every(...)`), and the judgement is made per argument.
fn attribute_is_static(args: &str, ctx: &ClassifyCtx) -> bool {
    let tokens = crate::build::lex::tokenize(args);
    let tokens: Vec<&crate::build::lex::Token> = tokens
        .iter()
        .filter(|token| token.kind != crate::build::lex::Kind::Comment)
        .collect();
    if tokens.is_empty() {
        return true;
    }
    split_ranges(&tokens, 0, tokens.len())
        .iter()
        .all(|(start, end)| is_regular_range(&tokens, *start, *end, ctx))
}

/// `isRegularAttrNode` for one expression, as a token range.
fn is_regular_range(
    tokens: &[&crate::build::lex::Token],
    start: usize,
    end: usize,
    ctx: &ClassifyCtx,
) -> bool {
    let (start, end) = strip_parens(tokens, start, end);
    if start >= end {
        return false;
    }
    let first = tokens[start];
    // `$r('app.…')` / `$rawfile('…')`: the resource pass rewrites the call into a literal object
    // (`{ id, type, params: [], bundleName, moduleName }`) before the reference classifies the
    // write, so it reads as an object literal. hvolt still holds the call at this point, so it is
    // taken as one (measured: clashbox `StylesEdit.ts`, where the write belongs inside the guard).
    if first.kind == crate::build::lex::Kind::Ident
        && matches!(first.text, "$r" | "$rawfile")
        && tokens
            .get(start + 1)
            .is_some_and(|token| token.is_punct('('))
        && matching_token(tokens, start + 1) == Some(end - 1)
    {
        return true;
    }
    if first.is_punct('{') && matching_token(tokens, start) == Some(end - 1) {
        return object_values_regular(tokens, start, end, ctx);
    }
    if first.is_punct('[') && matching_token(tokens, start) == Some(end - 1) {
        return split_ranges(tokens, start + 1, end - 1)
            .iter()
            .all(|(piece_start, piece_end)| {
                is_regular_range(tokens, *piece_start, *piece_end, ctx)
            });
    }
    if end - start == 1 {
        return match first.kind {
            crate::build::lex::Kind::Number | crate::build::lex::Kind::Str => true,
            crate::build::lex::Kind::Ident => matches!(first.text, "true" | "false"),
            _ => false,
        };
    }
    // An arrow (or function) expression is a value whose body the reference does not look into.
    if first.kind == crate::build::lex::Kind::Ident && first.text == "function" {
        return true;
    }
    if has_top_level_arrow(tokens, start, end) {
        return true;
    }
    let Some((root, member)) = property_chain(tokens, start, end) else {
        return false;
    };
    if root == "this" {
        return ctx.regulars.contains(member);
    }
    // A chain of three or more names (`A.B.C`) is not the `Ident.Ident` form the reference reads
    // as an enum member, so it stays dynamic.
    if tokens[start + 2].is_punct('.') {
        return false;
    }
    match ctx.declared_kind(root) {
        Some(DeclKind::Enum) => true,
        // A local class / namespace / variable: its member is a value, not an enum member.
        Some(DeclKind::Class | DeclKind::Namespace | DeclKind::Var) => false,
        // An external (SDK or cross-file) name: read as a constant. This is the degradation the
        // specification accepts (SPEC §8.1) — it gets `TextAlign.End` right and would get a
        // cross-file class's static member wrong.
        None => true,
    }
}

/// The values of an object literal: every member needs one, and every value has to be regular.
fn object_values_regular(
    tokens: &[&crate::build::lex::Token],
    start: usize,
    end: usize,
    ctx: &ClassifyCtx,
) -> bool {
    for (piece_start, piece_end) in split_ranges(tokens, start + 1, end - 1) {
        let Some(colon) = top_level_colon(tokens, piece_start, piece_end) else {
            // A shorthand property, a spread or a method has no value of its own.
            return false;
        };
        if !is_regular_range(tokens, colon + 1, piece_end, ctx) {
            return false;
        }
    }
    true
}

/// Drops the parentheses wrapping the whole range. Parentheses are not a node of their own in the
/// reference's tree, so `(LocalKind.A)` is judged as `LocalKind.A`.
fn strip_parens(
    tokens: &[&crate::build::lex::Token],
    mut start: usize,
    mut end: usize,
) -> (usize, usize) {
    while start + 1 < end
        && tokens[start].is_punct('(')
        && matching_token(tokens, start) == Some(end - 1)
    {
        start += 1;
        end -= 1;
    }
    (start, end)
}

/// The index of the bracket that closes the one at `open`.
fn matching_token(tokens: &[&crate::build::lex::Token], open: usize) -> Option<usize> {
    let close = match tokens.get(open)?.text {
        "(" => ")",
        "[" => "]",
        "{" => "}",
        _ => return None,
    };
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind != crate::build::lex::Kind::Punct {
            continue;
        }
        if token.text == tokens[open].text {
            depth += 1;
        } else if token.text == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// The ranges between the top-level commas of `[start, end)`; an empty trailing piece is dropped.
fn split_ranges(
    tokens: &[&crate::build::lex::Token],
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut pieces = Vec::new();
    let mut depth = 0i32;
    let mut piece = start;
    for (offset, token) in tokens[start..end].iter().enumerate() {
        let index = start + offset;
        if token.kind != crate::build::lex::Kind::Punct {
            continue;
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => {
                pieces.push((piece, index));
                piece = index + 1;
            }
            _ => {}
        }
    }
    if piece < end {
        pieces.push((piece, end));
    }
    pieces
}

/// The index of the `:` that separates a property's name from its value, at the range's own level.
fn top_level_colon(
    tokens: &[&crate::build::lex::Token],
    start: usize,
    end: usize,
) -> Option<usize> {
    let mut depth = 0i32;
    for (offset, token) in tokens[start..end].iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Punct {
            continue;
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            ":" if depth == 0 => return Some(start + offset),
            _ => {}
        }
    }
    None
}

/// Whether an `=>` sits at the range's own level (`(a, b) => …`, `a => …`).
fn has_top_level_arrow(tokens: &[&crate::build::lex::Token], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    let mut previous = "";
    for token in &tokens[start..end] {
        if token.kind == crate::build::lex::Kind::Punct {
            match token.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                ">" if depth == 0 && previous == "=" => return true,
                _ => {}
            }
        }
        previous = token.text;
    }
    false
}

/// The root and first member of a `Ident ('.' Ident)+` chain — `("this", "layer")` for
/// `this.layer.style`, `("TextAlign", "End")` for `TextAlign.End`.
fn property_chain<'a>(
    tokens: &[&'a crate::build::lex::Token],
    start: usize,
    end: usize,
) -> Option<(&'a str, &'a str)> {
    // An odd number of tokens, alternating name and `.`.
    let length = end - start;
    if length < 3 || length % 2 == 0 {
        return None;
    }
    for (offset, token) in tokens[start..end].iter().enumerate() {
        if offset % 2 == 0 {
            if token.kind != crate::build::lex::Kind::Ident {
                return None;
            }
        } else if !token.is_punct('.') {
            return None;
        }
    }
    Some((tokens[start].text, tokens[start + 2].text))
}

/// The declaration facts one attribute write is classified against.
///
/// This stands in for the type checker `isRegularAttrNode` asks. Set once per struct while its
/// body is rendered; a component emitted without one classifies as if every name were external.
#[derive(Default)]
struct ClassifyCtx {
    enums: std::collections::BTreeSet<String>,
    classes: std::collections::BTreeSet<String>,
    namespaces: std::collections::BTreeSet<String>,
    vars: std::collections::BTreeSet<String>,
    /// The struct's fields that carry no decorator (`regularCollection` in
    /// `validate_ui_syntax.js::traversalComponentProps` only holds those).
    regulars: std::collections::BTreeSet<String>,
    /// Fields carrying a state decorator: a read of one inside a builder proxy object is
    /// written with the backstore fallback (`this["__x"] ? this["__x"] : this["x"]`).
    observed: std::collections::BTreeSet<String>,
}

/// The kinds `declared_kind` has to tell apart: only `Enum` is regular.
enum DeclKind {
    Enum,
    Class,
    Namespace,
    Var,
}

impl ClassifyCtx {
    fn of(comp: &ComponentInfo, local: &FileFacts) -> Self {
        Self {
            enums: local.enums.clone(),
            classes: local.classes.clone(),
            namespaces: local.namespaces.clone(),
            vars: local.vars.clone(),
            regulars: comp
                .fields
                .iter()
                .filter(|field| {
                    field
                        .decorators
                        .iter()
                        .all(|decorator| decorator == "Require")
                })
                .map(|field| field.name.clone())
                .collect(),
            observed: comp
                .fields
                .iter()
                .filter(|field| {
                    field
                        .decorators
                        .iter()
                        .any(|decorator| decorator != "Require")
                })
                .map(|field| field.name.clone())
                .collect(),
        }
    }

    fn declared_kind(&self, name: &str) -> Option<DeclKind> {
        if self.enums.contains(name) {
            return Some(DeclKind::Enum);
        }
        if self.classes.contains(name) {
            return Some(DeclKind::Class);
        }
        if self.namespaces.contains(name) {
            return Some(DeclKind::Namespace);
        }
        if self.vars.contains(name) {
            return Some(DeclKind::Var);
        }
        None
    }
}

thread_local! {
    /// The classification context of the struct whose body is being rendered. A file renders one
    /// struct at a time, and a test lowers files on its own threads.
    static CLASSIFY: std::cell::RefCell<Option<std::rc::Rc<ClassifyCtx>>> =
        const { std::cell::RefCell::new(None) };
}

/// Installs a classification context for one struct; dropping restores the previous one.
struct ClassifyScope(Option<std::rc::Rc<ClassifyCtx>>);

impl ClassifyScope {
    fn enter(ctx: ClassifyCtx) -> Self {
        Self(CLASSIFY.with(|slot| slot.replace(Some(std::rc::Rc::new(ctx)))))
    }
}

impl Drop for ClassifyScope {
    fn drop(&mut self) {
        let previous = self.0.take();
        CLASSIFY.with(|slot| *slot.borrow_mut() = previous);
    }
}

/// The backstore fallback the reference writes for a read of a state-decorated field inside a
/// builder proxy object: `this["__x"] ? this["__x"] : this["x"]` (measured:
/// `fixtures/builder-param-proxy`, the member read inside the proxy object).
pub(crate) fn observed_backstore_read(name: &str) -> Option<String> {
    let ctx = classify_ctx()?;
    ctx.observed
        .contains(name)
        .then(|| format!("this[\"__{name}\"] ? this[\"__{name}\"] : this[\"{name}\"]"))
}

/// The context installed for the struct being rendered, if any.
fn classify_ctx() -> Option<std::rc::Rc<ClassifyCtx>> {
    CLASSIFY.with(|slot| slot.borrow().clone())
}

/// One `@AnimatableExtend` call site: `name(args, elmtId, isInitialRender, this);`.
///
/// `isExtendFunctionNode`/`bindComponentAttr` (`process_component_build.js`) build the call as a
/// plain identifier — no component receiver — and append the frame's `elmtId`, its
/// `isInitialRender` flag and the owning `this` after the declared arguments.
fn emit_animatable_call(attribute: &UiAttribute, indent: &str, out: &mut String) {
    let args = attribute.args.trim();
    if args.is_empty() {
        out.push_str(&format!(
            "{indent}{}(elmtId, isInitialRender, this);\n",
            attribute.name
        ));
    } else {
        out.push_str(&format!(
            "{indent}{}({}, elmtId, isInitialRender, this);\n",
            attribute.name, args
        ));
    }
}

/// `checkLazyCreate` (process_component_build.js) gives a `ListItem` the
/// `itemCreation`/`itemCreation2`/`deepRenderFunction` triple: `ListItem.create` takes the render
/// function as its first argument, and the three variables are that function and its two callers.
/// The exclusions are the string literal `'false'` as first argument, a `Repeat` template, and a
/// `LazyForEach` child — hbolt emits no `Repeat`, and a `LazyForEach` root is built by
/// `emit_lazy_for_each`, so only the literal check is left here.
///
/// ```text
/// const itemCreation = (elmtId, isInitialRender) => {
///     ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
///     ListItem.create(deepRenderFunction, true);
///     if (!isInitialRender) { ListItem.pop(); }
///     ViewStackProcessor.StopGetAccessRecording();
/// };
/// const itemCreation2 = (elmtId, isInitialRender) => {
///     ListItem.create(deepRenderFunction, true);
///     <attributes>
/// };
/// const deepRenderFunction = (elmtId, isInitialRender) => {
///     itemCreation(elmtId, isInitialRender);
///     <children>
///     ListItem.pop();
/// };
/// this.observeComponentCreation2(itemCreation2, ListItem);
/// ListItem.pop();
/// ```
fn emit_list_item(
    node: &UiNode,
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    let create = match &node.create_args {
        Some(args) => format!(
            "ListItem.create(deepRenderFunction, true, {})",
            rewrite_backstores(args)
        ),
        None => "ListItem.create(deepRenderFunction, true)".to_string(),
    };
    // `createItemBlock` returns a block, so each `ListItem` gets its own brace scope: the three
    // names would otherwise collide when one body holds several of them.
    out.push_str(&format!("{indent}{{\n"));
    let indent = &format!("{indent}    ");
    out.push_str(&format!(
        "{indent}const itemCreation = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!(
        "{indent}    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);\n"
    ));
    out.push_str(&format!("{indent}    {create};\n"));
    out.push_str(&format!("{indent}    if (!isInitialRender) {{\n"));
    // The comment above the `ListItem` travels to each of its three `pop()` calls — the
    // reference prints it in all three positions (`fixtures/list-item-comment`: `SIMPLE_ITEM`
    // at the guarded pop, the deep-render pop and the outer pop, one copy each).
    push_comments(out, &node.comment, &format!("{indent}        "));
    out.push_str(&format!("{indent}        ListItem.pop();\n"));
    out.push_str(&format!("{indent}    }}\n"));
    out.push_str(&format!(
        "{indent}    ViewStackProcessor.StopGetAccessRecording();\n"
    ));
    out.push_str(&format!("{indent}}};\n"));
    out.push_str(&format!(
        "{indent}const itemCreation2 = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!("{indent}    {create};\n"));
    // The animation attribute wraps the writes here exactly as it does in an ordinary frame
    // ([`emit_node`]): the context opens before the first write it wraps and closes at the
    // attribute's own position. `checkLazyCreate` builds this callback and the reference keeps
    // the pair inside it (clashbox `ConfigInfo.ts`: `ListItem.create(deepRenderFunction, true, …)`,
    // `globalThis.Context.animation({…})`, the writes, `globalThis.Context.animation(null)`).
    let animation = node
        .attributes
        .iter()
        .find(|attribute| attribute.name == "animation");
    let animation_opens = node
        .attributes
        .iter()
        .position(|attribute| attribute.name == "animation")
        .is_some_and(|position| position > 0);
    if let (true, Some(animation)) = (animation_opens, &animation) {
        out.push_str(&format!(
            "{indent}    globalThis.Context.animation({});\n",
            rewrite_arguments(&animation.args, "ListItem", "animation", builders)
        ));
    }
    let animation_closed = emit_attributes(
        node,
        "ListItem",
        out,
        indent,
        builders,
        extends,
        styles,
        animation.is_some(),
    );
    if animation.is_some() && !animation_closed {
        out.push_str(&format!(
            "{indent}    globalThis.Context.animation(null);\n"
        ));
    }
    out.push_str(&format!("{indent}}};\n"));
    out.push_str(&format!(
        "{indent}const deepRenderFunction = (elmtId, isInitialRender) => {{\n"
    ));
    out.push_str(&format!(
        "{indent}    itemCreation(elmtId, isInitialRender);\n"
    ));
    let inner = format!("{indent}    ");
    emit(&node.children, out, &inner, builders, extends, styles);
    push_comments(out, &node.comment, &format!("{indent}    "));
    out.push_str(&format!("{indent}    ListItem.pop();\n"));
    out.push_str(&format!("{indent}}};\n"));
    out.push_str(&format!(
        "{indent}this.observeComponentCreation2(itemCreation2, ListItem);\n"
    ));
    push_comments(out, &node.comment, indent);
    out.push_str(&format!("{indent}ListItem.pop();\n"));
    out.push_str(&format!("{indent}}}\n"));
}

/// Whether `checkLazyCreate` sends this `ListItem` through the ordinary create form instead: a
/// first argument of the string literal `'false'` opts out.
fn is_lazy_list_item(node: &UiNode) -> bool {
    match &node.create_args {
        Some(args) => args.trim() != "'false'",
        None => true,
    }
}

/// `LIST_ITEM` (pre_define.js), the one component `checkLazyCreate`
/// (process_component_build.js) gives the `create(deepRenderFunction, true)` form to.
const LIST_ITEM: &str = "ListItem";

/// Whether the item generator's root takes the deep-render form.
fn needs_deep_render(name: &str) -> bool {
    name == LIST_ITEM
}

/// A `@Builder` takes the parent component as its last parameter, so the generated method has
/// to declare it (`label(text: string, parent = null)`).
pub(crate) fn with_parent_param(signature: &str) -> String {
    // The parameter list is the *first* `(` of the declaration. Searching from the end picks
    // up a function type inside the parameters (`onCheck: () => void`) instead, which injects
    // the parent into the wrong list and leaves a bare `(parent = null) => void`.
    let tokens = crate::build::lex::tokenize(signature);
    let Some(paren) = tokens.iter().find(|t| t.is_punct('(')) else {
        return signature.to_string();
    };
    let Some(close) = crate::build::lex::matching_delimiter(signature, paren.start) else {
        return signature.to_string();
    };
    let head = &signature[..close];
    let tail = &signature[close..];
    if paren.end == close {
        format!("{head}parent = null{tail}")
    } else {
        format!("{head}, parent = null{tail}")
    }
}

/// The declared name of a struct builder method (`private badge(text)` → `badge`).
fn struct_builder_name(signature: &str) -> String {
    signature
        .split('(')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_string()
}

/// A `@LocalBuilder` method becomes a property whose value is an arrow function
/// (`localBuilderNode` in process_component_class.js): the modifiers are dropped and the
/// parameters stay as written, because the body may be rendered for any component it is bound
/// to.
fn local_builder_property(signature: &str) -> (String, String) {
    let tokens = crate::build::lex::tokenize(signature);
    let Some(paren) = tokens.iter().find(|t| t.is_punct('(')) else {
        return (struct_builder_name(signature), String::new());
    };
    let name = tokens
        .iter()
        .take_while(|t| t.start < paren.start)
        .filter(|t| t.kind == crate::build::lex::Kind::Ident)
        .map(|t| t.text)
        .last()
        .unwrap_or_default()
        .to_string();
    let params = crate::build::lex::matching_delimiter(signature, paren.start)
        .map(|close| signature[paren.end..close].trim().to_string())
        .unwrap_or_default();
    (name, params)
}

/// Struct-level `@Builder` methods: their bodies go through the same emission as `build()`,
/// and the decorator stays out of the output.
///
/// A `@LocalBuilder` method takes the other form: no `parent = null` parameter, the parent it
/// renders into read from the context stack at the top of the body, and the whole declaration
/// turned into a property (`process_component_class.js` `processComponentMethod`, which calls
/// `localBuilderNode` for `p.isLocalBuilder`).
pub(crate) fn emit_struct_builder_method(
    out: &mut String,
    comp: &ComponentInfo,
    method: &StructBuilderMethod,
    local: &FileFacts,
) {
    // A struct `@Builder` body writes attributes too, classified against the same struct.
    let _classify = ClassifyScope::enter(ClassifyCtx::of(comp, local));
    let is_local = local
        .names
        .contains(&struct_builder_name(&method.signature));
    // A struct `@Builder` method is a method, not a component: `transferBuilderCall`
    // (`process_component_build.js`) rewrites a statement-level `this.name(…)` call into
    // `this.name.bind(this)(…)`. The decision is the node's ([`bind_ui_builder_statements`]),
    // which is what keeps a call written as an attribute argument — `bindSheet(…, this.builder())`
    // — out of that spelling: it is a value, and `parseBuilderNode` gives it the arrow form.
    let prepared_body = method.body.to_string();
    let prepared_body = wrap_raw_object_reads(&prepared_body, comp, local);
    // Struct builders are registered as builder names for this emission only.
    let mut builders = comp.builders.clone();
    for other in &comp.struct_builder_methods {
        let name = struct_builder_name(&other.signature);
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
    // The block written above the declaration goes directly above the emitted method, one
    // line per comment, at the member indentation (`fixtures/declaration-comments`).
    for comment in &method.comment {
        out.push_str(&format!("    {comment}\n"));
    }
    if is_local {
        let (name, params) = local_builder_property(&method.signature);
        out.push_str(&format!("    {name} = ({params}) => {{\n"));
        // `globalBuilderParamAssignment()` is unshifted by `processComponentMethod` before the
        // body is rendered.
        out.push_str(&format!("        {BUILDER_PARENT}\n"));
    } else {
        out.push_str(&format!(
            "    {} {{\n",
            with_parent_param(&method.signature)
        ));
    }
    let builders = with_builder_params(builders, comp);
    let prepared_body = bind_ui_builder_statements(
        &prepared_body,
        &BuilderScope {
            builders: &builders,
            locals: Some(&local.names),
        },
    );
    let scope = is_local.then(LocalBuilderScope::enter);
    emit_builder_body(out, comp, method, &prepared_body, &builders);
    drop(scope);
    out.push_str(if is_local { "    };\n" } else { "    }\n" });
}

/// One builder body, rendered by the same walker `build()` uses.
fn emit_builder_body(
    out: &mut String,
    comp: &ComponentInfo,
    method: &StructBuilderMethod,
    prepared_body: &str,
    builders: &[Builder],
) {
    let types = collect_array_types(comp);
    // The coordinate a child's constructor records is relative to the body being parsed: a
    // `@Builder` method's own `{` line, not `build()`'s (`utils::StructBuilderMethod::body_line`
    // carries the same "brace line + newlines inside" arithmetic as `build_body_line`). Using
    // the struct's build body put every call inside a builder too early in the file (measured on
    // clashbox `ProxyPage.ts`: 5 coordinates, 147/153/126/129/132 instead of 393/399/420/496/499;
    // `Index.ts`: 9 coordinates).
    match crate::build::ets_compile::parse_build_at(prepared_body, &types, method.body_line) {
        Ok(nodes) => {
            let mut ui = String::new();
            crate::build::ets_compile::emit_ui(
                &nodes,
                &mut ui,
                "        ",
                builders,
                &comp.extends,
                &comp.styles,
            );
            for line in ui.lines() {
                out.push_str(line);
                out.push('\n');
            }
        }
        Err(_) => {
            // A body the walker cannot parse is emitted as written: a wrong shape here
            // would be worse than an unrendered block.
            for line in method.body.lines() {
                out.push_str("        ");
                out.push_str(line.trim_start());
                out.push('\n');
            }
        }
    }
}

/// build() → initialRender(): the same `observeComponentCreation2` emission the reference
/// produces (via ets_compile's `emit_ui`).
///
/// A file that declares a `@LocalBuilder` brackets the body with the context stack
/// (`process_component_build.js` `processComponentBuild`), because the builder it may call
/// reads `parent` from that stack. The bracket follows the reference's own guard: only a body
/// that has statements is wrapped, so `build() {}` stays empty.
pub(crate) fn emit_initial_render(
    out: &mut String,
    comp: &ComponentInfo,
    local: &FileFacts,
) -> Result<(), String> {
    crate::build::ets_compile::set_reusable_component(comp.is_reusable);
    // The attribute writes emitted from here are classified against this struct's declarations.
    let _classify = ClassifyScope::enter(ClassifyCtx::of(comp, local));
    for comment in &comp.build_comment {
        out.push_str(&format!("    {comment}\n"));
    }
    out.push_str("    initialRender() {\n");
    if let Some(body) = &comp.build_body {
        let bracketed = local.present && !body.trim().is_empty();
        if bracketed {
            out.push_str(&format!("        {CONTEXT_STACK_PUSH}\n"));
        }
        // The raw-object wrap runs on the body the source wrote, before any builder call is
        // bound: `isNeedGetRawObject` (`process_component_class.js`) requires the argument's
        // parent to be a call whose callee is a property access, which `this.titleText(this.title)`
        // still is and `this.titleText.bind(this)(…)` is not. `prepare_build_body` binds the
        // struct's own `@Builder` calls, so the wrap has to precede it; the struct `@Builder`
        // path already runs in that order ([`emit_struct_builder_method`]) and keeps its wrap
        // (measured: clashbox `Common.ts:3433/3438`, where the reference writes
        // `this.titleText.bind(this)(ObservedObject.GetRawObject(this.title))`).
        let prepared_body = wrap_raw_object_reads(body, comp, local);
        // A struct-level `@Builder` call stays where it is written: lifting it out of the
        // body loses its position (`ForEach` item scope, child blocks) and leaves a call
        // whose arguments are undefined at that point.
        let (prepared_body, builders) = prepare_build_body(&prepared_body, comp);
        let builders = with_builder_params(builders, comp);
        // Every statement-level builder call in the body — a `@Builder` method, a
        // `@BuilderParam`, a `@LocalBuilder` — takes the bound form, including the ones inside
        // a `ForEach` / `Repeat` generator or a component's child block
        // ([`bind_ui_builder_statements`]).
        let prepared_body = bind_ui_builder_statements(
            &prepared_body,
            &BuilderScope {
                builders: &builders,
                locals: Some(&local.names),
            },
        );
        let types = collect_array_types(comp);
        let nodes =
            crate::build::ets_compile::parse_build_at(&prepared_body, &types, comp.build_body_line)
                .map_err(|e| format!("{}: build() 解析失败: {e}", comp.name))?;
        let mut ui = String::new();
        crate::build::ets_compile::emit_ui(
            &nodes,
            &mut ui,
            "        ",
            &builders,
            &comp.extends,
            &comp.styles,
        );
        for line in ui.lines() {
            out.push_str(line);
            out.push('\n');
        }
        if bracketed {
            out.push_str(&format!("        {CONTEXT_STACK_POP}\n"));
        }
    }
    out.push_str("    }\n");
    Ok(())
}

/// `ObservedObject.GetRawObject(…)` around a read of an object-typed member that is handed to
/// a method call.
///
/// `process_component_class.js` `isNeedGetRawObject` decides it and `processBuildMember` applies
/// it to `build()` and to every builder body: the member's declared type is not one of
/// `string`/`number`/`boolean`/an enum (`simpleTypes` in `process_component_member.js`), the read
/// is a plain `this.<name>` whose value ends where it stands, and that value is the whole
/// argument of a call whose callee is a property access (`Object.keys(this.options)`). A field
/// without a type annotation, a `@BuilderParam` and a value that takes part in a larger
/// expression are all left as written.
fn wrap_raw_object_reads(body: &str, comp: &ComponentInfo, facts: &FileFacts) -> String {
    // `isNeedGetRawObject` opens with `!isInComponentV2Context()`: a `@ComponentV2` struct keeps
    // its members as written, because `ViewV2` reads them itself.
    if comp.is_v2 {
        return body.to_string();
    }
    let objects = object_state_members(comp, facts);
    if objects.is_empty() {
        return body.to_string();
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    collect_raw_object_wraps(body, &objects, 0, &mut edits);
    crate::build::ets_assemble::apply_replacements(body, &edits)
}

/// One span of `text`, whose first byte sits at `base` in the body: the walk itself.
///
/// A template literal is one token, so the `${…}` substitutions are scanned on their own, with
/// their offsets shifted back into the body; a read written inside one is still a read the
/// reference wraps (measured: `fixtures/raw-object-argument`).
fn collect_raw_object_wraps(
    text: &str,
    objects: &std::collections::BTreeSet<String>,
    base: usize,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let tokens = crate::build::lex::tokenize(text);
    for token in tokens
        .iter()
        .filter(|token| token.kind == crate::build::lex::Kind::Template)
    {
        for (start, end) in template_substitutions(token.text) {
            let from = token.start + start;
            let to = token.start + end;
            collect_raw_object_wraps(&text[from..to], objects, base + from, edits);
        }
    }
    let mut push = |start: usize, end: usize, text: &str| {
        edits.push((base + start, base + end, text.to_string()));
    };
    for index in 0..tokens.len() {
        if tokens[index].kind != crate::build::lex::Kind::Ident || tokens[index].text != "this" {
            continue;
        }
        if !tokens
            .get(index + 1)
            .is_some_and(|token| token.is_punct('.'))
        {
            continue;
        }
        let Some(name) = tokens.get(index + 2) else {
            continue;
        };
        if name.kind != crate::build::lex::Kind::Ident || !objects.contains(name.text) {
            continue;
        }
        // The read is the whole argument: `foo(this.x + 1)` and `foo(a ? b : this.x)` both keep
        // `this.x` as it stands, because there `e.parent` is not the call.
        let Some(after) = tokens.get(index + 3) else {
            continue;
        };
        if !(after.is_punct(',') || after.is_punct(')')) {
            continue;
        }
        let before = (0..index)
            .rev()
            .map(|at| &tokens[at])
            .find(|token| token.kind != crate::build::lex::Kind::Comment);
        if !before.is_some_and(|token| token.is_punct('(') || token.is_punct(',')) {
            continue;
        }
        let Some(open) = enclosing_call_paren(&tokens, index) else {
            continue;
        };
        if !is_property_access_callee(&tokens, open) {
            continue;
        }
        // `e.parent.expression.name !== GetRawObject`: a second wrap is never written.
        if open
            .checked_sub(1)
            .and_then(|at| tokens.get(at))
            .is_some_and(|token| token.text == "GetRawObject")
        {
            continue;
        }
        push(
            tokens[index].start,
            tokens[index].start,
            "ObservedObject.GetRawObject(",
        );
        push(name.end, name.end, ")");
    }
}

/// The `${…}` substitutions of one template literal, as byte ranges inside it.
fn template_substitutions(template: &str) -> Vec<(usize, usize)> {
    let bytes = template.as_bytes();
    let mut out = Vec::new();
    // The token's text opens with its own backtick.
    let mut at = 1usize;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'`' => break,
            b'$' if at + 1 < bytes.len() && bytes[at + 1] == b'{' => {
                // The matching delimiter is read from the `{` itself: the scanner already knows
                // how to step over nested strings, templates and braces.
                let Some(close) = crate::build::lex::matching_delimiter(&template[at + 1..], 0)
                else {
                    break;
                };
                out.push((at + 2, at + 1 + close));
                at += close + 2;
            }
            _ => at += 1,
        }
    }
    out
}

/// The names one struct contributes to `stateObjectCollection`: its decorated members, minus a
/// `@BuilderParam`, whose declared type is not one of the four `simpleTypes` (`string`, `number`,
/// `boolean`, an enum).
///
/// This is the registration half only. The set the reference consults is the accumulation of
/// every struct processed so far, which [`object_state_members`] assembles.
pub(crate) fn own_object_members(
    comp: &ComponentInfo,
    facts: &FileFacts,
) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for field in &comp.fields {
        if field.decorators.is_empty() || field.has("BuilderParam") {
            continue;
        }
        let Some(text) = field.type_text.as_deref() else {
            continue;
        };
        if crate::build::stage::process_component_class::is_simple_type_text(Some(text), facts) {
            continue;
        }
        out.insert(field.name.clone());
    }
    out
}

/// The names `stateObjectCollection` holds when `comp` is emitted: every struct processed before
/// it (`FileFacts::object_members`) plus its own registrations.
///
/// The reference's set is keyed by name and never cleared, so a member the *reading* struct
/// declares as `string` is still wrapped when another struct registered that name as an object
/// (`isNeedGetRawObject` asks the Set, not the declaration). Measured on clashbox
/// `entry/components/Common/Common.ets`: `Nothing`'s `@State name: Resource` precedes
/// `skipRowList`'s `@State name: string`, and the reference wraps the later read.
fn object_state_members(
    comp: &ComponentInfo,
    facts: &FileFacts,
) -> std::collections::BTreeSet<String> {
    let mut out = own_object_members(comp, facts);
    if let Some(earlier) = facts.object_members.get(&comp.declaration_offset) {
        out.extend(earlier.iter().cloned());
    }
    out
}

/// The `(` that opens the argument list `index` stands in, when that list belongs to a call.
fn enclosing_call_paren(tokens: &[crate::build::lex::Token<'_>], index: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut at = index;
    while at > 0 {
        at -= 1;
        let token = &tokens[at];
        if token.kind != crate::build::lex::Kind::Punct {
            continue;
        }
        match token.text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    return (token.text == "(").then_some(at);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Whether the call opened at `open` is written `object.method(…)`, which is the callee shape
/// `isNeedGetRawObject` requires (`isPropertyAccessExpression(e.parent.expression)`).
fn is_property_access_callee(tokens: &[crate::build::lex::Token<'_>], open: usize) -> bool {
    let Some(name) = open.checked_sub(1).and_then(|at| tokens.get(at)) else {
        return false;
    };
    if name.kind != crate::build::lex::Kind::Ident {
        return false;
    }
    open.checked_sub(2)
        .and_then(|at| tokens.get(at))
        .is_some_and(|token| token.is_punct('.'))
}

/// The two forms of a child's params: the constructor reads the parent's backstore, the params
/// lambda reads the same fields through their accessors.
pub(crate) fn child_params(
    node: &UiNode,
    meta: &crate::build::compile::component_index::Meta,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) -> (String, String) {
    let written = node.create_args.clone().unwrap_or_else(|| "{}".to_string());
    let link_fields = &meta.link_fields;
    let plain = |binding: Binding| rewrite_bindings(&written, binding, link_fields);
    // `processExpressionStatementChange` (`process_component_build.js:269`) is reached only for
    // a call that carries a trailing lambda, and that is where the `@BuilderParam` merge lives.
    // A construction written without a child block passes its object through untouched: the
    // component still declares the field, the call site simply does not fill it.
    let Some(field) = meta.builder_param.as_ref().filter(|_| node.has_block) else {
        return (plain(Binding::Backstore), plain(Binding::Accessor));
    };
    let mut body = String::new();
    let inner = format!("{indent}                ");
    emit(&node.children, &mut body, &inner, builders, extends, styles);
    let entry = format!("{field}: () => {{\n{body}{indent}            }}");
    let trimmed = written.trim();
    // `processBlockToExpression` (`:280`) appends: the call's own object is
    // `arguments[0].properties.concat([<builder field>])`, so the injected property follows
    // the written ones.
    let with_field = |fields: String| -> String {
        // The written object may end with a trailing comma (`{ a: 1, }`) or with an author's
        // trailing `//` comment; a comma appended straight after either one produces `,,` or
        // puts the injected property inside the comment. The trailing comma is dropped and the
        // property opens its own line, which is what the reference's node concatenation
        // amounts to once the text is written out.
        let fields = fields.trim_end().trim_end_matches(',').trim_end();
        if fields.is_empty() {
            format!("{{\n{indent}            {entry}\n{indent}        }}")
        } else {
            format!("{{ {fields},\n{indent}            {entry} }}")
        }
    };
    if let Some(rest) = trimmed.strip_prefix('{') {
        if let Some(inner_fields) = rest.trim_end().strip_suffix('}') {
            let fields = inner_fields.trim().to_string();
            let construct = with_field(rewrite_bindings(&fields, Binding::Backstore, link_fields));
            let lambda = with_field(rewrite_bindings(&fields, Binding::Accessor, link_fields));
            return (construct, lambda);
        }
    }
    (plain(Binding::Backstore), plain(Binding::Accessor))
}

#[cfg(test)]
mod tests {
    use super::rewrite_bindings;
    use crate::build::stage::process_custom_component::{construct_params, Binding};

    /// A `@Link` member is constructed from the parent's observed property, whichever way the
    /// source spelled it.
    #[test]
    fn link_members_are_constructed_from_the_backstore() {
        let link = vec!["linked".to_string()];
        for written in ["{ linked: $count }", "{ linked: this.count }"] {
            assert_eq!(
                rewrite_bindings(written, Binding::Backstore, &link),
                "{ linked: this.__count }",
                "{written}"
            );
        }
    }

    /// The params lambda reads the same member through its accessor.
    #[test]
    fn link_members_are_read_back_through_the_accessor() {
        let link = vec!["linked".to_string()];
        assert_eq!(
            rewrite_bindings("{ linked: $count }", Binding::Accessor, &link),
            "{ linked: this.count }"
        );
    }

    /// The shape follows the member's decorator, not the call site's spelling: a member that
    /// is not a link keeps the expression it was written with.
    #[test]
    fn non_link_members_keep_the_written_expression() {
        let link = vec!["linked".to_string()];
        assert_eq!(
            rewrite_bindings(
                "{ linked: $count, copied: this.flag }",
                Binding::Backstore,
                &link
            ),
            "{ linked: this.__count, copied: this.flag }"
        );
    }

    /// A resource call is not a parent property, so no backstore is invented for it.
    #[test]
    fn a_call_argument_is_not_turned_into_a_backstore() {
        let link = vec!["linked".to_string()];
        let written = "{ linked: $r('app.string.x') }";
        assert_eq!(
            rewrite_bindings(written, Binding::Backstore, &link),
            written
        );
    }

    /// An arrow body is an expression like any other: its own properties are not re-pointed.
    #[test]
    fn a_nested_expression_is_left_alone() {
        let link = vec!["linked".to_string()];
        let written = "{ onClick: () => { this.count = 1 } }";
        assert_eq!(
            rewrite_bindings(written, Binding::Backstore, &link),
            "{ onClick: () => { this.count = 1 } }"
        );
    }

    /// The dialog call site builds its params the way a `build()` call site does.
    #[test]
    fn the_constructor_form_is_the_dialog_call_sites_form() {
        assert_eq!(
            construct_params("{ linked: this.count }", &["linked".to_string()]),
            "{ linked: this.__count }"
        );
    }

    /// A params argument that is not an object literal has no fields to read.
    #[test]
    fn a_params_argument_that_is_not_an_object_is_kept() {
        let link = vec!["linked".to_string()];
        assert_eq!(
            rewrite_bindings("someParams", Binding::Backstore, &link),
            "someParams"
        );
    }
}

#[cfg(test)]
mod raw_object_tests {
    use super::wrap_raw_object_reads;
    use crate::build::stage::process_component_class::{file_facts, FileFacts};
    use crate::build::stage::utils::ComponentInfo;

    fn facts_of(source: &str) -> (ComponentInfo, FileFacts) {
        (
            crate::build::lower::ets_component::parse_components(source, &[])
                .expect("component source parses")
                .remove(0),
            file_facts(source),
        )
    }

    const V1: &str = "@Component\nstruct A {\n  @State options: Options = {}\n  @State size: number = 1\n  @State tag: string = ''\n  private untyped = {}\n  build() {\n    Text('x')\n  }\n}\n";

    /// An object-typed member handed to a method call is read through `ObservedObject`.
    #[test]
    fn an_object_member_argument_is_read_through_the_raw_object() {
        let (comp, facts) = facts_of(V1);
        assert_eq!(
            wrap_raw_object_reads("Object.keys(this.options)", &comp, &facts),
            "Object.keys(ObservedObject.GetRawObject(this.options))"
        );
    }

    /// A member whose declared type is one of the reference's `simpleTypes` is left alone.
    #[test]
    fn a_simple_typed_member_is_left_alone() {
        let (comp, facts) = facts_of(V1);
        for body in ["Math.max(this.size)", "join(this.tag)"] {
            assert_eq!(wrap_raw_object_reads(body, &comp, &facts), body, "{body}");
        }
    }

    /// `simpleTypes` counts an enum as simple (`process_component_member.js`), so a member
    /// declared with one the file declares is not wrapped.
    #[test]
    fn an_enum_typed_member_is_left_alone() {
        let source = "@Component\nstruct A {\n  @State mode: Mode = Mode.Off\n  @State mixed: Mode | undefined = undefined\n  build() {\n    Text('x')\n  }\n}\nenum Mode {\n  Off,\n  On\n}\n";
        let (comp, facts) = facts_of(source);
        for body in ["f(this.mode)", "f(this.mixed)"] {
            assert_eq!(wrap_raw_object_reads(body, &comp, &facts), body, "{body}");
        }
    }

    /// The read has to be the whole argument: a larger expression or a ternary branch keeps it.
    #[test]
    fn a_read_inside_a_larger_expression_is_left_alone() {
        let (comp, facts) = facts_of(V1);
        for body in [
            "Math.max(this.options.length)",
            "Text.width(a ? b : this.options)",
            "Text.width(this.options ?? b)",
            "Text.width(...this.options)",
        ] {
            assert_eq!(wrap_raw_object_reads(body, &comp, &facts), body, "{body}");
        }
    }

    /// The callee has to be a property access (`Object.keys(…)`), and a member without a
    /// declared type is not in the reference's collection.
    #[test]
    fn a_plain_callee_or_an_untyped_member_is_left_alone() {
        let (comp, facts) = facts_of(V1);
        for body in ["keys(this.options)", "f(this.untyped)"] {
            assert_eq!(wrap_raw_object_reads(body, &comp, &facts), body, "{body}");
        }
    }

    /// `isNeedGetRawObject` opens with `!isInComponentV2Context()`.
    #[test]
    fn a_component_v2_struct_keeps_its_members_as_written() {
        let (comp, facts) = facts_of("@ComponentV2\nstruct B {\n  @Local options: Options = {}\n  build() {\n    Text('x')\n  }\n}\n");
        assert_eq!(
            wrap_raw_object_reads("Object.keys(this.options)", &comp, &facts),
            "Object.keys(this.options)"
        );
    }

    /// `isExtendFunctionNode` emits an `@AnimatableExtend` attribute as a bare call: no
    /// component receiver, and `elmtId`, `isInitialRender` and `this` appended.
    #[test]
    fn an_animatable_extend_attribute_drops_the_component_receiver() {
        use super::{emit_attribute_list, UiAttribute};
        crate::build::stage::process_ui_syntax::set_animatable_extends(&[(
            "Text".to_string(),
            "animWidth".to_string(),
        )]);
        let attributes = vec![UiAttribute {
            name: "animWidth".to_string(),
            args: "this.target".to_string(),
        }];
        let mut out = String::new();
        emit_attribute_list(
            &attributes,
            &[],
            "Text",
            &mut out,
            "    ",
            &[],
            &[],
            &[],
            false,
        );
        assert_eq!(
            out.trim(),
            "animWidth(this.target, elmtId, isInitialRender, this);"
        );
    }

    /// The comments above an `else if` / `else` sit before the `else` keyword, which is outside
    /// the alternate's own span — and a line comment may end on a multi-byte character, so the
    /// backward scan must not slice it in the middle.
    #[test]
    fn an_else_branch_comment_is_found_before_the_keyword() {
        use super::else_branch_comments;
        // `else if (…)`: the alternate starts at its `if`.
        let text = "if (a) {\n} // 分支\nelse if (b) {\n}";
        let at = text.find("if (b)").unwrap();
        assert_eq!(else_branch_comments(text, at), vec!["// 分支".to_string()]);
        // A plain `else {`: the alternate starts at its `{`.
        let text = "if (a) {\n}\n// BETWEEN\nelse {\n}";
        let at = text.find("else {").unwrap() + "else ".len();
        assert_eq!(
            else_branch_comments(text, at),
            vec!["// BETWEEN".to_string()]
        );
        // No comment above the branch: nothing is carried.
        let text = "if (a) {\n} else {\n}";
        let at = text.find("else {").unwrap() + "else ".len();
        assert!(else_branch_comments(text, at).is_empty());
    }

    /// The same rule with no declared arguments keeps the appended triple alone.
    #[test]
    fn an_animatable_extend_attribute_without_arguments_still_appends_the_triple() {
        use super::{emit_attribute_list, UiAttribute};
        crate::build::stage::process_ui_syntax::set_animatable_extends(&[(
            "Text".to_string(),
            "animClear".to_string(),
        )]);
        let attributes = vec![UiAttribute {
            name: "animClear".to_string(),
            args: String::new(),
        }];
        let mut out = String::new();
        emit_attribute_list(
            &attributes,
            &[],
            "Text",
            &mut out,
            "    ",
            &[],
            &[],
            &[],
            false,
        );
        assert_eq!(out.trim(), "animClear(elmtId, isInitialRender, this);");
    }

    /// A multi-line object argument moves to the call's column, keeps the offset of its nested
    /// lines (a block comment's pointer included), and loses only the comments of rebuilt
    /// members — the last member's comment and a nested body's comment stay.
    #[test]
    fn a_multi_line_object_argument_is_reprinted_at_the_call_column() {
        use super::reprint_attribute_arguments;
        let written = "{\n      detents: [1, 2], // drop\n      /** doc\n       * more\n       */\n      onAppear: () => {\n        run(); // keep\n      },\n      onDismiss: () => { } // keep\n    }";
        let expected = [
            "{",
            "                detents: [1, 2],",
            "                /** doc",
            "                 * more",
            "                 */",
            "                onAppear: () => {",
            "                  run(); // keep",
            "                },",
            "                onDismiss: () => { } // keep",
            "            }",
        ]
        .join("\n");
        assert_eq!(
            reprint_attribute_arguments(written, "            "),
            expected
        );
    }

    /// The one-line shape is untouched: the layout would have nothing to reprint, and its
    /// inline comments belong to the call as written.
    #[test]
    fn a_one_line_object_argument_is_left_alone() {
        use super::reprint_attribute_arguments;
        for written in [
            "{ a: 1, /* x */ b: 2 }",
            "{ url: 'http://example.test/a' }",
            "this.state",
        ] {
            assert_eq!(
                reprint_attribute_arguments(written, "    "),
                written,
                "{written}"
            );
        }
    }

    /// A template literal's lines are string content: reprinting the object must not move them.
    #[test]
    fn a_template_literal_in_a_reprinted_object_keeps_its_columns() {
        use super::reprint_attribute_arguments;
        let written = "{\n      text: `a\nb`,\n      other: 1\n    }";
        let expected = "{\n                text: `a\nb`,\n                other: 1\n            }";
        assert_eq!(
            reprint_attribute_arguments(written, "            "),
            expected
        );
    }
}

#[cfg(test)]
mod ast_builder_tests {
    use super::{bind_ui_builder_statements, rewrite_arguments, BuilderScope};
    use crate::build::ets_compile::Builder;

    fn builder(name: &str) -> Builder {
        Builder {
            name: name.to_string(),
            params: Vec::new(),
            body: String::new(),
            is_export: false,
            order: 0,
            comment: Vec::new(),
        }
    }

    fn builders(names: &[&str]) -> Vec<Builder> {
        names.iter().map(|name| builder(name)).collect()
    }

    /// A statement of a UI body takes the bound form: `transferBuilderCall` is reached for a
    /// `ForEach` item generator, which is a body the reference renders.
    #[test]
    fn a_builder_statement_inside_a_for_each_generator_is_bound() {
        let body = "ForEach(this.items, (item: string) => {\n  this.rowBuilder(item)\n})";
        let out = bind_ui_builder_statements(
            body,
            &BuilderScope {
                builders: &builders(&["rowBuilder"]),
                locals: None,
            },
        );
        assert!(out.contains("this.rowBuilder.bind(this)(item)"), "{out}");
    }

    /// A component's child block is a UI body even when the call's argument list ends on an
    /// earlier line, which is the shape the old line scan read as a callback.
    #[test]
    fn a_builder_statement_inside_a_child_block_is_bound() {
        let body = "GridCol({\n  span: 1\n}, () =>\n{\n  this.cardBuilder(this.item.key)\n})";
        let out = bind_ui_builder_statements(
            body,
            &BuilderScope {
                builders: &builders(&["cardBuilder"]),
                locals: None,
            },
        );
        assert!(
            out.contains("this.cardBuilder.bind(this)(this.item.key)"),
            "{out}"
        );
    }

    /// A `Repeat(…).each(…)` callback is a UI body too (`processRepeatPropWithChild`).
    #[test]
    fn a_builder_statement_inside_a_repeat_callback_is_bound() {
        let body = "Repeat<Row>(this.rows).each((row: Row) => {\n  this.rowBuilder(row.name)\n})";
        let out = bind_ui_builder_statements(
            body,
            &BuilderScope {
                builders: &builders(&["rowBuilder"]),
                locals: None,
            },
        );
        assert!(
            out.contains("this.rowBuilder.bind(this)(row.name)"),
            "{out}"
        );
    }

    /// A callback arrow is a closure, not a UI body: its statements keep the spelling the
    /// source gave them (`fixtures/builder-arrow-call`).
    #[test]
    fn a_builder_statement_inside_a_callback_arrow_is_left_alone() {
        let body = "Column(() => {\n  Text('x').onClick(() => {\n    this.rowBuilder()\n  })\n})";
        let out = bind_ui_builder_statements(
            body,
            &BuilderScope {
                builders: &builders(&["rowBuilder"]),
                locals: None,
            },
        );
        assert!(out.contains("this.rowBuilder()"), "{out}");
        assert!(!out.contains("bind(this)"), "{out}");
    }

    /// A builder call in a builder-value position is the arrow form, not the statement form:
    /// `parseBuilderNode` wraps it and keeps `this` through `.call`.
    #[test]
    fn a_builder_call_as_a_bind_sheet_argument_takes_the_arrow_form() {
        let args = "this.shown, this.settingsSheet()";
        let out = rewrite_arguments(args, "MorePage", "bindSheet", &builders(&["settingsSheet"]));
        assert!(
            out.contains("{ builder: () => { this.settingsSheet.call(this) } }"),
            "{out}"
        );
        assert!(!out.contains("bind(this)"), "{out}");
    }

    /// The two spellings are different products: the same call is bound as a statement and
    /// wrapped as an argument.
    #[test]
    fn the_statement_and_argument_spellings_stay_distinct() {
        let scope = BuilderScope {
            builders: &builders(&["settingsSheet"]),
            locals: None,
        };
        let statement = bind_ui_builder_statements("this.settingsSheet()", &scope);
        assert_eq!(statement, "this.settingsSheet.bind(this)()");
        let argument = rewrite_arguments(
            "this.shown, this.settingsSheet()",
            "MorePage",
            "bindSheet",
            &builders(&["settingsSheet"]),
        );
        assert!(
            argument.contains("{ builder: () => { this.settingsSheet.call(this) } }"),
            "{argument}"
        );
    }

    /// `processDragStartBuilder` (`BIND_DRAG_SET`): the callback's returned builder call
    /// becomes the `{ builder: … }` object.
    #[test]
    fn a_drag_start_returned_builder_call_is_wrapped() {
        let args = "(event: ItemDragInfo, index: number) => {\n  return this.dragPreviewBuilder(this.dragItem.resourceName)\n}";
        let out = rewrite_arguments(
            args,
            "Sort",
            "onItemDragStart",
            &builders(&["dragPreviewBuilder"]),
        );
        assert!(
            out.contains(
                "{ builder: () => { this.dragPreviewBuilder.call(this, this.dragItem.resourceName) } }"
            ),
            "{out}"
        );
        assert!(!out.contains(".bind(this)"), "{out}");
    }

    /// A global `@Builder` call written as an object member's value is the reference's partial
    /// application: `<callee>.bind(this, makeBuilderParameterProxy(…))`, with the callee as the
    /// source spelled it (`transformBuilderCallExpression`).
    #[test]
    fn a_global_builder_call_member_takes_the_proxy_application() {
        crate::build::compile::component_index::set_builders(
            ["GlobalSheetBuilder".to_string()].into_iter().collect(),
        );
        let object =
            "{\n  builder: GlobalSheetBuilder({\n    title: this.pageTitle\n  }),\n  height: 10\n}";
        let out = super::rewrite_builder_properties(object, "title", &[], false);
        assert!(
            out.contains(
                "builder: GlobalSheetBuilder.bind(this, makeBuilderParameterProxy(\"GlobalSheetBuilder\", {"
            ),
            "{out}"
        );
        assert!(out.contains("title: () => this.pageTitle"), "{out}");
        assert!(out.contains("height: 10"), "{out}");
    }

    /// A component or dialog constructor is not a builder: `builder: ProbeDialog(…)` keeps its
    /// spelling here (the dialog construction is a later pass).
    #[test]
    fn a_component_constructor_member_is_left_alone() {
        let object = "{ builder: ProbeDialog({ title: 'probe' }), height: 10 }";
        let out = super::rewrite_builder_properties(object, "title", &[], false);
        assert_eq!(out, object);
    }

    /// A returned value that is not a builder keeps its spelling.
    #[test]
    fn a_drag_start_returned_value_is_left_alone() {
        let args = "(index: number) => {\n  return this.resourceListData[index]\n}";
        let out = rewrite_arguments(args, "Sort", "onItemDragStart", &builders(&[]));
        assert_eq!(out, args);
    }
}

#[cfg(test)]
mod route_object_tests {
    use super::*;

    /// A `NavDestination` names the module the task assembles while `pagePath` keeps the
    /// coordinate of the module that owns the source file — the reference shape measured on
    /// clashbox: `{ moduleName: "entry", pagePath: "xb_components/src/main/ets/view/Xb_Studio" }`.
    #[test]
    fn a_destination_names_the_assembled_module_and_its_own_page_path() {
        assert_eq!(
            route_object(
                pre_define::COMPONENT_NAV_DESTINATION,
                "xb_components/src/main/ets/view/Xb_Studio",
                "entry",
                false
            ),
            ", { moduleName: \"entry\", pagePath: \"xb_components/src/main/ets/view/Xb_Studio\" }"
        );
    }

    /// A `Navigation` adds its `isUserCreateStack` flag to the same two keys.
    #[test]
    fn a_navigation_adds_the_stack_flag_after_the_two_keys() {
        assert_eq!(
            route_object(
                pre_define::COMPONENT_NAVIGATION,
                "xb_components/src/main/ets/components/Settings/Settings",
                "entry",
                true
            ),
            ", { moduleName: \"entry\", pagePath: \"xb_components/src/main/ets/components/Settings/Settings\", isUserCreateStack: true }"
        );
    }
}

#[cfg(test)]
mod child_component_tests {
    use super::{emit_child, UiNode};
    use crate::build::compile::component_index::{Meta, Protocol};

    fn child(component: &str, create_args: &str) -> UiNode {
        UiNode {
            component: component.to_string(),
            line: 7,
            col: 9,
            create_args: Some(create_args.to_string()),
            has_block: false,
            attributes: Vec::new(),
            children: Vec::new(),
            comment: Vec::new(),
        }
    }

    fn frame(node: &UiNode, meta: &Meta) -> String {
        let mut out = String::new();
        emit_child(node, meta, &mut out, "", &[], &[], &[]);
        out
    }

    /// A `@CustomDialog` component used as an ordinary child constructs through the same frame
    /// as any other child.
    ///
    /// `is_dialog` is the class-side flag: it decides the `setController` setter a
    /// `CustomDialogController` assigns through (`process_component_member`). The call site the
    /// reference writes for `CustomColorPickerDialog({ … })` in `Xb_SettingItem` is a plain
    /// child construction — a `{ … }` block, `new <Cmp>(this, …)`, `ViewPU.create`,
    /// `updateStateVarsOfChildByElmtId`, no controller and no `jsDialog` — so a future branch on
    /// `is_dialog` here would be a divergence, not a refinement. This pins that.
    #[test]
    fn a_custom_dialog_child_takes_the_ordinary_child_frame() {
        let dialog = Meta {
            protocol: Protocol::V1,
            is_dialog: true,
            state_vars: vec!["title".to_string()],
            ..Meta::default()
        };
        let plain = Meta {
            is_dialog: false,
            ..dialog.clone()
        };
        let node = child("ProbeDialog", "{ title: this.title }");
        assert_eq!(
            frame(&node, &dialog),
            frame(&node, &plain),
            "is_dialog changes no call site"
        );
        let out = frame(&node, &dialog);
        assert!(out.starts_with("{\n"), "the child frame is a block: {out}");
        assert!(
            out.contains("new ProbeDialog(this, { title: this.title }"),
            "{out}"
        );
        assert!(out.contains("ViewPU.create(componentCall);"), "{out}");
        assert!(
            out.contains("updateStateVarsOfChildByElmtId(elmtId, { title: this.title });"),
            "{out}"
        );
        assert!(!out.contains("setController"), "{out}");
        assert!(!out.contains("jsDialog"), "{out}");
    }
}
