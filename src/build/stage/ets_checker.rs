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
    Class, ClassElement, Expression, ImportDeclaration, ImportDeclarationSpecifier,
    MethodDefinitionKind, ModuleExportName, PropertyDefinition, PropertyKey, TSType,
    TSTypeAnnotation,
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
        format!(
            "{} {} {}:{}:{} {}",
            self.severity, self.code, self.file, self.line, self.column, self.message
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
    };
    checker.visit_program(&parsed.program);
    checker.scan_call_sites();
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
        if system_module_declared(request) != Some(false) {
            return;
        }
        self.push(
            "ERROR",
            "10505001",
            it.source.span.start as usize,
            format!("Cannot find module '{request}' or its corresponding type declarations."),
        );
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

    /// A decorated field and a method with the same name: `@State sdkState` and `sdkState()`.
    /// The state field lowers to a `get`/`set` pair, so the runtime keeps a value where the call
    /// site expects a function (`sdkState is not callable`, 2026-11-12).
    ///
    /// The reference reports `Duplicate identifier` once per declaration, at each name.
    /// Measured on `fixtures/member-collision` (7:18 the field, 9:11 the method).
    fn visit_class(&mut self, class: &Class<'a>) {
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
        let source = "@Entry\n@Component\nstruct Index {\n  private value!: number;\n\n\
                      \x20 private tick = function () {\n    return 1;\n  };\n}\n";
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
        let source = "import \"@kit.AbilityKit\";\n\n@Entry\n@Component\nstruct Index {\n}\n";
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
