// process stage: `ets_checker.js` — the pass that decides a file is not provably correct.
//
// The reference builds one TypeScript program per module, runs `doArkTSLinter` over it and then
// prints `getSyntacticDiagnostics() + getSemanticDiagnostics()` as `ArkTS:ERROR` / `ArkTS:WARN`.
// That pass is why a file with `let x: any` or with `"three"` assigned to a `number` never
// reaches a package. hvolt compiles through oxc and es2abc with no type checker, so until now
// such a file built successfully and failed on the device instead.
//
// What is implemented here is a subset, and the subset is named rather than implied:
//
// * `arkts-no-any-unknown` (code 10605008): every type annotation that names `any` or `unknown`.
//   The rule is positional — it fires on the type, not on the value — so it needs no inference.
// * literal-to-primitive assignment (code 10505001): a property declared with a primitive type
//   keyword whose initialiser is a literal of a different primitive type. This is the first
//   slice of the reference's assignability check, not the check: a value that is a call, an
//   identifier, or a non-primitive type is not judged here, and stays a false negative until the
//   inference it needs exists.
//
// Everything else the reference reports — implicit `any`, member existence, argument counts,
// generics — is still missing. A file that passes here is "no diagnostic hvolt can produce yet",
// not "clean under the reference".
//
// Measured on `fixtures/type-error` (2026-11-20): the reference reports
//   10505001 `ets/pages/Index.ets:4:11 Type 'string' is not assignable to type 'number'.`
//   10605008 `ets/pages/Index.ets:6:20 Use explicit types instead of "any", "unknown" (arkts-no-any-unknown)`
// and these two positions are what the rules below reproduce.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    CatchClause, Class, ClassElement, Expression, FormalParameter, Function, FunctionBody,
    ImportDeclaration, ImportDeclarationSpecifier, MethodDefinition, MethodDefinitionKind,
    ModuleExportName, NewExpression, ObjectExpression, ObjectPropertyKind, Program,
    PropertyDefinition, PropertyKey, ReturnStatement, Statement, TSConditionalType,
    TSConstructSignatureDeclaration, TSConstructorType, TSEnumDeclaration, TSIndexSignature,
    TSMappedType, TSType, TSTypeAnnotation, TSTypeAssertion, TSTypeLiteral, TSTypeQuery,
    ThisExpression, UnaryOperator, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

/// One diagnostic, in the form both the console and `scripts/oracle/diagnostics.py` read:
/// `<severity> <code> <file>:<line>:<col> <message>`.
pub(crate) struct Diagnostic {
    pub(crate) severity: &'static str,
    /// The reference's console error code (`10605008`), or `-` when it prints none.
    pub(crate) code: &'static str,
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) message: String,
}

impl Diagnostic {
    pub(crate) fn line(&self) -> String {
        // A diagnostic the reference reports against the whole file (`10905231`, `10905402`) has
        // no line or column; both are zero and print as `-`, the form `diagnostics.py` documents.
        let line = if self.line == 0 {
            "-".to_string()
        } else {
            self.line.to_string()
        };
        let column = if self.column == 0 {
            "-".to_string()
        } else {
            self.column.to_string()
        };
        format!(
            "{} {} {}:{}:{} {}",
            self.severity, self.code, self.file, line, column, self.message
        )
    }
}

