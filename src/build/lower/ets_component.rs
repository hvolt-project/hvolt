// ets_transpile component expansion: `struct` → `ViewPU` / `ViewV2` class.
//
// Two protocols, one file each way (goldens: state-field, component-v2, reusable,
// 2026-11-05):
//
// V1 (`@Component` → `ViewPU`, process_component_class.js / process_component_member.js):
//   - header polyfill for `finalizeConstruction`;
//   - `interface <Name>_Params` with every field, optional, in source order;
//   - constructor(parent, params, __localStorage, elmtId = -1, paramsLambda =
//     undefined, extraInfo) with `paramsGenerator_`, then one backstore per decorated
//     field (`ObservedPropertySimplePU`/`ObjectPU`, `SynchedProperty*PU`,
//     `createStorageLink`/`Prop`, `initializeConsume`) in source order;
//   - lifecycle members: setInitiallyProvidedValue / updateStateVars /
//     purgeVariableDependenciesOnElmtId / aboutToBeDeleted, plus
//     `updateRecycleElmtId` for `@Reusable`;
//   - typed accessors (`private __x: <backstore><T>` + get/set; `@ObjectLink` has no setter)
//     interleaved with plain field declarations, then user methods;
//   - build() expands into initialRender() (view emission, T3b).
//
// V2 (`@ComponentV2` → `ViewV2`, process_struct_componentV2.js):
//   - no `_Params` interface and no backstores: the decorated members are kept verbatim
//     (`@Param` without `@Once` becomes `readonly`), the constructor calls `initParam` /
//     assigns, and `resetStateVarsOnReuse` resets every member (`resetParam`,
//     `resetConsumer`, `resetComputed`, `resetMonitorsOnReuse`);
//   - `updateStateVars` exists only when the struct declares `@Param` fields.
//
// A file may declare both; each struct picks its own base class.

