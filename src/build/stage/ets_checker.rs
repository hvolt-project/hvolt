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

/// Publishes the SDK's system-module names. Called once per build, before any file is checked.
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
}

thread_local! {
    /// Published by the build from the installed SDK (`publish_component_attributes`).
    static COMPONENT_ATTRIBUTES: std::cell::RefCell<Option<ComponentTables>> =
        const { std::cell::RefCell::new(None) };
}

/// Publishes the component tables; see `ets_assemble::publish_component_attributes`.
pub(crate) fn set_component_attributes(
    attributes: std::collections::HashMap<String, std::collections::HashSet<String>>,
    common: std::collections::HashSet<String>,
    parents: std::collections::HashMap<String, Vec<String>>,
) {
    COMPONENT_ATTRIBUTES.with(|cell| {
        *cell.borrow_mut() = Some(ComponentTables {
            attributes,
            common,
            parents,
        })
    });
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
        file: file.to_string(),
        source,
        line_starts: line_starts(source),
        out: Vec::new(),
        data_fields: Vec::new(),
        every_data_field: Vec::new(),
        lints: file.ends_with(".ets"),
        unannotated_fns: unannotated_functions(&parsed.program),
        class_names: class_names(&parsed.program),
        method_depth: 0,
    };
    checker.visit_program(&parsed.program);
    checker.scan_call_sites();
    // `validate_ui_syntax`, file level: a page must carry exactly one `@Entry`. Two rules are
    // reported together when there are two, both against the file rather than a position
    // (`fixtures/ui-entry-count`). Only pages are judged — the message itself says "a page
    // configured in main_pages.json or build-profile.json5".
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
    /// How many class methods enclose the node being visited. `this` is legal inside a method and
    /// rejected inside a stand-alone function (`arkts-no-standalone-this`).
    method_depth: usize,
}

impl Checker<'_> {
    /// `process_system_module.js`: an `@ohos.*` / `@system.*` request the SDK does not declare
    /// has no declaration file behind it. The reference reports it at the specifier instead of
    /// letting the runtime fail to resolve it (measured on `fixtures/ohos-missing-module`).
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

impl<'a> Visit<'a> for Checker<'_> {
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
    fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
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

    /// `arkts-no-untyped-obj-literals` (10605038): an object literal with no contextual type, at
    /// the `{`. Measured on `fixtures/arkts-object-shapes`: `return { ...a }` (2:10) and
    /// `const o = { … }` without an annotation (10:13); the same literal passed to a typed
    /// parameter is accepted, so only these two positions are judged.
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        // A type *literal* annotation declares no class or interface, so the initialiser still has
        // no contextual type: the reference reports both the type literal (10605040) and this
        // (measured at 17:16 and 17:32 of `fixtures/arkts-type-shapes`).
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
            if let Some(Expression::ObjectExpression(object)) = &it.argument {
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
        oxc_ast_visit::walk::walk_method_definition(self, it);
        self.method_depth -= 1;
    }

    /// Two rules about functions, both anchored on the function itself:
    ///
    /// · `arkts-no-standalone-this` (10605093) — `this` in a function that is not a method, at the
    ///   `this` (15:10 in the fixture);
    /// · `arkts-no-implicit-return-types` (10605090) — a function with no return type that
    ///   returns `this` or a call to another unannotated function, at the function name (14:10).
    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
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
                    if returns_limited_type(body, &self.unannotated_fns) {
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

/// `10905201`: a component nested where only some parents are allowed.
///
/// The restriction comes from the SDK (`components/*.json` has `parents` for 18 components, `Span`
/// among them), and the reference reports the child at its own identifier with the allowed parents
/// listed (`fixtures/ui-nested-parent`: `Span` in a `Column`, 8:7).
fn nesting_violations(source: &str, body: (usize, usize)) -> Vec<(usize, String, String)> {
    let (start, end) = body;
    let text = &source[start..end];
    let bytes = text.as_bytes();
    // Every `Name(…) {` block in the body: `(name, block_start, block_end)`.
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

/// `(structures carrying @Entry, structures carrying @Component)` in this file.
///
/// The component count is what makes the page rules apply at all: a page is a file with ArkUI
/// structures in it, and a plain TypeScript file that merely sits under `pages/` is not one.
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

/// Whether a body returns a `this` or a call to a function whose return type is itself inferred.
fn returns_limited_type(
    body: &FunctionBody<'_>,
    unannotated: &std::collections::BTreeSet<String>,
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