thread_local! {
    /// Every `@ohos.*` / `@system.*` module the installed SDK declares, published by the build
    /// once it has resolved the SDK (`publish_system_modules`). `None` until then: a module
    /// missing from a list that was never read is not a diagnostic.
    static SYSTEM_MODULES: std::cell::RefCell<Option<std::collections::HashSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// Names the installed SDK declares globally. `None` until the build has resolved the SDK, and
    /// stays `None` when it found no declaration files: an SDK laid out differently must not turn
    /// every local class into a redeclaration.
    static GLOBAL_DECLARATIONS: std::cell::RefCell<Option<std::collections::HashSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Publishes the names the SDK declares in the global scope.
pub(crate) fn set_global_declarations(names: std::collections::HashSet<String>) {
    GLOBAL_DECLARATIONS.with(|cell| *cell.borrow_mut() = Some(names));
}

/// `Some(true)` when the SDK declares the name globally, `None` when the set is unpublished.
fn sdk_declares(name: &str) -> Option<bool> {
    GLOBAL_DECLARATIONS.with(|cell| cell.borrow().as_ref().map(|set| set.contains(name)))
}

pub(crate) fn set_system_modules(modules: std::collections::HashSet<String>) {
    SYSTEM_MODULES.with(|cell| *cell.borrow_mut() = Some(modules));
}

/// What the SDK says about ArkUI components: each one's own attribute names, the names every
/// component inherits, and the parents a component may be nested in.
#[derive(Default)]
pub(crate) struct ComponentTables {
    attributes: std::collections::HashMap<String, std::collections::HashSet<String>>,
    common: std::collections::HashSet<String>,
    parents: std::collections::HashMap<String, Vec<String>>,
    children: std::collections::HashMap<String, Vec<String>>,
}

thread_local! {
    /// Published by the build from the installed SDK (`publish_component_attributes`).
    static COMPONENT_ATTRIBUTES: std::cell::RefCell<Option<ComponentTables>> =
        const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// Members of the primitive types, published by the build from the TypeScript standard library
    /// the loader ships (`publish_primitive_members`).
    static PRIMITIVE_MEMBERS: std::cell::RefCell<
        Option<std::collections::HashMap<String, std::collections::HashSet<String>>>,
    > = const { std::cell::RefCell::new(None) };
}

/// Publishes the primitive member sets; see `ets_assemble::publish_primitive_members`.
pub(crate) fn set_primitive_members(
    members: std::collections::HashMap<String, std::collections::HashSet<String>>,
) {
    PRIMITIVE_MEMBERS.with(|cell| *cell.borrow_mut() = Some(members));
}

/// `Some(true)` when the primitive has the member, `Some(false)` when it does not and the set is
/// known, `None` when the library was not read.
fn primitive_has_member(kind: &str, member: &str) -> Option<bool> {
    PRIMITIVE_MEMBERS.with(|cell| {
        let table = cell.borrow();
        let table = table.as_ref()?;
        Some(table.get(kind)?.contains(member))
    })
}

/// Publishes the component tables; see `ets_assemble::publish_component_attributes`.
pub(crate) fn set_component_attributes(
    attributes: std::collections::HashMap<String, std::collections::HashSet<String>>,
    common: std::collections::HashSet<String>,
    parents: std::collections::HashMap<String, Vec<String>>,
    children: std::collections::HashMap<String, Vec<String>>,
) {
    COMPONENT_ATTRIBUTES.with(|cell| {
        *cell.borrow_mut() = Some(ComponentTables {
            attributes,
            common,
            parents,
            children,
        })
    });
}

/// The children a component allows, when the SDK restricts them (`Text` accepts only `Span`,
/// `ImageSpan`, `ContainerSpan` and `SymbolSpan`).
fn component_children(component: &str) -> Option<Vec<String>> {
    COMPONENT_ATTRIBUTES.with(|cell| {
        let table = cell.borrow();
        table.as_ref()?.children.get(component).cloned()
    })
}

/// Every component name the SDK knows: what a `Name(` in a body may be.
fn known_components() -> std::collections::HashSet<String> {
    COMPONENT_ATTRIBUTES.with(|cell| {
        let table = cell.borrow();
        let Some(tables) = table.as_ref() else {
            return std::collections::HashSet::new();
        };
        tables.attributes.keys().cloned().collect()
    })
}

/// The parents a component may be nested in, when the SDK restricts them.
fn component_parents(component: &str) -> Option<Vec<String>> {
    COMPONENT_ATTRIBUTES.with(|cell| {
        let table = cell.borrow();
        table.as_ref()?.parents.get(component).cloned()
    })
}

/// `Some(true)` when the component accepts the attribute, `Some(false)` when it does not and the
/// component is known, `None` when the SDK was not read or the component is not built in.
fn component_accepts(component: &str, attribute: &str) -> Option<bool> {
    COMPONENT_ATTRIBUTES.with(|cell| {
        let table = cell.borrow();
        let tables = table.as_ref()?;
        let own = tables.attributes.get(component)?;
        Some(own.contains(attribute) || tables.common.contains(attribute))
    })
}

thread_local! {
    /// Top-level names each `@ohos.*` / `@system.*` module's declaration file declares. An entry
    /// that is `None` means the file names everything (`export * from`), so nothing is reported.
    static SYSTEM_MODULE_SYMBOLS: std::cell::RefCell<
        Option<std::collections::HashMap<String, Option<std::collections::HashSet<String>>>>,
    > = const { std::cell::RefCell::new(None) };
}

/// Publishes the declared names of every system module, alongside their existence.
pub(crate) fn set_system_module_symbols(
    symbols: std::collections::HashMap<String, Option<std::collections::HashSet<String>>>,
) {
    SYSTEM_MODULE_SYMBOLS.with(|cell| *cell.borrow_mut() = Some(symbols));
}

/// `Some(true)` when the SDK's declaration of the module has the name, `Some(false)` when it does
/// not, `None` when that module's names were not collected.
fn system_module_has_symbol(request: &str, symbol: &str) -> Option<bool> {
    SYSTEM_MODULE_SYMBOLS.with(|cell| {
        let table = cell.borrow();
        let table = table.as_ref()?;
        match table.get(request)? {
            None => Some(true),
            Some(names) => Some(names.contains(symbol)),
        }
    })
}

/// `Some(true)` when the SDK declares the request, `Some(false)` when it does not, `None` when no
/// list was published.
fn system_module_declared(request: &str) -> Option<bool> {
    SYSTEM_MODULES.with(|cell| cell.borrow().as_ref().map(|set| set.contains(request)))
}

/// Off switch, same shape as the undefined-identifier pass (`HVOLT_SEMANTIC`).
pub(crate) fn enabled() -> bool {
    match std::env::var("HVOLT_TYPE_CHECK") {
        Ok(value) => value != "off" && value != "0",
        Err(_) => true,
    }
}

/// `run_arkts_linter`: one source file in, the diagnostics it provably carries out.
pub(crate) fn check(file: &str, source: &str) -> Vec<Diagnostic> {
    // `.ets` is not TypeScript: `struct`, the decorators and the ArkUI call-with-a-block bodies
    // are the loader's own spelling. `process_ui_syntax::mask` is what turns a page into text a
    // parser accepts, and the lowering already reads every page through it; the mask rewrites in
    // place, so an offset in the masked text is the same offset in the source and a diagnostic
    // can be reported against the file the reader has open.
    let allocator = Allocator::default();
    let source_type = SourceType::default().with_typescript(true);
    // Two views, tried in order. The first keeps every body, so an annotation written inside one
    // is still judged; the second blanks them all, and exists because `@Builder` / `@Extend` /
    // `@Styles` bodies are the loader's syntax too. Dropping to the second view costs the
    // annotations inside bodies but keeps the file's declarations judged — better than skipping
    // the file, which is what happens when neither view parses.
    let mut view = parser_view(source);
    let mut parsed = Parser::new(&allocator, &view, source_type).parse();
    if !parsed.errors.is_empty() {
        view = parser_view_without_bodies(source);
        parsed = Parser::new(&allocator, &view, source_type).parse();
    }
    if std::env::var("HVOLT_TYPE_CHECK_DEBUG").is_ok() {
        eprintln!(
            "[typecheck] {file}: parse_errors={} masked={} src={}",
            parsed.errors.len(),
            view.len(),
            source.len()
        );
        for error in parsed.errors.iter().take(3) {
            eprintln!("[typecheck] err: {error:?}");
        }
    }
    if !parsed.errors.is_empty() {
        // ArkTS spelling this parser does not accept: judged by nobody rather than by a half
        // parsed tree. The build's own lowering still reads the file.
        return Vec::new();
    }
    let mut checker = Checker {
        style_names: std::collections::HashSet::new(),
        file: file.to_string(),
        source,
        line_starts: line_starts(source),
        out: Vec::new(),
        data_fields: Vec::new(),
        every_data_field: Vec::new(),
        lints: file.ends_with(".ets"),
        unannotated_fns: unannotated_functions(&parsed.program),
        class_names: class_names(&parsed.program),
        arity: declared_arity(&parsed.program),
        primitives: std::collections::HashMap::new(),
        inferred_numbers: std::collections::HashSet::new(),
        union_types: std::collections::HashMap::new(),
        member_types: interface_member_types(&parsed.program),
        optional_members: optional_members(&parsed.program),
        argument_types: {
            let known = interface_member_types(&parsed.program);
            function_parameter_named_types(&parsed.program, &known)
        },
        parameter_types: function_parameter_types(&parsed.program),
        array_elements: array_element_types(&parsed.program),
        enums: enum_names(&parsed.program),
        any_locals: any_typed_locals(&parsed.program),
        merged_types: declaration_merges(&parsed.program)
            .into_iter()
            .map(|(name, _)| name)
            .collect(),
        index_signatures: index_signature_types(source),
        method_parameter_types: method_parameter_types(&parsed.program),
        return_type: None,
        return_spelling: None,
        members: class_members(&parsed.program),
        statics: static_members(&parsed.program),
        entries: entry_classes(&parsed.program),
        structs: struct_names(&parsed.program, source),
        v2_classes: v2_classes(&parsed.program),
        current_class: None,
        class_locals: std::collections::HashMap::new(),
        arrow_depth: 0,
        method_depth: 0,
    };
    // Shared by the checks below: the component names, and the `@Extend`s no component answers.
    let known = known_components();
    let mut unknown_extends: Vec<String> = Vec::new();
    if checker.lints {
        // `validate_ui_syntax`: a `@Styles` takes no arguments (`10905105`, measured 8:3 of
        // `fixtures/ui-styles-arguments`, at the decorator). The mask blanks the whole member, so
        // the declaration is read from the file's own `@Styles` list rather than from the tree.
        let masked = crate::build::stage::process_ui_syntax::mask_builder_decorators(source);
        // An `@Extend` name is a style call too: `.fancy()` on a `Text` is the generated helper,
        // not an attribute the component lacks (`fixtures/ui-extend-duplicate`).
        for extend in &masked.extends {
            // The name stays accepted by the attribute scan either way: an `@Extend` whose target
            // no component answers is reported as its own warning instead
            // (`fixtures/ui-extend-target`), not as a missing attribute error.
            checker.style_names.insert(extend.name.clone());
            if !known.contains(extend.comp.as_str()) {
                unknown_extends.push(extend.name.clone());
            }
        }
        let styles = masked.styles;
        for style in &styles {
            if !style.params_text.is_empty() {
                checker.push(
                    "ERROR",
                    "10905105",
                    style.order,
                    "'@Styles' decorated functions and methods cannot have arguments.".to_string(),
                );
            }
            // Its call stays in the page as `.styledColumn(100)`, and that is a style call, not an
            // attribute the component lacks.
            checker.style_names.insert(style.name.clone());
        }
    }
    checker.visit_program(&parsed.program);
    checker.scan_call_sites();
    // `validate_ui_syntax`, file level: a page must carry exactly one `@Entry`. Two rules are
    // reported together when there are two, both against the file rather than a position
    // (`fixtures/ui-entry-count`). Only pages are judged — the message itself says "a page
    // configured in main_pages.json or build-profile.json5".
    if checker.lints {
        // `Cannot redeclare block-scoped variable 'Counter'.`: a class or interface may not take a
        // name the SDK declares globally (measured 1:7 of `fixtures/type-redeclare`, at the name).
        for (offset, name, is_class) in declared_type_names(&parsed.program, source) {
            if std::env::var("HVOLT_GLOBAL_DEBUG").is_ok() {
                eprintln!(
                    "[decl] {name} class={is_class} sdk={:?}",
                    sdk_declares(&name)
                );
            }
            // Only a `class` collides this way: an interface of the same name is answered by
            // `arkts-unique-names` instead (`fixtures/arkts-unique-names` keeps its single 10605004).
            if is_class && sdk_declares(&name) == Some(true) {
                checker.push(
                    "ERROR",
                    "10505001",
                    offset,
                    format!("Cannot redeclare block-scoped variable '{name}'."),
                );
            }
        }
        // `arkts-unique-names` (10605004): a type may not take a name the ArkUI components already
        // use — `interface Shape` collides with the `Shape` component
        // (measured 1:11 of `fixtures/arkts-unique-names`).
        let known = known_components();
        for offset in component_name_collisions(&parsed.program, &known) {
            checker.push(
                "ERROR",
                "10605004",
                offset,
                "Use unique names for types and namespaces. (arkts-unique-names)".to_string(),
            );
        }
    }
    if checker.lints && file.contains("/pages/") {
        let (entries, components) = structure_counts(&parsed.program);
        if components == 0 {
            checker
                .out
                .sort_by(|a, b| (a.line, a.column, a.code).cmp(&(b.line, b.column, b.code)));
            return checker.out;
        }
        if entries > 1 {
            checker.push_file_level(
                "ERROR",
                "10905231",
                "A page can't contain more than one '@Entry' decorator.".to_string(),
            );
        }
        // A global `function` may only carry the six decorators the reference lists; `@LocalBuilder`
        // belongs to a method (both at 1:1 of `fixtures/ui-local-builder`).
        for (offset, code, message) in global_function_decorator_violations(source) {
            checker.push("ERROR", code, offset, message);
        }
        // `this.<name>` that no struct of the file declares (measured 8:35 of
        // `fixtures/type-struct-member`).
        for (offset, message) in struct_member_violations(source, &checker.members) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // A child instantiation that hands a literal of the wrong type to a declared member
        // (measured 17:19 of `fixtures/ui-component-argument`).
        let struct_fields = struct_field_types(&parsed.program, source);
        for (offset, message) in component_argument_violations(source, &struct_fields) {
            checker.push("ERROR", "10505001", offset, message);
        }
        for (offset, message) in field_assignment_violations(source, &struct_fields) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // A `@Builder` method's declared parameter count, and the calls that disagree with it.
        let builders = builder_arities(source);
        for (offset, message) in builder_arity_violations(source, &builders) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // A name declared twice in this file.
        for (_, at) in declaration_merges(&parsed.program) {
            checker.push(
                "ERROR",
                "10605103",
                at,
                "Declaration merging is not supported (arkts-no-decl-merging)".to_string(),
            );
        }
        // An interface with an index signature, and the literals declared to be it.
        let signature_table = checker.index_signatures.clone();
        let mut signature_offsets: Vec<usize> =
            signature_table.values().map(|(at, _)| *at).collect();
        signature_offsets.sort_unstable();
        for at in signature_offsets {
            checker.push(
                "ERROR",
                "10605017",
                at,
                "Indexed signatures are not supported (arkts-no-indexed-signatures)".to_string(),
            );
        }
        for (offset, message) in
            index_signature_literal_violations(source, &record_alias_types(source))
        {
            let code = if message.starts_with("Object literal must") {
                "10605038"
            } else {
                "10505001"
            };
            checker.push("ERROR", code, offset, message);
        }
        for (offset, message) in index_signature_literal_violations(source, &signature_table) {
            let code = if message.starts_with("Object literal must") {
                "10605038"
            } else {
                "10505001"
            };
            checker.push("ERROR", code, offset, message);
        }
        // A `@Builder` method's declared parameter types, likewise.
        for (offset, message) in builder_type_violations(source, &builder_param_spellings(source)) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // `component_map` carries each component's constructor signature; `Text` takes
        // `string | Resource`, so a numeric literal is refused at the argument
        // (measured 7:12 of `fixtures/ui-text-argument`).
        for offset in text_number_arguments(source) {
            checker.push(
                "ERROR",
                "10505001",
                offset,
                "Argument of type 'number' is not assignable to parameter of type 'string | Resource'."
                    .to_string(),
            );
        }
        // An `@Extend(<component>)` whose target no component answers.
        let components: std::collections::HashSet<String> = known.into_iter().collect();
        for (offset, severity, message) in extend_target_violations(source, &components) {
            checker.push(
                severity,
                if severity == "ERROR" { "10505001" } else { "-" },
                offset,
                message,
            );
        }
        for (offset, message) in
            unknown_extend_call_violations(source, &unknown_extends, &components)
        {
            checker.push("WARN", "-", offset, message);
        }
        // Two `@Extend(C)` functions of the same name emit one helper twice, which the reference
        // reports as `Duplicate function implementation.` at each name (`fixtures/ui-extend-duplicate`:
        // 1:24 and 5:24).
        for offset in duplicate_extends(source) {
            checker.push(
                "ERROR",
                "10505001",
                offset,
                "Duplicate function implementation.".to_string(),
            );
        }
        // `this.<static>`: the reference names the fix
        // (`… Did you mean to access the static member 'UiStaticState.counter' instead?`, measured
        // 10:20 of `fixtures/ui-static-state`). The build body is blanked out of the checked view,
        // so the occurrence is found in the file's own text.
        let statics: Vec<(String, Vec<String>)> = checker
            .statics
            .iter()
            .map(|(class, members)| (class.clone(), members.iter().cloned().collect()))
            .collect();
        for (class, members) in &statics {
            for member in members {
                for offset in this_member_offsets(source, member) {
                    checker.push(
                        "ERROR",
                        "10505001",
                        offset,
                        format!(
                            "Property '{member}' does not exist on type '{class}'. Did you mean to access the static member '{class}.{member}' instead?"
                        ),
                    );
                }
            }
        }
        // A `struct` that is none of the component kinds: a warning when it carries `@Entry`
        // (22:7 of `fixtures/ui-entry-count`), an error with its own code otherwise (`10905233`
        // at 1:1 of `fixtures/ui-struct-decorator`).
        for (offset, name, entry) in undecorated_structs(&parsed.program, source) {
            let message = format!(
                "Decorator '@Component', '@ComponentV2', or '@CustomDialog' is missing for struct '{name}'."
            );
            if entry {
                checker.push("WARN", "-", offset, message);
            } else {
                checker.push("ERROR", "10905233", offset, message);
            }
        }
        if entries != 1 {
            checker.push_file_level(
                "ERROR",
                "10905402",
                "A page configured in 'main_pages.json or build-profile.json5' must have one and only one '@Entry' decorator.".to_string(),
            );
        }
    }
    checker
        .out
        .sort_by(|a, b| (a.line, a.column, a.code).cmp(&(b.line, b.column, b.code)));
    checker.out
}

/// The source as a TypeScript parser reads it, at the source's own offsets.
///
/// Two rewrites, both same-length so an offset means the same thing on both sides:
/// `process_ui_syntax::mask` turns the loader's decorator shorthand into ordinary syntax, and
/// `struct` becomes `class` — the keyword the loader spells differently from TypeScript. A UI
/// body (`Column() { … }`) is already ordinary TypeScript to a parser: a call followed by a
/// block, so it needs no rewrite.
fn parser_view(source: &str) -> String {
    let mut bytes = rewritten(source);
    blank_build_bodies(source, &mut bytes);
    String::from_utf8(bytes).unwrap_or_default()
}

/// [`parser_view`], with every function and method body blanked as well.
fn parser_view_without_bodies(source: &str) -> String {
    let mut bytes = rewritten(source);
    blank_all_bodies(source, &mut bytes);
    String::from_utf8(bytes).unwrap_or_default()
}

/// The two rewrites every view shares: the loader's decorator shorthand, and `struct`.
fn rewritten(source: &str) -> Vec<u8> {
    let masked = crate::build::stage::process_ui_syntax::mask_builder_decorators(source).source;
    let mut bytes = masked.into_bytes();
    for token in crate::build::lex::tokenize(source) {
        if token.kind != crate::build::lex::Kind::Ident {
            continue;
        }
        if source.get(token.start..token.end) != Some("struct") {
            continue;
        }
        // Six bytes for six bytes: `class` plus the space `struct` was followed by.
        bytes[token.start..token.start + 6].copy_from_slice(b"class ");
    }
    bytes
}

/// Blanks the inside of every block that follows a parameter list — a function, a method, a
/// constructor, and the `if`/`for` bodies inside them.
fn blank_all_bodies(source: &str, bytes: &mut [u8]) {
    use crate::build::lex::{matching_delimiter, tokenize};
    let tokens = tokenize(source);
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        let token = tokens[index];
        if !token.is_punct('{') {
            index += 1;
            continue;
        }
        if index == 0 {
            index += 1;
            continue;
        }
        let previous = tokens[index - 1];
        if !previous.is_punct(')') {
            index += 1;
            continue;
        }
        let Some(close) = matching_delimiter(source, token.start) else {
            index += 1;
            continue;
        };
        for byte in bytes.iter_mut().take(close).skip(token.start + 1) {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        index += 1;
    }
}

/// Blanks the inside of every `build() { … }` body, keeping the braces and the line breaks.
///
/// A UI body is the loader's own syntax: a component call followed by a block, an attribute chain
/// as a statement. Blanking it leaves `build() { }` — ordinary TypeScript — while every offset
/// outside the body keeps its meaning, so a diagnostic elsewhere in the file still lands on the
/// line the reader sees. Type annotations written *inside* a UI body are therefore not judged
/// yet; that is a false negative, and it is the price of parsing `.ets` with a TypeScript parser.
fn blank_build_bodies(source: &str, bytes: &mut [u8]) {
    use crate::build::lex::{matching_delimiter, tokenize, Kind};
    let tokens = tokenize(source);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident || source.get(token.start..token.end) != Some("build") {
            continue;
        }
        let Some(paren) = tokens.get(index + 1) else {
            continue;
        };
        if !paren.is_punct('(') {
            continue;
        }
        let Some(after_params) = matching_delimiter(source, paren.start) else {
            continue;
        };
        // The `{` after `)` may be separated from it by whitespace, so it is found by position
        // rather than by adjacency.
        let Some(brace) = tokens
            .iter()
            .skip(index + 2)
            .find(|candidate| candidate.start > after_params)
        else {
            continue;
        };
        if !brace.is_punct('{') {
            continue;
        }
        let Some(close) = matching_delimiter(source, brace.start) else {
            continue;
        };
        for byte in bytes.iter_mut().take(close).skip(brace.start + 1) {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

struct Checker<'a> {
    /// Names declared as `@Styles` in this file: a call to one is not an unknown attribute.
    style_names: std::collections::HashSet<String>,
    file: String,
    source: &'a str,
    line_starts: Vec<usize>,
    out: Vec<Diagnostic>,
    /// Decorated data fields of the class being walked: name and the type the reference prints
    /// (`@State private sdkState: string` → `sdkState` → `String`). A call site on one of these
    /// names is `This expression is not callable`.
    data_fields: Vec<(String, &'static str)>,
    /// The same fields, kept for the whole file: a call site inside a `build()` body is not in
    /// the parsed tree (that body is blanked for the parser), so it is found in the token stream
    /// instead, and the name has to outlive the class walk.
    every_data_field: Vec<(String, &'static str)>,
    /// The ArkTS rules below are the *linter's*, and the reference runs the linter over `.ets`
    /// files only. A plain `.ts` file is TypeScript: `function () { … }` is legal there, and
    /// flagging it would fail a build the reference accepts (measured on
    /// `fixtures/favorite-configuration`, whose `common/utils/*.ts` were reported).
    lints: bool,
    /// Functions of this file declared without a return type. `arkts-no-implicit-return-types`
    /// fires when such a function returns a call to *another* such function: the reference can
    /// infer `1` and `"x"` but refuses to chain two inferences (`fixtures/arkts-return-types`).
    unannotated_fns: std::collections::BTreeSet<String>,
    /// Classes declared in this file. A reference to one is not an object the reference accepts
    /// (`arkts-no-classes-as-obj`, a warning).
    class_names: std::collections::BTreeSet<String>,
    /// Arguments each function of this file takes: `name -> (minimum, maximum)`.
    arity: std::collections::HashMap<String, (usize, usize)>,
    /// Locals declared with a primitive annotation: `name -> "string" | "number" | "boolean"`.
    primitives: std::collections::HashMap<String, &'static str>,
    /// Locals whose type is a number only by their initialiser: `let x = 1`.
    inferred_numbers: std::collections::HashSet<String>,
    /// Locals declared with a primitive union: `u -> [number, string]`.
    union_types: std::collections::HashMap<String, Vec<String>>,
    /// Property types of the file's interfaces: `Holder -> {count: "number"}`.
    member_types: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    /// Members each interface declares optional (`?`).
    optional_members: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// Interface types named by each function's parameters, by position.
    argument_types: std::collections::HashMap<String, Vec<String>>,
    /// Declared parameter types of the file's functions, by name.
    parameter_types: std::collections::HashMap<String, Vec<String>>,
    array_elements: std::collections::HashMap<String, String>,
    /// Names of the file's enums: a string literal is not one of their values.
    enums: std::collections::HashSet<String>,
    /// Locals the file declares as `any`/`unknown`, or as `x as any`: a function returning what
    /// they hold cannot have its return type inferred (`fixtures/arkts-any-unknown`, 1:10).
    any_locals: std::collections::HashSet<String>,
    /// Names this file declares more than once; their literals are not judged.
    merged_types: std::collections::HashSet<String>,
    /// Interfaces of this file that declare an index signature, with its offset and type. Their
    /// literals are judged by the index-signature scan, not by the member tables.
    index_signatures: std::collections::HashMap<String, (usize, String)>,

    /// Declared parameter types of the file's class methods, keyed `Class.method`.
    method_parameter_types: std::collections::HashMap<String, Vec<String>>,
    /// The declared return type of the function being walked, when it names such an interface.
    return_type: Option<String>,
    /// The primitive a function's declared return type spells, when it is one.
    return_spelling: Option<String>,
    /// Members of each class this file declares.
    members: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// Static members of each class this file declares: `class -> {name}`.
    statics: std::collections::HashMap<String, std::collections::HashSet<String>>,
    entries: std::collections::HashSet<String>,
    structs: std::collections::HashSet<String>,
    v2_classes: std::collections::HashSet<String>,
    /// Which class the walk is inside, for `this.<static>`.
    current_class: Option<String>,
    /// Locals and parameters annotated with a class of this file: `name -> class name`.
    class_locals: std::collections::HashMap<String, String>,
    /// How many arrow functions enclose the node being visited. A callback's parameter is typed by
    /// the call it is passed to, so an unannotated one there is not `any` (the reference stays
    /// silent for the `(err) => …` callbacks of `ets/EntryAbility.ets`).
    arrow_depth: usize,
    /// How many class methods enclose the node being visited. `this` is legal inside a method and
    /// rejected inside a stand-alone function (`arkts-no-standalone-this`).
    method_depth: usize,
}

impl Checker<'_> {
    /// `process_system_module.js`: an `@ohos.*` / `@system.*` request the SDK does not declare
    /// has no declaration file behind it. The reference reports it at the specifier instead of
    /// letting the runtime fail to resolve it (measured on `fixtures/ohos-missing-module`).
    /// Records `name -> class` when the annotation names a class of this file, so a member access
    /// through it can be judged (`Property 'notAReal' does not exist on type 'Holder'.`).
    fn remember_class_type(&mut self, pattern: &oxc_ast::ast::BindingPattern<'_>) {
        let (oxc_ast::ast::BindingPatternKind::BindingIdentifier(name), Some(annotation)) =
            (&pattern.kind, pattern.type_annotation.as_ref())
        else {
            return;
        };
        let oxc_ast::ast::TSType::TSTypeReference(reference) = &annotation.type_annotation else {
            return;
        };
        let oxc_ast::ast::TSTypeName::IdentifierReference(type_name) = &reference.type_name else {
            return;
        };
        if self.members.contains_key(type_name.name.as_str()) {
            // The reference prints the type as written, arguments included
            // (`Property 'notAReal' does not exist on type 'Box<number>'.`, 10:41 of
            // `fixtures/type-generic-member`).
            let mut display = type_name.name.to_string();
            if let Some(arguments) = &reference.type_arguments {
                let rendered: Vec<String> = arguments
                    .params
                    .iter()
                    .map(|argument| match argument {
                        oxc_ast::ast::TSType::TSNumberKeyword(_) => "number".to_string(),
                        oxc_ast::ast::TSType::TSStringKeyword(_) => "string".to_string(),
                        oxc_ast::ast::TSType::TSBooleanKeyword(_) => "boolean".to_string(),
                        oxc_ast::ast::TSType::TSTypeReference(reference) => {
                            match &reference.type_name {
                                oxc_ast::ast::TSTypeName::IdentifierReference(identifier) => {
                                    identifier.name.to_string()
                                }
                                _ => "_".to_string(),
                            }
                        }
                        _ => "_".to_string(),
                    })
                    .collect();
                display = format!("{display}<{}>", rendered.join(", "));
            }
            self.class_locals.insert(name.name.to_string(), display);
        }
    }

    fn check_system_request(&mut self, it: &ImportDeclaration<'_>) {
        let request = it.source.value.as_str();
        if !(request.starts_with("@ohos.") || request.starts_with("@system.")) {
            return;
        }
        if system_module_declared(request) == Some(false) {
            self.push(
                "ERROR",
                "10505001",
                it.source.span.start as usize,
                format!("Cannot find module '{request}' or its corresponding type declarations."),
            );
            return;
        }
        // `process_system_module.js`: a named import the module's declaration does not have is
        // reported at the name, with the reference's own wording (`fixtures/ohos-missing-symbol`).
        if let Some(specifiers) = &it.specifiers {
            for specifier in specifiers.iter() {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = specifier.imported.name().as_str();
                if system_module_has_symbol(request, imported) != Some(false) {
                    continue;
                }
                self.push(
                    "ERROR",
                    "10505001",
                    specifier.span.start as usize,
                    format!(
                        "Module '\"{request}\"' has no exported member '{imported}'. Did you mean to use 'import {imported} from \"{request}\"' instead?"
                    ),
                );
            }
        }
    }

    /// A diagnostic that belongs to the file rather than to a position in it.
    fn push_file_level(&mut self, severity: &'static str, code: &'static str, message: String) {
        self.out.push(Diagnostic {
            severity,
            code,
            file: self.file.clone(),
            line: 0,
            column: 0,
            message,
        });
    }

    fn push(&mut self, severity: &'static str, code: &'static str, offset: usize, message: String) {
        let (line, column) = self.position(offset);
        self.out.push(Diagnostic {
            severity,
            code,
            file: self.file.clone(),
            line,
            column,
            message,
        });
    }

    /// 1-based line, 1-based column counted in characters (the reference counts columns the way
    /// an editor shows them, not in bytes).
    fn position(&self, offset: usize) -> (usize, usize) {
        let index = match self
            .line_starts
            .binary_search_by(|start| start.cmp(&offset))
        {
            Ok(hit) => hit,
            Err(insert) => insert.saturating_sub(1),
        };
        let start = self.line_starts.get(index).copied().unwrap_or(0);
        let prefix = &self.source[start..offset.min(self.source.len())];
        (index + 1, prefix.chars().count() + 1)
    }
}

impl Checker<'_> {
    /// Judges an object literal against the interface it is declared to be: value mismatches, a
    /// member the interface does not declare (with the untyped-literal rule), and a member the
    /// literal leaves out. `anchor` is where the missing-member line points — the declarator's name
    /// (`fixtures/type-missing-member`: 6:9) or the field's name
    /// (`fixtures/type-field-shape`: 8:10).
    fn judge_literal(
        &mut self,
        spelled: &str,
        expected: &std::collections::HashMap<String, String>,
        literal: &oxc_ast::ast::ObjectExpression<'_>,
        anchor: usize,
    ) {
        // An index-signature interface answers every key, so the member tables do not describe it:
        // its literals are covered by `index_signature_literal_violations`.
        if self.index_signatures.contains_key(spelled) {
            return;
        }
        // A merged declaration has no single member list either; the reference reports the merge
        // itself and stops (`fixtures/type-declaration-merge`).
        if self.merged_types.contains(spelled) {
            return;
        }
        for (offset, message) in object_literal_violations(literal, expected) {
            self.push("ERROR", "10505001", offset, message);
        }
        let provided: std::collections::HashSet<&str> = literal
            .properties
            .iter()
            .filter_map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => match &property.key {
                    PropertyKey::StaticIdentifier(key) => Some(key.name.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let mut excess: Vec<&str> = provided
            .iter()
            .copied()
            .filter(|member| !expected.contains_key(*member))
            .collect();
        excess.sort_unstable();
        if let Some(member) = excess.first() {
            let start = literal.span.start as usize;
            let at = self
                .source
                .get(start..)
                .and_then(|rest| rest.find(member))
                .map(|found| start + found)
                .unwrap_or(start);
            self.push(
                "ERROR",
                "10505001",
                at,
                format!(
                    "Type '{}' is not assignable to type '{spelled}'. Object literal may only specify known properties, and '{member}' does not exist in type '{spelled}'.",
                    object_spelling(literal)
                ),
            );
            self.push(
                "ERROR",
                "10605038",
                start,
                "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)"
                    .to_string(),
            );
        }
        let optional = self
            .optional_members
            .get(spelled)
            .cloned()
            .unwrap_or_default();
        let mut missing: Vec<&String> = expected
            .keys()
            .filter(|member| !provided.contains(member.as_str()))
            .filter(|member| !optional.contains(member.as_str()))
            .collect();
        missing.sort();
        if let Some(member) = missing.first() {
            self.push(
                "ERROR",
                "10505001",
                anchor,
                format!(
                    "Property '{member}' is missing in type '{}' but required in type '{spelled}'.",
                    object_spelling(literal)
                ),
            );
        }
    }

    /// A cast that cannot be honest: `"s" as number` reports at the expression
    /// (`Conversion of type 'string' to type 'number' may be a mistake …`, 8:13 of
    /// `fixtures/type-union`). `as const` keeps its own rule.
    fn judge_conversion(&mut self, it: &oxc_ast::ast::TSAsExpression<'_>) {
        let found = match &it.expression {
            Expression::StringLiteral(_) => "string",
            Expression::NumericLiteral(_) => "number",
            Expression::BooleanLiteral(_) => "boolean",
            _ => return,
        };
        let wanted = match &it.type_annotation {
            TSType::TSNumberKeyword(_) => "number",
            TSType::TSStringKeyword(_) => "string",
            TSType::TSBooleanKeyword(_) => "boolean",
            _ => return,
        };
        if found == wanted {
            return;
        }
        self.push(
            "ERROR",
            "10505001",
            it.expression.span().start as usize,
            format!(
                "Conversion of type '{found}' to type '{wanted}' may be a mistake because neither type sufficiently overlaps with the other. If this was intentional, convert the expression to 'unknown' first."
            ),
        );
    }

    /// `this.name(` where `name` is a decorated data field, at the member name.
    ///
    /// The parsed tree cannot answer this: a `build()` body is blanked so the parser accepts the
    /// file at all, and that is exactly where the call site of
    /// `fixtures/member-collision` lives (line 15). The token stream has no such blind spot.
    fn scan_call_sites(&mut self) {
        if !self.lints || self.every_data_field.is_empty() {
            return;
        }
        use crate::build::lex::{tokenize, Kind};
        let tokens = tokenize(self.source);
        let mut index = 0usize;
        while index + 3 < tokens.len() {
            let ok = tokens[index].kind == Kind::Ident
                && self.source.get(tokens[index].start..tokens[index].end) == Some("this")
                && tokens[index + 1].is_punct('.')
                && tokens[index + 2].kind == Kind::Ident
                && tokens[index + 3].is_punct('(');
            if !ok {
                index += 1;
                continue;
            }
            let name = self
                .source
                .get(tokens[index + 2].start..tokens[index + 2].end);
            if let Some(name) = name {
                if let Some((_, display)) = self
                    .every_data_field
                    .iter()
                    .find(|(field, _)| field == name)
                {
                    let display = *display;
                    self.push(
                        "ERROR",
                        "10505001",
                        tokens[index + 2].start,
                        format!(
                            "This expression is not callable. Type '{display}' has no call signatures."
                        ),
                    );
                }
            }
            index += 4;
        }
    }
}

/// `arkts-no-aliases-by-index` (10605028): an indexed access type, at the type it indexes
/// (measured 11:16 of `fixtures/arkts-indexed-and-apply`).
fn indexed_access_violation(it: &oxc_ast::ast::TSIndexedAccessType<'_>) -> usize {
    it.object_type.span().start as usize
}

/// Offset of a destructured parameter's pattern (`arkts-no-destruct-params`, 10605091).
fn destructuring_parameter(it: &FormalParameter<'_>) -> Option<usize> {
    matches!(
        it.pattern.kind,
        oxc_ast::ast::BindingPatternKind::ObjectPattern(_)
            | oxc_ast::ast::BindingPatternKind::ArrayPattern(_)
    )
    .then(|| it.pattern.span().start as usize)
}

impl<'a> Visit<'a> for Checker<'_> {
    /// `for (const key in obj)` — at the `in` keyword (measured 2:18 of
    /// `fixtures/arkts-for-in`).
    fn visit_for_in_statement(&mut self, it: &oxc_ast::ast::ForInStatement<'a>) {
        if self.lints {
            let from = it.left.span().end as usize;
            let to = it.right.span().start as usize;
            let window = self.source.get(from..to).unwrap_or("");
            let at = window.find("in").map(|found| from + found).unwrap_or(from);
            self.push(
                "ERROR",
                "10605080",
                at,
                "\"for .. in\" is not supported (arkts-no-for-in)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_for_in_statement(self, it);
    }

    /// `h["count"]` on an object: ArkTS has no indexed access to fields
    /// (`arkts-no-props-by-index`, 10605029 at the object — measured 7:13 of
    /// `fixtures/arkts-props-by-index`). An array read by index is not this rule's business.
    fn visit_computed_member_expression(
        &mut self,
        it: &oxc_ast::ast::ComputedMemberExpression<'a>,
    ) {
        if self.lints && matches!(&it.expression, Expression::StringLiteral(_)) {
            {
                if let Expression::Identifier(object) = &it.object {
                    // `delete o["a"]` is the delete rule's business; the reference reports
                    // `arkts-no-delete` there and not this one (`fixtures/unsupported-syntax`).
                    let head = &self.source[..object.span.start as usize];
                    let is_delete = head.trim_end().ends_with("delete");
                    if !is_delete && !self.array_elements.contains_key(object.name.as_str()) {
                        self.push(
                            "ERROR",
                            "10605029",
                            object.span.start as usize,
                            "Indexed access is not supported for fields (arkts-no-props-by-index)"
                                .to_string(),
                        );
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_computed_member_expression(self, it);
    }

    /// `arkts-no-any-unknown`: the annotation names the type `any` or `unknown`.
    fn visit_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'a>) {
        if self.lints
            && matches!(
                it.type_annotation,
                TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
            )
        {
            let offset = it.type_annotation.span().start as usize;
            self.push(
                "ERROR",
                "10605008",
                offset,
                "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_type_annotation(self, it);
    }

    /// The two Kit-import diagnostics the reference reports (`process_kit_import.js`).
    ///
    /// A Kit is the SDK's public namespace, so an import that names a symbol the Kit does not
    /// export is a compile error rather than something the runtime discovers: the reference
    /// prints `ETS2BUNDLE_EXTERNAL_IMPORT_NAME_NOT_EXPORTED_FROM_KIT` and the console code
    /// `10505001`, at the symbol. A side-effect-only import is the other error
    /// (`..._EMPTY_IMPORT_NOT_ALLOWED_WITH_KIT`, code `10311008`), reported at the statement.
    ///
    /// The Kit's symbol table is the one the build already loaded from the SDK
    /// (`load_kit_configs`); when it is not loaded nothing is claimed, because "not in a table I
    /// do not have" is not a diagnostic.
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        let request = it.source.value.as_str();
        if request.starts_with("@kit.") {
            let empty: &[ImportDeclarationSpecifier<'a>] = &[];
            let specifiers = match &it.specifiers {
                Some(list) => list.as_slice(),
                None => empty,
            };
            if specifiers.is_empty() {
                self.push(
                    "ERROR",
                    "10311008",
                    it.span.start as usize,
                    format!(
                        "Can not use empty import(side-effect import) statement with Kit \
                         '{request}'."
                    ),
                );
            } else if let Some(table) = crate::build::stage::process_import::kit_table(request) {
                for specifier in specifiers {
                    let ImportDeclarationSpecifier::ImportSpecifier(named) = specifier else {
                        continue;
                    };
                    let name = match &named.imported {
                        ModuleExportName::IdentifierName(identifier) => {
                            identifier.name.as_str().to_string()
                        }
                        ModuleExportName::StringLiteral(literal) => {
                            literal.value.as_str().to_string()
                        }
                        // `import { a as "b" }` names a string export on the left and a local on
                        // the right; the imported name is what the Kit must export.
                        ModuleExportName::IdentifierReference(identifier) => {
                            identifier.name.as_str().to_string()
                        }
                    };
                    if table.contains_key(&name) {
                        continue;
                    }
                    self.push(
                        "ERROR",
                        "10505001",
                        named.span.start as usize,
                        format!("Module '\"{request}\"' has no exported member '{name}'."),
                    );
                }
            }
        }
        self.check_system_request(it);
        oxc_ast_visit::walk::walk_import_declaration(self, it);
    }

    /// `arkts-no-ctor-signatures-iface` (10605027): a construct signature in an interface, at the
    /// `new` (2:3 of `fixtures/arkts-iface-and-stdlib`).
    fn visit_ts_construct_signature_declaration(
        &mut self,
        it: &TSConstructSignatureDeclaration<'a>,
    ) {
        if self.lints {
            self.push(
                "ERROR",
                "10605027",
                it.span.start as usize,
                "Construct signatures are not supported in interfaces (arkts-no-ctor-signatures-iface)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_construct_signature_declaration(self, it);
    }

    /// Two more syntactic refusals, anchored where the reference anchors them
    /// (`fixtures/arkts-ctor-and-casts`): a constructor parameter that declares a field, at the
    /// `public` (2:15), and an angle-bracket cast, at the `<` (10:19).
    /// A parameter that carries a modifier declares a field (`constructor(public label: string)`),
    /// which ArkTS refuses. The parser keeps the modifiers on the parameter, so the test is the
    /// presence of one; the anchor is the parameter's first token (2:15, the `public`).
    /// `arkts-no-any-unknown` for a parameter written without a type: the reference reports the
    /// parameter's own name (measured 11:21 of `fixtures/type-implicit-any`), not the function.
    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.arrow_depth += 1;
        oxc_ast_visit::walk::walk_arrow_function_expression(self, it);
        self.arrow_depth -= 1;
    }

    fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
        self.remember_class_type(&it.pattern);
        if self.lints {
            if let Some(offset) = destructuring_parameter(it) {
                self.push(
                    "ERROR",
                    "10605091",
                    offset,
                    "Destructuring parameter declarations are not supported (arkts-no-destruct-params)"
                        .to_string(),
                );
            }
        }
        if self.lints
            && self.arrow_depth == 0
            && it.pattern.type_annotation.is_none()
            && matches!(
                it.pattern.kind,
                oxc_ast::ast::BindingPatternKind::BindingIdentifier(_)
            )
        {
            self.push(
                "ERROR",
                "10605008",
                it.pattern.span().start as usize,
                "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                    .to_string(),
            );
        }
        if self.lints
            && (it.accessibility.is_some()
                || it.readonly
                || it.r#override
                || !it.decorators.is_empty())
        {
            self.push(
                "ERROR",
                "10605025",
                it.span.start as usize,
                "Declaring fields in \"constructor\" is not supported (arkts-no-ctor-prop-decls)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_formal_parameter(self, it);
    }

    fn visit_ts_type_assertion(&mut self, it: &TSTypeAssertion<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605053",
                it.span.start as usize,
                "Only \"as T\" syntax is supported for type casts (arkts-as-casts)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_type_assertion(self, it);
    }

    /// Three more shapes, anchored where the reference anchors them
    /// (`fixtures/arkts-classes-and-enums`): the `new` of a constructor type (10:16), the first
    /// member of an enum whose members disagree in kind (3:3), and a class used as a value
    /// (13:10, a warning).
    fn visit_ts_constructor_type(&mut self, it: &TSConstructorType<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605106",
                it.span.start as usize,
                "Constructor function type is not supported (arkts-no-ctor-signatures-funcs)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_constructor_type(self, it);
    }

    fn visit_ts_enum_declaration(&mut self, it: &TSEnumDeclaration<'a>) {
        if self.lints {
            // An enum that takes a name the SDK already declares: the merge is refused, and each
            // of its members is a duplicate identifier (measured 1:1 and 2:3/3:3 of
            // `fixtures/arkts-enum-merging` with `enum Color { Red, Green }`).
            let name = it.id.name.to_string();
            if sdk_declares(&name) == Some(true) {
                self.push(
                    "ERROR",
                    "10605113",
                    it.span.start as usize,
                    "\"enum\" declaration merging is not supported (arkts-no-enum-merging)"
                        .to_string(),
                );
                for member in &it.body.members {
                    let member_name = match &member.id {
                        oxc_ast::ast::TSEnumMemberName::Identifier(identifier) => {
                            identifier.name.to_string()
                        }
                        oxc_ast::ast::TSEnumMemberName::String(name) => name.value.to_string(),
                        _ => continue,
                    };
                    self.push(
                        "ERROR",
                        "10505001",
                        member.id.span().start as usize,
                        format!("Duplicate identifier '{member_name}'."),
                    );
                }
            }
            let kind_of = |member: &oxc_ast::ast::TSEnumMember<'a>| -> u8 {
                match member.initializer.as_ref() {
                    Some(Expression::NumericLiteral(_)) => 1,
                    Some(Expression::StringLiteral(_)) => 2,
                    _ => 0,
                }
            };
            let first = it.body.members.first().map(kind_of).unwrap_or(0);
            // The rule is reported at the member that breaks the kind, not at the first one
            // (`fixtures/arkts-classes-and-enums`: `A = 1` is fine, `B = "b"` is 3:3).
            let offender = it.body.members.iter().find(|member| {
                let kind = kind_of(member);
                kind != 0 && first != 0 && kind != first
            });
            if let Some(member) = offender.filter(|_| first != 0) {
                {
                    // The parser's span for an enum member does not land on the member's name
                    // (measured: it pointed into the `enum` keyword), so the name is found in the
                    // source from the body's start instead.
                    let name = match &member.id {
                        oxc_ast::ast::TSEnumMemberName::Identifier(id) => id.name.to_string(),
                        oxc_ast::ast::TSEnumMemberName::String(literal) => {
                            literal.value.to_string()
                        }
                        _ => String::new(),
                    };
                    let from = it.body.span.start as usize;
                    let offset = self
                        .source
                        .get(from..)
                        .and_then(|rest| rest.find(&name))
                        .map(|found| from + found)
                        .unwrap_or(from);
                    self.push(
                        "ERROR",
                        "10605111",
                        offset,
                        "Enumeration members can be initialized only with compile time expressions of the same type (arkts-no-enum-mixed-types)"
                            .to_string(),
                    );
                }
            }
        }
        oxc_ast_visit::walk::walk_ts_enum_declaration(self, it);
    }

    /// `arkts-limited-stdlib` (10605144): members the reference's restricted standard library
    /// refuses. Measured on `fixtures/arkts-iface-and-stdlib` — `Object.assign` (22:10),
    /// `Object.freeze` (2:18) and `Object.create` (5:19) are refused, while `Object.entries`,
    /// `Object.getOwnPropertyNames`, `Date.parse`, `parseInt` and `JSON.stringify` are not, so the
    /// table holds exactly the measured members, anchored at the `Object` identifier.
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        // `Expected N arguments, but got M.` — anchored at the call's closing `)` (measured 8:22 of
        // `fixtures/type-call-arity`).
        if self.lints {
            if let Expression::Identifier(callee) = &it.callee {
                if let Some((minimum, maximum)) = self.arity.get(callee.name.as_str()) {
                    let given = it.arguments.len();
                    if given > *maximum || given < *minimum {
                        let expected = if *maximum == usize::MAX {
                            format!("at least {minimum}")
                        } else if minimum == maximum {
                            format!("{minimum}")
                        } else {
                            format!("{minimum}-{maximum}")
                        };
                        // The reference says "arguments" even for one, and anchors at the last
                        // argument it was given (measured 8:22 of `fixtures/type-call-arity`).
                        let offset = it
                            .arguments
                            .last()
                            .map(|argument| argument.span().start as usize)
                            .unwrap_or_else(|| it.span.end.saturating_sub(1) as usize);
                        self.push(
                            "ERROR",
                            "10505001",
                            offset,
                            format!("Expected {expected} arguments, but got {given}."),
                        );
                    }
                }
            }
        }
        // `takeHolder({ count: "x" })` where the parameter is `Holder`: at the property name
        // (measured 11:23 of `fixtures/type-object-shape`).
        if self.lints {
            if let Expression::Identifier(callee) = &it.callee {
                if let Some(types) = self.argument_types.get(callee.name.as_str()).cloned() {
                    for (index, argument) in it.arguments.iter().enumerate() {
                        let Some(type_name) = types.get(index) else {
                            continue;
                        };
                        if type_name.is_empty() {
                            continue;
                        }
                        let Some(expected) = self.member_types.get(type_name).cloned() else {
                            continue;
                        };
                        let Some(Expression::ObjectExpression(literal)) = argument.as_expression()
                        else {
                            continue;
                        };
                        for (offset, message) in object_literal_violations(literal, &expected) {
                            self.push("ERROR", "10505001", offset, message);
                        }
                    }
                }
            }
        }
        // `xs.push("s")` on a `number[]`: at the argument (measured 3:11 of `fixtures/type-array`).
        if self.lints {
            if let Expression::StaticMemberExpression(member) = &it.callee {
                if let Expression::Identifier(object) = &member.object {
                    if let Some(element) = self.array_elements.get(object.name.as_str()).cloned() {
                        if matches!(member.property.name.as_str(), "push" | "unshift") {
                            for argument in &it.arguments {
                                let Some(expression) = argument.as_expression() else {
                                    continue;
                                };
                                let found = match expression {
                                    Expression::StringLiteral(_) => "string",
                                    Expression::NumericLiteral(_) => "number",
                                    Expression::BooleanLiteral(_) => "boolean",
                                    _ => continue,
                                };
                                if found != element {
                                    self.push(
                                        "ERROR",
                                        "10505001",
                                        expression.span().start as usize,
                                        format!(
                                            "Argument of type '{found}' is not assignable to parameter of type '{element}'."
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        // `counter.bump("x")` where the method takes a `number`: at the argument (measured 12:16 of
        // `fixtures/type-return`).
        if self.lints {
            if let Expression::StaticMemberExpression(member) = &it.callee {
                if let Expression::Identifier(object) = &member.object {
                    if let Some(class) = self.class_locals.get(object.name.as_str()).cloned() {
                        let base = class.split('<').next().unwrap_or(&class).to_string();
                        let key = format!("{}.{}", base, member.property.name);
                        if let Some(types) = self.method_parameter_types.get(&key).cloned() {
                            for (index, argument) in it.arguments.iter().enumerate() {
                                let Some(expected) = types.get(index) else {
                                    continue;
                                };
                                let Some(expression) = argument.as_expression() else {
                                    continue;
                                };
                                let found = match expression {
                                    Expression::StringLiteral(_) => "string",
                                    Expression::NumericLiteral(_) => "number",
                                    Expression::BooleanLiteral(_) => "boolean",
                                    _ => continue,
                                };
                                if expected != "_" && found != expected {
                                    self.push(
                                        "ERROR",
                                        "10505001",
                                        expression.span().start as usize,
                                        format!(
                                            "Argument of type '{found}' is not assignable to parameter of type '{expected}'."
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        // A call that hands a string literal to a `number` parameter, or the other way round:
        // `Argument of type 'string' is not assignable to parameter of type 'number'.` at the
        // argument (measured 6:15 of `fixtures/type-call-argument`).
        if self.lints {
            if let Expression::Identifier(callee) = &it.callee {
                if let Some(types) = self.parameter_types.get(callee.name.as_str()).cloned() {
                    for (index, argument) in it.arguments.iter().enumerate() {
                        let Some(expected) = types.get(index) else {
                            continue;
                        };
                        let Some(expression) = argument.as_expression() else {
                            continue;
                        };
                        let found = match expression {
                            Expression::StringLiteral(_) => "string",
                            Expression::NumericLiteral(_) => "number",
                            Expression::BooleanLiteral(_) => "boolean",
                            _ => continue,
                        };
                        if expected != "_" && found != expected {
                            self.push(
                                "ERROR",
                                "10505001",
                                expression.span().start as usize,
                                format!(
                                    "Argument of type '{found}' is not assignable to parameter of type '{expected}'."
                                ),
                            );
                        }
                    }
                }
            }
        }
        // `arkts-no-func-apply-call` (10605152): `f.apply(…)` and `f.call(…)`, at the method name
        // (measured 14:26 of `fixtures/arkts-indexed-and-apply`); `f.bind(…)` is accepted.
        if self.lints {
            if let Expression::StaticMemberExpression(member) = &it.callee {
                if matches!(member.property.name.as_str(), "apply" | "call") {
                    self.push(
                        "ERROR",
                        "10605152",
                        member.property.span.start as usize,
                        "'Function.apply', 'Function.call' are not supported (arkts-no-func-apply-call)"
                            .to_string(),
                    );
                }
                // `f.bind(…)` is refused too, as a warning with no code of its own
                // (12:17 of `fixtures/arkts-indexed-and-apply`).
                if member.property.name.as_str() == "bind" {
                    self.push(
                        "WARN",
                        "-",
                        member.property.span.start as usize,
                        "'Function.bind' is not supported (arkts-no-func-bind)".to_string(),
                    );
                }
            }
        }
        if self.lints {
            if let Expression::StaticMemberExpression(member) = &it.callee {
                if let Expression::Identifier(object) = &member.object {
                    if object.name.as_str() == "Object"
                        && matches!(
                            member.property.name.as_str(),
                            "assign" | "freeze" | "create"
                        )
                    {
                        self.push(
                            "ERROR",
                            "10605144",
                            object.span.start as usize,
                            "Usage of standard library is restricted (arkts-limited-stdlib)"
                                .to_string(),
                        );
                        // `Object.assign({}, …)`: the parameters are untyped, so the literal has
                        // no contextual type either and draws the same rule a declarator would
                        // (7:24 of `fixtures/arkts-iface-and-stdlib`). An object literal passed to
                        // a typed parameter — the ordinary component call — does not.
                        if member.property.name.as_str() == "assign" {
                            for argument in &it.arguments {
                                if let Some(Expression::ObjectExpression(literal)) =
                                    argument.as_expression()
                                {
                                    self.push(
                                        "ERROR",
                                        "10605038",
                                        literal.span.start as usize,
                                        "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_call_expression(self, it);
    }

    /// `new Holder(...)` names the class as a constructor, which is allowed; every other mention
    /// of a class name is a value use.
    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        for argument in &it.arguments {
            if let Some(expression) = argument.as_expression() {
                self.visit_expression(expression);
            }
        }
    }

    /// Five type forms ArkTS does not have, each anchored where the reference anchors it
    /// (`fixtures/arkts-type-shapes`): the indexed signature's `[` (2:3), a conditional type's
    /// check type (5:23), a mapped type's `{` (7:15), the `typeof` (12:15), and an object literal
    /// used as a type (17:16).
    fn visit_ts_index_signature(&mut self, it: &TSIndexSignature<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605017",
                it.span.start as usize,
                "Indexed signatures are not supported (arkts-no-indexed-signatures)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_index_signature(self, it);
    }

    fn visit_ts_conditional_type(&mut self, it: &TSConditionalType<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605022",
                it.check_type.span().start as usize,
                "Conditional types are not supported (arkts-no-conditional-types)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_conditional_type(self, it);
    }

    fn visit_ts_mapped_type(&mut self, it: &TSMappedType<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605083",
                it.span.start as usize,
                "Mapped type expression is not supported (arkts-no-mapped-types)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_mapped_type(self, it);
    }

    fn visit_ts_type_query(&mut self, it: &TSTypeQuery<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605060",
                it.span.start as usize,
                "\"typeof\" operator is allowed only in expression contexts (arkts-no-type-query)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_type_query(self, it);
    }

    fn visit_ts_type_literal(&mut self, it: &TSTypeLiteral<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605040",
                it.span.start as usize,
                "Object literals cannot be used as type declarations (arkts-no-obj-literals-as-types)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_type_literal(self, it);
    }

    /// `arkts-no-call-signatures` (10605014): a call signature in an interface, at the `(`
    /// (measured 2:3 of `fixtures/arkts-call-signature-and-symbol`).
    fn visit_ts_call_signature_declaration(
        &mut self,
        it: &oxc_ast::ast::TSCallSignatureDeclaration<'a>,
    ) {
        if self.lints {
            self.push(
                "ERROR",
                "10605014",
                it.span.start as usize,
                "Use \"class\" instead of a type with call signature (arkts-no-call-signatures)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_call_signature_declaration(self, it);
    }

    /// `arkts-no-symbol` (10605002): the `symbol` type, at the keyword (measured 9:28 and 9:37 —
    /// the parameter's type and the return type).
    fn visit_ts_symbol_keyword(&mut self, it: &oxc_ast::ast::TSSymbolKeyword) {
        if self.lints {
            self.push(
                "ERROR",
                "10605002",
                it.span.start as usize,
                "\"Symbol()\" API is not supported (arkts-no-symbol)".to_string(),
            );
        }
    }

    /// An ambient module declaration is refused at the `declare` (10605128, measured 1:1 of
    /// `fixtures/arkts-ambient-decl`). A probe that also carried an overloaded function and a
    /// default export drew a second diagnostic — `Invalid module name in augmentation, module
    /// 'virtual' cannot be found.` at the name — that this fixture does not reproduce, so only the
    /// rule is judged until the condition behind that one is measured.
    fn visit_ts_module_declaration(&mut self, it: &oxc_ast::ast::TSModuleDeclaration<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605128",
                it.span.start as usize,
                "Ambient module declaration is not supported (arkts-no-ambient-decls)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_module_declaration(self, it);
    }

    /// `arkts-no-aliases-by-index` (10605028), reported at the indexed type.
    fn visit_ts_indexed_access_type(&mut self, it: &oxc_ast::ast::TSIndexedAccessType<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605028",
                indexed_access_violation(it),
                "Indexed access types are not supported (arkts-no-aliases-by-index)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_ts_indexed_access_type(self, it);
    }

    /// `arkts-no-as-const` (10605142): an `as const` assertion, anchored at the `as`
    /// (measured 6:28 of `fixtures/arkts-as-const`).
    fn visit_ts_as_expression(&mut self, it: &oxc_ast::ast::TSAsExpression<'a>) {
        if self.lints {
            if let TSType::TSTypeReference(reference) = &it.type_annotation {
                if matches!(&reference.type_name,
                    oxc_ast::ast::TSTypeName::IdentifierReference(name) if name.name.as_str() == "const")
                {
                    let after = it.expression.span().end as usize;
                    let offset = self
                        .source
                        .get(after..)
                        .and_then(|rest| rest.find("as"))
                        .map(|at| after + at)
                        .unwrap_or(after);
                    self.push(
                        "ERROR",
                        "10605142",
                        offset,
                        "\"as const\" assertions are not supported (arkts-no-as-const)".to_string(),
                    );
                }
            }
        }
        self.judge_conversion(it);
        oxc_ast_visit::walk::walk_ts_as_expression(self, it);
    }

    /// `arkts-no-private-identifiers` (10605003): a `#name`, at the `#` on both its declaration and
    /// its use (measured 2:3 and 5:17 of `fixtures/arkts-private-identifiers`).
    fn visit_private_field_expression(&mut self, it: &oxc_ast::ast::PrivateFieldExpression<'a>) {
        if self.lints {
            self.push(
                "ERROR",
                "10605003",
                it.field.span.start as usize,
                "Private \"#\" identifiers are not supported (arkts-no-private-identifiers)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_private_field_expression(self, it);
    }

    /// `arkts-no-untyped-obj-literals` (10605038): an object literal with no contextual type, at
    /// the `{`. Measured on `fixtures/arkts-object-shapes`: `return { ...a }` (2:10) and
    /// `const o = { … }` without an annotation (10:13); the same literal passed to a typed
    /// parameter is accepted, so only these two positions are judged.
    /// A member the primitive does not have (`Property 'notARealMethod' does not exist on type
    /// 'string'.`, 7:24 of `fixtures/type-primitive-member`).
    fn visit_static_member_expression(&mut self, it: &oxc_ast::ast::StaticMemberExpression<'a>) {
        if self.lints {
            // An element of an array local: `xs.push("s")` is refused at the argument and
            // `xs.notAReal` as a member the array does not have (3:11 and 4:13 of
            // `fixtures/type-array`).
            if let Expression::Identifier(object) = &it.object {
                if let Some(element) = self.array_elements.get(object.name.as_str()).cloned() {
                    let member = it.property.name.as_str();
                    if !ARRAY_MEMBERS.contains(&member) {
                        self.push(
                            "ERROR",
                            "10505001",
                            it.property.span.start as usize,
                            format!("Property '{member}' does not exist on type '{element}[]'."),
                        );
                    }
                }
            }
            if let Expression::Identifier(object) = &it.object {
                // `Property 'notAReal' does not exist on type 'Holder'.` — a class of this file,
                // reached through a local or a parameter annotated with it (measured 10:51 of
                // `fixtures/type-class-member`).
                if let Some(class) = self.class_locals.get(object.name.as_str()).cloned() {
                    // The table is keyed by the base name; the message shows the written type.
                    let base = class.split('<').next().unwrap_or(&class).to_string();
                    if let Some(members) = self.members.get(&base) {
                        let member = it.property.name.as_str();
                        if !members.contains(member) {
                            self.push(
                                "ERROR",
                                "10505001",
                                it.property.span.start as usize,
                                format!("Property '{member}' does not exist on type '{class}'."),
                            );
                        }
                    }
                }
                if let Some(kind) = self.primitives.get(object.name.as_str()).copied() {
                    let member = it.property.name.as_str();
                    if primitive_has_member(kind, member) == Some(false) {
                        self.push(
                            "ERROR",
                            "10505001",
                            it.property.span.start as usize,
                            format!("Property '{member}' does not exist on type '{kind}'."),
                        );
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_static_member_expression(self, it);
    }

    /// `x = "s"` where `x` is a number: `Type 'string' is not assignable to type 'number'.` at
    /// the target (measured 3:3 of `fixtures/type-assign`).
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        if self.lints {
            if let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) = &it.left {
                let numeric = self.primitives.get(target.name.as_str()) == Some(&"number")
                    || self.inferred_numbers.contains(target.name.as_str());
                if numeric && matches!(&it.right, Expression::StringLiteral(_)) {
                    self.push(
                        "ERROR",
                        "10505001",
                        target.span.start as usize,
                        "Type 'string' is not assignable to type 'number'.".to_string(),
                    );
                }
                // `u = true` against `let u: number | string` — the union is spelled in the order
                // the reference prints it, which is the declaration's reversed (measured 3:3 of
                // `fixtures/type-union`: declared `number | string`, reported `string | number`).
                if let Some(members) = self.union_types.get(target.name.as_str()).cloned() {
                    let found = match &it.right {
                        Expression::StringLiteral(_) => "string",
                        Expression::NumericLiteral(_) => "number",
                        Expression::BooleanLiteral(_) => "boolean",
                        _ => "",
                    };
                    if !found.is_empty() && !members.iter().any(|member| member == found) {
                        let mut spelled = members.clone();
                        spelled.reverse();
                        self.push(
                            "ERROR",
                            "10505001",
                            target.span.start as usize,
                            format!(
                                "Type '{found}' is not assignable to type '{}'.",
                                spelled.join(" | ")
                            ),
                        );
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_assignment_expression(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        // A type *literal* annotation declares no class or interface, so the initialiser still has
        // no contextual type: the reference reports both the type literal (10605040) and this
        // (measured at 17:16 and 17:32 of `fixtures/arkts-type-shapes`).
        self.remember_class_type(&it.id);
        if let (oxc_ast::ast::BindingPatternKind::BindingIdentifier(name), Some(initialiser)) =
            (&it.id.kind, &it.init)
        {
            // `const x: any = 1` — at the annotation (2:12), and `const y = 2 as any` at both the
            // declarator's name and the assertion's type (3:9 and 3:18 of
            // `fixtures/arkts-any-unknown`).
            if let Some(annotation) = &it.id.type_annotation {
                let is_any = matches!(
                    annotation.type_annotation,
                    TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
                );
                if is_any {
                    self.push(
                        "ERROR",
                        "10605008",
                        annotation.type_annotation.span().start as usize,
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                }
            }
            if let Some(Expression::TSAsExpression(assertion)) = &it.init {
                let is_any = matches!(
                    assertion.type_annotation,
                    TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
                );
                if is_any {
                    self.push(
                        "ERROR",
                        "10605008",
                        name.span.start as usize,
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                    self.push(
                        "ERROR",
                        "10605008",
                        assertion.type_annotation.span().start as usize,
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                }
            }
            // `const t: Tone = "s"` where `Tone` is an enum: the literal is spelled as written,
            // quotes included (`Type '"s"' is not assignable to type 'Tone'.`, 7:9 of
            // `fixtures/type-enum`).
            if let Some(annotation) = &it.id.type_annotation {
                if let TSType::TSTypeReference(reference) = &annotation.type_annotation {
                    if let oxc_ast::ast::TSTypeName::IdentifierReference(type_name) =
                        &reference.type_name
                    {
                        let spelled = type_name.name.to_string();
                        if self.enums.contains(&spelled) {
                            if let Expression::StringLiteral(literal) = initialiser {
                                self.push(
                                    "ERROR",
                                    "10505001",
                                    name.span.start as usize,
                                    format!(
                                        "Type '\"{}\"' is not assignable to type '{}'.",
                                        literal.value, spelled
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            // `const h: Holder = { count: "s" }` — at the property name (measured 10:23 of
            // `fixtures/type-object-shape`).
            if let Some(annotation) = &it.id.type_annotation {
                if let TSType::TSTypeReference(reference) = &annotation.type_annotation {
                    if let oxc_ast::ast::TSTypeName::IdentifierReference(type_name) =
                        &reference.type_name
                    {
                        let spelled = type_name.name.to_string();
                        if let Some(expected) = self.member_types.get(&spelled).cloned() {
                            if let Expression::ObjectExpression(literal) = initialiser {
                                // A member the literal leaves out is reported at the declarator's
                                // name (measured 6:9 of `fixtures/type-missing-member`); the struct
                                // field of `fixtures/type-field-shape` anchors at its own name.
                                self.judge_literal(
                                    &spelled,
                                    &expected,
                                    literal,
                                    name.span.start as usize,
                                );
                            }
                        }
                    }
                }
            }
            // `const n: number = "s"` — at the declarator's name (measured 2:9 of
            // `fixtures/type-declaration`); `const s: string = 1` at 10:9 of
            // `fixtures/type-call-argument`.
            // `let u: number | string = 1` — remember the union so a later assignment can be
            // judged against it (`fixtures/type-union`).
            if let Some(annotation) = &it.id.type_annotation {
                if let TSType::TSUnionType(union) = &annotation.type_annotation {
                    let members: Vec<String> = union
                        .types
                        .iter()
                        .filter_map(|member| match member {
                            TSType::TSNumberKeyword(_) => Some("number".to_string()),
                            TSType::TSStringKeyword(_) => Some("string".to_string()),
                            TSType::TSBooleanKeyword(_) => Some("boolean".to_string()),
                            _ => None,
                        })
                        .collect();
                    if !members.is_empty() {
                        self.union_types.insert(name.name.to_string(), members);
                    }
                }
            }
            if let Some(annotation) = &it.id.type_annotation {
                let mismatch = match (&annotation.type_annotation, initialiser) {
                    (TSType::TSNumberKeyword(_), Expression::StringLiteral(_)) => {
                        Some(("string", "number"))
                    }
                    (TSType::TSStringKeyword(_), Expression::NumericLiteral(_)) => {
                        Some(("number", "string"))
                    }
                    _ => None,
                };
                if let Some((found, expected)) = mismatch {
                    self.push(
                        "ERROR",
                        "10505001",
                        name.span.start as usize,
                        format!("Type '{found}' is not assignable to type '{expected}'."),
                    );
                }
            }
            // Only an *unannotated* declaration infers its type from the initialiser: `let u:
            // number | string = 1` is a union, not a number (`fixtures/type-union` caught this —
            // the union local drew a spurious `string is not assignable to number`).
            if it.id.type_annotation.is_none()
                && matches!(initialiser, Expression::NumericLiteral(_))
            {
                self.inferred_numbers.insert(name.name.to_string());
            }
            // `const arr: number[] = ["a"]` — the literal is refused at the `[` (measured 8:26 of
            // `fixtures/type-assign`).
            if let Some(annotation) = &it.id.type_annotation {
                if let TSType::TSArrayType(array) = &annotation.type_annotation {
                    if matches!(array.element_type, TSType::TSNumberKeyword(_)) {
                        if let Expression::ArrayExpression(literal) = initialiser {
                            let offending = literal.elements.iter().any(|element| {
                                matches!(
                                    element,
                                    oxc_ast::ast::ArrayExpressionElement::StringLiteral(_)
                                )
                            });
                            if offending {
                                // The reference anchors the offending element, not the bracket:
                                // `["a"]` is refused at the `"a"` (8:26).
                                let anchor = literal
                                    .elements
                                    .iter()
                                    .find_map(|element| match element {
                                        oxc_ast::ast::ArrayExpressionElement::StringLiteral(
                                            string,
                                        ) => Some(string.span.start as usize),
                                        _ => None,
                                    })
                                    .unwrap_or(literal.span.start as usize);
                                self.push(
                                    "ERROR",
                                    "10505001",
                                    anchor,
                                    "Type 'string' is not assignable to type 'number'.".to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        // `arkts-no-destruct-decls` (10605074): a destructured declaration, at the pattern
        // (measured 12:9 and 17:9 of `fixtures/arkts-destructuring`).
        if self.lints
            && matches!(
                it.id.kind,
                oxc_ast::ast::BindingPatternKind::ObjectPattern(_)
                    | oxc_ast::ast::BindingPatternKind::ArrayPattern(_)
            )
        {
            self.push(
                "ERROR",
                "10605074",
                it.id.span().start as usize,
                "Destructuring variable declarations are not supported (arkts-no-destruct-decls)"
                    .to_string(),
            );
        }
        if let (
            oxc_ast::ast::BindingPatternKind::BindingIdentifier(name),
            Some(oxc_ast::ast::TSType::TSStringKeyword(_)),
        ) = (
            &it.id.kind,
            it.id.type_annotation.as_ref().map(|a| &a.type_annotation),
        ) {
            self.primitives.insert(name.name.to_string(), "string");
        }
        if let (
            oxc_ast::ast::BindingPatternKind::BindingIdentifier(name),
            Some(oxc_ast::ast::TSType::TSNumberKeyword(_)),
        ) = (
            &it.id.kind,
            it.id.type_annotation.as_ref().map(|a| &a.type_annotation),
        ) {
            self.primitives.insert(name.name.to_string(), "number");
        }
        // `arkts-no-any-unknown` also covers a declaration that has to take its type from something
        // that is `any`: a call of a member the primitive does not have has no type, so the local
        // that receives it is `any` (measured 3:9 of `fixtures/type-any-cascade`, next to the
        // member error at 3:20).
        if self.lints {
            let untyped = it.id.type_annotation.is_none();
            let missing_member_call = matches!(&it.init, Some(Expression::CallExpression(call))
                if matches!(&call.callee, Expression::StaticMemberExpression(member)
                    if matches!(&member.object, Expression::Identifier(object)
                        if self
                            .primitives
                            .get(object.name.as_str())
                            .is_some_and(|kind| primitive_has_member(kind, member.property.name.as_str()) == Some(false)))));
            if untyped && missing_member_call {
                if let oxc_ast::ast::BindingPatternKind::BindingIdentifier(name) = &it.id.kind {
                    self.push(
                        "ERROR",
                        "10605008",
                        name.span.start as usize,
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                }
            }
        }
        let typed = matches!(
            it.id.type_annotation.as_ref().map(|annotation| &annotation.type_annotation),
            Some(annotation) if !matches!(annotation, TSType::TSTypeLiteral(_))
        );
        if self.lints && !typed {
            if let Some(Expression::ObjectExpression(object)) = &it.init {
                self.push(
                    "ERROR",
                    "10605038",
                    object.span.start as usize,
                    "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)".to_string(),
                );
            }
        }
        oxc_ast_visit::walk::walk_variable_declarator(self, it);
    }

    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        if self.lints {
            // `return "s"` in a function declared to return `number`: at the `return`
            // (measured 8:3 of `fixtures/type-return`).
            if let Some(expected) = self.return_spelling.clone() {
                if let Some(argument) = &it.argument {
                    let found = match argument {
                        Expression::StringLiteral(_) => "string",
                        Expression::NumericLiteral(_) => "number",
                        Expression::BooleanLiteral(_) => "boolean",
                        _ => "",
                    };
                    if !found.is_empty() && found != expected {
                        self.push(
                            "ERROR",
                            "10505001",
                            it.span.start as usize,
                            format!("Type '{found}' is not assignable to type '{expected}'."),
                        );
                    }
                }
            }
            // A class returned as a value: `arkts-no-classes-as-obj`, a warning. Only this
            // position is measured (`fixtures/arkts-classes-and-enums`, 13:10); a class named in
            // a type position (`new () => Holder`) draws nothing.
            if let Some(Expression::Identifier(id)) = &it.argument {
                if self.class_names.contains(id.name.as_str()) {
                    self.push(
                        "WARN",
                        "-",
                        id.span.start as usize,
                        "Classes cannot be used as objects (arkts-no-classes-as-obj)".to_string(),
                    );
                }
            }
            // A returned literal whose function declares a return type of this file has a
            // contextual type, so it is a typed literal like any other
            // (`fixtures/type-declaration`, where the interface answers for it).
            if let Some(Expression::ObjectExpression(object)) = &it.argument {
                if self.return_type.is_some() {
                    oxc_ast_visit::walk::walk_return_statement(self, it);
                    return;
                }
                self.push(
                    "ERROR",
                    "10605038",
                    object.span.start as usize,
                    "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)".to_string(),
                );
            }
        }
        oxc_ast_visit::walk::walk_return_statement(self, it);
    }

    /// `arkts-no-spread` (10605099): a spread inside an object literal (arrays only), at the
    /// `...`.
    fn visit_object_expression(&mut self, it: &ObjectExpression<'a>) {
        if self.lints {
            // `return { count: "s" }` against `interface Holder { count: number }` — at the
            // property name (measured 15:12 of `fixtures/type-declaration`).
            if let Some(name) = self.return_type.clone() {
                if let Some(properties) = self.member_types.get(&name).cloned() {
                    for property in &it.properties {
                        let ObjectPropertyKind::ObjectProperty(property) = property else {
                            continue;
                        };
                        let PropertyKey::StaticIdentifier(key) = &property.key else {
                            continue;
                        };
                        let Some(expected) = properties.get(key.name.as_str()) else {
                            continue;
                        };
                        let found = match &property.value {
                            Expression::StringLiteral(_) => "string",
                            Expression::NumericLiteral(_) => "number",
                            Expression::BooleanLiteral(_) => "boolean",
                            _ => continue,
                        };
                        if found != expected {
                            self.push(
                                "ERROR",
                                "10505001",
                                key.span.start as usize,
                                format!("Type '{found}' is not assignable to type '{expected}'."),
                            );
                        }
                    }
                }
            }
            for property in &it.properties {
                if let ObjectPropertyKind::SpreadProperty(spread) = property {
                    self.push(
                        "ERROR",
                        "10605099",
                        spread.span.start as usize,
                        "It is possible to spread only arrays or classes derived from arrays into the rest parameter or array literals (arkts-no-spread)"
                            .to_string(),
                    );
                }
            }
        }
        oxc_ast_visit::walk::walk_object_expression(self, it);
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        self.method_depth += 1;
        // A method's declared return type answers for a literal it returns, the same way a
        // function's does (`fixtures/type-field-assign`: 6:5).
        let previous_spelling = self.return_spelling.clone();
        if let Some(annotation) = &it.value.return_type {
            self.return_spelling = match &annotation.type_annotation {
                TSType::TSNumberKeyword(_) => Some("number".to_string()),
                TSType::TSStringKeyword(_) => Some("string".to_string()),
                TSType::TSBooleanKeyword(_) => Some("boolean".to_string()),
                _ => None,
            };
        }
        oxc_ast_visit::walk::walk_method_definition(self, it);
        self.return_spelling = previous_spelling;
        self.method_depth -= 1;
    }

    /// Two rules about functions, both anchored on the function itself:
    ///
    /// · `arkts-no-standalone-this` (10605093) — `this` in a function that is not a method, at the
    ///   `this` (15:10 in the fixture);
    /// · `arkts-no-implicit-return-types` (10605090) — a function with no return type that
    ///   returns `this` or a call to another unannotated function, at the function name (14:10).
    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        // The declared return type decides what an object literal returned here must look like.
        let previous_return = self.return_type.clone();
        let previous_spelling = self.return_spelling.clone();
        self.return_spelling = None;
        if let Some(annotation) = &it.return_type {
            match &annotation.type_annotation {
                TSType::TSNumberKeyword(_) => self.return_spelling = Some("number".to_string()),
                TSType::TSStringKeyword(_) => self.return_spelling = Some("string".to_string()),
                TSType::TSBooleanKeyword(_) => self.return_spelling = Some("boolean".to_string()),
                TSType::TSTypeReference(reference) => {
                    if let oxc_ast::ast::TSTypeName::IdentifierReference(name) =
                        &reference.type_name
                    {
                        let spelling = name.name.to_string();
                        if self.member_types.contains_key(&spelling) {
                            self.return_type = Some(spelling);
                        }
                    }
                }
                _ => {}
            }
        }
        if self.lints {
            if self.method_depth == 0 {
                for span in this_spans(it) {
                    self.push(
                        "ERROR",
                        "10605093",
                        span,
                        "Using \"this\" inside stand-alone functions is not supported (arkts-no-standalone-this)".to_string(),
                    );
                }
            }
            if it.return_type.is_none() {
                if let (Some(id), Some(body)) = (&it.id, &it.body) {
                    let any_params: std::collections::HashSet<String> = it
                        .params
                        .items
                        .iter()
                        .filter(|parameter| parameter.pattern.type_annotation.is_none())
                        .filter_map(|parameter| match &parameter.pattern.kind {
                            oxc_ast::ast::BindingPatternKind::BindingIdentifier(name) => {
                                Some(name.name.to_string())
                            }
                            _ => None,
                        })
                        .collect();
                    if returns_limited_type(
                        body,
                        &self.unannotated_fns,
                        &any_params,
                        &self.array_elements,
                        &self.any_locals,
                    ) {
                        self.push(
                            "ERROR",
                            "10605090",
                            id.span.start as usize,
                            "Function return type inference is limited (arkts-no-implicit-return-types)"
                                .to_string(),
                        );
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_function(self, it, flags);
        self.return_type = previous_return;
        self.return_spelling = previous_spelling;
    }

    /// Three rules the reference reports on constructs ArkTS does not have.
    ///
    /// · `arkts-no-delete` (10605059): the `delete` operator, at the operator;
    /// · `arkts-no-globalthis` (a warning, no code): `globalThis`, at the identifier;
    /// · `arkts-no-types-in-catch` (10605079): a type annotation on a catch parameter, at the
    ///   `:`; the same file also draws the type system's own error, at the type itself.
    ///
    /// Measured on `fixtures/unsupported-syntax`: 2:3, 6:3, 12:12 and 12:15.
    fn visit_unary_expression(&mut self, it: &oxc_ast::ast::UnaryExpression<'a>) {
        if self.lints && it.operator == UnaryOperator::Delete {
            self.push(
                "ERROR",
                "10605059",
                it.span.start as usize,
                "\"delete\" operator is not supported (arkts-no-delete)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_unary_expression(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if self.lints && it.name.as_str() == "globalThis" {
            self.push(
                "WARN",
                "-",
                it.span.start as usize,
                "\"globalThis\" is not supported (arkts-no-globalthis)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_identifier_reference(self, it);
    }

    fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
        if self.lints {
            if let Some(parameter) = &it.param {
                if let Some(annotation) = &parameter.pattern.type_annotation {
                    // The reference anchors the rule at the parameter itself (`e`), one column
                    // before the `:` the annotation span starts at.
                    self.push(
                        "ERROR",
                        "10605079",
                        parameter.pattern.span().start as usize,
                        "Type annotation in catch clause is not supported (arkts-no-types-in-catch)"
                            .to_string(),
                    );
                    // The type system rejects the annotation too, unless it names `any` or
                    // `unknown`; the reference prints both diagnostics.
                    let allowed = matches!(
                        annotation.type_annotation,
                        TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
                    );
                    if !allowed {
                        self.push(
                            "ERROR",
                            "10505001",
                            annotation.type_annotation.span().start as usize,
                            "Catch clause variable type annotation must be 'any' or 'unknown' if specified."
                                .to_string(),
                        );
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_catch_clause(self, it);
    }

    /// `arkts-no-var`: a `var` declaration, which ArkTS rejects in favour of `let`.
    /// Measured on `fixtures/var-declaration` (11:1, the keyword).
    fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
        if self.lints && it.kind == VariableDeclarationKind::Var {
            self.push(
                "ERROR",
                "10605005",
                it.span.start as usize,
                "Use \"let\" instead of \"var\" (arkts-no-var)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_variable_declaration(self, it);
    }

    /// A decorated field and a method with the same name: `@State sdkState` and `sdkState()`.
    /// The state field lowers to a `get`/`set` pair, so the runtime keeps a value where the call
    /// site expects a function (`sdkState is not callable`, 2026-11-12).
    ///
    /// The reference reports `Duplicate identifier` once per declaration, at each name.
    /// Measured on `fixtures/member-collision` (7:18 the field, 9:11 the method).
    fn visit_class(&mut self, class: &Class<'a>) {
        let previous_class = self.current_class.clone();
        if let Some(id) = &class.id {
            self.current_class = Some(id.name.to_string());
            // `10905114`: a `@Builder` method outside a struct, anchored at the decorator's name
            // (measured 2:4 of `fixtures/ui-builder-scope`).
            if !self.structs.contains(id.name.as_str()) {
                for element in &class.body.body {
                    let ClassElement::MethodDefinition(method) = element else {
                        continue;
                    };
                    for decorator in &method.decorators {
                        if let Expression::Identifier(name) = &decorator.expression {
                            if name.name.as_str() == "Builder" {
                                self.push(
                                    "ERROR",
                                    "10905114",
                                    decorator.span.start as usize + 1,
                                    "Use the '@Builder' decorator only in the global scope or in a struct."
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        // `@Entry` is what makes the page-level and body-level rules apply to this structure.
        let entry = class.decorators.iter().any(|decorator| {
            matches!(
                &decorator.expression,
                Expression::Identifier(name) if name.name.as_str() == "Entry"
            )
        });
        let component = class.decorators.iter().any(|decorator| {
            matches!(
                &decorator.expression,
                Expression::Identifier(name) if name.name.as_str() == "Component"
            )
        });
        // A `@ComponentV2` struct is a struct too: its V1-decorator complaint is `10905339`, and
        // `10905337` must not answer for it (`fixtures/ui-v1-decorator-in-v2`).
        let v2 = class.decorators.iter().any(|decorator| {
            matches!(
                &decorator.expression,
                Expression::Identifier(name) if name.name.as_str() == "ComponentV2"
            )
        });
        if self.lints && !component && !v2 {
            // `10905337`: a state decorator belongs to a `struct`, and this class is not one
            // (measured 2:4 of `fixtures/ui-decorator-on-class`, at the decorator's name).
            for element in &class.body.body {
                let ClassElement::PropertyDefinition(property) = element else {
                    continue;
                };
                for decorator in &property.decorators {
                    let Expression::Identifier(name) = &decorator.expression else {
                        continue;
                    };
                    // Measured for `@State`, `@Prop`, `@Link` and `@Provide`, each with its own name
                    // in the message (`fixtures/ui-state-decorator-on-class`).
                    if !matches!(name.name.as_str(), "State" | "Prop" | "Link" | "Provide") {
                        continue;
                    }
                    self.push(
                        "ERROR",
                        "10905337",
                        name.span.start as usize,
                        format!(
                            "The '@{}' decorator can only be used with 'struct'.",
                            name.name
                        ),
                    );
                }
            }
        }
        if self.lints {
            // `validate_ui_syntax`: a `@Component` struct needs exactly one `build` method,
            // reported at the struct name (`fixtures/ui-build-method`: 10905103, 2:8). The masked
            // view turns `struct X` into `class X`, so the test is a decorated class without one.
            if component {
                let builds = class
                    .body
                    .body
                    .iter()
                    .filter(|element| {
                        matches!(
                            element,
                            ClassElement::MethodDefinition(method)
                                if method.key.static_name().as_deref() == Some("build")
                        )
                    })
                    .count();
                if builds != 1 {
                    if let Some(id) = &class.id {
                        self.push(
                            "ERROR",
                            "10905103",
                            id.span.start as usize,
                            format!(
                                "The struct '{}' must have at least and at most one 'build' method.",
                                id.name
                            ),
                        );
                    }
                }
            }
        }

        let base = self.data_fields.len();
        if self.lints {
            let mut fields: Vec<(String, u32)> = Vec::new();
            let mut methods: Vec<(String, u32)> = Vec::new();
            for element in &class.body.body {
                match element {
                    ClassElement::PropertyDefinition(property)
                        if !property.decorators.is_empty() =>
                    {
                        if let PropertyKey::StaticIdentifier(ident) = &property.key {
                            fields.push((ident.name.to_string(), ident.span.start));
                        }
                    }
                    ClassElement::MethodDefinition(method)
                        if method.kind == MethodDefinitionKind::Method =>
                    {
                        if let PropertyKey::StaticIdentifier(ident) = &method.key {
                            methods.push((ident.name.to_string(), ident.span.start));
                        }
                    }
                    _ => {}
                }
            }
            // The call-site rule needs the types as the reference prints them.
            for element in &class.body.body {
                if let ClassElement::PropertyDefinition(property) = element {
                    if property.decorators.is_empty() {
                        continue;
                    }
                    let (PropertyKey::StaticIdentifier(ident), Some(annotation)) =
                        (&property.key, &property.type_annotation)
                    else {
                        continue;
                    };
                    if let Some(primitive) = primitive_name(&annotation.type_annotation) {
                        let display = match primitive {
                            "number" => "Number",
                            "string" => "String",
                            _ => "Boolean",
                        };
                        self.data_fields.push((ident.name.to_string(), display));
                        self.every_data_field
                            .push((ident.name.to_string(), display));
                    }
                }
            }
            for (name, field_start) in &fields {
                let Some((_, method_start)) = methods.iter().find(|(method, _)| method == name)
                else {
                    continue;
                };
                let message = format!("Duplicate identifier '{name}'.");
                self.push("ERROR", "10505001", *field_start as usize, message.clone());
                self.push("ERROR", "10505001", *method_start as usize, message);
            }
        } else {
            // A file that carries no lints still walks; the call-site stack stays empty.
        }

        // `10905209` / `10905210`: the body of an `@Entry` component's `build`. A declaration
        // statement is not UI syntax, and the body needs exactly one root node — measured as
        // 5:5 for the declaration and 12:12 for two roots (`fixtures/ui-build-statements`,
        // `fixtures/ui-build-roots`).
        if self.lints && (entry || component) {
            for element in &class.body.body {
                let ClassElement::MethodDefinition(method) = element else {
                    continue;
                };
                if method.key.static_name().as_deref() != Some("build") {
                    continue;
                }
                let from = method.span.start as usize;
                let Some(brace) = self.source[from..].find('{').map(|at| from + at) else {
                    continue;
                };
                let Some(body) = build_body(self.source, brace) else {
                    continue;
                };
                let brace_end = body.brace + 1 + body.span_len;
                for offset in &body.declarations {
                    self.push(
                        "ERROR",
                        "10905209",
                        *offset,
                        "Only UI component syntax can be written here.".to_string(),
                    );
                }
                let builders: std::collections::HashSet<String> = class
                    .body
                    .body
                    .iter()
                    .filter_map(|element| {
                        let ClassElement::MethodDefinition(method) = element else {
                            return None;
                        };
                        let builder = method.decorators.iter().any(|decorator| {
                            matches!(
                                &decorator.expression,
                                Expression::Identifier(name) if name.name.as_str() == "Builder"
                            )
                        });
                        if !builder {
                            return None;
                        }
                        method.key.static_name().map(|name| name.to_string())
                    })
                    .collect();
                for (offset, quoted) in
                    non_component_statements(self.source, (body.brace, brace_end), &builders)
                {
                    self.push(
                        "ERROR",
                        "10905204",
                        offset,
                        format!("'{quoted}' does not meet UI component syntax."),
                    );
                }
                {
                    let blocks = component_blocks(self.source, (body.brace, brace_end));
                    let known = known_components();
                    for (offset, parent, allowed) in restricted_children_violations(
                        self.source,
                        (body.brace, brace_end),
                        &blocks,
                        &known,
                    ) {
                        self.push(
                            "ERROR",
                            "10905219",
                            offset,
                            format!(
                                "The component '{parent}' can only have the child component '{allowed}'."
                            ),
                        );
                    }
                }
                for (offset, child, allowed) in
                    nesting_violations(self.source, (body.brace, brace_end))
                {
                    self.push(
                        "ERROR",
                        "10905201",
                        offset,
                        format!(
                            "The '{child}' component can only be nested in the '{allowed}' parent component."
                        ),
                    );
                }
                for statement in &body.statements {
                    // `10905202`: a `Button` whose label argument is a string (or a `$r`/`Resource`
                    // call) cannot also carry children. Reported at the `Button`
                    // (measured 6:7 of `fixtures/ui-button-label`); the call is looked for
                    // anywhere in the statement, since a nested component is not a top-level one.
                    for offset in button_label_with_children(self.source, *statement) {
                        self.push(
                            "ERROR",
                            "10905202",
                            offset,
                            "The Button component with a label parameter can not have any child."
                                .to_string(),
                        );
                    }
                    for (offset, attribute, type_name) in
                        attribute_violations(self.source, *statement)
                    {
                        if self.style_names.contains(&attribute) {
                            continue;
                        }
                        self.push(
                            "ERROR",
                            "10505001",
                            offset,
                            format!("Property '{attribute}' does not exist on type '{type_name}'."),
                        );
                    }
                }
                if body.nodes != 1 {
                    self.push(
                        "ERROR",
                        "10905210",
                        // The reference anchors one character past the `{` (measured 4:12 on
                        // `fixtures/ui-build-roots`, where the `{` itself is column 11).
                        body.brace + 1,
                        "In an '@Entry' decorated component, the 'build' method can have only one root node, which must be a container component."
                            .to_string(),
                    );
                }
            }
        }
        oxc_ast_visit::walk::walk_class(self, class);
        self.current_class = previous_class;
        self.data_fields.truncate(base);
    }

    /// `arkts-no-func-expressions`: a `function () { … }` expression, which ArkTS rejects in
    /// favour of an arrow function. Measured on `fixtures/func-expression`.
    fn visit_expression(&mut self, it: &Expression<'a>) {
        if !self.lints {
            oxc_ast_visit::walk::walk_expression(self, it);
            return;
        }
        // `this.sdkState()` where `sdkState` is a decorated data field: the field wins at run
        // time, so the call site is calling a value. Position is the member name, the message is
        // the reference's (`fixtures/member-collision`: 15:17).
        if let Expression::CallExpression(call) = it {
            if let Expression::StaticMemberExpression(member) = &call.callee {
                if matches!(member.object, Expression::ThisExpression(_)) {
                    if let Some((_, display)) = self
                        .data_fields
                        .iter()
                        .find(|(name, _)| *name == member.property.name.as_str())
                    {
                        self.push(
                            "ERROR",
                            "10505001",
                            member.property.span.start as usize,
                            format!(
                                "This expression is not callable. Type '{display}' has no call signatures."
                            ),
                        );
                    }
                }
            }
        }
        if let Expression::FunctionExpression(function) = it {
            self.push(
                "ERROR",
                "10605046",
                function.span.start as usize,
                "Use arrow functions instead of function expressions (arkts-no-func-expressions)"
                    .to_string(),
            );
        }
        oxc_ast_visit::walk::walk_expression(self, it);
    }

    /// The assignability slice: a property whose declared type and literal initialiser disagree.
    fn visit_property_definition(&mut self, it: &PropertyDefinition<'a>) {
        // `arkts-no-private-identifiers` (10605003): a `#name` field, at the `#`
        // (measured 2:3 of `fixtures/arkts-private-identifiers`).
        if self.lints {
            if let PropertyKey::PrivateIdentifier(name) = &it.key {
                self.push(
                    "ERROR",
                    "10605003",
                    name.span.start as usize,
                    "Private \"#\" identifiers are not supported (arkts-no-private-identifiers)"
                        .to_string(),
                );
            }
        }
        if self.lints {
            // A field whose annotation names an interface and whose initialiser is an object
            // literal: judged like a declaration (`fixtures/type-field-shape`: 8:10).
            if let Some(annotation) = &it.type_annotation {
                if let TSType::TSTypeReference(reference) = &annotation.type_annotation {
                    if let oxc_ast::ast::TSTypeName::IdentifierReference(type_name) =
                        &reference.type_name
                    {
                        let spelled = type_name.name.to_string();
                        if let Some(expected) = self.member_types.get(&spelled).cloned() {
                            if let Some(Expression::ObjectExpression(literal)) = &it.value {
                                let anchor = it.key.span().start as usize;
                                self.judge_literal(&spelled, &expected, literal, anchor);
                            }
                        }
                    }
                }
            }
            // `10905313`: a `static` field of a struct cannot carry a built-in decorator
            // (measured 4:3 of `fixtures/ui-static-state`, at the decorator).
            let in_struct = self
                .current_class
                .as_ref()
                .is_some_and(|class| self.structs.contains(class));
            // `10905337` (`The '@Prop' decorator can only be used with 'struct'.`) answers a
            // built-in decorator on a plain class instead, so the struct-only rules below stay
            // out of that case (`fixtures/ui-state-decorator-on-class`).
            if !it.decorators.is_empty() && it.r#static && in_struct {
                if let Some(decorator) = it.decorators.first() {
                    self.push(
                        "ERROR",
                        "10905313",
                        decorator.span.start as usize,
                        "The static variable of struct cannot be used together with built-in decorators."
                            .to_string(),
                    );
                }
            }
            // `10905304`: `@Link` takes no default value (measured 6:9 of `fixtures/ui-static-state`,
            // at the property name).
            let link = it.decorators.iter().any(|decorator| {
                matches!(&decorator.expression, Expression::Identifier(name) if name.name.as_str() == "Link")
            });
            if link && it.value.is_some() && in_struct {
                self.push(
                    "ERROR",
                    "10905304",
                    it.key.span().start as usize,
                    "The '@Link' property cannot be specified a default value.".to_string(),
                );
            }
            // `10905301`: `@Watch('name')` has to name a method of the struct (measured 4:17 of
            // `fixtures/ui-watch-name`, at the string).
            for decorator in &it.decorators {
                let Expression::CallExpression(call) = &decorator.expression else {
                    continue;
                };
                let Expression::Identifier(callee) = &call.callee else {
                    continue;
                };
                if callee.name.as_str() != "Watch" {
                    continue;
                }
                let Some(argument) = call.arguments.first() else {
                    continue;
                };
                let oxc_ast::ast::Argument::StringLiteral(literal) = argument else {
                    continue;
                };
                let Some(class) = self.current_class.clone() else {
                    continue;
                };
                let known = self
                    .members
                    .get(&class)
                    .is_some_and(|set| set.contains(literal.value.as_str()));
                if !known {
                    self.push(
                        "ERROR",
                        "10905301",
                        literal.span.start as usize,
                        format!(
                            "'@Watch' cannot be used with '{}'. Apply it only to parameters that correspond to existing methods.",
                            literal.value
                        ),
                    );
                }
            }
            // `10905339`: the mirror rule — a V1 decorator belongs to an `@Component` struct
            // (measured 4:4 of `fixtures/ui-v1-decorator-in-v2`, at the decorator's name).
            let in_v2 = self
                .current_class
                .as_ref()
                .is_some_and(|class| self.v2_classes.contains(class));
            if in_v2 {
                for decorator in &it.decorators {
                    if let Expression::Identifier(name) = &decorator.expression {
                        if matches!(
                            name.name.as_str(),
                            "State"
                                | "Link"
                                | "Prop"
                                | "Provide"
                                | "Consume"
                                | "ObjectLink"
                                | "StorageLink"
                                | "StorageProp"
                        ) {
                            self.push(
                                "ERROR",
                                "10905339",
                                decorator.span.start as usize + 1,
                                format!(
                                    "The '@{}' decorator can only be used in a 'struct' decorated with '@Component'.",
                                    name.name
                                ),
                            );
                        }
                    }
                }
            }
            // `10905338`: a V2-only decorator belongs to an `@ComponentV2` struct (measured 4:4 and
            // 6:4 of `fixtures/ui-v2-decorators`, at the decorator's name).
            let v2_only = self
                .current_class
                .as_ref()
                .is_some_and(|class| !self.v2_classes.contains(class));
            if v2_only {
                for decorator in &it.decorators {
                    if let Expression::Identifier(name) = &decorator.expression {
                        if matches!(
                            name.name.as_str(),
                            "Param"
                                | "Local"
                                | "Once"
                                | "Event"
                                | "Monitor"
                                | "Computed"
                                | "Provider"
                                | "Consumer"
                        ) {
                            self.push(
                                "ERROR",
                                "10905338",
                                decorator.span.start as usize + 1,
                                format!(
                                    "The '@{}' decorator can only be used in a 'struct' decorated with '@ComponentV2'.",
                                    name.name
                                ),
                            );
                        }
                    }
                }
            }
            // `10905303`: a `@State` field needs a default value (measured 8:10 of
            // `fixtures/ui-state-default`, at the name).
            let state = it.decorators.iter().any(|decorator| {
                matches!(&decorator.expression, Expression::Identifier(name) if name.name.as_str() == "State")
            });
            if state && it.value.is_none() && in_struct {
                if let Some(name) = it.key.static_name() {
                    self.push(
                        "ERROR",
                        "10905303",
                        it.key.span().start as usize,
                        format!(
                            "The '@State' property '{name}' must be specified a default value."
                        ),
                    );
                }
            }
            // A page takes no `@Link` at all (measured 6:9 of `fixtures/ui-static-state`, a
            // warning on the same name as the default-value error above).
            if link {
                if let Some(class) = self.current_class.clone() {
                    if self.entries.contains(&class) {
                        if let Some(name) = it.key.static_name() {
                            self.push(
                                "WARN",
                                "-",
                                it.key.span().start as usize,
                                format!(
                                    "The '@Entry' component '{class}' cannot have the '@Link' property '{name}'."
                                ),
                            );
                        }
                    }
                }
            }
        }
        // `arkts-no-definite-assignment`: `value!: number`. The reference reports it as a
        // warning, at the property, and the build still produces a package.
        if it.definite && self.lints {
            self.push(
                "WARN",
                "-",
                it.span.start as usize,
                "Definite assignment assertions are not supported (arkts-no-definite-assignment)"
                    .to_string(),
            );
        }
        if let (Some(annotation), Some(value)) = (&it.type_annotation, &it.value) {
            let declared = primitive_name(&annotation.type_annotation);
            let given = literal_name(value);
            if let (Some(declared), Some(given)) = (declared, given) {
                if declared != given {
                    let offset = it.key.span().start as usize;
                    self.push(
                        "ERROR",
                        "10505001",
                        offset,
                        format!("Type '{given}' is not assignable to type '{declared}'."),
                    );
                }
            }
        }
        oxc_ast_visit::walk::walk_property_definition(self, it);
    }
}

/// The primitive a type keyword names, when the annotation is one.
fn primitive_name(ts_type: &TSType<'_>) -> Option<&'static str> {
    match ts_type {
        TSType::TSNumberKeyword(_) => Some("number"),
        TSType::TSStringKeyword(_) => Some("string"),
        TSType::TSBooleanKeyword(_) => Some("boolean"),
        _ => None,
    }
}

/// The primitive a literal initialiser carries, when it is one.
fn literal_name(value: &Expression<'_>) -> Option<&'static str> {
    match value {
        Expression::StringLiteral(_) => Some("string"),
        Expression::NumericLiteral(_) => Some("number"),
        Expression::BooleanLiteral(_) => Some("boolean"),
        _ => None,
    }
}

/// `Property '<attribute>' does not exist on type '<Component>Attribute'.`
///
/// Measured on a `Text(...).notARealAttribute(1)` chain (8:17) and on a `Column() { … }
/// .alsoNotAReal(2)` chain (10:6): the head identifier of the statement names the component, and
/// every `.name(` after it has to be an attribute that component accepts or inherits.
fn attribute_violations(source: &str, statement: (usize, usize)) -> Vec<(usize, String, String)> {
    let (start, end) = statement;
    let text = &source[start..end];
    let head: String = text
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
        .collect();
    if head.is_empty() {
        return Vec::new();
    }
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    // The component a `.name(` belongs to: the last component call at the same nesting depth, or
    // the statement's head when the chain sits outside every block. `Text(…).notAReal(1)` inside a
    // `Column` block is `Text`'s (8:17 of the measurement), while `.alsoNotAReal(2)` after the
    // block's `}` is `Column`'s (10:6).
    let mut depth = 0usize;
    let mut last_component = head.clone();
    let mut last_component_depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => depth = depth.saturating_sub(1),
            b'.' => {
                let name_start = index + 1;
                let mut cursor = name_start;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric()
                        || bytes[cursor] == b'_'
                        || bytes[cursor] == b'$')
                {
                    cursor += 1;
                }
                let name = &text[name_start..cursor];
                if !name.is_empty() && bytes.get(cursor) == Some(&b'(') {
                    // An attribute attaches to the component call at the *same* nesting depth;
                    // a `.name(` deeper than that sits inside the call's arguments
                    // (`Text(this.sdkState())` is not `Text`'s attribute).
                    // At depth zero the chain belongs to the statement's head (`Column() { … }
                    // .alsoNotReal(2)`); deeper, it belongs to the component call at that same
                    // depth, and a `.name(` deeper still is inside that call's arguments
                    // (`Text(this.sdkState())` is not `Text`'s attribute).
                    let owner = if depth == 0 {
                        head.clone()
                    } else if depth == last_component_depth {
                        last_component.clone()
                    } else {
                        index = cursor;
                        continue;
                    };
                    if component_accepts(&owner, name) == Some(false) {
                        found.push((
                            start + name_start,
                            name.to_string(),
                            format!("{owner}Attribute"),
                        ));
                    }
                    index = cursor;
                    continue;
                }
                index = name_start;
                continue;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' => {
                let name_start = index;
                let mut cursor = index;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric()
                        || bytes[cursor] == b'_'
                        || bytes[cursor] == b'$')
                {
                    cursor += 1;
                }
                let name = &text[name_start..cursor];
                if bytes.get(cursor) == Some(&b'(') {
                    last_component = name.to_string();
                    last_component_depth = depth;
                }
                index = cursor;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    found
}

/// Names the SDK restricts to specific parents, whoever nests them.
fn parent_restricted_names() -> Vec<String> {
    COMPONENT_ATTRIBUTES.with(|cell| {
        let table = cell.borrow();
        let Some(tables) = table.as_ref() else {
            return Vec::new();
        };
        tables.parents.keys().cloned().collect()
    })
}

/// `10905219`: a component whose children the SDK restricts, given a child it does not accept.
///
/// Reported at the parent, with the accepted children listed (`fixtures/ui-restricted-children`:
/// `Text` holding a `Text`, 13:7).
fn restricted_children_violations(
    source: &str,
    body: (usize, usize),
    blocks: &[(String, usize, usize)],
    known: &std::collections::HashSet<String>,
) -> Vec<(usize, String, String)> {
    let mut found = Vec::new();
    // `component_blocks` reports offsets relative to the body text; the caller's window starts
    // there too, so both are taken from the same slice.
    let (start, end) = body;
    let text = &source[start..end];
    for (parent, block_start, block_end) in blocks {
        let Some(allowed) = component_children(parent) else {
            continue;
        };
        let inner = &text[*block_start..*block_end];
        let bytes = inner.as_bytes();
        let mut index = 0usize;
        let mut offending = false;
        while index < bytes.len() {
            if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
                index += 1;
                continue;
            }
            let name_start = index;
            let mut cursor = index;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric()
                    || bytes[cursor] == b'_'
                    || bytes[cursor] == b'$')
            {
                cursor += 1;
            }
            let name = &inner[name_start..cursor];
            if bytes.get(cursor) == Some(&b'(')
                && known.contains(name)
                && !allowed.iter().any(|a| a == name)
            {
                offending = true;
                break;
            }
            index = cursor;
        }
        if offending {
            // The reference anchors at the parent's own name: the last `Name(` before the block
            // opens (`fixtures/ui-restricted-children`: 11:7).
            let before = &text[..*block_start];
            let offset = match before.rfind(&format!("{parent}(")) {
                Some(at) => start + at,
                None => start + *block_start,
            };
            found.push((offset, parent.clone(), allowed.join(",")));
        }
    }
    let _ = body;
    found
}

/// `10905201`: a component nested where only some parents are allowed.
///
/// The restriction comes from the SDK (`components/*.json` has `parents` for 18 components, `Span`
/// among them), and the reference reports the child at its own identifier with the allowed parents
/// listed (`fixtures/ui-nested-parent`: `Span` in a `Column`, 8:7).
/// Every `Name(…) { … }` block in a body: `(name, block_start, block_end)`, offsets absolute.
fn component_blocks(source: &str, body: (usize, usize)) -> Vec<(String, usize, usize)> {
    let (start, end) = body;
    let text = &source[start..end];
    let bytes = text.as_bytes();
    let mut blocks: Vec<(String, usize, usize)> = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
            index += 1;
            continue;
        }
        let name_start = index;
        let mut cursor = index;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric()
                || bytes[cursor] == b'_'
                || bytes[cursor] == b'$')
        {
            cursor += 1;
        }
        let name = &text[name_start..cursor];
        if bytes.get(cursor) != Some(&b'(') {
            index = cursor;
            continue;
        }
        let mut depth = 1usize;
        let mut scan = cursor + 1;
        while scan < bytes.len() && depth > 0 {
            match bytes[scan] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            scan += 1;
        }
        let after = text[scan.min(text.len())..].trim_start();
        if after.starts_with('{') {
            let block_start = scan + (text[scan.min(text.len())..].len() - after.len());
            let mut depth = 0usize;
            let mut close = block_start;
            while close < bytes.len() {
                match bytes[close] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                close += 1;
            }
            blocks.push((name.to_string(), block_start, close));
        }
        index = cursor;
    }
    blocks
}

/// `10905201`: a component nested where only some parents are allowed.
fn nesting_violations(source: &str, body: (usize, usize)) -> Vec<(usize, String, String)> {
    let (start, end) = body;
    let text = &source[start..end];
    let bytes = text.as_bytes();
    let blocks = component_blocks(source, body);
    let mut found = Vec::new();
    // A child is looked for by name anywhere in the body: a leaf component (`Span("x")`) opens no
    // block of its own, so walking the blocks alone misses it.
    let restricted: Vec<String> = blocks
        .iter()
        .map(|(name, _, _)| name.clone())
        .chain(parent_restricted_names())
        .collect();
    for name in restricted {
        let Some(allowed) = component_parents(&name) else {
            continue;
        };
        let mut index = 0usize;
        while let Some(at) = text[index..].find(&format!("{name}(")) {
            let position = index + at;
            let before_is_word = position
                .checked_sub(1)
                .and_then(|previous| bytes.get(previous))
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'.');
            index = position + name.len() + 1;
            if before_is_word {
                continue;
            }
            let parent = blocks
                .iter()
                .filter(|(_, inner_start, inner_end)| {
                    inner_start < &position && position < *inner_end
                })
                .max_by_key(|(_, inner_start, _)| *inner_start)
                .map(|(parent, _, _)| parent.clone());
            let Some(parent) = parent else {
                continue;
            };
            if allowed.iter().any(|name| name == &parent) {
                continue;
            }
            found.push((start + position, name.clone(), allowed.join(",")));
        }
    }
    found
}

/// `10905204`: a statement in a `build()` body that is neither a component call nor a builder
/// call, quoted as the reference quotes it (`'this.helper();' does not meet UI component syntax.`,
/// 10:7 of `fixtures/ui-non-component-statement`).
///
/// Only a `this.<method>(…)` statement is judged, and only when that method carries no `@Builder`:
/// a builder call is UI syntax, and telling every other statement apart (an `if`, a `ForEach`, a
/// component the SDK grew after this table was read) needs the loader's own kind table. The
/// statement is looked for anywhere in the body, since a nested component's statements are not
/// top-level ones.
fn non_component_statements(
    source: &str,
    body: (usize, usize),
    builders: &std::collections::HashSet<String>,
) -> Vec<(usize, String)> {
    let (start, end) = body;
    let text = &source[start..end];
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0usize;
    while let Some(at) = text[index..].find("this.") {
        let position = index + at;
        index = position + "this.".len();
        // A statement position: the nearest code before it opened or closed a block, or ended a
        // statement.
        let before = text[..position]
            .trim_end_matches(|c: char| c.is_whitespace())
            .len();
        let at_statement =
            before == 0 || matches!(bytes.get(before - 1), Some(b'{') | Some(b'}') | Some(b';'));
        if !at_statement {
            continue;
        }
        let rest = &text[index..];
        let method: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if method.is_empty() || builders.contains(&method) {
            continue;
        }
        let Some(close) = text[index..].find(';') else {
            continue;
        };
        let quoted = text[position..index + close + 1].trim().to_string();
        found.push((start + position, quoted));
    }
    found
}

/// Offsets of `Button("label") { … }` inside a statement: the label makes children illegal.
fn button_label_with_children(source: &str, statement: (usize, usize)) -> Vec<usize> {
    let (start, end) = statement;
    let text = &source[start..end];
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0usize;
    while let Some(at) = text[index..].find("Button(") {
        let name_start = index + at;
        let before_is_word = name_start
            .checked_sub(1)
            .and_then(|previous| bytes.get(previous))
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'.');
        let mut cursor = name_start + "Button(".len();
        if before_is_word {
            index = cursor;
            continue;
        }
        let arguments = text[cursor..].trim_start();
        let labelled = arguments.starts_with('"')
            || arguments.starts_with('\'')
            || arguments.starts_with("$r(")
            || arguments.starts_with("Resource(");
        // Walk to the matching `)` of the call, then look for a block after it.
        let mut depth = 1usize;
        while cursor < bytes.len() && depth > 0 {
            match bytes[cursor] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            cursor += 1;
        }
        let after = text[cursor.min(text.len())..].trim_start();
        if labelled && after.starts_with('{') {
            found.push(start + name_start);
        }
        index = cursor;
    }
    found
}

/// What a `build()` body holds: its top-level nodes, and the declaration statements among them.
struct BuildBody {
    /// Offset of the body's `{`.
    brace: usize,
    /// Length of the body between the braces.
    span_len: usize,
    /// `(start, end)` of every top-level statement that is not a declaration.
    statements: Vec<(usize, usize)>,
    /// One offset per top-level statement that is a declaration (`10905209`, at its first token).
    declarations: Vec<usize>,
    /// How many top-level statements are not declarations (`10905210` wants exactly one).
    nodes: usize,
}

/// Reads one `build()` body from the source text.
///
/// The body is the loader's own spelling — `Column() { … }` is not something a TypeScript parser
/// accepts — so it is walked as text: the statement boundaries are the `;` at nesting depth zero
/// and a closing brace at depth zero that is followed by more code (`Column() { … } Text(…)` is
/// two statements without a semicolon between them). Measured anchors: a declaration at 5:5 of
/// `fixtures/ui-build-statements`, the `{` at 12:12 of `fixtures/ui-build-roots`.
fn build_body(source: &str, brace: usize) -> Option<BuildBody> {
    let bytes = source.as_bytes();
    if bytes.get(brace) != Some(&b'{') {
        return None;
    }
    let body_start = brace + 1;
    let mut depth = 1usize;
    let mut position = body_start;
    while position < bytes.len() && depth > 0 {
        match bytes[position] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        position += 1;
    }
    let body_end = position.saturating_sub(1);
    let mut declarations = Vec::new();
    let mut statements = Vec::new();
    let mut nodes = 0usize;
    let mut statement_start: Option<usize> = None;
    let mut depth = 0usize;
    let mut index = body_start;
    let close_statement = |start: usize,
                           end: usize,
                           declarations: &mut Vec<usize>,
                           statements: &mut Vec<(usize, usize)>,
                           nodes: &mut usize| {
        let text = &source[start..end];
        let trimmed = text.trim_start();
        let lead = text.len() - trimmed.len();
        let trimmed = trimmed.trim_end();
        if trimmed.is_empty() {
            return;
        }
        for keyword in ["const ", "let ", "var "] {
            if trimmed.starts_with(keyword) {
                declarations.push(start + lead);
                return;
            }
        }
        *nodes += 1;
        statements.push((start + lead, end));
    };
    while index < body_end {
        let byte = bytes[index];
        match byte {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && byte == b'}' {
                    // A node that closes its block starts a new statement only if more code
                    // follows before the next `;`.
                    let rest = &source[index + 1..body_end];
                    let next = rest.trim_start();
                    // `.attr(…)` after the block continues the same chain, not a new statement.
                    if !next.is_empty() && !next.starts_with(';') && !next.starts_with('.') {
                        if let Some(start) = statement_start.take() {
                            close_statement(
                                start,
                                index + 1,
                                &mut declarations,
                                &mut statements,
                                &mut nodes,
                            );
                        }
                    }
                }
            }
            b';' if depth == 0 => {
                if let Some(start) = statement_start.take() {
                    close_statement(start, index, &mut declarations, &mut statements, &mut nodes);
                }
            }
            _ => {}
        }
        if statement_start.is_none() && !matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
            statement_start = Some(index);
        }
        index += 1;
    }
    if let Some(start) = statement_start {
        close_statement(
            start,
            body_end,
            &mut declarations,
            &mut statements,
            &mut nodes,
        );
    }
    Some(BuildBody {
        brace,
        span_len: body_end.saturating_sub(body_start),
        statements,
        declarations,
        nodes,
    })
}

/// Offsets and names of the classes, interfaces and type aliases this file declares.
fn declared_type_names(program: &Program<'_>, _source: &str) -> Vec<(usize, String, bool)> {
    struct Collect {
        found: Vec<(usize, String, bool)>,
    }
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                self.found
                    .push((id.span.start as usize, id.name.to_string(), true));
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }

        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            self.found
                .push((it.id.span.start as usize, it.id.name.to_string(), false));
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }

        fn visit_ts_type_alias_declaration(
            &mut self,
            it: &oxc_ast::ast::TSTypeAliasDeclaration<'a>,
        ) {
            self.found
                .push((it.id.span.start as usize, it.id.name.to_string(), false));
            oxc_ast_visit::walk::walk_ts_type_alias_declaration(self, it);
        }
    }
    let mut collect = Collect { found: Vec::new() };
    collect.visit_program(program);
    collect.found
}

/// Offsets of declared type names that collide with an ArkUI component name
/// (`arkts-unique-names`, 10605004).
fn component_name_collisions(
    program: &Program<'_>,
    known: &std::collections::HashSet<String>,
) -> Vec<usize> {
    struct Collect<'k> {
        known: &'k std::collections::HashSet<String>,
        found: Vec<usize>,
    }
    impl<'a> Visit<'a> for Collect<'_> {
        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            if self.known.contains(it.id.name.as_str()) {
                self.found.push(it.id.span.start as usize);
            }
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }

        fn visit_ts_type_alias_declaration(
            &mut self,
            it: &oxc_ast::ast::TSTypeAliasDeclaration<'a>,
        ) {
            if self.known.contains(it.id.name.as_str()) {
                self.found.push(it.id.span.start as usize);
            }
            oxc_ast_visit::walk::walk_ts_type_alias_declaration(self, it);
        }
    }
    let mut collect = Collect {
        known,
        found: Vec::new(),
    };
    collect.visit_program(program);
    collect.found
}

/// Members each class of this file declares: fields and methods by name.
/// Decorators the reference accepts on a global `function`, and the two diagnostics it raises
/// when it gets another one (measured 1:1 of `fixtures/ui-local-builder`).
fn global_function_decorator_violations(source: &str) -> Vec<(usize, &'static str, String)> {
    const ALLOWED: [&str; 6] = [
        "AnimatableExtend",
        "Builder",
        "Extend",
        "Styles",
        "Concurrent",
        "Sendable",
    ];
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find('@') {
        let start = from + at;
        let rest = &source[start + 1..];
        let name_len = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .count();
        if name_len == 0 {
            from = start + 1;
            continue;
        }
        let name = &rest[..name_len];
        // Past an optional argument list, the next word decides whether this decorates a function.
        let mut after = rest[name_len..].trim_start();
        if let Some(inner) = after.strip_prefix('(') {
            if let Some(close) = inner.find(')') {
                after = inner[close + 1..].trim_start();
            }
        }
        if !after.starts_with("function") {
            from = start + 1;
            continue;
        }
        if !ALLOWED.contains(&name) {
            found.push((
                start,
                "10905117",
                "A function can only be decorated by one of the '@AnimatableExtend', '@Builder', '@Extend', '@Styles', '@Concurrent' and '@Sendable'."
                    .to_string(),
            ));
        }
        if name == "LocalBuilder" {
            found.push((
                start,
                "10905115",
                "'@LocalBuilder' can only decorate method.".to_string(),
            ));
        }
        from = start + 1 + name_len;
    }
    found
}

/// Interfaces of this file that declare an index signature, with the type it answers, and the
/// offset of its `[`: ArkTS has no indexed signatures (`arkts-no-indexed-signatures`, 10605017 at
/// the bracket — 2:3 of `fixtures/type-index-signature`), and a literal declared to be one of them
/// counts as untyped (10605038 at the `{`, 6:18) with each value judged against that type (10505001
/// at the key, 6:20).
fn index_signature_types(source: &str) -> std::collections::HashMap<String, (usize, String)> {
    let mut found = std::collections::HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("interface ") {
        let start = from + at;
        let after = &source[start + "interface ".len()..];
        let name_len = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .count();
        from = start + "interface ".len() + name_len.max(1);
        if name_len == 0 {
            continue;
        }
        let name = after[..name_len].to_string();
        let tail = &after[name_len..];
        let Some(open) = tail.find('{') else {
            continue;
        };
        let body_at = start + "interface ".len() + name_len + open;
        let Some(body_end) = matching_brace(source, body_at) else {
            continue;
        };
        let body = &source[body_at + 1..body_end];
        // `[key: string]: number;`
        let mut cursor = 0usize;
        let bytes = body.as_bytes();
        while cursor < bytes.len() {
            if bytes[cursor] == b'[' {
                let rest = &body[cursor..];
                if let Some(close) = rest.find(']') {
                    let after_close = &rest[close + 1..];
                    if let Some(colon) = after_close.find(':') {
                        let value = after_close[colon + 1..]
                            .split([';', '\n'])
                            .next()
                            .unwrap_or("")
                            .trim();
                        let spelling = match value {
                            "number" => Some("number"),
                            "string" => Some("string"),
                            "boolean" => Some("boolean"),
                            _ => None,
                        };
                        if let Some(spelling) = spelling {
                            found
                                .insert(name.clone(), (body_at + 1 + cursor, spelling.to_string()));
                            break;
                        }
                    }
                }
            }
            cursor += 1;
        }
    }
    found
}

/// Type aliases of the form `type Names = Record<string, T>`. ArkTS has no mapped types either: a
/// literal declared to be one is untyped (10605038 at the `{`) and each value is judged against `T`
/// (10505001 at the key — measured 4:20 and 4:22 of `fixtures/type-mapped-alias`). Unlike an
/// interface index signature this shape draws no `10605017`.
fn record_alias_types(source: &str) -> std::collections::HashMap<String, (usize, String)> {
    let mut found = std::collections::HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("type ") {
        let start = from + at;
        let after = &source[start + "type ".len()..];
        let name_len = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .count();
        from = start + "type ".len() + name_len.max(1);
        if name_len == 0 {
            continue;
        }
        let name = after[..name_len].to_string();
        let rest = &after[name_len..];
        let Some(equal) = rest.find('=') else {
            continue;
        };
        let tail = rest[equal + 1..].trim_start();
        let Some(inner) = tail.strip_prefix("Record<") else {
            continue;
        };
        let Some(close) = inner.find(',') else {
            continue;
        };
        let value = inner[close + 1..].split('>').next().unwrap_or("").trim();
        let spelling = match value {
            "number" => Some("number"),
            "string" => Some("string"),
            "boolean" => Some("boolean"),
            _ => None,
        };
        if let Some(spelling) = spelling {
            found.insert(name, (start, spelling.to_string()));
        }
    }
    found
}

/// The offset of the `}` that closes the `{` at `open`.
fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in source[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Literals declared to be an index-signature interface: the `{` and each key whose literal
/// disagrees with the index type.
fn index_signature_literal_violations(
    source: &str,
    tables: &std::collections::HashMap<String, (usize, String)>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (name, (_, spelling)) in tables {
        let needle = format!(": {name} =");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at + needle.len();
            from = start;
            let Some(brace_rel) = source[start..].find('{') else {
                break;
            };
            let brace = start + brace_rel;
            let Some(close) = matching_brace(source, brace) else {
                break;
            };
            found.push((
                brace,
                "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)"
                    .to_string(),
            ));
            let body = &source[brace + 1..close];
            let mut cursor = 0usize;
            for part in body.split(',') {
                let piece = part.trim();
                if let Some(colon) = piece.find(':') {
                    let key = piece[..colon].trim().trim_matches(['"', '\'']);
                    if !key.is_empty() {
                        let value = piece[colon + 1..].trim();
                        let literal = if value.starts_with('"') || value.starts_with('\'') {
                            Some("string")
                        } else if value.starts_with(|c: char| c.is_ascii_digit()) {
                            Some("number")
                        } else if value.starts_with("true") || value.starts_with("false") {
                            Some("boolean")
                        } else {
                            None
                        };
                        if let Some(literal) = literal {
                            if literal != spelling {
                                let key_at = body[cursor..]
                                    .find(key)
                                    .map(|found| brace + 1 + cursor + found);
                                if let Some(key_at) = key_at {
                                    found.push((
                                        key_at,
                                        format!(
                                            "Type '{literal}' is not assignable to type '{spelling}'."
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
                cursor += part.len() + 1;
            }
        }
    }
    found.sort_unstable();
    found
}

/// Declared parameter spellings of every `@Builder` method, keyed by name, for the literal check
/// `this.card(1)` against `@Builder card(title: string)` (measured 13:17 of
/// `fixtures/ui-builder-type`).
fn builder_param_spellings(source: &str) -> std::collections::HashMap<String, (bool, Vec<String>)> {
    let mut table = std::collections::HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("@Builder") {
        let start = from + at;
        let after = &source[start + "@Builder".len()..];
        let rest = after.trim_start();
        from = start + "@Builder".len();
        // `@Builder function card(...)` is called bare, `@Builder card(...)` as `this.card(...)`.
        let (rest, bare) = match rest.strip_prefix("function") {
            Some(rest) => (rest.trim_start(), true),
            None => (rest, false),
        };
        let name_len = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .count();
        if name_len == 0 {
            continue;
        }
        let name = rest[..name_len].to_string();
        let tail = &rest[name_len..];
        let Some(params_at) = tail.find('(') else {
            continue;
        };
        let params = &tail[params_at + 1..];
        let Some(close) = params.find(')') else {
            continue;
        };
        let inside = params[..close].trim();
        if inside.is_empty() {
            table.insert(name, (bare, Vec::new()));
            continue;
        }
        let spelled: Vec<String> = split_top_level(inside)
            .iter()
            .map(|parameter| {
                let Some(colon) = parameter.find(':') else {
                    return "_".to_string();
                };
                let type_text = parameter[colon + 1..].trim();
                match type_text {
                    "number" => "number".to_string(),
                    "string" => "string".to_string(),
                    "boolean" => "boolean".to_string(),
                    _ => "_".to_string(),
                }
            })
            .collect();
        table.insert(name, (bare, spelled));
    }
    table
}

/// `this.<builder>(<literal>)` whose literal disagrees with the declared parameter spelling:
/// the offset of the argument and the message.
fn builder_type_violations(
    source: &str,
    builders: &std::collections::HashMap<String, (bool, Vec<String>)>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (name, (bare, spellings)) in builders {
        let needle = if *bare {
            format!("{name}(")
        } else {
            format!("this.{name}(")
        };
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            if *bare {
                // The declaration itself, and a member call, are not calls of the global builder.
                let head = &source[..start];
                if head.trim_end().ends_with("function") || head.ends_with('.') {
                    from = start + needle.len();
                    continue;
                }
            }
            let open = start + needle.len() - 1;
            let Some(close) = matching_paren(source, open) else {
                break;
            };
            from = close;
            let arguments = argument_offsets(&source[open + 1..close]);
            for (index, offset) in arguments.iter().enumerate() {
                let Some(expected) = spellings.get(index) else {
                    continue;
                };
                if expected == "_" {
                    continue;
                }
                let value = &source[open + 1 + offset..close];
                let spelled = if value.starts_with('"') || value.starts_with('\'') {
                    "string"
                } else if value.starts_with(|c: char| c.is_ascii_digit()) {
                    "number"
                } else if value.starts_with("true") || value.starts_with("false") {
                    "boolean"
                } else {
                    continue;
                };
                if spelled != expected {
                    found.push((
                        open + 1 + offset,
                        format!("Argument of type '{spelled}' is not assignable to parameter of type '{expected}'."),
                    ));
                }
            }
        }
    }
    found.sort_unstable();
    found
}

/// Declared parameter count of every `@Builder` method in the file, keyed by its name.
fn builder_arities(source: &str) -> std::collections::HashMap<String, usize> {
    let mut table = std::collections::HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("@Builder") {
        let start = from + at;
        let after = &source[start + "@Builder".len()..];
        let rest = after.trim_start();
        // `@Builder x(…)` is a method; `@Builder function …` is a global builder, whose calls are
        // not `this.<name>(…)` and are not judged here.
        if rest.starts_with("function") {
            from = start + "@Builder".len();
            continue;
        }
        let name_len = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .count();
        if name_len == 0 {
            from = start + "@Builder".len();
            continue;
        }
        let name = rest[..name_len].to_string();
        let tail = &rest[name_len..];
        if let Some(params_at) = tail.find('(') {
            let params = &tail[params_at + 1..];
            if let Some(close) = params.find(')') {
                let inside = params[..close].trim();
                let count = if inside.is_empty() {
                    0
                } else {
                    argument_offsets(inside).len()
                };
                table.insert(name, count);
            }
        }
        from = start + "@Builder".len();
    }
    table
}

/// Start offsets of each argument of a call, given the text between its parentheses. Commas
/// inside nested brackets, strings and template literals do not separate arguments.
fn argument_offsets(inside: &str) -> Vec<usize> {
    let bytes = inside.as_bytes();
    let mut starts = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut current: Option<usize> = None;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(open) = quote {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == open {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' | b'`' => {
                quote = Some(byte);
                current.get_or_insert(index);
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                current.get_or_insert(index);
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                starts.push(current.unwrap_or(index));
                current = None;
            }
            b if b.is_ascii_whitespace() => {}
            _ => {
                current.get_or_insert(index);
            }
        }
        index += 1;
    }
    if let Some(start) = current {
        starts.push(start);
    }
    starts
}

/// `this.<builder>(...)` calls whose argument count differs from the declaration: the offset to
/// report and the message. Too few arguments are reported at the method name, too many at the
/// first extra argument (measured 13:12 and 14:22 of `fixtures/ui-builder-arity`).
fn builder_arity_violations(
    source: &str,
    builders: &std::collections::HashMap<String, usize>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (name, expected) in builders {
        let needle = format!("this.{name}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            let open = start + needle.len() - 1;
            let mut depth = 0usize;
            let mut close = None;
            for (offset, byte) in source[open..].bytes().enumerate() {
                match byte {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(open + offset);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { break };
            let arguments = argument_offsets(&source[open + 1..close]);
            let got = arguments.len();
            if got < *expected {
                found.push((
                    start + "this.".len(),
                    format!("Expected {expected} arguments, but got {got}."),
                ));
            } else if got > *expected {
                let extra = open + 1 + arguments[*expected];
                found.push((
                    extra,
                    format!("Expected {expected} arguments, but got {got}."),
                ));
            }
            from = close;
        }
    }
    found.sort_unstable();
    found
}

/// Offsets of numeric literal arguments passed to `Text(...)`, which takes `string | Resource`.
/// Only a bare literal is judged: `Text(this.count)` and a template keep their own typing, and the
/// reference reports neither (`fixtures/ui-text-argument`).
fn text_number_arguments(source: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("Text(") {
        let start = from + at;
        // The call has to be the component itself, not the tail of a longer identifier.
        let boundary = start == 0
            || !source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
        let argument_at = start + "Text(".len();
        let rest = &source[argument_at..];
        let literal_len = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .count();
        if boundary && literal_len > 0 && rest[literal_len..].trim_start().starts_with(')') {
            found.push(argument_at);
        }
        from = argument_at;
    }
    found
}

/// An `@Extend(<component>)` whose target no component answers leaves its body as written, and the
/// parser then trips over the attribute chain: `Declaration or statement expected.` at the `.` and
/// `Cannot find name '<member>'.` at the member (measured 2:3 and 2:4 of
/// `fixtures/ui-extend-target`).
fn extend_target_violations(
    source: &str,
    components: &std::collections::HashSet<String>,
) -> Vec<(usize, &'static str, String)> {
    // No component list means no SDK was read: claiming a target is unknown would then be a guess.
    if components.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(usize, &'static str, String)> = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("@Extend(") {
        let start = from + at;
        let rest = &source[start + "@Extend(".len()..];
        let Some(close) = rest.find(')') else {
            break;
        };
        let target = rest[..close].trim();
        from = start + "@Extend(".len();
        if components.contains(target) {
            continue;
        }
        // The decorator itself is refused, at its name (`'Nope' parameter cannot be passed in the
        // '@Extend' decorator.`, 1:2).
        found.push((
            start + 1,
            "WARN",
            format!("'{target}' parameter cannot be passed in the '@Extend' decorator."),
        ));
        // The declaration's body: from the first `{` after `function`, to its matching `}`.
        let after = &rest[close + 1..];
        let Some(body_start) = after.find('{') else {
            continue;
        };
        let body_at = start + "@Extend(".len() + close + 1 + body_start;
        let mut depth = 0usize;
        let mut body_end = None;
        for (offset, byte) in source[body_at..].bytes().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        body_end = Some(body_at + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(body_end) = body_end else {
            continue;
        };
        let body = &source[body_at + 1..body_end];
        let base = body_at + 1;
        let mut cursor = 0usize;
        for line in body.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if let Some(chain) = trimmed.strip_prefix('.') {
                let member: String = chain
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                let dot_at = base + cursor + (line.len() - trimmed.len());
                found.push((
                    dot_at,
                    "ERROR",
                    "Declaration or statement expected.".to_string(),
                ));
                if !member.is_empty() {
                    found.push((dot_at + 1, "ERROR", format!("Cannot find name '{member}'.")));
                    found.push((
                        dot_at + 1,
                        "WARN",
                        "Only UI component syntax can be written here.".to_string(),
                    ));
                }
            }
            cursor += line.len();
        }
    }
    found.sort_unstable();
    found
}

/// `.name(...)` where `name` belongs to an `@Extend` whose target no component answers: the call is
/// a warning naming the receiver's attribute type
/// (`Property 'fancy' does not exist on type 'TextAttribute'.`, 10:17 of
/// `fixtures/ui-extend-target`).
fn unknown_extend_call_violations(
    source: &str,
    names: &[String],
    components: &std::collections::HashSet<String>,
) -> Vec<(usize, String)> {
    if components.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for name in names {
        let needle = format!(".{name}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let dot = from + at;
            from = dot + needle.len();
            // The receiver: the identifier before the `.`, past its argument list when it has one
            // (`Text("x").fancy()`).
            let before = &source[..dot];
            let mut end = before.len();
            if before.trim_end().ends_with(')') {
                let mut depth = 0usize;
                let mut index = before.len();
                while index > 0 {
                    index -= 1;
                    match before.as_bytes()[index] {
                        b')' => depth += 1,
                        b'(' => {
                            depth -= 1;
                            if depth == 0 {
                                end = index;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            let receiver: String = before[..end]
                .trim_end()
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect::<Vec<char>>()
                .into_iter()
                .rev()
                .collect();
            if !components.contains(&receiver) {
                continue;
            }
            found.push((
                dot + 1,
                format!("Property '{name}' does not exist on type '{receiver}Attribute'."),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// Offsets of the names in `@Extend(C) function name(...)` declarations whose emitted helper
/// collides with another of the same name (`__C__name`).
fn duplicate_extends(source: &str) -> Vec<usize> {
    let mut seen: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("@Extend(") {
        let start = from + at;
        let rest = &source[start + "@Extend(".len()..];
        let Some(close) = rest.find(')') else {
            break;
        };
        let component = rest[..close].trim().to_string();
        let after = &rest[close + 1..];
        if let Some(keyword) = after.find("function") {
            let tail = after[keyword + "function".len()..].trim_start();
            let name_len = tail
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .count();
            if name_len > 0 {
                let name = tail[..name_len].to_string();
                let name_at = start
                    + "@Extend(".len()
                    + close
                    + 1
                    + keyword
                    + "function".len()
                    + (after[keyword + "function".len()..].len() - tail.len());
                seen.entry(format!("__{component}__{name}"))
                    .or_default()
                    .push(name_at);
            }
        }
        from = start + "@Extend(".len();
    }
    let mut found: Vec<usize> = seen
        .into_iter()
        .filter(|(_, offsets)| offsets.len() > 1)
        .flat_map(|(_, offsets)| offsets)
        .collect();
    found.sort_unstable();
    found
}

/// Offsets of the name in every `this.<member>` the text contains.
fn this_member_offsets(source: &str, member: &str) -> Vec<usize> {
    let needle = format!("this.{member}");
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find(&needle) {
        let start = from + at;
        // Not `this.<member>` when the member name continues (a longer identifier).
        let after = start + needle.len();
        let continues = source[after..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$');
        if !continues {
            found.push(start + "this.".len());
        }
        from = after;
    }
    found
}

/// A property of an object literal whose literal value disagrees with the declared member type:
/// `{ count: "s" }` against `interface Holder { count: number }`.
fn object_literal_violations(
    literal: &oxc_ast::ast::ObjectExpression<'_>,
    expected: &std::collections::HashMap<String, String>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for property in &literal.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let PropertyKey::StaticIdentifier(key) = &property.key else {
            continue;
        };
        let Some(wanted) = expected.get(key.name.as_str()) else {
            continue;
        };
        let found_spelling = match &property.value {
            Expression::StringLiteral(_) => "string",
            Expression::NumericLiteral(_) => "number",
            Expression::BooleanLiteral(_) => "boolean",
            _ => continue,
        };
        if found_spelling != wanted {
            found.push((
                key.span.start as usize,
                format!("Type '{found_spelling}' is not assignable to type '{wanted}'."),
            ));
        }
    }
    found
}

/// Parameter type names of each function, for the ones that name an interface or class of the file.
fn function_parameter_named_types(
    program: &Program<'_>,
    known: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) -> std::collections::HashMap<String, Vec<String>> {
    struct Collect<'k> {
        known: &'k std::collections::HashMap<String, std::collections::HashMap<String, String>>,
        found: std::collections::HashMap<String, Vec<String>>,
    }
    impl<'a> Visit<'a> for Collect<'_> {
        fn visit_function(
            &mut self,
            it: &oxc_ast::ast::Function<'a>,
            flags: oxc_syntax::scope::ScopeFlags,
        ) {
            if let Some(id) = &it.id {
                let spelled: Vec<String> = it
                    .params
                    .items
                    .iter()
                    .map(|parameter| {
                        let Some(annotation) = parameter
                            .pattern
                            .type_annotation
                            .as_ref()
                            .map(|it| &it.type_annotation)
                        else {
                            return String::new();
                        };
                        let TSType::TSTypeReference(reference) = annotation else {
                            return String::new();
                        };
                        let oxc_ast::ast::TSTypeName::IdentifierReference(name) =
                            &reference.type_name
                        else {
                            return String::new();
                        };
                        let spelled = name.name.to_string();
                        if self.known.contains_key(&spelled) {
                            spelled
                        } else {
                            String::new()
                        }
                    })
                    .collect();
                if spelled.iter().any(|name| !name.is_empty()) {
                    self.found.insert(id.name.to_string(), spelled);
                }
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
        }
    }
    let mut collect = Collect {
        known,
        found: std::collections::HashMap::new(),
    };
    collect.visit_program(program);
    collect.found
}

/// How the reference spells an object literal in a missing-member message: `{}` when empty and
/// `{ count: number; }` otherwise, each member typed by its literal.
fn object_spelling(literal: &oxc_ast::ast::ObjectExpression<'_>) -> String {
    if literal.properties.is_empty() {
        return "{}".to_string();
    }
    let mut parts = Vec::new();
    for property in &literal.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let PropertyKey::StaticIdentifier(key) = &property.key else {
            continue;
        };
        let spelled = match &property.value {
            Expression::StringLiteral(_) => "string",
            Expression::NumericLiteral(_) => "number",
            Expression::BooleanLiteral(_) => "boolean",
            _ => continue,
        };
        parts.push(format!("{}: {};", key.name, spelled));
    }
    format!("{{ {} }}", parts.join(" "))
}

/// Members `Array` has, for the element-type checks: a name outside this list is not one
/// (`Property 'notAReal' does not exist on type 'number[]'.`, 4:13 of `fixtures/type-array`).
const ARRAY_MEMBERS: [&str; 40] = [
    "at",
    "concat",
    "copyWithin",
    "entries",
    "every",
    "fill",
    "filter",
    "find",
    "findIndex",
    "flat",
    "flatMap",
    "forEach",
    "includes",
    "indexOf",
    "join",
    "keys",
    "lastIndexOf",
    "length",
    "map",
    "pop",
    "push",
    "reduce",
    "reduceRight",
    "reverse",
    "shift",
    "slice",
    "some",
    "sort",
    "splice",
    "toString",
    "unshift",
    "values",
    "toLocaleString",
    "toReversed",
    "toSorted",
    "toSpliced",
    "with",
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
];

/// Locals declared `: any`/`: unknown`, or initialised from an `as any` assertion.
fn any_typed_locals(program: &Program<'_>) -> std::collections::HashSet<String> {
    struct Collect(std::collections::HashSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
            let annotated = it
                .id
                .type_annotation
                .as_ref()
                .map(|annotation| {
                    matches!(
                        annotation.type_annotation,
                        TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
                    )
                })
                .unwrap_or(false);
            let asserted = matches!(
                &it.init,
                Some(Expression::TSAsExpression(assertion))
                    if matches!(
                        assertion.type_annotation,
                        TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
                    )
            );
            if annotated || asserted {
                if let oxc_ast::ast::BindingPatternKind::BindingIdentifier(identifier) = &it.id.kind
                {
                    self.0.insert(identifier.name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_variable_declarator(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashSet::new());
    collect.visit_program(program);
    collect.0
}

/// Declarations of the same name in one file. ArkTS refuses the merge
/// (`arkts-no-decl-merging`, 10605103 at each declaration's start — measured 1:1 and 5:1 of
/// `fixtures/type-declaration-merge` with two `interface Bag` blocks).
fn declaration_merges(program: &Program<'_>) -> Vec<(String, usize)> {
    struct Collect(std::collections::HashMap<String, Vec<usize>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            self.0
                .entry(it.id.name.to_string())
                .or_default()
                .push(it.span.start as usize);
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
        fn visit_class(&mut self, it: &oxc_ast::ast::Class<'a>) {
            if let Some(id) = &it.id {
                self.0
                    .entry(id.name.to_string())
                    .or_default()
                    .push(it.span.start as usize);
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
        fn visit_ts_type_alias_declaration(
            &mut self,
            it: &oxc_ast::ast::TSTypeAliasDeclaration<'a>,
        ) {
            self.0
                .entry(it.id.name.to_string())
                .or_default()
                .push(it.span.start as usize);
            oxc_ast_visit::walk::walk_ts_type_alias_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    let mut found: Vec<(String, usize)> = Vec::new();
    for (name, declarations) in collect.0 {
        if declarations.len() > 1 {
            for at in declarations {
                found.push((name.clone(), at));
            }
        }
    }
    found.sort_unstable_by_key(|(_, at)| *at);
    found
}

/// Names of the enums this file declares.
fn enum_names(program: &Program<'_>) -> std::collections::HashSet<String> {
    struct Collect(std::collections::HashSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_ts_enum_declaration(&mut self, it: &oxc_ast::ast::TSEnumDeclaration<'a>) {
            self.0.insert(it.id.name.to_string());
            oxc_ast_visit::walk::walk_ts_enum_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashSet::new());
    collect.visit_program(program);
    collect.0
}

/// Element type of each local declared as a primitive array: `xs -> "number"`.
/// The element type of each local declared as a primitive array: `xs -> "number"`.
fn array_element_types(program: &Program<'_>) -> std::collections::HashMap<String, String> {
    struct Collect(std::collections::HashMap<String, String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
            if let oxc_ast::ast::BindingPatternKind::BindingIdentifier(name) = &it.id.kind {
                if let Some(annotation) = &it.id.type_annotation {
                    if let TSType::TSArrayType(array) = &annotation.type_annotation {
                        let spelling = match array.element_type {
                            TSType::TSNumberKeyword(_) => "number",
                            TSType::TSStringKeyword(_) => "string",
                            TSType::TSBooleanKeyword(_) => "boolean",
                            _ => "",
                        };
                        if !spelling.is_empty() {
                            self.0.insert(name.name.to_string(), spelling.to_string());
                        }
                    }
                }
            }
            oxc_ast_visit::walk::walk_variable_declarator(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// `this.<field> = "s"` where the declaration types that field `number`: reported at the `this`
/// (measured 16:5 of `fixtures/type-field-assign`).
fn field_assignment_violations(
    source: &str,
    structs: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("this.") {
        let start = from + at;
        let after = start + "this.".len();
        let member: String = source[after..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        from = after;
        if member.is_empty() {
            continue;
        }
        // An assignment only: `=` that is not `==`, `===`, `=>`.
        let rest = &source[after + member.len()..];
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('=') {
            continue;
        }
        let after_equals = &trimmed[1..];
        if after_equals.starts_with('=') || after_equals.starts_with('>') {
            continue;
        }
        let value = after_equals.trim_start();
        let spelled = if value.starts_with('"') || value.starts_with('\'') {
            "string"
        } else if value.starts_with(|c: char| c.is_ascii_digit()) {
            "number"
        } else if value.starts_with("true") || value.starts_with("false") {
            "boolean"
        } else {
            continue;
        };
        let Some(owner) = enclosing_declaration(source, start) else {
            continue;
        };
        let Some(fields) = structs.get(&owner) else {
            continue;
        };
        let Some(expected) = fields.get(&member) else {
            continue;
        };
        if spelled != expected {
            found.push((
                start,
                format!("Type '{spelled}' is not assignable to type '{expected}'."),
            ));
        }
    }
    found
}

/// `this.<name>` where the enclosing class or struct declares no such member: the reference reports
/// `Property 'notAField' does not exist on type 'FieldProbe'.` at the name (measured 8:35 of
/// `fixtures/type-struct-member`).
///
/// The page's build body is blanked out of the checked view, so the occurrences are read from the
/// file's own text. The owner is the declaration the `this` sits in — a `class Box` member is known
/// from `Box`, and a page's member from the struct (`fixtures/type-generic-member` caught the first
/// attempt, which only knew struct members).
fn struct_member_violations(
    source: &str,
    members: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("this.") {
        let start = from + at;
        let after = start + "this.".len();
        let member: String = source[after..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        from = after;
        if member.is_empty() {
            continue;
        }
        let Some(owner) = enclosing_declaration(source, start) else {
            continue;
        };
        let Some(known) = members.get(&owner) else {
            continue;
        };
        if known.contains(member.as_str()) {
            continue;
        }
        found.push((
            after,
            format!("Property '{member}' does not exist on type '{owner}'."),
        ));
    }
    found
}

/// The name of the nearest `class`/`struct` declared before `offset`.
fn enclosing_declaration(source: &str, offset: usize) -> Option<String> {
    let before = &source[..offset];
    let mut best: Option<(usize, String)> = None;
    for keyword in ["class", "struct"] {
        let mut from = 0usize;
        while let Some(at) = before[from..].find(keyword) {
            let start = from + at;
            let after = start + keyword.len();
            let boundary = after < before.len() && before[after..].starts_with(char::is_whitespace);
            if boundary {
                let name: String = before[after..]
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if !name.is_empty() && best.as_ref().is_none_or(|(seen, _)| *seen < start) {
                    best = Some((start, name));
                }
            }
            from = after;
        }
    }
    best.map(|(_, name)| name)
}

/// Field types of each struct in this file whose members spell a primitive:
/// `ChildCard -> {label: "string"}`.
fn struct_field_types(
    program: &Program<'_>,
    source: &str,
) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
    struct Collect<'s> {
        source: &'s str,
        found: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    }
    impl<'a> Visit<'a> for Collect<'_> {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                let is_struct = self.source[..id.span.start as usize]
                    .trim_end()
                    .ends_with("struct");
                if is_struct {
                    let mut fields = std::collections::HashMap::new();
                    for element in &it.body.body {
                        let ClassElement::PropertyDefinition(property) = element else {
                            continue;
                        };
                        let Some(name) = property.key.static_name() else {
                            continue;
                        };
                        let Some(annotation) = &property.type_annotation else {
                            continue;
                        };
                        let spelling = match &annotation.type_annotation {
                            TSType::TSNumberKeyword(_) => "number",
                            TSType::TSStringKeyword(_) => "string",
                            TSType::TSBooleanKeyword(_) => "boolean",
                            _ => continue,
                        };
                        fields.insert(name.to_string(), spelling.to_string());
                    }
                    if !fields.is_empty() {
                        self.found.insert(id.name.to_string(), fields);
                    }
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect {
        source,
        found: std::collections::HashMap::new(),
    };
    collect.visit_program(program);
    collect.found
}

/// A child instantiation whose argument object hands a literal of the wrong type to a declared
/// member: `ChildCard({ label: 1 })` against `@Prop label: string` reports at the property name
/// (measured 17:19 of `fixtures/ui-component-argument`), and `ChildCard({ label: "ok" })` beside it
/// draws nothing.
fn component_argument_violations(
    source: &str,
    structs: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (name, fields) in structs {
        let needle = format!("{name}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            let boundary = start == 0
                || !source[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
            let open = start + needle.len() - 1;
            let Some(close) = matching_paren(source, open) else {
                break;
            };
            from = close;
            if !boundary {
                continue;
            }
            let inside = &source[open + 1..close];
            let trimmed = inside.trim_start();
            let Some(body) = trimmed.strip_prefix('{') else {
                continue;
            };
            let Some(end) = body.rfind('}') else {
                continue;
            };
            let body_start = open + 1 + (inside.len() - trimmed.len()) + 1;
            for item in split_top_level(&body[..end]) {
                let item_start = body[..end]
                    .find(&item)
                    .map(|at| body_start + at)
                    .unwrap_or(body_start);
                let Some(colon) = item.find(':') else {
                    continue;
                };
                let key = item[..colon].trim();
                let value = item[colon + 1..].trim();
                let Some(expected) = fields.get(key) else {
                    continue;
                };
                let spelled = literal_spelling(value);
                if spelled.is_empty() || &spelled == expected {
                    continue;
                }
                let key_at = item_start + item[..colon].find(key).unwrap_or(0);
                found.push((
                    key_at,
                    format!("Type '{spelled}' is not assignable to type '{expected}'."),
                ));
            }
        }
    }
    found.sort_unstable();
    found
}

/// The offset of the `)` matching the `(` at `open`.
fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in source[open..].bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits an object literal's body at its own commas.
fn split_top_level(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for byte in body.chars() {
        match byte {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(byte);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(byte);
            }
            ',' if depth == 0 => {
                items.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(byte),
        }
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }
    items
}

/// The primitive a literal spells, or `""` when the value is not a literal at all.
fn literal_spelling(value: &str) -> String {
    if value.starts_with('"') || value.starts_with('\'') {
        return "string".to_string();
    }
    let number: String = value
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    if !number.is_empty() && number != "-" {
        return "number".to_string();
    }
    if value == "true" || value == "false" {
        return "boolean".to_string();
    }
    String::new()
}

/// Declared parameter types of each class method in this file, keyed `Class.method`.
fn method_parameter_types(program: &Program<'_>) -> std::collections::HashMap<String, Vec<String>> {
    struct Collect(std::collections::HashMap<String, Vec<String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                for element in &it.body.body {
                    let ClassElement::MethodDefinition(method) = element else {
                        continue;
                    };
                    let Some(name) = method.key.static_name() else {
                        continue;
                    };
                    let spelled: Vec<String> = method
                        .value
                        .params
                        .items
                        .iter()
                        .map(|parameter| {
                            let Some(annotation) = parameter
                                .pattern
                                .type_annotation
                                .as_ref()
                                .map(|it| &it.type_annotation)
                            else {
                                return "_".to_string();
                            };
                            match annotation {
                                TSType::TSNumberKeyword(_) => "number".to_string(),
                                TSType::TSStringKeyword(_) => "string".to_string(),
                                TSType::TSBooleanKeyword(_) => "boolean".to_string(),
                                _ => "_".to_string(),
                            }
                        })
                        .collect();
                    if spelled.iter().any(|spelling| spelling != "_") {
                        self.0.insert(format!("{}.{}", id.name, name), spelled);
                    }
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// Declared parameter types of each function of this file that spells them with a primitive:
/// `takesNumber` → `["number"]`. A parameter left unannotated contributes `"_"`.
fn function_parameter_types(
    program: &Program<'_>,
) -> std::collections::HashMap<String, Vec<String>> {
    struct Collect(std::collections::HashMap<String, Vec<String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(
            &mut self,
            it: &oxc_ast::ast::Function<'a>,
            flags: oxc_syntax::scope::ScopeFlags,
        ) {
            if let Some(id) = &it.id {
                // A parameter typed by the function's own type parameter takes the spelling of
                // that parameter's constraint: `f<T extends number>(value: T)` called with a string
                // is refused as `… parameter of type 'number'.` (measured 6:22 of
                // `fixtures/type-generic-constraint`).
                let mut constrained: std::collections::HashMap<String, String> =
                    std::collections::HashMap::new();
                if let Some(params) = &it.type_parameters {
                    for parameter in &params.params {
                        let Some(constraint) = &parameter.constraint else {
                            continue;
                        };
                        let spelling = match constraint {
                            TSType::TSNumberKeyword(_) => "number",
                            TSType::TSStringKeyword(_) => "string",
                            TSType::TSBooleanKeyword(_) => "boolean",
                            _ => continue,
                        };
                        constrained.insert(parameter.name.to_string(), spelling.to_string());
                    }
                }
                let spelled: Vec<String> = it
                    .params
                    .items
                    .iter()
                    .map(|parameter| {
                        let Some(annotation) = parameter
                            .pattern
                            .type_annotation
                            .as_ref()
                            .map(|it| &it.type_annotation)
                        else {
                            return "_".to_string();
                        };
                        match annotation {
                            TSType::TSNumberKeyword(_) => "number".to_string(),
                            TSType::TSStringKeyword(_) => "string".to_string(),
                            TSType::TSBooleanKeyword(_) => "boolean".to_string(),
                            TSType::TSTypeReference(reference) => match &reference.type_name {
                                oxc_ast::ast::TSTypeName::IdentifierReference(name) => constrained
                                    .get(name.name.as_str())
                                    .cloned()
                                    .unwrap_or_else(|| "_".to_string()),
                                _ => "_".to_string(),
                            },
                            _ => "_".to_string(),
                        }
                    })
                    .collect();
                if spelled.iter().any(|spelling| spelling != "_") {
                    self.0.insert(id.name.to_string(), spelled);
                }
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// Property types of each interface this file declares, for the ones spelled with a primitive:
/// `interface Holder { count: number }` → `Holder.count = "number"`.
fn interface_member_types(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
    struct Collect(std::collections::HashMap<String, std::collections::HashMap<String, String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            let mut types = std::collections::HashMap::new();
            for element in &it.body.body {
                let oxc_ast::ast::TSSignature::TSPropertySignature(property) = element else {
                    continue;
                };
                let PropertyKey::StaticIdentifier(name) = &property.key else {
                    continue;
                };
                let Some(annotation) = &property.type_annotation else {
                    continue;
                };
                let spelling = match &annotation.type_annotation {
                    TSType::TSNumberKeyword(_) => "number",
                    TSType::TSStringKeyword(_) => "string",
                    TSType::TSBooleanKeyword(_) => "boolean",
                    _ => continue,
                };
                types.insert(name.name.to_string(), spelling.to_string());
            }
            self.0.insert(it.id.name.to_string(), types);
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// Names of the structs decorated `@ComponentV2`: the V2-only decorators belong to those.
fn v2_classes(program: &Program<'_>) -> std::collections::HashSet<String> {
    struct Collect(std::collections::HashSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            let v2 = it.decorators.iter().any(|decorator| {
                matches!(&decorator.expression, Expression::Identifier(name) if name.name.as_str() == "ComponentV2")
            });
            if v2 {
                if let Some(id) = &it.id {
                    self.0.insert(id.name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashSet::new());
    collect.visit_program(program);
    collect.0
}

/// Names of the `struct`s this file declares, whatever decorators they carry. The masked view
/// turns `struct X` into `class X`, so the keyword is read back from the file's own text.
fn struct_names(program: &Program<'_>, source: &str) -> std::collections::HashSet<String> {
    struct Collect<'s> {
        source: &'s str,
        found: std::collections::HashSet<String>,
    }
    impl<'a> Visit<'a> for Collect<'_> {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                if self.source[..id.span.start as usize]
                    .trim_end()
                    .ends_with("struct")
                {
                    self.found.insert(id.name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect {
        source,
        found: std::collections::HashSet::new(),
    };
    collect.visit_program(program);
    collect.found
}

/// Names of the structs that carry `@Entry`: a page cannot take a `@Link`
/// (measured 6:9 of `fixtures/ui-static-state`).
fn entry_classes(program: &Program<'_>) -> std::collections::HashSet<String> {
    struct Collect(std::collections::HashSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            let entry = it.decorators.iter().any(|decorator| {
                matches!(&decorator.expression, Expression::Identifier(name) if name.name.as_str() == "Entry")
            });
            if entry {
                if let Some(id) = &it.id {
                    self.0.insert(id.name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashSet::new());
    collect.visit_program(program);
    collect.0
}

/// Members each interface declares with `?`: these are not required in a literal
/// (`fixtures/type-optional-member`).
fn optional_members(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    struct Collect(std::collections::HashMap<String, std::collections::HashSet<String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            let mut optional = std::collections::HashSet::new();
            for element in &it.body.body {
                let oxc_ast::ast::TSSignature::TSPropertySignature(property) = element else {
                    continue;
                };
                let PropertyKey::StaticIdentifier(name) = &property.key else {
                    continue;
                };
                if property.optional {
                    optional.insert(name.name.to_string());
                }
            }
            if !optional.is_empty() {
                self.0.insert(it.id.name.to_string(), optional);
            }
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// Members each class of this file declares `static`: reaching one through `this` is a mistake the
/// reference names, with the fix in the message (measured 10:20 of `fixtures/ui-static-state`).
fn static_members(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    struct Collect(std::collections::HashMap<String, std::collections::HashSet<String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                let set = self.0.entry(id.name.to_string()).or_default();
                for element in &it.body.body {
                    let ClassElement::PropertyDefinition(property) = element else {
                        continue;
                    };
                    if property.r#static {
                        if let Some(name) = property.key.static_name() {
                            set.insert(name.to_string());
                        }
                    }
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

fn class_members(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    struct Collect(std::collections::HashMap<String, std::collections::HashSet<String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                let set = self.0.entry(id.name.to_string()).or_default();
                for element in &it.body.body {
                    match element {
                        ClassElement::PropertyDefinition(property) => {
                            if let Some(name) = property.key.static_name() {
                                set.insert(name.to_string());
                            }
                        }
                        ClassElement::MethodDefinition(method) => {
                            if let Some(name) = method.key.static_name() {
                                set.insert(name.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }

        /// An interface's members are judged the same way
        /// (`Property 'notAReal' does not exist on type 'Rect'.`).
        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            let set = self.0.entry(it.id.name.to_string()).or_default();
            for element in &it.body.body {
                match element {
                    oxc_ast::ast::TSSignature::TSPropertySignature(property) => {
                        if let PropertyKey::StaticIdentifier(name) = &property.key {
                            set.insert(name.name.to_string());
                        }
                    }
                    oxc_ast::ast::TSSignature::TSMethodSignature(method) => {
                        if let PropertyKey::StaticIdentifier(name) = &method.key {
                            set.insert(name.name.to_string());
                        }
                    }
                    _ => {}
                }
            }
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// `name -> (minimum, maximum)` arguments for the functions this file declares.
///
/// The type system reports a call of such a function with too many arguments
/// (`Expected 1 arguments, but got 2.`, 8:22 of `fixtures/type-call-arity`).
fn declared_arity(program: &Program<'_>) -> std::collections::HashMap<String, (usize, usize)> {
    struct Collect(std::collections::HashMap<String, (usize, usize)>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            if let Some(id) = &it.id {
                let mut minimum = 0usize;
                let mut maximum = 0usize;
                for parameter in &it.params.items {
                    // `x = 1` is an assignment pattern and `x?` carries `optional`; both make the
                    // parameter unrequired.
                    let optional = parameter.pattern.optional
                        || matches!(
                            parameter.pattern.kind,
                            oxc_ast::ast::BindingPatternKind::AssignmentPattern(_)
                        );
                    if !optional {
                        minimum += 1;
                    }
                    maximum += 1;
                }
                if it.params.rest.is_some() {
                    maximum = usize::MAX;
                }
                self.0.insert(id.name.to_string(), (minimum, maximum));
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// `(structures carrying @Entry, structures carrying @Component)` in this file.
///
/// The component count is what makes the page rules apply at all: a page is a file with ArkUI
/// structures in it, and a plain TypeScript file that merely sits under `pages/` is not one.
/// Structs declared in this file that carry none of the three decorators that make one a
/// component: offset one character before the name, which is where the reference anchors the
/// warning (measured 22:7 of `fixtures/ui-entry-count`, `struct` occupying columns 1–6).
fn undecorated_structs(program: &Program<'_>, source: &str) -> Vec<(usize, String, bool)> {
    struct Collect<'s> {
        source: &'s str,
        found: Vec<(usize, String, bool)>,
    }
    impl<'a> Visit<'a> for Collect<'_> {
        fn visit_class(&mut self, it: &Class<'a>) {
            let decorated = |name: &str| {
                it.decorators.iter().any(|decorator| {
                    matches!(
                        &decorator.expression,
                        Expression::Identifier(identifier) if identifier.name.as_str() == name
                    )
                })
            };
            if let Some(id) = &it.id {
                let is_struct = self.source[..id.span.start as usize]
                    .trim_end()
                    .ends_with("struct");
                let component =
                    decorated("Component") || decorated("ComponentV2") || decorated("CustomDialog");
                if is_struct && !component {
                    // The two shapes the reference distinguishes: a struct that would be a page
                    // (`@Entry` present) draws a warning one character before the name (measured
                    // 22:7 of `fixtures/ui-entry-count`), while one with no component decorator at
                    // all is an error anchored at the `struct` keyword (`10905233`, 1:1 of
                    // `fixtures/ui-struct-decorator`).
                    let start = id.span.start as usize;
                    let keyword = self.source[..start].rfind("struct").unwrap_or(start - 1);
                    let anchor = if decorated("Entry") {
                        start - 1
                    } else {
                        keyword
                    };
                    self.found
                        .push((anchor, id.name.to_string(), decorated("Entry")));
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect {
        source,
        found: Vec::new(),
    };
    collect.visit_program(program);
    collect.found
}

fn structure_counts(program: &Program<'_>) -> (usize, usize) {
    struct Collect(usize, usize);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            let decorated = |name: &str| {
                it.decorators.iter().any(|decorator| {
                    matches!(
                        &decorator.expression,
                        Expression::Identifier(identifier) if identifier.name.as_str() == name
                    )
                })
            };
            if decorated("Entry") {
                self.0 += 1;
            }
            if decorated("Component") {
                self.1 += 1;
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(0, 0);
    collect.visit_program(program);
    (collect.0, collect.1)
}

/// Classes declared in this file.
fn class_names(program: &Program<'_>) -> std::collections::BTreeSet<String> {
    struct Collect(std::collections::BTreeSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                self.0.insert(id.name.to_string());
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(std::collections::BTreeSet::new());
    collect.visit_program(program);
    collect.0
}

/// Functions declared in this file without a return type.
fn unannotated_functions(program: &Program<'_>) -> std::collections::BTreeSet<String> {
    struct Collect(std::collections::BTreeSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            if it.return_type.is_none() {
                if let Some(id) = &it.id {
                    self.0.insert(id.name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
        }
    }
    let mut collect = Collect(std::collections::BTreeSet::new());
    collect.visit_program(program);
    collect.0
}

/// Byte offsets of every `this` inside a function.
fn this_spans(function: &Function<'_>) -> Vec<usize> {
    struct Collect(Vec<usize>);
    impl<'a> Visit<'a> for Collect {
        fn visit_this_expression(&mut self, it: &ThisExpression) {
            self.0.push(it.span.start as usize);
        }
    }
    let mut collect = Collect(Vec::new());
    if let Some(body) = &function.body {
        for statement in &body.statements {
            collect.visit_statement(statement);
        }
    }
    collect.0
}

/// Whether a body returns a `this`, a call to a function whose return type is itself inferred, or a
/// parameter that has no type of its own (and is therefore `any`).
fn returns_limited_type(
    body: &FunctionBody<'_>,
    unannotated: &std::collections::BTreeSet<String>,
    any_params: &std::collections::HashSet<String>,
    arrays: &std::collections::HashMap<String, String>,
    any_locals: &std::collections::HashSet<String>,
) -> bool {
    body.statements.iter().any(|statement| {
        let Statement::ReturnStatement(ret) = statement else {
            return false;
        };
        let Some(argument) = &ret.argument else {
            return false;
        };
        match argument {
            Expression::ThisExpression(_) => true,
            Expression::CallExpression(call) => match &call.callee {
                Expression::Identifier(id) => unannotated.contains(id.name.as_str()),
                // Returning `f.apply(…)`, `f.call(…)` or `f.bind(…)` also leaves the return type
                // to be inferred through a library signature (measured 9:10 of
                // `fixtures/type-return-member-call`).
                Expression::StaticMemberExpression(member) => {
                    matches!(member.property.name.as_str(), "apply" | "call" | "bind")
                }
                _ => false,
            },
            // `return param;` where the parameter carries no type: the return type would have to
            // come from an `any` (measured 1:10 of `fixtures/type-implicit-any`).
            Expression::Identifier(id) => {
                any_params.contains(id.name.as_str()) || any_locals.contains(id.name.as_str())
            }
            // `return x + y;` where either side is an `any` local (1:10 of
            // `fixtures/arkts-any-unknown`).
            Expression::BinaryExpression(binary) => {
                [&binary.left, &binary.right].iter().any(|side| match side {
                    Expression::Identifier(id) => any_locals.contains(id.name.as_str()),
                    _ => false,
                })
            }
            // `return xs.notAReal;` — a member read the array does not have, so the return type
            // cannot be inferred (`fixtures/type-array`: 1:10). A member the array *does* have
            // (`return xs.length`) is fine, which the fixtures that returned one measure.
            Expression::StaticMemberExpression(member) => match &member.object {
                Expression::Identifier(object) => {
                    arrays.contains_key(object.name.as_str())
                        && !ARRAY_MEMBERS.contains(&member.property.name.as_str())
                }
                _ => false,
            },
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::check;

    fn lines(source: &str) -> Vec<String> {
        check("ets/pages/Index.ets", source)
            .into_iter()
            .map(|diagnostic| diagnostic.line())
            .collect()
    }

    #[test]
    fn any_and_unknown_are_reported_at_the_type() {
        let source = "class A {\n  private payload: any = undefined;\n}\n";
        assert_eq!(
            lines(source),
            vec![
                "ERROR 10605008 ets/pages/Index.ets:2:20 Use explicit types instead of \"any\", \
                 \"unknown\" (arkts-no-any-unknown)"
                    .to_string()
            ]
        );
        let source = "class A {\n  private payload: unknown = 1;\n}\n";
        assert_eq!(lines(source).len(), 1);
    }

    #[test]
    fn a_mismatched_literal_is_reported_at_the_property_name() {
        let source = "class A {\n  private count: number = \"three\";\n}\n";
        assert_eq!(
            lines(source),
            vec![
                "ERROR 10505001 ets/pages/Index.ets:2:11 Type 'string' is not assignable to type \
                 'number'."
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_matching_literal_is_silent() {
        assert!(lines("class A {\n  private count: number = 3;\n}\n").is_empty());
    }

    #[test]
    fn a_function_expression_is_an_error_and_a_definite_assertion_is_a_warning() {
        // A `@Component` struct needs one `build` (10905103), so the snippet carries it; the two
        // diagnostics under test are the ones below it.
        let source = "@Entry\n@Component\nstruct Index {\n  private value!: number;\n\n\
                      \x20 private tick = function () {\n    return 1;\n  };\n\n\
                      \x20 build() {\n    Column() {\n    }\n  }\n}\n";
        let reported = lines(source);
        assert_eq!(reported.len(), 2, "{reported:?}");
        // Sorted by position, which is what the comparison and the console both read.
        assert_eq!(
            reported[0],
            "WARN - ets/pages/Index.ets:4:3 Definite assignment assertions are not supported \
             (arkts-no-definite-assignment)"
        );
        assert_eq!(
            reported[1],
            "ERROR 10605046 ets/pages/Index.ets:6:18 Use arrow functions instead of function \
             expressions (arkts-no-func-expressions)"
        );
    }

    #[test]
    fn a_state_field_and_a_method_of_one_name_collide_and_its_call_site_is_not_callable() {
        let source = "@Entry\n@Component\nstruct Index {\n  @State private sdkState: string = \
                      \'\';\n\n  private sdkState(): string {\n    return \'disk\';\n  }\n\n  build() \
                      {\n    Column() {\n      Text(this.sdkState())\n    }\n  }\n}\n";
        let reported = lines(source);
        assert_eq!(reported.len(), 3, "{reported:?}");
        assert!(reported.iter().any(|line| line
            == "ERROR 10505001 ets/pages/Index.ets:4:18 Duplicate identifier \'sdkState\'."));
        assert!(reported.iter().any(|line| line
            == "ERROR 10505001 ets/pages/Index.ets:6:11 Duplicate identifier \'sdkState\'."));
        assert!(reported.iter().any(|line| line
            == "ERROR 10505001 ets/pages/Index.ets:12:17 This expression is not callable. Type \
               \'String\' has no call signatures."));
    }

    #[test]
    fn an_arrow_function_is_silent() {
        assert!(lines("class A {\n  private tick = () => 1;\n}\n").is_empty());
    }

    #[test]
    fn a_side_effect_kit_import_is_reported_at_the_statement() {
        let source = "import \"@kit.AbilityKit\";\n\n@Entry\n@Component\nstruct Index {\n  build() {\n    Column() {\n    }\n  }\n}\n";
        let reported = lines(source);
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert_eq!(
            reported[0],
            "ERROR 10311008 ets/pages/Index.ets:1:1 Can not use empty import(side-effect \
             import) statement with Kit '@kit.AbilityKit'."
        );
    }

    #[test]
    fn an_unparseable_source_is_skipped_rather_than_judged() {
        assert!(lines("@Entry @@@ struct {").is_empty());
    }

    #[test]
    fn a_decorator_body_the_parser_rejects_does_not_hide_the_declarations() {
        // `@Extend` bodies are attribute chains — `{ .fontSize(12) }` — which no TypeScript
        // parser reads. The field above them is still the file's own declaration.
        let source = "@Extend(Text)\nfunction wide() {\n  .fontSize(12)\n}\n\n\
                      @Entry\n@Component\nstruct Index {\n  private count: number = \"three\";\n\
                      \n  build() {\n    Column() {\n      Text('x')\n    }\n  }\n}\n";
        let reported = lines(source);
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert!(reported[0].starts_with("ERROR 10505001 ets/pages/Index.ets:9:11 "));
    }
}