use oxc_allocator::Allocator;
use oxc_ast::ast::{ClassElement, PropertyKey, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::build::compile::ets_source_map::SourcePosition;
use crate::build::ets_compile::preprocess;
use crate::build::stage::process_component_class::{
    context_stack_header, emit_class, emit_params, file_facts, reusable_v2_reflect_function,
};
use crate::build::stage::utils::protocol_for;
use crate::build::stage::utils::{
    class_decorator_start, decorator_name, is_import_stmt, strip_leading_decorators,
    struct_builder_names, struct_method_names, take_with_comments, ComponentFile, ComponentInfo,
    FieldInfo, Protocol, StructBuilderMethod, V1_PROTOCOL, V2_DECORATORS,
};

pub(crate) fn parse_components(
    source: &str,
    project_builders: &[String],
) -> Result<Vec<ComponentInfo>, String> {
    Ok(read_component_file(source, project_builders)?.components)
}

pub(crate) fn read_component_file(
    source: &str,
    project_builders: &[String],
) -> Result<ComponentFile, String> {
    // The full preprocessing chain: mask @Builder/@Extend/@Styles decorators,
    // struct → class, then `Foo(args) { … }` → `Foo(args, () => { … })`. Only
    // with all three does oxc see parseable TypeScript (measured: asr Index.ets
    // fails at the first child block without rewrite_builders).
    let crate::build::stage::process_ui_syntax::Masked {
        source: masked,
        builders: mut builder_names,
        locals,
        mut extends,
        styles,
        animatable_extends,
    } = crate::build::ets::mask::mask_builder_decorators_with_locals(source);
    for name in project_builders {
        if !builder_names.contains(name) {
            builder_names.push(name.clone());
        }
    }
    // A `@Builder` imported from another file is called with a child block like a built-in
    // component, and the mask only sees declarations in this file: without these names
    // `toggleRowList({ …props }) { … }` keeps its block and the parser rejects the file
    // (`entry/components/Settings/Backgrounder.ets`, importing it from `../Common/Common`).
    for name in crate::build::ets::children::imported_names(&masked) {
        if !builder_names.contains(&name) {
            builder_names.push(name);
        }
    }
    // A component declared in this file can be called bare — `Notice.ets` writes
    // `noticeInputRow() { … }` for its own `struct noticeInputRow` — and the mask records only
    // `@Builder function` names, so these have to come from the declaration itself.
    for name in crate::build::ets::children::declared_struct_names(&masked) {
        if !builder_names.contains(&name) {
            builder_names.push(name);
        }
    }
    for name in crate::build::ets_compile::builtin_component_names() {
        if !builder_names.contains(name) {
            builder_names.push(name.clone());
        }
    }
    let struct_builders = struct_builder_names(source);
    let mut locals = locals;
    for name in struct_method_names(source) {
        if !locals.contains(&name) {
            locals.push(name);
        }
    }
    let (prepared, widened) = crate::build::ets::children::rewrite_builders_with_shifts(
        &preprocess(&masked),
        &builder_names,
        &locals,
    );
    // The child-block rewrite inserts the callback the parser needs before the block it wraps,
    // so a node inside a block written on its parent's line sits that many columns too far
    // right. The reference reads the column off the source node, and so does the `{ page, line,
    // col }` object: the widening is published here, once per file, for `position_of` to take
    // back (`clashbox`'s `Xb_Collaborators.ets` writes `ListItem() { IntroduceItem(…) }`).
    crate::build::ets_compile::set_column_shifts(
        &widened
            .iter()
            .map(|(offset, added)| {
                let before = &prepared[..(*offset).min(prepared.len())];
                let line = before.matches('\n').count() + 1;
                let column = offset - before.rfind('\n').map_or(0, |at| at + 1) + 1;
                (line, column, *added)
            })
            .collect::<Vec<_>>(),
    );
    // A helper is emitted where its declaration stood, and the merge that places it compares
    // byte offsets. `mask` measured those on the source, but the parser reads `prepared`, which
    // the child-block rewrite widened before every construction — so a declaration that follows
    // one is that many bytes further right. Left in source coordinates, a helper sorts *ahead*
    // of the statements it follows as soon as the widening before them outweighs the gap
    // between the two: `__Text__smallText` is declared right behind
    // `export default Connect;` in clashbox `More/Connect.ts`, and the struct's child blocks
    // widen the file by thousands of bytes before both, so it jumped in front of the export.
    // The widening is already measured here, so it is what the piece list gets; the source
    // offset stays in `source_order` for the one reader that slices the source (the comment
    // above a declaration).
    for extend in &mut extends {
        extend.order = widen_offset(&widened, extend.source_order);
    }
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &prepared, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        let first = ret
            .errors
            .first()
            .map(|e| format!("{e:?}"))
            .unwrap_or_else(|| "parse panicked".to_string());
        // Attach the text around the first error: an offset alone does not say what the
        // parser actually received after the decorator masking pass.
        let snippet = ret
            .errors
            .first()
            .and_then(|e| e.labels.as_ref())
            .and_then(|labels| labels.first())
            .and_then(|label| label.inner().offset().into())
            .map(|at: usize| {
                let mut start = at.saturating_sub(80);
                let mut end = (at + 80).min(prepared.len());
                while start < prepared.len() && !prepared.is_char_boundary(start) {
                    start += 1;
                }
                while end > start && !prepared.is_char_boundary(end) {
                    end -= 1;
                }
                let line = prepared[..at].matches('\n').count() + 1;
                let raw = prepared.get(start..end).unwrap_or("");
                let shown: String = raw
                    .chars()
                    .map(|c| match c {
                        '\n' => '⏎',
                        ' ' => '·',
                        other => other,
                    })
                    .collect();
                format!("line {line}: {shown}")
            })
            .unwrap_or_default();
        return Err(format!("struct parse failed: {first}\n  near: {snippet}"));
    }

    let mut top_builders = Vec::new();
    for statement in &ret.program.body {
        let (func, is_export) = match statement {
            Statement::FunctionDeclaration(func) => (func, false),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(oxc_ast::ast::Declaration::FunctionDeclaration(func)) => (func, true),
                _ => continue,
            },
            _ => continue,
        };
        let Some(id) = &func.id else {
            continue;
        };
        if !builder_names.iter().any(|n| n == &id.name.to_string()) {
            continue;
        }
        top_builders.push(crate::build::ets_compile::Builder {
            name: id.name.to_string(),
            is_export,
            order: func.span.start as usize,
            // The comment block above the declaration travels with it: the emitted `function`
            // line carries it (`fixtures/declaration-comments`, `BUILDER_DECL_COMMENT`).
            // The comment sits above the `@Builder` decorator, not above `function`, so the
            // scan starts at the decorator when one is written.
            comment: crate::build::ets::comments::leading_comments_before(
                source,
                builder_comment_offset(source, func.span.start as usize),
            ),
            // Each parameter keeps its declared text: the emitted helper has to carry the type
            // annotation, and the reference never renames them (measured: `ConnectBuilder(name:
            // string, param: Object, parent = null)`).
            params: func
                .params
                .items
                .iter()
                .filter(|p| {
                    // `name: T` and `name: T = default` both name a parameter; the second is
                    // an `AssignmentPattern` around the identifier.
                    matches!(
                        &p.pattern.kind,
                        oxc_ast::ast::BindingPatternKind::BindingIdentifier(_)
                            | oxc_ast::ast::BindingPatternKind::AssignmentPattern(_)
                    )
                })
                .map(|p| p.span.source_text(&prepared).to_string())
                .collect(),
            // The body is recorded without its braces: the walker reads statements, and a
            // block would be carried through as one opaque node.
            //
            // The lines before the declaration are replayed as blank ones, so the body keeps
            // its place in the file: `emit_builder` parses it from line 1, and a child
            // construction inside it writes its `{ page, line, col }` from that number
            // (`fixtures/create-new-config`: the reference prints the call's own line in the
            // file, not its line within the builder body).
            body: func
                .body
                .as_ref()
                .map(|b| {
                    let span = b.span();
                    let text = span.source_text(&prepared);
                    let before = prepared
                        .get(..span.start as usize)
                        .map(|prefix| prefix.bytes().filter(|&byte| byte == b'\n').count())
                        .unwrap_or(0);
                    let inner = text
                        .trim()
                        .strip_prefix('{')
                        .and_then(|inner| inner.strip_suffix('}'))
                        .unwrap_or(text.trim());
                    format!("{}{inner}", "\n".repeat(before))
                })
                .unwrap_or_default(),
        });
    }

    let mut components = Vec::new();
    let mut top_level: Vec<String> = Vec::new();
    // The same statements with their offsets: a struct-less file interleaves them with its
    // helpers by position, the way the reference emits them.
    let mut top_level_ordered: Vec<(usize, String)> = Vec::new();
    // A top-level `@Builder function` is emitted by `emit_builders` from its recorded body;
    // carrying the declaration through `top_level` as well would put the same function in the
    // file twice, and the es2abc merge rejects the duplicate.
    let is_declared_builder = |statement: &Statement<'_>| -> bool {
        let function = match statement {
            Statement::FunctionDeclaration(function) => Some(&**function),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(oxc_ast::ast::Declaration::FunctionDeclaration(function)) => Some(&**function),
                _ => None,
            },
            _ => None,
        };
        function
            .and_then(|function| function.id.as_ref())
            .is_some_and(|id| {
                builder_names
                    .iter()
                    .any(|name| name == &id.name.to_string())
            })
    };
    // A leading directive (`"use shared"` in a shared module, `"use strict"`) is not part of
    // `program.body`: oxc keeps the prologue in `program.directives`. The reference re-emits it
    // in place — after the `_Params` interfaces and the imports, before the class — which is the
    // position `top_level` statements take below (measured: `fixtures/use-shared`, 2026-11-06).
    for directive in &ret.program.directives {
        let text = directive.span.source_text(&prepared).to_string();
        top_level.push(text.clone());
        // A directive is a prologue: it precedes every statement in the body.
        top_level_ordered.push((0, text));
    }
    let mut imports: Vec<String> = Vec::new();
    for statement in &ret.program.body {
        // `export struct X` becomes `export class X`, which oxc reports as an
        // ExportNamedDeclaration wrapping the class.
        let class = match statement {
            // An import is one node whatever its line breaks; the reference rewrites the
            // clause it was handed, so the node's own text is what gets emitted.
            Statement::ImportDeclaration(decl) => {
                imports.push(decl.span.source_text(&prepared).to_string());
                continue;
            }
            Statement::ClassDeclaration(class) => class,
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(oxc_ast::ast::Declaration::ClassDeclaration(class)) => class,
                _ => {
                    if is_declared_builder(statement) {
                        continue;
                    }
                    let span = statement.span();
                    // A re-export (`export { X } from '…'`) is printed bare: the reference drops
                    // a comment written above it, while a `function` / `const` / `class`
                    // declaration keeps its own (`fixtures/barrel-comment`: `RE_EXPORT_COMMENT`
                    // has no counterpart in the golden, the other three do).
                    let text = if export.source.is_some() {
                        span.source_text(&prepared).to_string()
                    } else {
                        take_with_comments(&prepared, span.start as usize, span.end as usize)
                    };
                    let text = without_masked_extend_comment(&text);
                    if !is_import_stmt(statement) {
                        top_level.push(text.clone());
                        top_level_ordered.push((span.start as usize, text));
                    }
                    continue;
                }
            },
            _ => {
                if is_declared_builder(statement) {
                    continue;
                }
                // top-level non-class statements (consts, interfaces) kept verbatim
                let span = statement.span();
                let text = take_with_comments(&prepared, span.start as usize, span.end as usize);
                let text = without_masked_extend_comment(&text);
                // The masked remains of an `@Extend` declaration carry only its comment; the
                // comment is printed above the generated helper instead (see
                // `with_extend_comment`).
                let masked_extend = is_extend_leftover(&text);
                if !is_import_stmt(statement) && !masked_extend {
                    top_level.push(text.clone());
                    top_level_ordered.push((span.start as usize, text));
                }
                continue;
            }
        };
        // Read the decorators from the token stream rather than from the AST node: ArkTS
        // allows them ahead of `export` (`@Entry @Component export struct X`), where oxc does
        // not attach them to the class and `@Entry` would be lost.
        let class_name = class
            .id
            .as_ref()
            .map(|i| i.name.to_string())
            .unwrap_or_default();
        let declaration_offset = class_decorator_start(class, statement.span().start as usize);
        let declaration = class
            .id
            .as_ref()
            .map(|id| {
                crate::build::compile::ets_source_map::line_column(
                    &prepared,
                    id.span.start as usize,
                )
            })
            .unwrap_or_else(|| {
                crate::build::compile::ets_source_map::line_column(
                    &prepared,
                    statement.span().start as usize,
                )
            });
        let (lex_decorators, lex_exported) =
            crate::build::lex::declaration_attributes(&prepared, &class_name);
        let decorators: Vec<String> = if lex_decorators.is_empty() {
            class
                .decorators
                .iter()
                .filter_map(|d| decorator_name(&d.expression))
                .collect()
        } else {
            lex_decorators
        };
        // `@CustomDialog` structs are components too: the reference compiles them through
        // the same view class, and skipping them leaves every dialog name unexported.
        if !decorators
            .iter()
            .any(|d| d == "Component" || d == "ComponentV2" || d == "CustomDialog")
        {
            // A plain class or enum declared among the components is part of the file, not a
            // component: dropping it would leave every importer of the name unlinked.
            let span = statement.span();
            let start = class_decorator_start(class, span.start as usize);
            let text = take_with_comments(&prepared, start, span.end as usize);
            if !is_import_stmt(statement) {
                top_level.push(text.clone());
                top_level_ordered.push((start, text));
            }
            continue;
        }
        // `@ComponentV2` is handled by the V2 protocol (base class `ViewV2`), chosen by the
        // caller; the parser only has to recognise the struct here.
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut method_offsets: Vec<usize> = Vec::new();
        let mut computed: Vec<String> = Vec::new();
        let mut has_monitor = false;
        let mut struct_builder_methods: Vec<StructBuilderMethod> = Vec::new();
        let mut build_body: Option<String> = None;
        let mut build_comment: Vec<String> = Vec::new();
        let mut build_body_line: usize = 0;
        // A struct without `build()` emits `initialRender` after its members, as before.
        let mut build_offset: usize = usize::MAX;
        let mut build_position: Option<SourcePosition> = None;
        for element in &class.body.body {
            let ClassElement::MethodDefinition(m) = element else {
                continue;
            };
            let PropertyKey::StaticIdentifier(key) = &m.key else {
                continue;
            };
            if key.name == "build" {
                build_offset = key.span().start as usize;
                build_position = Some(crate::build::compile::ets_source_map::line_column(
                    &prepared,
                    key.span().start as usize,
                ));
                build_comment = crate::build::ets::comments::leading_comments_before(
                    &prepared,
                    m.span().start as usize,
                );
                if let Some(fbody) = &m.value.body {
                    let text = fbody.span.source_text(&prepared).to_string();
                    let inner = text
                        .trim()
                        .strip_prefix('{')
                        .and_then(|t| t.strip_suffix('}'))
                        .unwrap_or(text.trim());
                    // Line of `inner` inside the file: the span start plus the newlines the
                    // `{` and any leading whitespace account for.
                    let span_start = fbody.span.start as usize;
                    let offset = text.find(inner).unwrap_or(0);
                    let newlines_before = prepared[span_start..span_start + offset]
                        .matches('\n')
                        .count();
                    build_body_line =
                        prepared[..span_start].matches('\n').count() + 1 + newlines_before;
                    build_body = Some(inner.to_string());
                }
                continue;
            }
            let span = m.span();
            let text = take_with_comments(&prepared, span.start as usize, span.end as usize);
            // A struct-level `@Builder` method carries component child blocks; its body has
            // to go through the same emission `build()` gets, and the decorator cannot reach
            // the output (it compiles to a call of an undefined `Builder`).
            let is_builder = m
                .decorators
                .iter()
                .filter_map(|d| decorator_name(&d.expression))
                // `@LocalBuilder` marks the same kind of body; only its call sites differ
                // (they stay ordinary `this.name()` calls).
                .any(|n| n == "Builder" || n == "LocalBuilder");
            if is_builder {
                // The signature starts after the method's decorators and stops at its body,
                // both from the parser's own spans: splitting on a brace breaks as soon as a
                // parameter carries an object type, and the decorator spans are already known.
                let signature = m
                    .value
                    .body
                    .as_ref()
                    .map(|b| {
                        let raw = strip_leading_decorators(&prepared, m, b.span.start as usize);
                        raw.trim().trim_end_matches('{').trim().to_string()
                    })
                    .unwrap_or_default();
                let body = m
                    .value
                    .body
                    .as_ref()
                    .map(|b| {
                        let t = b.span.source_text(&prepared).to_string();
                        t.trim()
                            .strip_prefix('{')
                            .and_then(|x| x.strip_suffix('}'))
                            .unwrap_or(t.trim())
                            .to_string()
                    })
                    .unwrap_or_default();
                struct_builder_methods.push(StructBuilderMethod {
                    signature,
                    body,
                    // The line this method's own body starts on: a child construction reports
                    // its source coordinate from it, and `build()`'s line is a different one.
                    body_line: m.value.body.as_ref().map_or(0, |b| {
                        crate::build::stage::utils::body_start_line(
                            &prepared,
                            b.span.start as usize,
                        )
                    }),
                    offset: span.start as usize,
                    // The span starts at the method's decorators, so the block written above
                    // `@Builder` counts as the method's own (`fixtures/declaration-comments`).
                    comment: crate::build::ets::comments::leading_comments_before(
                        &prepared,
                        span.start as usize,
                    ),
                });
                continue;
            }
            // `@Computed` getters and `@Monitor` methods are V2 members: the class keeps
            // them verbatim, and `resetStateVarsOnReuse` resets them by name
            // (process_struct_componentV2.js `traverseStructInfo`, `createResetStateVarsOnReuse`).
            for name in m
                .decorators
                .iter()
                .filter_map(|d| decorator_name(&d.expression))
            {
                match name.as_str() {
                    "Computed" => computed.push(key.name.to_string()),
                    "Monitor" | "SyncMonitor" => has_monitor = true,
                    _ => {}
                }
            }
            method_offsets.push(span.start as usize);
            methods.push(text);
        }
        for element in &class.body.body {
            let ClassElement::PropertyDefinition(prop) = element else {
                continue;
            };
            let PropertyKey::StaticIdentifier(key) = &prop.key else {
                continue;
            };
            // Decorators are kept verbatim: the V2 class prints them back and the V1 field
            // classification reads them by name.
            let decorators: Vec<String> = prop
                .decorators
                .iter()
                .filter_map(|d| decorator_name(&d.expression))
                .collect();
            let decorator_texts: Vec<String> = prop
                .decorators
                .iter()
                .filter(|d| decorator_name(&d.expression).as_deref() != Some("Require"))
                .map(|d| d.span().source_text(&prepared).trim().replace('\n', " "))
                .collect();
            // `@Provide('alias')` registers the alias, not the field name; without it a
            // `@Consume` in a descendant cannot find the value. `@StorageLink('key')` /
            // `@StorageProp('key')` carry the AppStorage key, and the `@LocalStorage*` pair
            // carries the LocalStorage key. The argument is kept as written: a string
            // literal stays a literal and an identifier stays an identifier.
            let key_arg = prop
                .decorators
                .iter()
                .find(|d| {
                    matches!(
                        decorator_name(&d.expression).as_deref(),
                        Some("Provide")
                            | Some("Consume")
                            | Some("StorageLink")
                            | Some("StorageProp")
                            | Some("LocalStorageLink")
                            | Some("LocalStorageProp")
                    )
                })
                .and_then(|d| match &d.expression {
                    oxc_ast::ast::Expression::CallExpression(call) => call.arguments.first(),
                    _ => None,
                });
            let (key_expr, key_value) = match key_arg {
                Some(oxc_ast::ast::Argument::StringLiteral(s)) => (
                    Some(s.span().source_text(&prepared).trim().to_string()),
                    Some(s.value.to_string()),
                ),
                Some(arg) => (
                    Some(arg.span().source_text(&prepared).trim().to_string()),
                    None,
                ),
                None => (None, None),
            };
            // `@Watch('method')` registers the callback on the field. A member can carry more
            // than one: the reference calls `processWatch` once per decorator and each call
            // writes the same prop→watcher slot, so the *last* decorator is the watcher that
            // survives (measured: clashbox `entry/pages/Index.ts`,
            // `@StorageLink('currentHeightBreakpoint') @Watch('onWindowChange')
            // @Watch('isSecondaryChange') heightBp` emits
            // `this.declareWatch("heightBp", this.isSecondaryChange);`).
            let watch_method = prop
                .decorators
                .iter()
                .rev()
                .find(|d| matches!(decorator_name(&d.expression).as_deref(), Some("Watch")))
                .and_then(|d| match &d.expression {
                    oxc_ast::ast::Expression::CallExpression(call) => {
                        call.arguments.first().and_then(|arg| match arg {
                            oxc_ast::ast::Argument::StringLiteral(s) => Some(s.value.to_string()),
                            _ => None,
                        })
                    }
                    _ => None,
                });
            // The comment above the member, including one written above its decorator and one
            // written between the decorator and the name (`@Consume('x')` / `// page stack` /
            // `private pageInfos: NavPathStack`): the reference prints the second one too, in
            // the class body beside the other members (fixtures/create-new-config, `// 页面栈`).
            let comment_at = prop
                .decorators
                .first()
                .map(|d| d.span().start as usize)
                .unwrap_or(prop.span().start as usize);
            let mut comment =
                crate::build::ets::comments::leading_comments_before(&prepared, comment_at);
            if let Some(last) = prop.decorators.last() {
                let start = (last.span().end as usize).min(prepared.len());
                let end = (key.span().start as usize).min(prepared.len());
                if start <= end {
                    let between = &prepared[start..end];
                    for token in crate::build::lex::tokenize(between) {
                        if token.kind == crate::build::lex::Kind::Comment {
                            comment.push(between[token.start..token.end].to_string());
                        }
                    }
                }
            }
            // A comment written after the declaration on its own line stays there: the
            // reference emits `private __x; // X`, so the field carries it separately from the
            // leading block.
            let trailing_comment = {
                let start = (prop.span().end as usize).min(prepared.len());
                let end = prepared[start..]
                    .find('\n')
                    .map_or(prepared.len(), |offset| start + offset);
                let mut found = Vec::new();
                for token in crate::build::lex::tokenize(&prepared[start..end]) {
                    if token.kind == crate::build::lex::Kind::Comment {
                        found.push(prepared[start + token.start..start + token.end].to_string());
                    }
                }
                found
            };
            let comment_column = {
                let at = comment_at.min(prepared.len());
                let line = prepared[..at]
                    .rfind('\n')
                    .map_or(0, |previous| previous + 1);
                at - line
            };
            fields.push(FieldInfo {
                comment_indent: comment_column,
                name: key.name.to_string(),
                position: crate::build::compile::ets_source_map::line_column(
                    &prepared,
                    key.span().start as usize,
                ),
                offset: key.span().start as usize,
                is_readonly: prop.readonly,
                is_static: prop.r#static,
                comment,
                trailing_comment,
                accessibility: prop.accessibility.map(|a| {
                    match a {
                        oxc_ast::ast::TSAccessibility::Private => "private",
                        oxc_ast::ast::TSAccessibility::Protected => "protected",
                        oxc_ast::ast::TSAccessibility::Public => "public",
                    }
                    .to_string()
                }),
                watch_method,
                decorators,
                decorator_texts,
                key_expr,
                key_value,
                type_text: prop
                    .type_annotation
                    .as_ref()
                    .map(|t| t.span().source_text(&prepared).trim().to_string())
                    // oxc's TSTypeAnnotation span starts at the ':' itself.
                    .map(|t| {
                        t.strip_prefix(':')
                            .map(|t| t.trim().to_string())
                            .unwrap_or(t)
                    }),
                initial: prop
                    .value
                    .as_ref()
                    .map(|v| v.span().source_text(&prepared).to_string()),
                is_optional: prop.optional,
            });
        }
        let component_name = class
            .id
            .as_ref()
            .map(|i| i.name.to_string())
            .unwrap_or_default();
        // A member `@Styles` answers only its own struct's calls; a top-level
        // `@Styles function` has no owner and is visible to every component in the file
        // (measured: `fixtures/reusable-guard-boundary`, `StylesScopeProbe.cardStyle` 11/12
        // versus `StyleProbe.cardStyle` 40/20/5, and `GlobalOnlyProbe` expanding the global
        // body 98/99).
        // A member shadows a top-level declaration of the same name
        // (measured: `GlobalStyleShadowProbe` expands its own 21/22, not the global 98/99),
        // so the struct's own styles come first and the global ones follow as the fallback
        // the lookup's `find` never reaches when a member answers.
        let mut visible_styles: Vec<crate::build::ets::StyleFn> = styles
            .iter()
            .filter(|decl| decl.comp == component_name)
            .cloned()
            .collect();
        visible_styles.extend(styles.iter().filter(|decl| decl.comp.is_empty()).cloned());
        components.push(ComponentInfo {
            name: component_name,
            declaration,
            declaration_offset,
            build_position,
            is_entry: decorators.iter().any(|d| d == "Entry"),
            is_dialog: decorators.iter().any(|d| d == "CustomDialog"),
            // The protocol is chosen per struct, not per file: a file may declare an
            // `@Component` helper next to a `@ComponentV2` page.
            is_v2: decorators
                .iter()
                .any(|d| V2_DECORATORS.contains(&d.as_str())),
            is_reusable: decorators.iter().any(|d| d == "Reusable"),
            is_exported: lex_exported || matches!(statement, Statement::ExportNamedDeclaration(_)),
            struct_builders: {
                // A `@BuilderParam` field is called the same way a struct-level `@Builder`
                // method is: through `bind(this)`, because the closure the parent passed in
                // carries no receiver.
                let mut names = struct_builders.clone();
                for field in &fields {
                    if field.has("BuilderParam") && !names.contains(&field.name) {
                        names.push(field.name.clone());
                    }
                }
                names
            },
            fields,
            computed,
            has_monitor,
            top_level: top_level.clone(),
            methods,
            method_offsets,
            build_offset,
            build_body,
            build_comment,
            build_body_line,
            builders: top_builders.clone(),
            struct_builder_methods,
            extends: extends.clone(),
            styles: visible_styles,
        });
    }
    // Statements written after the final struct (typically `export { A, B }`) were never
    // part of any component's snapshot; they belong to the file, so the last component
    // carries them.
    // Builder functions are emitted from the builder table; keeping their source text in
    // `top_level` too declares every one of them twice.
    top_level.retain(|stmt| {
        function_name_of(stmt).is_none_or(|name| !builder_names.iter().any(|n| n == &name))
    });
    if let Some(last) = components.last_mut() {
        last.top_level = top_level.clone();
    }
    let value_used = crate::build::stage::process_import::value_used_names(&ret.program);
    Ok(ComponentFile {
        value_used,
        imports,
        components,
        top_level_ordered,
        builders: top_builders,
        extends,
        // The attribute list is reached without the file, so the table is published for it
        // here, at the point the file's declarations are known.
        animatable_extends: {
            crate::build::stage::process_ui_syntax::set_animatable_extends(&animatable_extends);
            animatable_extends
        },
    })
}

/// Expand a component file to the .ts text es2abc compiles.
/// Which component protocol an expansion targets.
///
/// The two generations share the file-level scaffolding (`initialRender`, `rerender`,
/// `paramsLambda`, the route footer) but not the class body: V1 builds `__` backstores with
/// get/set accessors and lifecycle stubs, V2 keeps the decorated members and lets `ViewV2`
/// observe them (process_component_class.js vs process_struct_componentV2.js; measured:
/// state-field and component-v2 goldens, 2026-11-05). The flag picks the body emitter.
pub fn expand_component_file(
    source: &str,
    opts: &crate::build::lower::ets_transpile::TranspileOptions,
) -> Result<String, String> {
    expand_component_file_with(source, opts, V1_PROTOCOL)
}

pub fn expand_component_file_with(
    source: &str,
    opts: &crate::build::lower::ets_transpile::TranspileOptions,
    protocol: Protocol,
) -> Result<String, String> {
    // One read of the file: the masking pass blanks the `@Styles`/`@Extend` bodies (attribute
    // chains no parser accepts) at identical offsets, `struct` becomes `class`, child blocks are
    // rewritten, and the imports and components below all come from that single parse.
    let file = read_component_file(source, opts.project_builders)?;
    // A file with no struct but with helpers still has to be expanded: the reference expands
    // every top-level `@Builder function` wherever it stands, and a `PipManager.ets` has no
    // view class to hang them on. When there is nothing to expand the caller keeps its plain
    // path, which is the only other route.
    if file.components.is_empty() {
        if file.builders.is_empty() && file.extends.is_empty() {
            return Err("no @Component found".to_string());
        }
        let kits = crate::build::stage::process_import::load_kit_configs(opts.kit_config_dirs)
            .unwrap_or_default();
        return expand_helper_file(&file, source, &kits, page_coordinate(opts).as_deref());
    }
    let components = &file.components;
    let comp = &components[0];

    if let Some(route) = &opts.route {
        // The coordinate names the file the way the reference does: module root, source root,
        // then the page path with its extension.
        crate::build::ets_compile::set_source_page(&format!(
            "{}/src/main/ets/{}.ets",
            route.module, route.page
        ));
        crate::build::ets_compile::set_compile_har(route.compile_har);
    }
    // The file is a list of statements in the order the reference emits them. Nothing here
    // decides how a statement is laid out: `ets_output::print_statements` prints every one of
    // them, on the same printer the plain path uses.
    let mut items: Vec<String> = Vec::new();
    // Header polyfill. Always `ViewPU`: the reference's `checkFinalizeConstruction` names that
    // class even for a file whose structs all lower to `ViewV2`.
    items.push(
        "if (!(\"finalizeConstruction\" in ViewPU.prototype)) {\n    Reflect.set(ViewPU.prototype, \"finalizeConstruction\", () => { });\n}"
            .to_string(),
    );
    // The `@LocalBuilder` facts of the whole file, read once: `validate_ui_syntax.js`
    // `setLocalBuilderInFile` sets `hasLocalBuilderInFile` from the file's decorators, and
    // `process_ui_syntax.js` `processUISyntax` then unshifts `checkContextStack()` ahead of the
    // `_Params` interfaces, i.e. right after `checkFinalizeConstruction()`.
    let mut local = file_facts(source);
    // `stateObjectCollection` (`process_component_member.js`) is a Set of member names filled as
    // structs are processed and never cleared, so a name that is object-typed in one struct is
    // read through `ObservedObject.GetRawObject` wherever it appears afterwards — even in a
    // struct that declares it `string` (measured: clashbox `entry/components/Common/Common.ets`,
    // where `Nothing`'s `@State name: Resource` is what makes `skipRowList`'s
    // `@State name: string` read wrap). The reference's set spans the whole compilation; hvolt
    // lowers one file at a time, so the approximation is this file's structs in declaration
    // order — a name registered by a later struct is not seen, and a name registered only in
    // another file is not seen either (known gap).
    {
        let mut accumulated: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut ordered: Vec<&ComponentInfo> = components.iter().collect();
        ordered.sort_by_key(|component| component.declaration_offset);
        for component in ordered {
            accumulated.extend(
                crate::build::stage::process_component_build::own_object_members(component, &local),
            );
            local
                .object_members
                .insert(component.declaration_offset, accumulated.clone());
        }
    }
    if local.present {
        items.push(context_stack_header().to_string());
    }
    // `hasReusableV2`: `process_ui_syntax.js` unshifts the reflect helper the file's
    // `@ReusableV2` class is decorated with, once per file, between `checkContextStack()` and
    // the `_Params` interfaces (`l.hasReusableV2 && a.unshift(createReusableV2ReflectFunction())`).
    if local.has_reusable_v2 {
        items.push(reusable_v2_reflect_function().to_string());
    }
    // Every `_Params` interface sits ahead of every class, and the reference lists them in
    // reverse declaration order (the last struct's interface comes first). V2 structs have
    // none: their constructor reads the raw `params` object.
    for extra in components
        .iter()
        .rev()
        .filter(|c| !protocol_for(c, protocol).v2)
    {
        let mut params = String::new();
        emit_params(&mut params, extra);
        items.push(params);
    }
    // Imports, each emitted statement kept whole: a statement spread over several lines is
    // emitted as written, so a comment inside its binding list survives.
    let kits: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, crate::build::stage::process_import::KitEntry>,
    > = crate::build::stage::process_import::load_kit_configs(opts.kit_config_dirs)
        .unwrap_or_default();
    // One emitted statement per import node. A clause spread over several lines, or carrying a
    // comment, comes out whole because the text is the node's own span.
    for import in &file.imports {
        items.extend(crate::build::stage::process_import::emit_import(
            import,
            &file.value_used,
            &kits,
        ));
    }
    // Every piece is emitted where its declaration stood: the reference keeps source order
    // across plain statements, `@Builder`/`@Extend` helpers, `export { … }` / `export default`
    // and the classes alike (`fixtures/barrel-comment`: an `export { X } from '…'` written
    // first comes out first; `fixtures/import-forms`: `export { Area };`,
    // `export { Area as AreaAlias } from …` and `export default class Widget` come out in the
    // order they were written, after the struct). Nothing is deferral: a statement written
    // after the code that uses it stays there, which is what the reference does with
    // `export default class Widget` and its earlier `Widget.tag` read.
    // across plain statements, `@Builder`/`@Extend` helpers and the classes alike
    // (measured: `Xb_SettingItem.ts`, whose class order is `ColorPickerV1`, `Xb_SettingsItem`,
    // `makeSettingItems`, `RightAction`, `__Text__menuText`, … — exactly the `.ets` order).
    // The `@Styles function`s no struct owns: a top-level `@Builder function` has no owning
    // struct either, so the file-level declarations are the only style table its body can call
    // (`GLOBAL_STYLE_FUNCTION` is filled per file in `process_component_build.js`). The list is
    // read off a component because that is where `read_component_file` hangs the file's tables;
    // every component carries the same file-global entries.
    let global_styles: Vec<crate::build::ets::StyleFn> = components
        .first()
        .map(|first| {
            first
                .styles
                .iter()
                .filter(|style| style.comp.is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let pieces = ordered_pieces(&file, components);
    for (_order, piece) in pieces {
        match piece {
            Piece::Statement(text) => {
                // The blanked remains of an `@Extend` declaration: its comment is printed above
                // the generated helper, so emitting the leftover would duplicate it.
                if is_extend_leftover(text) {
                    continue;
                }
                // Every top-level statement keeps its written position here as well — the
                // helper-only route merges statements, `@Builder` and `@Extend` by source
                // offset and nothing is moved afterwards.
                items.push(text.to_string());
            }
            Piece::Builder(builder) => items.push(
                crate::build::stage::process_ui_syntax::emit_builder(
                    builder,
                    &file.builders,
                    &file.extends,
                    &global_styles,
                )
                .map_err(|e| format!("{}: builder 展开失败: {e}", comp.name))?,
            ),
            Piece::Extend(extend) => {
                // The comment written above the `@Extend` declaration goes above the helper the
                // reference generates for it (measured: `fixtures/comment-leading`, marker
                // `EXTEND_TOP`, whose `// EXTEND_TOP` sits directly above
                // `function __Text__loudExtend`; `fixtures/proxy-node-item` keeps a single
                // `//Item中type（节点类型）公用样式` there).
                items.push(with_extend_comment(source, extend))
            }
            Piece::Component(component) => {
                let mut text = String::new();
                emit_class(
                    &mut text,
                    component,
                    protocol_for(component, protocol),
                    &local,
                )?;
                items.push(text);
            }
        }
    }
    // @Entry pages get the reference's route-registration footer (needs project
    // metadata, which the CLI supplies through `opts.route`).
    // Every entry page in the file gets its route: a file may declare several components and
    // the entry is not necessarily the first one, so the check belongs to each component.
    if let Some(route) = &opts.route {
        // `moduleName` is `projectConfig.moduleName` — the module this CompileArkTS task
        // assembles — not the module that owns the source file: a sibling module's `@Entry`
        // page registers under the HAP's module (measured on clashbox: `xb_components` pages
        // register under `entry`). A single-file CLI lowering publishes no assembled module
        // and keeps the page's own.
        let module_name = match crate::build::compile::abc_compile::assemble_module() {
            Some(module) => module,
            None => route.module,
        };
        for component in components {
            if component.is_entry {
                items.push(named_route_footer(
                    &component.name,
                    route.bundle,
                    module_name,
                    route.module,
                    route.page,
                ));
            }
        }
    }
    // `createNavigationInit` (`process_ui_syntax.js`): a file the project's `routerMap` names
    // registers each of its global `@Builder` functions under the map's own `name`, in one
    // IIFE after every declaration the file emits.
    if let Some(page) = page_coordinate(opts) {
        let declared: Vec<&str> = file.builders.iter().map(|b| b.name.as_str()).collect();
        if let Some(init) =
            crate::build::stage::process_ui_syntax::navigation_init(&page, &declared)
        {
            items.push(init);
        }
    }
    Ok(crate::build::lower::ets_output::print_statements(&items))
}

/// The `registerNamedRoute` footer an `@Entry` page carries.
///
/// `module_name` is the assembled module (`assemble_module`), `page_module` the one that owns
/// the source file. The reference splits the two the same way: `moduleName` comes from
/// `projectConfig.moduleName` while `pagePath`/`pageFullPath` name the file's own coordinate
/// (`process_ui_syntax.js::createRegisterNamedRoute` + `process_module_package.js::
/// routerOrNavPathWrite`, measured on clashbox where `xb_components`' pages register under
/// `entry`).
fn named_route_footer(
    component: &str,
    bundle: &str,
    module_name: &str,
    page_module: &str,
    page: &str,
) -> String {
    format!(
        "registerNamedRoute(() => new {}(undefined, {{}}), \"\", {{ bundleName: \"{}\", moduleName: \"{}\", pagePath: \"{}\", pageFullPath: \"{}/src/main/ets/{}\", integratedHsp: \"false\", moduleType: \"followWithHap\" }});",
        component, bundle, module_name, page, page_module, page
    )
}

/// `<module>/src/main/ets/<page>.ets` — the coordinate `source_page()` carries, read from the
/// route this file is being lowered under. A file lowered without a route has none, and an
/// unnamed coordinate must not be borrowed from whichever file ran last on this thread.
fn page_coordinate(opts: &crate::build::lower::ets_transpile::TranspileOptions) -> Option<String> {
    opts.route
        .as_ref()
        .map(|route| format!("{}/src/main/ets/{}.ets", route.module, route.page))
}

/// The same expansion for the plain route, which is where a file with no struct lands.
///
/// `None` when the file declares no helper to expand — the caller then keeps its own printing.
/// A source this reader cannot parse also returns `None` rather than an error: the plain route
/// has its own parser, and a file it accepts must not be failed here.
pub(crate) fn expand_helpers_only(
    source: &str,
    kits: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, crate::build::stage::process_import::KitEntry>,
    >,
) -> Result<Option<String>, String> {
    let Ok(file) = read_component_file(source, &[]) else {
        return Ok(None);
    };
    if !file.components.is_empty() || (file.builders.is_empty() && file.extends.is_empty()) {
        return Ok(None);
    }
    expand_helper_file(&file, source, kits, None).map(Some)
}

/// The generated `@Extend` helper, with the comment block the declaration carried above it.
fn with_extend_comment(source: &str, extend: &crate::build::ets::ExtendFn) -> String {
    let mut text = String::new();
    // The source offset, not `order`: the latter names the widened text the parser reads, and
    // this scan reads the file as written (`ExtendFn::source_order`).
    for comment in crate::build::ets::comments::leading_comments_before(source, extend.source_order)
    {
        text.push_str(&comment);
        text.push('\n');
    }
    text.push_str(&crate::build::stage::process_ui_syntax::emit_extend(extend));
    text
}

/// Whether a line is the blanked remains of a masked declaration: nothing but spaces, and long
/// enough that it cannot be ordinary indentation.
fn is_blanked_declaration_line(line: &str) -> bool {
    // A blanked decorator alone (`@Concurrent` → eleven spaces) is not a declaration; only a
    // body's worth of blanking is, so the threshold keeps a decorated function's own comment in
    // place (measured: `fixtures/concurrent-function`).
    line.len() >= 20 && line.trim().is_empty()
}

/// Whether a top-level statement is the blanked remains of an `@Extend` declaration.
///
/// The masking pass blanks the declaration at identical offsets but leaves the comment above it,
/// and the leftover statement carries no code — only that comment and the blanked region.
fn is_extend_leftover(text: &str) -> bool {
    let mut saw_comment = false;
    let mut saw_blanked = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            saw_blanked |= is_blanked_declaration_line(line);
            continue;
        }
        let is_comment = trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.ends_with("*/");
        if !is_comment {
            return false;
        }
        saw_comment = true;
    }
    saw_comment && saw_blanked
}

/// A statement's text with the comment block of a masked `@Extend` removed.
///
/// The masking pass blanks the declaration but leaves the comment above it, and
/// `take_with_comments` walks back over that blanked region and attaches the comment to the next
/// declaration. The comment belongs to the helper and is printed above it, so it must not be
/// emitted here as well (measured: `fixtures/proxy-node-item`, where
/// `//Item中type（节点类型）公用样式` otherwise comes out twice).
fn without_masked_extend_comment(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut index = 0usize;
    let mut saw_comment = false;
    let mut saw_blanked = false;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            saw_blanked |= is_blanked_declaration_line(lines[index]);
            index += 1;
            continue;
        }
        let is_comment = trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.ends_with("*/");
        if !is_comment {
            break;
        }
        saw_comment = true;
        index += 1;
    }
    if !(saw_comment && saw_blanked) {
        return text.to_string();
    }
    lines[index..].join("\n")
}

/// One emitted item: a plain statement, a helper, or a class.
enum Piece<'a> {
    Statement(&'a str),
    Builder(&'a crate::build::ets::Builder),
    Extend(&'a crate::build::ets::ExtendFn),
    Component(&'a ComponentInfo),
}

/// Every piece of the file with the source offset it starts at, in declaration order.
///
/// The reference emits each declaration where it stood, so a plain statement, an `@Extend`
/// helper and a class interleave in source order (`Xb_SettingItem.ts`: `ColorPickerV1`,
/// `Xb_SettingsItem`, `makeSettingItems`, `RightAction`, `__Text__menuText`, …).
/// Where a `@Builder function` declaration starts for the purpose of its leading comment.
///
/// The decorator sits between the comment and the `function` keyword, so the scan starts at the
/// decorator; `export` may stand between the two (`// BUILDER_DECL_COMMENT` / `@Builder` /
/// `export function …`, measured: `fixtures/declaration-comments`).
fn builder_comment_offset(source: &str, function_start: usize) -> usize {
    let prefix = &source[..function_start.min(source.len())];
    if let Some(found) = prefix.rfind("@Builder") {
        let between = prefix[found + "@Builder".len()..].trim();
        if between.is_empty() || between == "export" {
            return found;
        }
    }
    prefix.trim_end().len()
}

/// Where a source offset lands in the text the child-block rewrite produced.
///
/// Each entry is `(offset in the rewritten text, bytes added)`, in file order. The second field
/// is a net addition — an entry replaces as many bytes as it dislodges — so an offset past an
/// entry moves by the running total of every entry before it, and an offset before all of them
/// stays put. `at` is compared in the *source* coordinates the caller measured it in, which is
/// why each entry's own source offset is recovered by taking the earlier additions back out —
/// and why the walk stops at the first entry that starts past it (recovering a position cannot
/// reorder the file).
///
/// A helper's declaration is what this is for: `mask` measures it on the source, the piece list
/// merges it with offsets read off the rewritten text, and the two only line up once the
/// widening between them is accounted for (`ExtendFn::order` / `ExtendFn::source_order`).
fn widen_offset(widened: &[(usize, usize)], at: usize) -> usize {
    let mut added = 0usize;
    for (output_at, bytes) in widened {
        let inserted_at = output_at.saturating_sub(added);
        if inserted_at > at {
            break;
        }
        added += bytes;
    }
    at + added
}

fn ordered_pieces<'a>(
    file: &'a ComponentFile,
    components: &'a [ComponentInfo],
) -> Vec<(usize, Piece<'a>)> {
    let mut pieces: Vec<(usize, Piece<'a>)> = file
        .top_level_ordered
        .iter()
        .map(|(order, text)| (*order, Piece::Statement(text.as_str())))
        .chain(
            file.builders
                .iter()
                .map(|builder| (builder.order, Piece::Builder(builder))),
        )
        .chain(
            file.extends
                .iter()
                .map(|extend| (extend.order, Piece::Extend(extend))),
        )
        .chain(
            components
                .iter()
                .map(|component| (component.declaration_offset, Piece::Component(component))),
        )
        .collect();
    pieces.sort_by_key(|(order, _)| *order);
    pieces
}

/// A file that declares no struct but declares `@Builder` / `@Extend` helpers.
///
/// The reference emits each helper where its declaration stood, so the statements and the two
/// tables are merged by source offset — the same order `emit_helpers` gives a component file,
/// only here there is no class to sit between them.
fn expand_helper_file(
    file: &ComponentFile,
    source: &str,
    kits: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, crate::build::stage::process_import::KitEntry>,
    >,
    page: Option<&str>,
) -> Result<String, String> {
    let mut items: Vec<String> = Vec::new();
    for import in &file.imports {
        items.extend(crate::build::stage::process_import::emit_import(
            import,
            &file.value_used,
            kits,
        ));
    }
    // `@Styles` has no emitted function: every call site is inlined, so the table only marks a
    // declaration that must not be copied through.
    let pieces = ordered_pieces(file, &[]);
    for (_order, piece) in pieces {
        match piece {
            Piece::Statement(text) => {
                // The blanked remains of an `@Extend` declaration: its comment is printed above
                // the generated helper, so emitting the leftover would duplicate it.
                if is_extend_leftover(text) {
                    continue;
                }
                // `export default Home;` is an ExportAssignment and stays where the source
                // wrote it, between the classes it follows (measured: clashbox
                // `entry/src/main/ets/pages/HomePage.ts:1450`, between `class Home` and
                // `class HomeCard`; `fixtures/proxy-group-item` golden). `export { … }` and
                // `export default class Foo {}` still move to the end: the reference keeps
                // those two in source order behind every declaration (measured:
                // `fixtures/import-forms`, whose golden is `export { Area };` /
                // `export { Area as AreaAlias } from …` / `export default class Widget {}`).
                items.push(text.to_string());
            }
            Piece::Builder(builder) => items.push(
                // A file that declares no struct still sees its own `@Builder` table and its
                // `@Extend` helpers. Its `@Styles function`s are out of reach on this path: the
                // file's styles are collected per component (`ComponentFile` carries none), and
                // there is no component to read them from. Known gap, not an oversight — a
                // `.styleFn(…)` call inside a helper-only file's builder body stays a plain
                // attribute write.
                crate::build::stage::process_ui_syntax::emit_builder(
                    builder,
                    &file.builders,
                    &file.extends,
                    &[],
                )?,
            ),
            Piece::Extend(extend) => {
                // The comment written above the `@Extend` declaration goes above the helper the
                // reference generates for it (measured: `fixtures/comment-leading`, marker
                // `EXTEND_TOP`, whose `// EXTEND_TOP` sits directly above
                // `function __Text__loudExtend`; `fixtures/proxy-node-item` keeps a single
                // `//Item中type（节点类型）公用样式` there).
                items.push(with_extend_comment(source, extend))
            }
            // This path is only taken when the file declares no struct at all.
            Piece::Component(_) => {}
        }
    }
    if let Some(page) = page {
        let declared: Vec<&str> = file.builders.iter().map(|b| b.name.as_str()).collect();
        if let Some(init) = crate::build::stage::process_ui_syntax::navigation_init(page, &declared)
        {
            items.push(init);
        }
    }
    Ok(crate::build::lower::ets_output::print_statements(&items))
}

/// The name a top-level `function name(...)` statement declares, if it is one.
fn function_name_of(stmt: &str) -> Option<String> {
    let text = stmt.trim_start();
    let text = text.strip_prefix("export ").unwrap_or(text).trim_start();
    let rest = text.strip_prefix("function ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// A file that declares no struct still gets its `@Builder function` expanded: the reference
/// expands every one of them wherever it stands (`PipManager.ets`, `Xb_Menu.ets`).
#[test]
fn a_file_without_a_struct_expands_its_builder() {
    let src = "@Builder\nfunction badge(t: string) {\n  Text(t)\n}\n";
    let opts = crate::build::lower::ets_transpile::TranspileOptions {
        kit_config_dirs: &[],
        project_builders: &[],
        route: None,
    };
    let out = expand_component_file(src, &opts).expect("expand");
    assert!(
        out.contains("function badge(t: string, parent = null)"),
        "{out}"
    );
    assert!(out.contains("const __t__ = t;"), "{out}");
    assert!(!out.contains("@Builder"), "{out}");
}

/// The route decision has to send a struct-less helper file down this path: the marker is the
/// `@Builder` decorator, not the presence of a struct.
#[test]
fn the_route_sends_a_helper_only_file_here() {
    let src = "class Params {\n  t: string = ''\n}\n@Builder\nfunction pb(params: Params) {\n  Row() {\n    Text(params.t)\n  }\n}\n";
    assert!(matches!(
        crate::build::lower::ui_syntax::route::detect(src),
        crate::build::lower::ui_syntax::route::Route::V1
    ));
    let opts = crate::build::lower::ets_transpile::TranspileOptions {
        kit_config_dirs: &[],
        project_builders: &[],
        route: None,
    };
    let out = crate::build::lower::ets_transpile::transpile(src, &opts).expect("transpile");
    assert!(
        out.contains("function pb(params: Params, parent = null)"),
        "{out}"
    );
    assert!(out.contains("const __params__ = params;"), "{out}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::stage::utils::V2_PROTOCOL;

    fn expand(source: &str, protocol: Protocol) -> String {
        let opts = crate::build::lower::ets_transpile::TranspileOptions {
            kit_config_dirs: &[],
            project_builders: &[],
            route: None,
        };
        expand_component_file_with(source, &opts, protocol).expect("expand")
    }

    /// A V2 struct keeps its decorated members for `ViewV2` to observe: no `_Params`
    /// interface, no `__` backstore, no accessors.
    #[test]
    fn v2_struct_keeps_its_decorated_members() {
        let out = expand(
            "@ComponentV2\nstruct Card {\n  @Param title: string = ''\n  @Local n: number = 0\n  build() {\n    Text(this.title)\n  }\n}\n",
            V2_PROTOCOL,
        );
        assert!(out.contains("class Card extends ViewV2"), "{out}");
        assert!(!out.contains("interface Card_Params"), "{out}");
        assert!(!out.contains("__title"), "{out}");
        assert!(out.contains("@Param readonly title: string;"), "{out}");
        // The printer owns layout and quoting, so these two assertions use its own form.
        assert!(
            crate::build::lower::ets_output::flattened(&out).contains(
                "this.initParam('title', params && 'title' in params ? params.title : '')"
            ),
            "{out}"
        );
        assert!(
            out.contains("public resetStateVarsOnReuse(params: Object): void"),
            "{out}"
        );
        assert!(
            crate::build::lower::ets_output::flattened(&out).contains("this.resetParam('title',"),
            "{out}"
        );
        assert!(!out.contains("setInitiallyProvidedValue"), "{out}");
    }

    /// A `@Reusable` V1 struct rebinds its backstores to a new element id and has no reuse
    /// reset at the project's API level.
    #[test]
    fn reusable_v1_struct_rebinds_element_ids() {
        let out = expand(
            "@Reusable\n@Component\nstruct Card {\n  @State n: number = 0\n  build() {\n    Text(`${this.n}`)\n  }\n}\n",
            V1_PROTOCOL,
        );
        assert!(out.contains("class Card extends ViewPU"), "{out}");
        assert!(
            out.contains("updateRecycleElmtId(oldElmtId, newElmtId)"),
            "{out}"
        );
        assert!(
            out.contains("this.__n.updateElmtId(oldElmtId, newElmtId);"),
            "{out}"
        );
        assert!(!out.contains("resetStateVarsOnReuse"), "{out}");
    }

    /// A decorated member is re-declared as its backstore with the modifier list the source
    /// wrote, and `private` is added only when the source wrote none.
    ///
    /// `createPropertyDeclaration` (`process_component_class.js`) re-modifies the declaration
    /// with `concatenateDecoratorsAndModifiers(void 0, getModifiers(source) || [private])`, so a
    /// `public` member stays `public` on the `__` member (measured on clashbox:
    /// `entry/components/Common/Common.ts:3364` writes `public __providerListData` for
    /// `@State public providerListData: Array<Provider>`).
    #[test]
    fn a_decorated_member_keeps_the_accessibility_the_source_wrote() {
        let out = expand(
            "@Component\nstruct Card {\n  @State public providerListData: Array<string> = []\n  @State hidden: number = 0\n  build() {\n    Text('x')\n  }\n}\n",
            V1_PROTOCOL,
        );
        assert!(
            out.contains("public __providerListData: ObservedPropertyObjectPU<Array<string>>;"),
            "{out}"
        );
        // A member written without a modifier keeps the reference's `private` default.
        assert!(
            out.contains("private __hidden: ObservedPropertySimplePU<number>;"),
            "{out}"
        );
    }

    /// A branch the source wrote inside a top-level `@Builder function` updates through the
    /// builder's `parent` fallback, while the empty `else` the compiler adds to a bare `if` —
    /// and every branch a class method runs — stay on `this`.
    ///
    /// `createIfBranchFunc` (`process_component_build.js`) writes
    /// `isBuilder ? parentConditionalExpression() : createThis()`, but the `else` that
    /// `processElseStatement` synthesises is built with a bare `createThis()` and never reads
    /// that flag (measured: clashbox `entry/components/Common/Common.ts`, where the three `if`s
    /// of `BindSheetTitleCustomBuilder` emit `(parent ? parent : this)` at `:469/533/561` and
    /// `this` at `:495/548/595`; `Tooltip.ts:27` and `:39` are the same pair).
    #[test]
    fn a_global_builder_branch_uses_the_parent_but_its_empty_else_stays_this() {
        let out = expand(
            "@Builder\nfunction badge(on: boolean) {\n  if (on) {\n    Text('a')\n  }\n}\n@Component\nstruct Card {\n  @State on: boolean = false\n  build() {\n    if (this.on) {\n      Text('b')\n    } else {\n      Text('c')\n    }\n  }\n}\n",
            V1_PROTOCOL,
        );
        let flat = crate::build::lower::ets_output::flattened(&out);
        // The builder's one written branch takes the parent fallback …
        assert_eq!(
            flat.matches("(parent ? parent : this).ifElseBranchUpdateFunction")
                .count(),
            1,
            "{out}"
        );
        // … and three branches stay on `this`: the empty `else` synthesized for the builder's
        // bare `if`, plus both branches of the class method's written `if`/`else`.
        assert_eq!(
            flat.matches("this.ifElseBranchUpdateFunction").count(),
            3,
            "{out}"
        );
    }

    /// A `.mono(…)` call written inside a top-level `@Builder function` takes the file's
    /// `@Extend` helper, not a plain attribute write.
    ///
    /// `EXTEND_ATTRIBUTE` is a per-file table and a free builder function is not excluded from
    /// it (`isExtendFunctionNode`, `process_component_build.js`). `emit_builder` used to hand
    /// `build_body::emit` three empty tables, so the attribute list could not recognise the
    /// helper and fell through to `Text.mono(…)` (measured: `@mcui/mccharts`
    /// `pageView/Tooltip.ts`, whose `globalBuilder` calls the file's `@Extend(Text) fancy` and
    /// whose reference emits `__Text__fancy($$.tooltipInfo);`).
    #[test]
    fn a_global_builder_expands_a_file_extend() {
        let out = expand(
            "@Extend(Text)\nfunction mono(size: number) {\n  .fontFamily('mono')\n}\n@Builder\nfunction badge(t: string) {\n  Text(t)\n    .mono(12)\n}\n@Component\nstruct Card {\n  build() {\n    Text('x')\n  }\n}\n",
            V1_PROTOCOL,
        );
        assert!(
            crate::build::lower::ets_output::flattened(&out).contains("__Text__mono(12);"),
            "{out}"
        );
        assert!(!out.contains("Text.mono("), "{out}");
    }

    /// A `.animation(…)` written inside a `@Styles` body opens and closes a context of its own:
    /// the open goes above the body's writes, the close at the attribute's own position, and the
    /// enclosing component's chain is not consulted.
    ///
    /// `traverseStateStylesAttr` hands the body to `bindComponentAttr` a second time
    /// (`process_component_build.js`), which is why the call site's own attributes stay outside
    /// the context (measured: clashbox `entry/components/Common/Common.ts:699-703`, where
    /// `Column.alignItems(…)` precedes the open and the body's `padding` sits inside it).
    #[test]
    fn a_styles_body_animation_opens_its_own_context() {
        let out = expand(
            "@Styles\nfunction cardStyle() {\n  .padding(16)\n  .animation({ duration: 100, curve: Curve.Linear })\n}\n@Component\nstruct Card {\n  build() {\n    Column()\n      .alignItems(HorizontalAlign.Start)\n      .cardStyle()\n  }\n}\n",
            V1_PROTOCOL,
        );
        let flat = crate::build::lower::ets_output::flattened(&out);
        let at = |needle: &str| {
            flat.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {out}"))
        };
        // The animation is not a plain attribute of the component any more.
        assert!(!flat.contains("Column.animation("), "{out}");
        let align = at("Column.alignItems(HorizontalAlign.Start);");
        let open = at("globalThis.Context.animation({ duration: 100, curve: Curve.Linear });");
        let padding = at("Column.padding(16);");
        let close = at("globalThis.Context.animation(null);");
        assert!(align < open, "{out}");
        assert!(open < padding, "{out}");
        assert!(padding < close, "{out}");
    }

    /// A read of an object-typed member handed to a method call is wrapped in
    /// `ObservedObject.GetRawObject`, and the wrap survives the binding of a `@Builder` call.
    ///
    /// `isNeedGetRawObject` (`process_component_class.js`) requires the argument's parent to be
    /// a call whose callee is a property access, which `this.titleText(this.title)` is and
    /// `this.titleText.bind(this)(…)` is not. The bound form comes from `prepare_build_body`
    /// (utils.rs), so the wrap has to run before it (measured: clashbox `Common.ts:3433`).
    #[test]
    fn a_builder_call_wraps_its_object_argument() {
        let out = expand(
            "@Component\nstruct Card {\n  @State title: string | Resource = ''\n  @Builder\n  titleText(t: string | Resource) {\n    Text(t)\n  }\n  build() {\n    this.titleText(this.title)\n  }\n}\n",
            V1_PROTOCOL,
        );
        assert!(
            crate::build::lower::ets_output::flattened(&out)
                .contains("this.titleText.bind(this)(ObservedObject.GetRawObject(this.title))"),
            "{out}"
        );
    }

    /// The object-member table is keyed by **name** and accumulated across the file's structs in
    /// declaration order, so a member the reading struct declares `string` is still wrapped when
    /// an earlier struct registered that name as an object.
    ///
    /// `stateObjectCollection` (`process_component_member.js`) is filled per struct and never
    /// cleared, and `isNeedGetRawObject` asks the Set rather than the reading declaration
    /// (measured: clashbox `entry/components/Common/Common.ets`, where `Nothing`'s
    /// `@State name: Resource` makes `skipRowList`'s `@State name: string` read wrap).
    #[test]
    fn an_earlier_struct_registers_a_name_for_the_later_reads() {
        let out = expand(
            "@Component\nstruct First {\n  @State name: Resource = $r('app.string.x')\n  build() {\n    Text('a')\n  }\n}\n@Component\nstruct Second {\n  @State name: string = ''\n  @State plain: string = ''\n  build() {\n    Text('b')\n      .geometryTransition(this.name)\n      .fontColor(this.plain)\n  }\n}\n",
            V1_PROTOCOL,
        );
        let flat = crate::build::lower::ets_output::flattened(&out);
        assert!(
            flat.contains("Text.geometryTransition(ObservedObject.GetRawObject(this.name));"),
            "{out}"
        );
        // A name no struct registered as an object stays as written.
        assert!(flat.contains("Text.fontColor(this.plain);"), "{out}");
    }

    /// A struct-level `@Builder` method keeps its declaration and loses its decorator: the
    /// boundary comes from the method's own decorator spans, not from a walk of the text.
    #[test]
    fn a_builder_method_signature_excludes_its_decorator() {
        let out = expand(
            "@Component\nstruct Card {\n  @Builder\n  private badge(text: string) {\n    Text(text)\n  }\n  build() {\n    Text('x')\n  }\n}\n",
            V1_PROTOCOL,
        );
        assert!(
            out.contains("private badge(text: string, parent = null)"),
            "{out}"
        );
        assert!(!out.contains("@Builder"), "{out}");
    }

    /// The import test reads the statement node: a dynamic `import(…)` is an expression, not
    /// an import declaration, so it stays among the file's top-level statements instead of
    /// disappearing from the output.
    #[test]
    fn a_dynamic_import_statement_is_not_an_import_declaration() {
        let out = expand(
            "import('./late');\n@Component\nstruct Card {\n  build() {\n    Text('x')\n  }\n}\n",
            V1_PROTOCOL,
        );
        assert!(out.contains("import('./late')"), "{out}");
    }

    /// Two `@Watch` decorators on one member: the reference calls `processWatch` for each of
    /// them and both write the same prop→watcher slot, so the last one is what the constructor
    /// declares (measured: clashbox `entry/pages/Index.ts`, `heightBp` and `widthBp`, whose
    /// `@Watch('onWindowChange') @Watch('isSecondaryChange')` pair emits
    /// `this.declareWatch("heightBp", this.isSecondaryChange);`).
    #[test]
    fn the_last_watch_decorator_names_the_watcher() {
        let out = expand(
            "@Component\nstruct Card {\n  @StorageLink('heightBp') @Watch('onWindowChange') @Watch('isSecondaryChange') heightBp: string = ''\n  build() {\n    Text(this.heightBp)\n  }\n}\n",
            V1_PROTOCOL,
        );
        let flat = crate::build::lower::ets_output::flattened(&out);
        assert!(
            flat.contains("this.declareWatch('heightBp', this.isSecondaryChange);"),
            "{out}"
        );
        assert!(!flat.contains("this.onWindowChange"), "{out}");
    }

    /// The widening is a running total: an offset before the first insertion stays put, one
    /// between the two moves by the first alone, and one past both moves by the sum.
    #[test]
    fn a_source_offset_moves_by_every_widening_before_it() {
        // Two insertions: five bytes at source offset 10, seven more at source offset 25 (which
        // is `30` in the rewritten text, because the first insertion precedes it).
        let widened = [(10usize, 5usize), (30, 7)];
        assert_eq!(widen_offset(&widened, 0), 0, "before the first");
        assert_eq!(widen_offset(&widened, 9), 9, "just before the first");
        assert_eq!(widen_offset(&widened, 10), 15, "at the first");
        assert_eq!(widen_offset(&widened, 20), 25, "between the two");
        assert_eq!(widen_offset(&widened, 25), 37, "at the second");
        assert_eq!(widen_offset(&widened, 40), 52, "past both");
        assert_eq!(widen_offset(&[], 40), 40, "a file nothing widened");
    }

    /// A helper is emitted where its declaration stood, which is a *statement* offset as far as
    /// the merge is concerned: `__Text__tail` is declared behind `export default Probe`, and the
    /// struct's eight child blocks widen the file by forty bytes — more than the twenty-two byte
    /// gap between the two — so the helper used to jump in front of the export (clashbox
    /// `More/Connect.ts`, `More/Request.ts`, `Settings/Appearance.ts`).
    #[test]
    fn an_extend_helper_after_an_export_default_stays_behind_it() {
        let out = expand(
            "@Entry\n@Component\nstruct Probe {\n  build() {\n    Column() {\n      Text('a')\n      Row() {\n        Text('b')\n      }\n      Column() {\n        Text('c')\n      }\n      Row() {\n        Text('d')\n      }\n      Column() {\n        Text('e')\n      }\n      Row() {\n        Text('f')\n      }\n      Column() {\n        Text('g')\n      }\n      Row() {\n        Text('h')\n      }\n    }\n  }\n}\nexport default Probe\n\n@Extend(Text)\nfunction tail(pad: number) {\n  .padding(pad)\n}\n",
            V1_PROTOCOL,
        );
        let export = out.find("export default Probe;").expect("the export");
        let helper = out
            .find("function __Text__tail(pad: number): void {")
            .expect("the helper");
        assert!(
            export < helper,
            "the helper is emitted where it was declared, behind the export:\n{out}"
        );
        assert!(out.contains("Text.padding(pad);"), "{out}");
    }
}

#[cfg(test)]
mod directive_tests {
    use super::*;

    /// A leading `"use shared"` directive survives the expansion, in the position the reference
    /// emits it: after the `_Params` interface, before the class
    /// (measured: `fixtures/use-shared`, 2026-11-06).
    #[test]
    fn a_leading_directive_is_kept_before_the_class() {
        let opts = crate::build::lower::ets_transpile::TranspileOptions {
            kit_config_dirs: &[],
            project_builders: &[],
            route: None,
        };
        let out = expand_component_file_with(
            "\"use shared\"\n\n@Entry\n@Component\nstruct Shared {\n  build() {\n    Text('x')\n  }\n}\n",
            &opts,
            V1_PROTOCOL,
        )
        .expect("expand");
        // The printer owns quoting; the directive is found in its printed form.
        let out = crate::build::lower::ets_output::flattened(&out);
        let directive = out.find("'use shared'").expect("directive kept");
        let interface = out
            .find("interface Shared_Params")
            .expect("params interface");
        let class = out.find("class Shared extends ViewPU").expect("class");
        assert!(interface < directive, "{out}");
        assert!(directive < class, "{out}");
    }
}

#[cfg(test)]
mod named_route_tests {
    use super::*;

    /// A `@Entry` page in a module that is not the assembled one registers under the module the
    /// build assembles (`projectConfig.moduleName`), while its coordinate keeps the module that
    /// owns the source file — the reference shape measured on clashbox:
    /// `{ moduleName: "entry", pagePath: "xb_components/src/main/ets/view/Xb_Studio" }`.
    #[test]
    fn a_page_in_a_sibling_module_registers_under_the_assembled_module() {
        crate::build::compile::abc_compile::set_assemble_module("entry");
        let route = crate::build::lower::ets_transpile::RouteInfo {
            bundle: "com.example.app",
            module: "xb_components",
            page: "view/Xb_Studio",
            compile_har: false,
        };
        let opts = crate::build::lower::ets_transpile::TranspileOptions {
            kit_config_dirs: &[],
            project_builders: &[],
            route: Some(route),
        };
        let out = expand_component_file_with(
            "@Entry\n@Component\nstruct Xb_Studio {\n  build() {\n    Text('x')\n  }\n}\n",
            &opts,
            V1_PROTOCOL,
        )
        .expect("expand");
        // Both sides go through the printer, so quoting and layout are not the subject.
        let expected = crate::build::lower::ets_output::print_module(
            "registerNamedRoute(() => new Xb_Studio(undefined, {}), \"\", { bundleName: \"com.example.app\", moduleName: \"entry\", pagePath: \"view/Xb_Studio\", pageFullPath: \"xb_components/src/main/ets/view/Xb_Studio\", integratedHsp: \"false\", moduleType: \"followWithHap\" });",
        )
        .expect("the expectation prints");
        assert!(
            crate::build::lower::ets_output::flattened(&out)
                .contains(&crate::build::lower::ets_output::flattened(&expected)),
            "{out}"
        );
    }

    /// The footer keeps the two module slots apart: `moduleName` is the assembled module,
    /// `pageFullPath` the page's own.
    #[test]
    fn the_footer_separates_the_assembled_module_from_the_page_module() {
        let footer = named_route_footer("X", "b", "entry", "xb_components", "pages/X");
        assert!(footer.contains("moduleName: \"entry\""), "{footer}");
        assert!(footer.contains("pagePath: \"pages/X\""), "{footer}");
        assert!(
            footer.contains("pageFullPath: \"xb_components/src/main/ets/pages/X\""),
            "{footer}"
        );
    }
}
