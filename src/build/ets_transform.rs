//! Assembly-stage transforms, located on the AST.
//!
//! The passes that run after lowering used to find their targets by scanning the token stream:
//! a `from` here, a `builder:` there. The node that carries the decision already exists in the
//! parsed program, so this module walks it and reports the span to edit. The edit itself stays
//! a span replacement, which keeps the layout and the source map exactly as the lowering left
//! them; re-printing the program would move every line and invalidate both.

use std::collections::HashSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::{Program, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

/// One module specifier: the request as written, and the span of its quoted literal.
pub(crate) struct Specifier {
    pub(crate) request: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// The statement opens with `import type` / `export type`.
    pub(crate) type_only: bool,
}

/// Every module specifier in `text`, or `None` when the program does not parse.
///
/// The lowering hands this module valid TypeScript, so a parse failure is a defect rather than
/// an input the caller should fall back from; the caller reports it.
pub(crate) fn module_specifiers(text: &str) -> Option<Vec<Specifier>> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    Some(specifiers_in(&ret.program))
}

fn specifiers_in(program: &Program<'_>) -> Vec<Specifier> {
    let mut out = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(decl) => {
                // `import type …` carries the modifier on the kind.
                push(&mut out, &decl.source, decl.import_kind.is_type());
            }
            Statement::ExportNamedDeclaration(export) => {
                let type_only = export.export_kind.is_type();
                if let Some(source) = &export.source {
                    push(&mut out, source, type_only);
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let type_only = export.export_kind.is_type();
                push(&mut out, &export.source, type_only);
            }
            _ => {}
        }
    }
    out
}

fn push(out: &mut Vec<Specifier>, source: &oxc_ast::ast::StringLiteral<'_>, type_only: bool) {
    out.push(Specifier {
        request: source.value.to_string(),
        start: source.span.start as usize,
        end: source.span.end as usize,
        type_only,
    });
}

/// The requests a file loads at runtime: one per statement that keeps something the program
/// runs.
///
/// Resolution is per request, not per statement: once a request is loaded, every statement
/// that names it is addressed by the same module url, the type-only copies included. The
/// reference does the same (`proxy_core/Index.ets` writes `export type { UpdateConfigParams }`
/// with the url of the value statement that names the same `models/ClashConfig`), while a
/// request nothing loads keeps the path as written (`FavoriteProxy.ts`:
/// `import type … from '../../common/datasources/ProxyData'`, although `ProxyData` is
/// compiled).
///
/// The reading is exact after [`rewrite_imports`] has split the statements: what it keeps as a
/// value statement is what the file runs, and what it writes as `import type` the compiler
/// erases.
pub(crate) fn loaded_requests(specifiers: &[Specifier]) -> HashSet<&str> {
    specifiers
        .iter()
        .filter(|specifier| !specifier.type_only)
        .map(|specifier| specifier.request.as_str())
        .collect()
}

/// Module requests found by scanning the token stream.
///
/// For callers that walk raw project sources: an `.ets` file contains ArkTS constructs
/// (`struct`, builders) that the TypeScript parser rejects, and these callers only need the
/// strings a statement points at, not the tree.
pub(crate) fn module_requests(text: &str) -> Vec<String> {
    use crate::build::lex::Kind;
    let tokens = crate::build::lex::tokenize(text);
    let mut out = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident {
            continue;
        }
        let literal = match token.text {
            "from" => tokens.get(index + 1),
            "import" => tokens.get(index + 1).filter(|next| next.kind == Kind::Str),
            _ => continue,
        };
        if let Some(literal) = literal.filter(|t| t.kind == Kind::Str) {
            out.push(
                literal
                    .text
                    .trim_matches(|c| c == '\'' || c == '"')
                    .to_string(),
            );
        }
    }
    out
}

/// One import (or `export … from`) statement, read from the tree.
pub(crate) struct ImportStatement {
    /// The whole statement, including its `;`.
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) is_export: bool,
    /// The statement itself is written `import type` / `export type`.
    pub(crate) type_only: bool,
    pub(crate) default_binding: Option<String>,
    pub(crate) request: String,
    /// Span of the quoted module literal, so a request can be replaced without re-printing
    /// the statement (which would drop the comments inside the binding list).
    pub(crate) request_start: usize,
    pub(crate) request_end: usize,
    /// `(imported name, local name)` for each named binding.
    pub(crate) bindings: Vec<(String, String)>,
    /// `import * as name`: the whole module, so no per-name binding list decides it.
    pub(crate) namespace: Option<String>,
    /// `export * from …`: the same, on the re-export side.
    pub(crate) star: bool,
    /// `export * as name from …`: the star carries a name of its own.
    pub(crate) namespace_export: Option<String>,
    /// Bindings the source itself spelled `type` (`{ type Foo, Bar }`): they are types whatever
    /// the module does with them.
    pub(crate) inline_types: HashSet<String>,
    /// The names the source wrote with an explicit `as` (`Foo as Foo`, not `Foo`).
    ///
    /// The reference rebuilds a clause from the source's own specifier nodes, `propertyName`
    /// included (`transformTypeExportImportAndConstEnumInTypeScript` reuses the node), so the
    /// spelling survives even when the alias repeats the name
    /// (`import type { ProxyItem as ProxyItem }`). The pair alone cannot tell the two spellings
    /// apart; the span can, because the parser clones one node for `Foo` and keeps two for
    /// `Foo as Foo`.
    pub(crate) explicit_alias: HashSet<String>,
}

/// Every top-level import/re-export statement, or `None` when the program does not parse.
pub(crate) fn import_statements(text: &str) -> Option<Vec<ImportStatement>> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    Some(import_statements_in(text, &ret.program))
}

fn import_statements_in(text: &str, program: &Program<'_>) -> Vec<ImportStatement> {
    let mut out = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(decl) => {
                let mut bindings = Vec::new();
                let mut default_binding = None;
                let mut namespace = None;
                let mut inline_types = HashSet::new();
                let mut explicit_alias = HashSet::new();
                for specifier in decl.specifiers.iter().flatten() {
                    match specifier {
                        oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(named) => {
                            use oxc_span::GetSpan;
                            let imported = named.imported.name().to_string();
                            if named.import_kind.is_type() {
                                inline_types.insert(imported.clone());
                            }
                            let local = named.local.name.to_string();
                            if named.imported.span() != named.local.span {
                                explicit_alias.insert(local.clone());
                            }
                            bindings.push((imported, local));
                        }
                        oxc_ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(
                            default,
                        ) => {
                            default_binding = Some(default.local.name.to_string());
                        }
                        oxc_ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(ns) => {
                            namespace = Some(ns.local.name.to_string());
                        }
                    }
                }
                out.push(ImportStatement {
                    start: decl.span.start as usize,
                    end: statement_end(text, decl.span.end as usize),
                    is_export: false,
                    type_only: decl.import_kind.is_type(),
                    default_binding,
                    request: decl.source.value.to_string(),
                    request_start: decl.source.span.start as usize,
                    request_end: decl.source.span.end as usize,
                    bindings,
                    namespace,
                    star: false,
                    namespace_export: None,
                    inline_types,
                    explicit_alias,
                });
            }
            Statement::ExportNamedDeclaration(export) => {
                let Some(source) = &export.source else {
                    continue;
                };
                let mut inline_types = HashSet::new();
                let mut explicit_alias = HashSet::new();
                let bindings = export
                    .specifiers
                    .iter()
                    .map(|specifier| {
                        use oxc_span::GetSpan;
                        let local = specifier.local.name().to_string();
                        if specifier.export_kind.is_type() {
                            inline_types.insert(local.clone());
                        }
                        let exported = specifier.exported.name().to_string();
                        if specifier.local.span() != specifier.exported.span() {
                            explicit_alias.insert(exported.clone());
                        }
                        (local, exported)
                    })
                    .collect();
                out.push(ImportStatement {
                    start: export.span.start as usize,
                    end: statement_end(text, export.span.end as usize),
                    is_export: true,
                    type_only: export.export_kind.is_type(),
                    default_binding: None,
                    request: source.value.to_string(),
                    request_start: source.span.start as usize,
                    request_end: source.span.end as usize,
                    bindings,
                    namespace: None,
                    star: false,
                    namespace_export: None,
                    inline_types,
                    explicit_alias,
                });
            }
            Statement::ExportAllDeclaration(export) => {
                out.push(ImportStatement {
                    start: export.span.start as usize,
                    end: statement_end(text, export.span.end as usize),
                    is_export: true,
                    type_only: export.export_kind.is_type(),
                    default_binding: None,
                    request: export.source.value.to_string(),
                    request_start: export.source.span.start as usize,
                    request_end: export.source.span.end as usize,
                    bindings: Vec::new(),
                    namespace: None,
                    star: true,
                    namespace_export: export.exported.as_ref().map(|name| name.name().to_string()),
                    inline_types: HashSet::new(),
                    explicit_alias: HashSet::new(),
                });
            }
            _ => {}
        }
    }
    out
}

/// The end of a statement, `;` included: the parser's span may stop before it.
fn statement_end(text: &str, end: usize) -> usize {
    let mut cursor = end;
    let bytes = text.as_bytes();
    while cursor < bytes.len() && (bytes[cursor] == b' ' || bytes[cursor] == b'\t') {
        cursor += 1;
    }
    if cursor < bytes.len() && bytes[cursor] == b';' {
        cursor += 1;
    }
    cursor
}

/// The `export { … }` statements a module prints as `export type { … }`.
///
/// The reference prints a local export clause that names nothing the program runs as a type-only
/// export (`process_kit_import.js` runs `ts.getTypeExportImportAndConstEnumTransformer` over
/// every file before the kit clauses are rewritten), and such a clause then declares no runtime
/// binding: es2abc writes no `LOCAL_EXPORT` module tag for it. `chartInterface.ts` exports nine
/// interfaces through a value clause, and the spelling adds nine tags the reference does not
/// have (measured 2026-11-07: `lines 48/57`, `mnemonic_blocks 1`).
///
/// The decision stays with what this file can see: an interface or a type alias declared here,
/// or a binding an `import type` clause (or an inline `type` specifier) brought in. A clause
/// that names anything else is left as written — a re-export of a sibling module's type is
/// settled by `declares_type` when the clause names the module it comes from, and this one
/// names no module at all. An inline `type` marker is left as written too: the clause is
/// already type-only for the compiler, so rewriting it would add a spelling the reference never
/// prints.
///
/// The offset is the `export` keyword's end: `export type { … }` is the same statement with one
/// word added, and re-printing would move every line the file already has.
fn type_only_local_exports(
    program: &Program<'_>,
    statements: &[ImportStatement],
) -> Vec<(usize, usize, String)> {
    let mut types: HashSet<String> = HashSet::new();
    for statement in &program.body {
        let declaration = match statement {
            Statement::TSInterfaceDeclaration(declaration) => {
                types.insert(declaration.id.name.to_string());
                continue;
            }
            Statement::TSTypeAliasDeclaration(declaration) => {
                types.insert(declaration.id.name.to_string());
                continue;
            }
            // `export interface X` is the same declaration behind a wrapper.
            Statement::ExportNamedDeclaration(export) => export.declaration.as_ref(),
            _ => None,
        };
        match declaration {
            Some(oxc_ast::ast::Declaration::TSInterfaceDeclaration(declaration)) => {
                types.insert(declaration.id.name.to_string());
            }
            Some(oxc_ast::ast::Declaration::TSTypeAliasDeclaration(declaration)) => {
                types.insert(declaration.id.name.to_string());
            }
            _ => {}
        }
    }
    // A name the file imported as a type is a type where it is re-exported as well.
    for statement in statements {
        if statement.is_export {
            continue;
        }
        types.extend(statement.inline_types.iter().cloned());
        if statement.type_only {
            types.extend(
                statement
                    .bindings
                    .iter()
                    .map(|(_, local)| local.clone())
                    .chain(statement.default_binding.iter().cloned()),
            );
        }
    }
    let mut edits = Vec::new();
    for statement in &program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        if export.export_kind.is_type()
            || export.source.is_some()
            || export.declaration.is_some()
            || export.specifiers.is_empty()
        {
            continue;
        }
        let every_name_is_a_type = export.specifiers.iter().all(|specifier| {
            !specifier.export_kind.is_type() && types.contains(specifier.local.name().as_str())
        });
        if every_name_is_a_type {
            let at = export.span.start as usize + "export".len();
            edits.push((at, at, " type".to_string()));
        }
    }
    edits
}

/// The names a module reads, split by how it reads them.
pub(crate) struct UsedNames {
    /// Referenced where the program needs a value (`new Foo()`, `Foo.member`, `foo`).
    pub(crate) value: HashSet<String>,
    /// Referenced anywhere, type positions included.
    pub(crate) all: HashSet<String>,
}

/// The two readings of one parsed module. `value` is the resolved reading — a name a local
/// declaration answers is not a value read of an import spelled the same way ([`value_uses`]) —
/// so the pass takes the decision the reference's emit resolver makes.
pub(crate) fn used_names(program: &Program<'_>) -> UsedNames {
    use oxc_ast_visit::{walk, Visit};

    struct Collector {
        all: HashSet<String>,
        reexported: HashSet<String>,
    }

    impl<'a> Visit<'a> for Collector {
        fn visit_identifier_reference(&mut self, ident: &oxc_ast::ast::IdentifierReference<'a>) {
            self.all.insert(ident.name.to_string());
        }

        // An import statement names its own bindings; naming a binding is not reading it.
        fn visit_import_declaration(&mut self, _: &oxc_ast::ast::ImportDeclaration<'a>) {}

        // `export { X }` hands a local binding to the module's consumers, so the value is
        // needed even though no expression in this file reads it.
        fn visit_export_named_declaration(
            &mut self,
            declaration: &oxc_ast::ast::ExportNamedDeclaration<'a>,
        ) {
            if declaration.source.is_none() && !declaration.export_kind.is_type() {
                for specifier in &declaration.specifiers {
                    self.reexported.insert(specifier.local.name().to_string());
                }
            }
            walk::walk_export_named_declaration(self, declaration);
        }
    }

    let mut collector = Collector {
        all: HashSet::new(),
        reexported: HashSet::new(),
    };
    collector.visit_program(program);
    let mut value = value_uses(program);
    value.extend(collector.reexported.iter().cloned());
    collector.all.extend(collector.reexported);
    UsedNames {
        value,
        all: collector.all,
    }
}

/// The names a module reads in a value position, resolved against the module's own scopes.
///
/// The reading is the lowering's own — a name written inside a type is not a value — with one
/// difference: a reference a *local* declaration answers is not a read of an import that happens
/// to have the same spelling. `GlobalContext.ets` writes `initWindow(window: window.Window)` and
/// calls `window.getWindowProperties()` inside: every `window` there is the parameter, while the
/// `window` the file takes from `@kit.ArkUI` appears only under a type.
///
/// The reference asks its emit resolver for exactly that (`process_kit_import.js`:
/// `needSkipTypeSymbolOfNode` → `isReferencedAliasDeclaration`), which answers for the symbol a
/// reference binds to rather than for its spelling, and so prints `import type window from
/// "@ohos:window"`. A reading by name calls the parameter's uses reads of the import, prints a
/// value import, and es2abc then keeps `@ohos:window` in the module request array with a
/// `REGULAR_IMPORT` tag behind it (measured 2026-11-07: `lines 296/298`, `mnemonic_blocks 7`).
///
/// The scopes are the ones an expression can bind into: a function's parameters, a function
/// body, a block, a `for` header, a `switch` body and a `catch` parameter. The module's own top
/// level is not one of them — an import is a module-scope binding there, and a second
/// declaration of the same spelling is an error rather than a shadow.
fn value_uses(program: &Program<'_>) -> HashSet<String> {
    use oxc_ast::ast_kind::AstKind;
    use oxc_ast_visit::{walk, Visit};

    struct Finder {
        /// One entry per scope currently open, holding the names it declares.
        scopes: Vec<HashSet<String>>,
        names: HashSet<String>,
    }

    impl Finder {
        fn is_shadowed(&self, name: &str) -> bool {
            self.scopes.iter().any(|scope| scope.contains(name))
        }
    }

    impl<'a> Visit<'a> for Finder {
        // Scopes are tracked on the node boundary the walker already crosses, so the stack
        // follows the nesting of the tree: every `enter_node` with a scope has its `leave_node`
        // in the same subtree.
        fn enter_node(&mut self, kind: AstKind<'a>) {
            if let Some(declared) = scope_bindings(kind) {
                self.scopes.push(declared);
            }
        }

        fn leave_node(&mut self, kind: AstKind<'a>) {
            if opens_scope(kind) {
                self.scopes.pop();
            }
        }

        fn visit_identifier_reference(&mut self, ident: &oxc_ast::ast::IdentifierReference<'a>) {
            if !self.is_shadowed(ident.name.as_str()) {
                self.names.insert(ident.name.to_string());
            }
        }

        // A type is erased at run time; nothing inside it keeps a binding alive.
        fn visit_ts_type(&mut self, _ty: &oxc_ast::ast::TSType<'a>) {}
        fn visit_ts_type_annotation(&mut self, _annotation: &oxc_ast::ast::TSTypeAnnotation<'a>) {}
        // `class A implements B` is erased with the class: the clause names a type even
        // though the parser spells it as a type name rather than a `TSType`
        // (`TSClassImplements.expression`).
        fn visit_ts_class_implements(&mut self, _implements: &oxc_ast::ast::TSClassImplements<'a>) {
        }
        fn visit_ts_type_parameter_instantiation(
            &mut self,
            _arguments: &oxc_ast::ast::TSTypeParameterInstantiation<'a>,
        ) {
        }
        fn visit_ts_interface_declaration(
            &mut self,
            declaration: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            // The heritage clause names values; the body only names types.
            for clause in &declaration.extends {
                walk::walk_ts_interface_heritage(self, clause);
            }
        }
        fn visit_ts_type_alias_declaration(
            &mut self,
            _declaration: &oxc_ast::ast::TSTypeAliasDeclaration<'a>,
        ) {
        }
    }

    let mut finder = Finder {
        scopes: Vec::new(),
        names: HashSet::new(),
    };
    finder.visit_program(program);
    finder.names
}

/// The names a node declares in the scope it opens, or `None` when it opens none.
fn scope_bindings(kind: oxc_ast::ast_kind::AstKind<'_>) -> Option<HashSet<String>> {
    use oxc_ast::ast_kind::AstKind;

    let mut names = HashSet::new();
    match kind {
        AstKind::Function(function) => {
            for parameter in &function.params.items {
                collect_pattern(&parameter.pattern, &mut names);
            }
        }
        AstKind::ArrowFunctionExpression(arrow) => {
            for parameter in &arrow.params.items {
                collect_pattern(&parameter.pattern, &mut names);
            }
        }
        AstKind::FunctionBody(body) => collect_declarations(&body.statements, &mut names),
        AstKind::BlockStatement(block) => collect_declarations(&block.body, &mut names),
        AstKind::CatchClause(clause) => {
            if let Some(param) = &clause.param {
                collect_pattern(&param.pattern, &mut names);
            }
        }
        AstKind::ForStatement(statement) => {
            if let Some(oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration)) =
                &statement.init
            {
                collect_variables(declaration, &mut names);
            }
        }
        AstKind::ForInStatement(statement) => {
            if let oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) =
                &statement.left
            {
                collect_variables(declaration, &mut names);
            }
        }
        AstKind::ForOfStatement(statement) => {
            if let oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) =
                &statement.left
            {
                collect_variables(declaration, &mut names);
            }
        }
        AstKind::SwitchStatement(statement) => {
            for case in &statement.cases {
                collect_declarations(&case.consequent, &mut names);
            }
        }
        _ => return None,
    }
    Some(names)
}

/// Whether a node opens a scope in [`scope_bindings`].
fn opens_scope(kind: oxc_ast::ast_kind::AstKind<'_>) -> bool {
    use oxc_ast::ast_kind::AstKind;

    matches!(
        kind,
        AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_)
            | AstKind::FunctionBody(_)
            | AstKind::BlockStatement(_)
            | AstKind::CatchClause(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::SwitchStatement(_)
    )
}

/// The names a statement list declares in the scope it opens.
fn collect_declarations(statements: &[oxc_ast::ast::Statement<'_>], out: &mut HashSet<String>) {
    use oxc_ast::ast::Statement;

    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) => collect_variables(declaration, out),
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    out.insert(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    out.insert(id.name.to_string());
                }
            }
            Statement::TSEnumDeclaration(declaration) => {
                out.insert(declaration.id.name.to_string());
            }
            // `export class X` declares `X` in this scope as much as a bare `class X` does.
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    collect_declaration(declaration, out);
                }
            }
            _ => {}
        }
    }
}

/// The names one declaration binds.
fn collect_declaration(declaration: &oxc_ast::ast::Declaration<'_>, out: &mut HashSet<String>) {
    use oxc_ast::ast::Declaration;

    match declaration {
        Declaration::VariableDeclaration(declaration) => collect_variables(declaration, out),
        Declaration::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                out.insert(id.name.to_string());
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                out.insert(id.name.to_string());
            }
        }
        Declaration::TSEnumDeclaration(declaration) => {
            out.insert(declaration.id.name.to_string());
        }
        _ => {}
    }
}

/// The names a `var`/`let`/`const` statement binds.
fn collect_variables(
    declaration: &oxc_ast::ast::VariableDeclaration<'_>,
    out: &mut HashSet<String>,
) {
    for declarator in &declaration.declarations {
        collect_pattern(&declarator.id, out);
    }
}

/// The names a binding pattern binds, destructuring included.
fn collect_pattern(pattern: &oxc_ast::ast::BindingPattern<'_>, out: &mut HashSet<String>) {
    use oxc_ast::ast::BindingPatternKind;

    match &pattern.kind {
        BindingPatternKind::BindingIdentifier(id) => {
            out.insert(id.name.to_string());
        }
        BindingPatternKind::ObjectPattern(object) => {
            for property in &object.properties {
                collect_pattern(&property.value, out);
            }
            if let Some(rest) = &object.rest {
                collect_pattern(&rest.argument, out);
            }
        }
        BindingPatternKind::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_pattern(element, out);
            }
            if let Some(rest) = &array.rest {
                collect_pattern(&rest.argument, out);
            }
        }
        BindingPatternKind::AssignmentPattern(assignment) => {
            collect_pattern(&assignment.left, out);
        }
    }
}

/// Settle the statements the lowering produced itself: the per-symbol lines a `@kit.*`
/// import expands into, plus any other system module (`@ohos:…`, `@hms:…`, `@native:…`).
///
/// Those requests need no url resolution, so the layer that knows which bindings the file
/// reads can finish them on the spot: an unread symbol is erased and a type-only one keeps
/// the `import type` the expansion gave it. Everything else — a relative path or a package
/// request — is left for the assembly pass, which owns the url space *and* is the only place
/// that can still tell a written `import type` from one this pipeline inferred
/// (`fixtures/import-type-audit` vs `fixtures/import-forms`).
pub(crate) fn settle_system_imports(text: &str, source: &str) -> String {
    rewrite(text, Some(source), &|_| None, &|_, _| false, Scope::System)
        .unwrap_or_else(|| text.to_string())
}

/// Whether a request is addressed by the loader's system-module form.
fn is_system_request(request: &str) -> bool {
    ["@ohos", "@hms", "@system", "@arkts"]
        .iter()
        .any(|ns| request.starts_with(&format!("{ns}:")) || request.starts_with(&format!("{ns}.")))
}

/// Which statements a pass settles.
enum Scope {
    /// Only the system modules: what the lowering itself emitted.
    System,
    /// Every statement, the module-relative requests included (the assembly pass).
    All,
}

/// Emit every import and re-export statement the way the reference compiler does.
///
/// For each statement:
///   - a binding the module never reads is erased (the reference prints the program after
///     TypeScript's import elision, so an unread name leaves no statement behind);
///   - the bindings that remain are split into the ones read as values and the ones read only
///     as types, each group becoming its own statement — values first, the type group written
///     `import type { … }` (`entry/entryability/EntryAbility.ts` in the reference cache splits
///     `import { AppConfig, AppState, ClashCore }` into those two exact shapes);
///   - a statement that keeps no value binding is left addressed by the request as written:
///     the reference resolves module urls only for the imports the program runs, which is why
///     `import type { CustomThemeName } from "xb_components/…/AppThemeManager"` keeps its
///     package path while the value half of the same statement gets `@normalized:…`;
///   - `request` normalizes the request of a statement that does keep a value binding;
///   - `declares_type` decides a re-exported name's side from the module it comes from, since
///     a re-export is never read locally.
///
/// `None` means the text does not parse; the caller keeps it as it stands.
#[allow(dead_code)] // the assembly call site lands with the import-emission handoff
pub(crate) fn rewrite_imports(
    text: &str,
    source: Option<&str>,
    request: &dyn Fn(&str) -> Option<String>,
    declares_type: &dyn Fn(&str, &str) -> bool,
) -> Option<String> {
    rewrite(text, source, request, declares_type, Scope::All)
}

fn rewrite(
    text: &str,
    source: Option<&str>,
    request: &dyn Fn(&str) -> Option<String>,
    declares_type: &dyn Fn(&str, &str) -> bool,
    scope: Scope,
) -> Option<String> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    let mut used = used_names(&ret.program);
    let statements = import_statements_in(text, &ret.program);
    // The lowering erases some decorators (`@InsightIntentLink` goes with the struct), and the
    // reference's usage analysis runs on the source, so a name the source reads keeps its
    // import even when the lowered text no longer mentions it (`fixtures/user-intent`).
    if let Some(source) = source {
        rescue_from_source(source, &statements, &mut used);
    }
    // The module-url resolution is a property of the request, not of the statement: a file
    // that runs `proxy_core` somewhere addresses every `proxy_core` statement by the same url,
    // including the type-only half (`components/Common/Common.ts` carries
    // `import type { Provider } from "@normalized:N&&&proxy_core/Index&1.0.0"` next to the
    // value import that made the request resolvable).
    let mut resolved: HashSet<String> = HashSet::new();
    for statement in &statements {
        if classify(statement, &used, declares_type).keeps_value {
            resolved.insert(statement.request.clone());
        }
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    // A re-export written above the file's first class keeps that position: the reference
    // emits declarations in source order, while the component lowering defers `export { … }`
    // statements to the end of the file (it defers names it cannot resolve yet). The early
    // exports are expanded there and re-inserted before the first class in source order.
    let early = source.map(early_kit_export_keys).unwrap_or_default();
    let mut early_blocks: Vec<(usize, String)> = Vec::new();
    for statement in &statements {
        // A re-export from a Kit names the Kit, not a module the runtime has: the reference
        // expands it into one statement per underlying module before the file is printed
        // (`process_kit_import.js`: `ExportSpecifierKitInfo` / `ExportStarKitInfo`).
        if statement.is_export && statement.request.starts_with("@kit.") {
            if let Some(replacement) = render_kit_export(text, statement) {
                if let Some(position) = early.iter().position(|key| key.matches(statement)) {
                    if !replacement.is_empty() {
                        early_blocks.push((position, replacement));
                    }
                    let mut end = statement.end;
                    if text[end..].starts_with("\r\n") {
                        end += 2;
                    } else if text[end..].starts_with('\n') {
                        end += 1;
                    }
                    edits.push((statement.start, end, String::new()));
                    continue;
                }
                let mut end = statement.end;
                if replacement.is_empty() {
                    if text[end..].starts_with("\r\n") {
                        end += 2;
                    } else if text[end..].starts_with('\n') {
                        end += 1;
                    }
                }
                edits.push((statement.start, end, replacement));
                continue;
            }
        }
        if matches!(scope, Scope::System) && !is_system_request(&statement.request) {
            continue;
        }
        let rewrite = resolved.contains(&statement.request);
        let replacement = render_import(text, statement, &used, request, declares_type, rewrite);
        let mut end = statement.end;
        // An erased statement takes its line with it, so a file whose imports were all unused
        // does not accumulate blank lines.
        if replacement.is_empty() {
            if text[end..].starts_with("\r\n") {
                end += 2;
            } else if text[end..].starts_with('\n') {
                end += 1;
            }
        }
        edits.push((statement.start, end, replacement));
    }
    edits.extend(type_only_local_exports(&ret.program, &statements));
    if !early_blocks.is_empty() {
        if let Some(anchor) = first_class_start(&ret.program) {
            early_blocks.sort_by_key(|(position, _)| *position);
            let block = early_blocks
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            edits.push((anchor, anchor, format!("{block}\n")));
        }
    }
    Some(crate::build::ets_assemble::apply_replacements(text, &edits))
}

/// Re-root the relative requests a module entry leaves as written, for the flat layout.
///
/// The flat layout stages a module as `<module>/<rel>` where the reference stages
/// `<module>/src/main/ets/<rel>`; only the package entry keeps the module root on both sides
/// (`xb_components/Index.ts`, `proxy_core/Index.ts`). A relative request the import pass left
/// as written is type-only by then (a value request resolves to a module url), so the
/// compiler erases the statement and the text can take the reference's shape: the reference's
/// source sits at the module root and names the source root explicitly
/// (`xb_components/Index.ets`: `./src/main/ets/controllers/ControllerEventHandler`), which
/// the flat source could not (`./controllers/ControllerEventHandler`).
///
/// A request that already names the source root is left alone, so the pass is idempotent on a
/// source tree laid out the way the reference expects.
pub(crate) fn reroot_entry_requests(text: &str) -> Option<String> {
    let specifiers = module_specifiers(text)?;
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for specifier in specifiers {
        let Some(rest) = specifier.request.strip_prefix("./") else {
            continue;
        };
        if rest == crate::build::compile::abc_compile::MODULE_SOURCE_ROOT
            || rest.starts_with(&format!(
                "{}/",
                crate::build::compile::abc_compile::MODULE_SOURCE_ROOT
            ))
        {
            continue;
        }
        let quote = &text[specifier.start..specifier.start + 1];
        let rooted = format!(
            "{quote}./{}/{rest}{quote}",
            crate::build::compile::abc_compile::MODULE_SOURCE_ROOT
        );
        edits.push((specifier.start, specifier.end, rooted));
    }
    if edits.is_empty() {
        return Some(text.to_string());
    }
    Some(crate::build::ets_assemble::apply_replacements(text, &edits))
}

/// A Kit re-export read from the source, before the lowering moved it.
///
/// The lowering defers `export { … }` statements to the end of the file, so a re-export the
/// source wrote above the first class has to be recognised on the source and put back. The key
/// is the statement's shape — request, clause, and the bindings in order — because the lowered
/// text no longer carries the source position.
struct KitExportKey {
    request: String,
    star: bool,
    type_only: bool,
    bindings: Vec<(String, String)>,
    namespace: Option<String>,
}

impl KitExportKey {
    fn matches(&self, statement: &ImportStatement) -> bool {
        self.request == statement.request
            && self.star == statement.star
            && self.type_only == statement.type_only
            && self.bindings == statement.bindings
            && self.namespace == statement.namespace_export
    }
}

/// The Kit re-exports the source writes above its first class, in source order.
fn early_kit_export_keys(source: &str) -> Vec<KitExportKey> {
    let prepared = prepared_source(source);
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &prepared, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return Vec::new();
    }
    let mut keys = Vec::new();
    let mut has_class = false;
    for statement in &ret.program.body {
        if matches!(statement, Statement::ClassDeclaration(_)) {
            has_class = true;
            break;
        }
        match statement {
            // `export class X` is a class statement with a wrapper; it ends the "before the
            // first class" range as much as a bare `class` does.
            Statement::ExportNamedDeclaration(export) if export.declaration.is_some() => {
                has_class = true;
                break;
            }
            Statement::ExportNamedDeclaration(export) => {
                let Some(source) = &export.source else {
                    continue;
                };
                if !source.value.starts_with("@kit.") {
                    continue;
                }
                keys.push(KitExportKey {
                    request: source.value.to_string(),
                    star: false,
                    type_only: export.export_kind.is_type(),
                    bindings: export
                        .specifiers
                        .iter()
                        .map(|specifier| {
                            (
                                specifier.local.name().to_string(),
                                specifier.exported.name().to_string(),
                            )
                        })
                        .collect(),
                    namespace: None,
                });
            }
            Statement::ExportAllDeclaration(export) => {
                if !export.source.value.starts_with("@kit.") {
                    continue;
                }
                keys.push(KitExportKey {
                    request: export.source.value.to_string(),
                    star: true,
                    type_only: export.export_kind.is_type(),
                    bindings: Vec::new(),
                    namespace: export.exported.as_ref().map(|name| name.name().to_string()),
                });
            }
            _ => {}
        }
    }
    // A file with no class has no deferral to compensate for: the exports are already in
    // source order, and moving them would reorder the file.
    if !has_class {
        return Vec::new();
    }
    keys
}

/// Where the first top-level class starts, the anchor a source-order re-export is put back at.
fn first_class_start(program: &Program<'_>) -> Option<usize> {
    for statement in &program.body {
        match statement {
            Statement::ClassDeclaration(class) => return Some(class.span.start as usize),
            Statement::ExportNamedDeclaration(export) => {
                if let Some(oxc_ast::ast::Declaration::ClassDeclaration(class)) =
                    &export.declaration
                {
                    return Some(class.span.start as usize);
                }
            }
            _ => {}
        }
    }
    None
}

/// One `export … from '@kit.X'` statement as the module statements it stands for.
///
/// `ExportSpecifierKitInfo.transform` groups the named bindings by the module each Kit symbol
/// comes from and writes one `export { … } from "<module>";` per group; the exported name is the
/// config's `bindings` field whenever the clause renamed it (`export { curves }` with
/// `bindings: "default"` becomes `export { default as curves }`). `ExportStarKitInfo.transform`
/// writes one `export * from "<module>";` per distinct module, in the config file's order.
///
/// `None` when the Kit has no config: the reference reports
/// `ETS2BUNDLE_EXTERNAL_KIT_CONFIG_FILE_NOT_FOUND` and leaves the statement as written. A
/// namespace clause takes the namespace error and is dropped
/// (`ETS2BUNDLE_EXTERNAL_KIT_NAMESPACE_IMPORT_EXPORT_NOT_SUPPORTED`), matching the empty
/// replacement this returns for it.
fn render_kit_export(text: &str, statement: &ImportStatement) -> Option<String> {
    let indent = line_indent(text, statement.start);
    let kit = statement.request.as_str();
    let keyword = if statement.type_only {
        "export type "
    } else {
        "export "
    };
    if statement.namespace_export.is_some() {
        // `export * as ArkTS from '@kit.ArkUI'`: the reference reports the error and emits
        // nothing for the statement.
        return Some(String::new());
    }
    if statement.star {
        let sources = crate::build::stage::process_import::kit_sources(kit)?;
        if sources.is_empty() {
            return None;
        }
        return Some(
            sources
                .iter()
                .map(|request| format!("{indent}{keyword}* from \"{request}\";"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let table = crate::build::stage::process_import::kit_table(kit)?;
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (imported, exported) in &statement.bindings {
        // A name the Kit does not export is an error in the reference
        // (`ETS2BUNDLE_EXTERNAL_IMPORT_NAME_NOT_EXPORTED_FROM_KIT`) and contributes no
        // specifier; the rest of the statement still expands.
        let Some((request, bindings)) = table.get(imported) else {
            continue;
        };
        let mut specifier = String::new();
        if statement.inline_types.contains(imported) {
            specifier.push_str("type ");
        }
        if exported == bindings {
            specifier.push_str(exported);
        } else {
            specifier.push_str(&format!("{bindings} as {exported}"));
        }
        match groups
            .iter_mut()
            .find(|(candidate, _)| candidate == request)
        {
            Some((_, specifiers)) => specifiers.push(specifier),
            None => groups.push((request.clone(), vec![specifier])),
        }
    }
    if groups.is_empty() {
        return Some(String::new());
    }
    Some(
        groups
            .iter()
            .map(|(request, specifiers)| {
                format!(
                    "{indent}{keyword}{{ {} }} from \"{request}\";",
                    specifiers.join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// One statement's bindings after the usage decision.
struct Classified {
    values: Vec<(String, String)>,
    types: Vec<(String, String)>,
    default_value: Option<String>,
    default_type: Option<String>,
    /// The statement keeps something the program runs (a value binding, a namespace, a
    /// side effect), so its module request has to resolve to a record url.
    keeps_value: bool,
}

/// Split a statement's bindings into the ones read as values and the ones read only as types.
fn classify(
    statement: &ImportStatement,
    used: &UsedNames,
    declares_type: &dyn Fn(&str, &str) -> bool,
) -> Classified {
    let mut values: Vec<(String, String)> = Vec::new();
    let mut types: Vec<(String, String)> = Vec::new();
    let mut default_value = None;
    let mut default_type = None;
    // A statement the source wrote `import type` names no value by construction: every
    // binding it still reads stays as a type. The ones the module never reads are elided like
    // any other (the reference prints the program after TypeScript's import elision, and a
    // name nothing reads leaves no statement behind).
    if statement.type_only {
        types = statement
            .bindings
            .iter()
            .filter(|(_, local)| used.all.contains(local))
            .cloned()
            .collect();
        default_type = statement
            .default_binding
            .clone()
            .filter(|default| used.all.contains(default));
        return Classified {
            values,
            types,
            default_value,
            default_type,
            keeps_value: false,
        };
    }
    {
        if let Some(default) = &statement.default_binding {
            if used.value.contains(default) {
                default_value = Some(default.clone());
            } else if used.all.contains(default) {
                default_type = Some(default.clone());
            }
        }
        for (first, second) in &statement.bindings {
            // An import binding is read in this file; a re-export names a binding of its
            // target module, which decides whether that name is a value there.
            let is_type = if statement.inline_types.contains(first) {
                // `{ type Foo }` says it outright.
                true
            } else if statement.is_export {
                // A re-export is never read locally: the target decides.
                declares_type(&statement.request, first)
            } else if used.value.contains(second) {
                // A value position is not proof of a runtime export. `implements IClashManager`
                // is a type position that a syntactic reading counts as a value, and an
                // interface has no runtime export; importing one as a value kills module
                // instantiation on the device (`the requested module … does not provide an
                // export name 'IClashManager'`, measured on clashbox). The target's own
                // declaration settles it, with the usage reading as the fallback when the
                // target is not available.
                declares_type(&statement.request, first)
            } else if used.all.contains(second) {
                true
            } else {
                continue;
            };
            let binding = (first.clone(), second.clone());
            if is_type {
                types.push(binding);
            } else {
                values.push(binding);
            }
        }
    }
    // A bare `import './x'` (or `export *`) runs code elsewhere: it has no binding to judge and
    // is kept for its side effect.
    let side_effect = statement.namespace.is_none()
        && !statement.star
        && statement.default_binding.is_none()
        && statement.bindings.is_empty();
    let keeps_value = !statement.type_only
        && (statement.star
            || statement.namespace.is_some()
            || default_value.is_some()
            || !values.is_empty()
            || side_effect);
    Classified {
        values,
        types,
        default_value,
        default_type,
        keeps_value,
    }
}

/// The replacement for one statement: empty when it is erased, one statement per surviving
/// binding group otherwise.
fn render_import(
    text: &str,
    statement: &ImportStatement,
    used: &UsedNames,
    request: &dyn Fn(&str) -> Option<String>,
    declares_type: &dyn Fn(&str, &str) -> bool,
    resolved: bool,
) -> String {
    // The source's own `import type` / `export type` is a decision this pass reproduces, not
    // one it re-derives: the reference keeps the clause's flag on every line a kit statement
    // expands into (`process_kit_import.js`: `createImportClause(o.importClause.isTypeOnly, …)`).
    let Classified {
        values,
        types,
        default_value,
        default_type,
        keeps_value,
    } = classify(statement, used, declares_type);
    let indent = line_indent(text, statement.start);
    let quote = quote_of(&text[statement.start..statement.end], &statement.request);
    let keyword = if statement.is_export {
        "export"
    } else {
        "import"
    };
    if !keeps_value && default_type.is_none() && types.is_empty() {
        // Nothing survives: the module reads none of these bindings, and an `import type`
        // whose names are all unread is elided exactly like a value import
        // (`fixtures/kit-arkui-state`: the unread `PersistenceV2` leaves no statement).
        return String::new();
    }
    // A system module is addressed with `@ohos:<name>` whether or not the statement runs:
    // the loader's system-api rewrite (`processSystemApi`) is not a usage decision, and the
    // reference emits `import type { TypeConstructorWithArgs } from "@ohos:arkui.StateManagement"`
    // (`xb_components/model/di.ts`). The module-url resolution of `request` *is* one.
    let request_text = if resolved {
        request(&statement.request).unwrap_or_else(|| normalize_system(&statement.request))
    } else {
        normalize_system(&statement.request)
    };
    let has_value = keeps_value;
    // Nothing about the binding list changes, so the request is the only text to touch: the
    // reference's url rewrite is a text edit too, and `fixtures/import-comment` pins the
    // comments written inside the binding list.
    let unchanged = types.is_empty()
        && default_type.is_none()
        && default_value.as_deref() == statement.default_binding.as_deref()
        && values.len() == statement.bindings.len()
        && values.iter().zip(&statement.bindings).all(|(a, b)| a == b);

    if unchanged {
        return if request_text == statement.request {
            text[statement.start..statement.end].to_string()
        } else {
            replace_request(text, statement, &request_text)
        };
    }
    let mut lines: Vec<String> = Vec::new();
    if has_value {
        lines.push(render_one(
            indent,
            keyword,
            false,
            default_value.as_deref(),
            &values,
            if statement.star {
                statement.namespace_export.as_deref()
            } else {
                statement.namespace.as_deref()
            },
            statement.star,
            quote,
            &request_text,
            &statement.explicit_alias,
        ));
    }
    if default_type.is_some() || !types.is_empty() {
        lines.push(render_one(
            indent,
            keyword,
            true,
            default_type.as_deref(),
            &types,
            None,
            false,
            quote,
            &request_text,
            &statement.explicit_alias,
        ));
    }
    lines.join("\n")
}

/// One emitted statement. `named` holds `(written, local-or-exported)` pairs; `explicit_alias`
/// holds the names the source wrote with a bare `as` (`Foo as Foo`), a spelling the pair cannot
/// carry and the reference keeps (`import type { ProxyItem as ProxyItem }`).
#[allow(clippy::too_many_arguments)]
fn render_one(
    indent: &str,
    keyword: &str,
    is_type: bool,
    default: Option<&str>,
    named: &[(String, String)],
    namespace: Option<&str>,
    star: bool,
    quote: char,
    request: &str,
    explicit_alias: &HashSet<String>,
) -> String {
    let head = if is_type {
        format!("{indent}{keyword} type ")
    } else {
        format!("{indent}{keyword} ")
    };
    let literal = format!("{quote}{request}{quote}");
    let from = format!(" from {literal};");
    if star {
        return match namespace {
            Some(namespace) => format!("{head}* as {namespace}{from}"),
            None => format!("{head}*{from}"),
        };
    }
    if let Some(namespace) = namespace {
        return format!("{head}* as {namespace}{from}");
    }
    let list = named
        .iter()
        .map(|(written, local)| {
            if written == local && !explicit_alias.contains(written) {
                written.clone()
            } else {
                format!("{written} as {local}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    match (default, named.is_empty()) {
        (Some(default), true) => format!("{head}{default}{from}"),
        (Some(default), false) => format!("{head}{default}, {{ {list} }}{from}"),
        // An empty clause is a side-effect import: `import './x';`.
        (None, true) => format!("{indent}{keyword} {literal};"),
        (None, false) => format!("{head}{{ {list} }}{from}"),
    }
}

/// The system-module form of a request, which applies to every statement.
///
/// `@ohos.file.fs` → `@ohos:file.fs`, a native module → `@native:…`, a native library →
/// `@normalized:Y&&&name.so&`; a request that is none of those is returned unchanged, and a
/// request already in its normalized form passes through as well.
fn normalize_system(request: &str) -> String {
    if request.starts_with('@') {
        crate::build::ets_assemble::system_module_request(request)
    } else if request.ends_with(".so") {
        format!("@normalized:Y&&&{request}&")
    } else {
        request.to_string()
    }
}

/// The statement with its module literal replaced, everything else copied verbatim.
fn replace_request(text: &str, statement: &ImportStatement, request: &str) -> String {
    let quote = text[statement.request_start..]
        .chars()
        .next()
        .unwrap_or('\'');
    let mut out = String::with_capacity(statement.end - statement.start);
    out.push_str(&text[statement.start..statement.request_start]);
    out.push(quote);
    out.push_str(request);
    out.push(quote);
    out.push_str(&text[statement.request_end..statement.end]);
    out
}

/// Every `ESObject` type reference becomes `any`.
///
/// ArkTS's `ESObject` is a dynamic type the reference compiler never prints as such: its UI
/// syntax visitor replaces a node whose kind is `TypeReference` and whose type name is the bare
/// identifier `ESObject` with `SyntaxKind.AnyKeyword` (`process_ui_syntax.js`:
/// `e.kind === ts.SyntaxKind.TypeReference && e.typeName?.kind === ts.SyntaxKind.Identifier &&
/// "ESObject" === e.typeName.escapedText`). The decision is the node's shape, not its
/// position: a qualified `ns.ESObject` is a `QualifiedName` and an expression `new ESObject()`
/// is not a type reference, so both keep their spelling, while a `TypeReference` nested in an
/// array, a union or a type argument is rewritten like any other.
///
/// Measured on clashbox (2026-11-07): the reference cache holds no `ESObject` — the
/// `then((ns: …))` parameter in `EntryAbility.ts` and `AddAppListComponent.ts`'s `catch`
/// parameter (inside a struct's `build()`, so component bodies too) are `any` — while both
/// sources write `ESObject`.
///
/// This is a **source-shape** fix only. A type annotation is erased by es2abc; compiling the
/// reference and hvolt files with and without the rewrite yields the same bytecode (measured:
/// `lines 5873/5875`, `mnemonic_blocks 18`, `symbols_total 628/628`, `only_ref 0 /
/// only_ours 0` either way on `EntryAbility`), so it moves the AST census and nothing
/// functional.
///
/// The reference applies the rewrite to `.ets` modules only: a `.ts` source takes the loader's
/// early return (`/\.ts$/.test(fileName)` → a visitor that only checks module specifiers).
/// This pass cannot make that distinction — `ets_assemble` hands `.ts` and `.ets` sources to
/// the same lowering and neither route receives the extension — so it applies to both. The one
/// `.ts` hit in the project is a dependency's `oh_modules/…/@cxy/sandboxfinder/…/Response.ts`
/// (`data?: ESObject`), whose rewrite is bytecode-neutral (a type annotation is erased:
/// `lines 48/48`, `mnemonic_blocks 0` either way) and which the reference cache does not carry
/// at all; no module source writes `ESObject` in a `.ts`. Discriminating exactly would need the
/// extension threaded through `AssembleOptions`.
pub(crate) fn es_object_to_any(text: &str) -> String {
    use oxc_ast_visit::{walk, Visit};

    struct Finder {
        spans: Vec<(usize, usize)>,
    }

    impl<'a> Visit<'a> for Finder {
        fn visit_ts_type_reference(&mut self, reference: &oxc_ast::ast::TSTypeReference<'a>) {
            if let oxc_ast::ast::TSTypeName::IdentifierReference(name) = &reference.type_name {
                if name.name == "ESObject" {
                    self.spans
                        .push((reference.span.start as usize, reference.span.end as usize));
                }
            }
            walk::walk_ts_type_reference(self, reference);
        }
    }

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return text.to_string();
    }
    let mut finder = Finder { spans: Vec::new() };
    finder.visit_program(&ret.program);
    if finder.spans.is_empty() {
        return text.to_string();
    }
    // A nested reference inside an outer `ESObject<…>` shares its replacement: the outer span
    // starts first, and the applier drops the edit that starts inside an already rewritten one.
    let edits: Vec<(usize, usize, String)> = finder
        .spans
        .iter()
        .map(|(start, end)| (*start, *end, "any".to_string()))
        .collect();
    crate::build::ets_assemble::apply_replacements(text, &edits)
}

/// Add the names the source reads but the lowered text no longer does.
///
/// Only the imports are consulted: a name that is missing from the lowered reading is looked
/// for in the source, and one the source mentions — as a value or only as a type — is rescued.
/// Erasing it instead would drop a decorator's import (`fixtures/user-intent`) or a type the
/// lowering erased a mention of: `ForEach(items, (v: SettingsItem) => …)` becomes
/// `(_item, i: number) => …`, which no longer names the annotation
/// (`entry/components/Home/CardEdit.ts`). The reference decides on the source, where both are
/// still references.
///
/// The distinction between the two readings is the reference's own: it keeps a value import for
/// a name its emit resolver sees read as a value (`isReferencedAliasDeclaration`) and moves a
/// name that is only referenced to the type clause (`isReferenced`), where a type position
/// counts.
///
/// The reading is syntactic, not textual: a name that only occurs as a property key or inside a
/// string literal is not a reference. That distinction is not a nicety — a component library's
/// struct declares fields named after the options it imports (`@mcui/mccharts`'s
/// `pageView/Charts.ets` declares `legend: LegendInterface = {}`, `tooltip: …`, `dataZoom: …`,
/// `radar: …`, `animation: …` while importing five unrelated values of the same spelling), and
/// a textual test keeps those imports alive for good.
fn rescue_from_source(source: &str, statements: &[ImportStatement], used: &mut UsedNames) {
    let read = Reads::of(source);
    for statement in statements {
        if statement.is_export {
            continue;
        }
        for (_, local) in &statement.bindings {
            rescue_name(&read, local, used);
        }
        if let Some(default) = &statement.default_binding {
            rescue_name(&read, default, used);
        }
    }
}

/// Keep a binding the lowering erased a mention of: a value mention restores the value reading,
/// any mention keeps the binding alive as a type.
fn rescue_name(read: &Reads<'_>, name: &str, used: &mut UsedNames) {
    if used.all.contains(name) || !read.mentions(name) {
        return;
    }
    if read.reads_value(name) {
        used.value.insert(name.to_string());
    }
    used.all.insert(name.to_string());
}

/// How the source is read for the names it uses.
///
/// The syntactic reading is the rule; the textual one is the fallback for a source the
/// component pipeline cannot turn into TypeScript (a `struct` is not TypeScript, and a body
/// this pipeline cannot rewrite leaves the prepared text unparsable). Keeping the fallback
/// means a file the parser cannot open is decided the way it always was, rather than silently
/// losing every import the lowering erased a mention of.
enum Reads<'a> {
    /// The names the prepared source reads, in each of the reference's two readings.
    Parsed {
        value: HashSet<String>,
        all: HashSet<String>,
    },
    /// The source itself, decided by [`mentioned_outside_imports`].
    Textual(&'a str),
}

impl<'a> Reads<'a> {
    fn of(source: &'a str) -> Self {
        match source_reads(source) {
            Some((value, all)) => Self::Parsed { value, all },
            None => Self::Textual(source),
        }
    }

    /// The name is read where the program needs a value (`new Foo()`, `Foo.member`, `foo`).
    fn reads_value(&self, name: &str) -> bool {
        match self {
            Self::Parsed { value, .. } => value.contains(name),
            Self::Textual(source) => mentioned_outside_imports(source, name),
        }
    }

    /// The name is referenced anywhere, a type position included: that keeps the binding as a
    /// type, which is what TypeScript's `isReferenced` decides for the type-only transform.
    fn mentions(&self, name: &str) -> bool {
        match self {
            Self::Parsed { all, .. } => all.contains(name),
            Self::Textual(source) => mentioned_outside_imports(source, name),
        }
    }
}

/// The names the source reads in a value position and anywhere at all, or `None` when it does
/// not parse.
///
/// The text is prepared exactly the way the component pipeline prepares it before parsing a
/// module (`struct` → `class`, the child blocks rewritten), and the reading is the same one
/// the lowering applies to its own text — [`used_names`], whose `value` set skips type
/// declarations and every type position while its `all` set keeps them.
fn source_reads(source: &str) -> Option<(HashSet<String>, HashSet<String>)> {
    let prepared = prepared_source(source);
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &prepared, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    let names = used_names(&ret.program);
    Some((names.value, names.all))
}

/// The source in the shape the TypeScript parser accepts.
fn prepared_source(source: &str) -> String {
    let (masked, builders, _extends, _styles, locals) =
        crate::build::ets_compile::mask_builder_decorators_with_locals(source);
    crate::build::ets_compile::rewrite_builders_with_locals(
        &crate::build::ets_compile::preprocess(&masked),
        &builders,
        &locals,
    )
}

/// Whether `name` appears in `text` as an identifier outside an import or re-export statement.
fn mentioned_outside_imports(text: &str, name: &str) -> bool {
    use crate::build::lex::Kind;
    let tokens = crate::build::lex::tokenize(text);
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        let opens_import = token.kind == Kind::Ident
            && (token.text == "import"
                || (token.text == "export"
                    && tokens
                        .get(index + 1)
                        .is_some_and(|next| next.is_punct('{') || next.is_punct('*'))));
        if opens_import {
            index += 1;
            while index < tokens.len() && !tokens[index].is_punct(';') {
                index += 1;
            }
            index += 1;
            continue;
        }
        if token.kind == Kind::Ident && token.text == name {
            return true;
        }
        if token.kind == Kind::Template
            && crate::build::lex::template_expressions(token.text)
                .into_iter()
                .any(|inner| mentioned_outside_imports(inner, name))
        {
            return true;
        }
        index += 1;
    }
    false
}

/// The whitespace a statement starts after, so a split keeps the column it was written at.
fn line_indent(text: &str, start: usize) -> &str {
    let line_start = text[..start].rfind('\n').map(|at| at + 1).unwrap_or(0);
    let prefix = &text[line_start..start];
    if prefix.chars().all(char::is_whitespace) {
        prefix
    } else {
        ""
    }
}

/// The quote the request is written with, so a rewritten statement keeps the file's style.
fn quote_of(original: &str, request: &str) -> char {
    if original.contains(&format!("\"{request}\"")) {
        '"'
    } else {
        '\''
    }
}

/// A `builder: X(…)` site inside an object literal, with the span of the call to replace.
pub(crate) struct BuilderSite {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) component: String,
    pub(crate) arguments: String,
}

/// Every `builder: X(…)` where `X` is a registered component.
///
/// The site is an `ObjectProperty` whose value is a call to a component, so the decision reads
/// the node rather than a `builder` token that a string or a comment could imitate.
pub(crate) fn builder_sites(text: &str) -> Option<Vec<BuilderSite>> {
    use oxc_ast_visit::{walk, Visit};
    use oxc_span::GetSpan;

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    struct Collector<'a> {
        text: &'a str,
        sites: Vec<BuilderSite>,
    }

    impl<'a> Visit<'a> for Collector<'a> {
        fn visit_object_property(&mut self, property: &oxc_ast::ast::ObjectProperty<'a>) {
            let is_builder = matches!(
                &property.key,
                oxc_ast::ast::PropertyKey::StaticIdentifier(name) if name.name == "builder"
            );
            if is_builder {
                if let oxc_ast::ast::Expression::CallExpression(call) = &property.value {
                    if let oxc_ast::ast::Expression::Identifier(callee) = &call.callee {
                        let component = callee.name.to_string();
                        if crate::build::compile::component_index::protocol_of(&component).is_some()
                        {
                            let arguments = if call.arguments.is_empty() {
                                String::new()
                            } else {
                                let first = call.arguments[0].span();
                                let last = call.arguments[call.arguments.len() - 1].span();
                                self.text[first.start as usize..last.end as usize].to_string()
                            };
                            self.sites.push(BuilderSite {
                                start: call.span.start as usize,
                                end: call.span.end as usize,
                                component,
                                arguments,
                            });
                        }
                    }
                }
            }
            walk::walk_object_property(self, property);
        }
    }

    let mut collector = Collector {
        text,
        sites: Vec::new(),
    };
    collector.visit_program(&ret.program);
    Some(collector.sites)
}

/// The `new CustomDialogController({ … })` constructions that take an owner argument.
///
/// `createCustomDialogController` (`process_ui_syntax.js:18945`) rewrites a construction only
/// when it carries exactly one argument and that argument is an object literal; the owner is
/// then appended as the second argument (`new CustomDialogController(objectLiteral, this)`). A
/// construction written any other way keeps its call. The offsets are the `new` keyword's,
/// which is where the call site is recognised.
pub(crate) fn dialog_controller_owner_sites(text: &str) -> Option<Vec<usize>> {
    use oxc_ast_visit::{walk, Visit};

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    struct Collector {
        sites: Vec<usize>,
    }

    impl<'a> Visit<'a> for Collector {
        fn visit_new_expression(&mut self, expression: &oxc_ast::ast::NewExpression<'a>) {
            let names_controller = matches!(
                &expression.callee,
                oxc_ast::ast::Expression::Identifier(callee) if callee.name == "CustomDialogController"
            );
            let one_object = expression.arguments.len() == 1
                && matches!(
                    &expression.arguments[0],
                    oxc_ast::ast::Argument::ObjectExpression(_)
                );
            if names_controller && one_object {
                self.sites.push(expression.span.start as usize);
            }
            walk::walk_new_expression(self, expression);
        }
    }

    let mut collector = Collector { sites: Vec::new() };
    collector.visit_program(&ret.program);
    Some(collector.sites)
}

/// What a `$r` / `$rawfile` call site asks the loader to build.
pub(crate) enum ResourceKind {
    /// `$r('app.<kind>.<name>', extra…)`: the key resolves against the id table, and the
    /// arguments after it become `params`
    /// (`$r('app.string.x', item.name)` carries `["item.name"]`).
    Named { key: String, params: Vec<String> },
    /// `$rawfile('x.jpg')`: a file asset the table cannot name, so the loader writes `id: 0`
    /// with the file name in `params` and type code 30000.
    RawFileName { name: String },
    /// `$rawfile(<expression>)`: the file name is only known at run time, so the loader writes
    /// `id: -1` and carries every argument through as written
    /// (`params: [this.startImage]`).
    RawFileValue { params: Vec<String> },
}

/// One `$r('app.…')` / `$rawfile(…)` call site and the span of the whole call.
pub(crate) struct ResourceCall {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: ResourceKind,
}

/// Every `$r('<key>')` / `$rawfile(…)` call in the program, or `None` when it does not parse.
///
/// The call is a `CallExpression` whose callee is the `$r` / `$rawfile` identifier; either name
/// inside a string or a comment is not a call and does not appear. `$rawfile` is collected for
/// any first argument: the loader resolves a literal file name at build time, and keeps an
/// expression for the runtime (`process_ui_syntax.js::processResourceData`).
pub(crate) fn resource_calls(text: &str) -> Option<Vec<ResourceCall>> {
    use oxc_ast_visit::{walk, Visit};

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    struct Collector<'a> {
        calls: Vec<ResourceCall>,
        text: &'a str,
    }

    impl<'a> Collector<'a> {
        /// The argument as written, which is what the loader copies into `params`.
        fn argument_text(&self, argument: &oxc_ast::ast::Argument<'a>) -> String {
            use oxc_span::GetSpan;
            let span = argument.span();
            self.text[span.start as usize..span.end as usize]
                .trim()
                .to_string()
        }
    }

    impl<'a> Visit<'a> for Collector<'a> {
        fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
            // `$r` is a table resource: only a key literal can be resolved. `$rawfile` is a file
            // asset, which the loader also turns into a resource literal; a literal name is
            // resolved here, any other expression is handed to the runtime unchanged.
            use oxc_ast::ast::Argument;
            let kind = match &call.callee {
                oxc_ast::ast::Expression::Identifier(name) if name.name == "$r" => {
                    match call.arguments.first() {
                        Some(Argument::StringLiteral(literal)) => Some(ResourceKind::Named {
                            key: literal.value.to_string(),
                            params: call
                                .arguments
                                .iter()
                                .skip(1)
                                .map(|argument| self.argument_text(argument))
                                .collect(),
                        }),
                        _ => None,
                    }
                }
                oxc_ast::ast::Expression::Identifier(name) if name.name == "$rawfile" => {
                    match call.arguments.first() {
                        Some(Argument::StringLiteral(literal)) => Some(ResourceKind::RawFileName {
                            name: literal.value.to_string(),
                        }),
                        Some(_) => Some(ResourceKind::RawFileValue {
                            params: call
                                .arguments
                                .iter()
                                .map(|argument| self.argument_text(argument))
                                .collect(),
                        }),
                        None => None,
                    }
                }
                _ => None,
            };
            if let Some(kind) = kind {
                self.calls.push(ResourceCall {
                    start: call.span.start as usize,
                    end: call.span.end as usize,
                    kind,
                });
            }
            walk::walk_call_expression(self, call);
        }
    }

    let mut collector = Collector {
        calls: Vec::new(),
        text,
    };
    collector.visit_program(&ret.program);
    Some(collector.calls)
}

/// Whether the module declares anything a running program can observe.
///
/// Decided on the tree, not by scanning for `{`/`;`: a body made only of `interface` / `type`
/// aliases, `import type` lines, `export type` clauses and empty exports is erased by the
/// reference compiler, so it leaves no record in the abc.  Anything else — a function, a
/// `const`, a class, a non-type import — keeps the module.
///
/// `None` means the text does not parse; the caller must then keep the module rather than
/// drop a record the runtime will ask for.
pub(crate) fn has_runtime_code(text: &str) -> Option<bool> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }
    for statement in &ret.program.body {
        let erased = match statement {
            Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => true,
            Statement::EmptyStatement(_) => true,
            // A plain `import './side-effect'` runs code elsewhere, so it keeps the module.
            Statement::ImportDeclaration(decl) => decl.import_kind.is_type(),
            Statement::ExportNamedDeclaration(export) => {
                export.export_kind.is_type()
                    || (export.declaration.is_none()
                        && export.source.is_none()
                        && export.specifiers.is_empty())
            }
            // A directive prologue (`"use strict"`) is not a runtime declaration.
            Statement::ExpressionStatement(expr) => {
                matches!(&expr.expression, oxc_ast::ast::Expression::StringLiteral(_))
            }
            _ => false,
        };
        if !erased {
            return Some(false);
        }
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request pass that marks every value-bearing statement, so a test can see which
    /// statements the reference would resolve onto a record url.
    fn normalizing(request: &str) -> Option<String> {
        Some(format!("normalized:{request}"))
    }

    fn no_types(_request: &str, _name: &str) -> bool {
        false
    }

    fn rewrite(text: &str) -> String {
        rewrite_imports(text, None, &normalizing, &no_types).expect("the probe parses")
    }

    /// A type the lowering erased a mention of still keeps its binding.
    ///
    /// `ForEach(items, (v: SettingsItem, i: number) => …)` is expanded to
    /// `const forEachItemGenFunction = (_item, i: number) => { const v = _item; … }`, so the
    /// lowered text no longer carries the callback's annotation. The reference decides on the
    /// source, where the annotation is a reference, so the binding stays — as a type clause, since
    /// nothing in the lowered text needs it at run time (`entry/components/Home/CardEdit.ts`:
    /// `import type { HomeCardItem } from "@normalized:…Constants&"`).
    #[test]
    fn a_type_the_lowering_erased_keeps_its_binding() {
        let source =
            "import { SettingsGroup, SettingsItem } from '../settings/SettingsCommonOp';\n\
                  const items: Array<SettingsGroup> = [];\n\
                  items.forEach((v: SettingsItem) => { log(v); });\n";
        let lowered =
            "import { SettingsGroup, SettingsItem } from '../settings/SettingsCommonOp';\n\
                   const items: Array<SettingsGroup> = [];\n\
                   items.forEach((v) => { log(v); });\n";
        assert_eq!(
            rewrite_imports(lowered, Some(source), &|_| None, &no_types).expect("parses"),
            "import type { SettingsGroup, SettingsItem } from '../settings/SettingsCommonOp';\n\
         const items: Array<SettingsGroup> = [];\n\
         items.forEach((v) => { log(v); });\n"
        );
    }

    /// A binding written `Foo as Foo` keeps that spelling when the clause is regenerated.
    ///
    /// The pair `(imported, local)` cannot tell `Foo` from `Foo as Foo`, and the reference prints
    /// the `propertyName` its own specifier node carries (`ProxyItem as ProxyItem` in
    /// `entry/components/Proxy/ProxyNodeItem.ts`) — see [`ImportStatement::explicit_alias`].
    #[test]
    fn a_bare_alias_survives_the_type_split() {
        let lowered = "import { ProxyItem as ProxyItem, ProxyData } from '../../common/datasources/ProxyData';\n\
                   const x = ProxyData;\n\
                   let y: ProxyItem;\n";
        assert_eq!(
        rewrite(lowered),
        "import { ProxyData } from 'normalized:../../common/datasources/ProxyData';\n\
         import type { ProxyItem as ProxyItem } from 'normalized:../../common/datasources/ProxyData';\n\
         const x = ProxyData;\n\
         let y: ProxyItem;\n"
    );
    }

    /// The two shapes the reference cache has for `ESObject` (2026-11-07): a `then` callback's
    /// parameter and a `catch` callback's parameter are `any`; both sources write `ESObject`.
    /// The rewrite is on the node, so a generic argument and an array element go too.
    #[test]
    fn an_es_object_type_reference_becomes_any() {
        let src = "import('@cxy/sandboxfinder').then(async (ns: ESObject) => {\n\
                   \x20 ns.run(1);\n\
                   });\n\
                   promise.catch((err: ESObject) => { log(err); });\n\
                   let xs: Array<ESObject>;\n";
        assert_eq!(
            es_object_to_any(src),
            "import('@cxy/sandboxfinder').then(async (ns: any) => {\n\
             \x20 ns.run(1);\n\
             });\n\
             promise.catch((err: any) => { log(err); });\n\
             let xs: Array<any>;\n"
        );
    }

    /// A qualified name, an expression and a string are not the type reference the reference
    /// rewrites: only a bare `TypeReference` is.
    #[test]
    fn es_object_outside_a_type_reference_is_untouched() {
        for src in [
            "let x: ns.ESObject;\n",
            "const y = new ESObject();\n",
            "const s = 'ESObject';\n",
        ] {
            assert_eq!(es_object_to_any(src), src);
        }
    }

    /// `entry/entryability/EntryAbility.ts` in the reference cache: the value half of a
    /// mixed import keeps the record url, the type half gets `import type` and the same url,
    /// and the value statement comes first.
    #[test]
    fn values_and_types_split_into_two_statements() {
        let src = "import { AppConfig, AppState, ClashCore } from './AppState';\n\
                   const state = new AppState();\n\
                   const core = ClashCore;\n\
                   let config: AppConfig;\n";
        assert_eq!(
            rewrite(src),
            "import { AppState, ClashCore } from 'normalized:./AppState';\n\
             import type { AppConfig } from 'normalized:./AppState';\n\
             const state = new AppState();\n\
             const core = ClashCore;\n\
             let config: AppConfig;\n"
        );
    }

    /// A statement no value binding survives in is addressed by the request as written: the
    /// reference only resolves the imports the program runs
    /// (`entry/common/utils/Animation.ts` keeps `../../entryability/AppState`).
    #[test]
    fn a_type_only_statement_keeps_its_request() {
        let src =
            "import { UIConfig } from '../../entryability/AppState';\nlet config: UIConfig;\n";
        assert_eq!(
            rewrite(src),
            "import type { UIConfig } from '../../entryability/AppState';\nlet config: UIConfig;\n"
        );
    }

    /// The request is resolved per module, not per statement: a file that runs `proxy_core`
    /// somewhere addresses every `proxy_core` statement by the same url
    /// (`components/Common/Common.ts`).
    #[test]
    fn a_request_resolved_elsewhere_in_the_file_resolves_everywhere() {
        let src = "import { SubscriptionInfo } from 'proxy_core';\n\
                   import { Provider } from 'proxy_core';\n\
                   const info = new SubscriptionInfo();\n\
                   let provider: Provider;\n";
        assert_eq!(
            rewrite(src),
            "import { SubscriptionInfo } from 'normalized:proxy_core';\n\
             import type { Provider } from 'normalized:proxy_core';\n\
             const info = new SubscriptionInfo();\n\
             let provider: Provider;\n"
        );
    }

    /// TypeScript's elision: a binding the module never reads leaves no statement, and a
    /// statement left with no binding leaves no line (`ProxyData.ts` loses `DelayInfo`).
    #[test]
    fn unread_bindings_are_erased() {
        let src = "import ClashViewModel, { DelayInfo } from './ClashViewModel';\n\
                   const view = new ClashViewModel();\n\
                   const next = 1;\n";
        assert_eq!(
            rewrite(src),
            "import ClashViewModel from 'normalized:./ClashViewModel';\n\
             const view = new ClashViewModel();\n\
             const next = 1;\n"
        );
        assert_eq!(
            rewrite("import { Unused } from './x';\nconst a = 1;\n"),
            "const a = 1;\n"
        );
    }

    /// A system request is normalized even when the statement only carries a type: the
    /// loader's system-api rewrite is not a usage decision
    /// (`xb_components/model/di.ts`: `@ohos:arkui.StateManagement`).
    #[test]
    fn a_type_only_system_request_is_still_normalized() {
        let src = "import { TypeConstructorWithArgs } from '@ohos.arkui.StateManagement';\n\
                   let ctor: TypeConstructorWithArgs;\n";
        assert_eq!(
            rewrite(src),
            "import type { TypeConstructorWithArgs } from '@ohos:arkui.StateManagement';\n\
             let ctor: TypeConstructorWithArgs;\n"
        );
    }

    /// A default binding read only as a type becomes `import type <name> from …`, which is
    /// the shape the reference emits for `distributedKVStore` and `common`.
    #[test]
    fn a_type_only_default_keeps_a_statement() {
        let src = "import distributedKVStore from '@ohos:data.distributedKVStore';\n\
                   let entry: distributedKVStore.Entry;\n";
        assert_eq!(
            rewrite(src),
            "import type distributedKVStore from '@ohos:data.distributedKVStore';\n\
             let entry: distributedKVStore.Entry;\n"
        );
    }

    /// Default value plus named types: the default keeps the value statement and the types
    /// move to their own (`components/Settings/Backup.ts` in the reference cache).
    #[test]
    fn a_default_and_named_types_split() {
        let src = "import BackupRestoreUtil, { BackupConfig } from './BackupRestoreUtil';\n\
                   new BackupRestoreUtil().run();\n\
                   let config: BackupConfig;\n";
        assert_eq!(
            rewrite(src),
            "import BackupRestoreUtil from 'normalized:./BackupRestoreUtil';\n\
             import type { BackupConfig } from 'normalized:./BackupRestoreUtil';\n\
             new BackupRestoreUtil().run();\n\
             let config: BackupConfig;\n"
        );
    }

    /// A name the target declares as an `interface` is a type even when the module reads it in
    /// a value position: `implements IClashManager` is a type position that a syntactic reading
    /// counts as a value, and importing the interface as a value fails module instantiation on
    /// the device (`the requested module … does not provide an export name 'IClashManager'`).
    #[test]
    fn a_declared_type_wins_over_a_value_position() {
        let declares = |_request: &str, name: &str| name == "IClashManager";
        let src = "import { ClashRpcType, IClashManager } from './IClashManager';\n\
                   class Service implements IClashManager {\n\
                   \x20 mode: ClashRpcType = ClashRpcType.A;\n\
                   }\n";
        assert_eq!(
            rewrite_imports(src, None, &normalizing, &declares).expect("parses"),
            "import { ClashRpcType } from 'normalized:./IClashManager';\n\
             import type { IClashManager } from 'normalized:./IClashManager';\n\
             class Service implements IClashManager {\n\
             \x20 mode: ClashRpcType = ClashRpcType.A;\n\
             }\n"
        );
    }

    /// A re-export is never read locally, so the target module decides whether the name is a
    /// value there (`xb_components/Index.ts` splits `Xb_TabComponentConfig` out this way).
    #[test]
    fn a_re_export_splits_by_the_target_declaration() {
        let declares = |_request: &str, name: &str| name == "TabConfig";
        let src = "export { TabComponent, TabConfig } from './components/Tab';\n";
        assert_eq!(
            rewrite_imports(src, None, &normalizing, &declares).expect("parses"),
            "export { TabComponent } from 'normalized:./components/Tab';\n\
             export type { TabConfig } from 'normalized:./components/Tab';\n"
        );
    }

    /// A statement whose binding list survives untouched is edited in place, so the comments
    /// written inside it stay (`fixtures/import-comment`).
    #[test]
    fn a_surviving_statement_keeps_its_comments() {
        let src = "import {\n  // how much to scale by\n  SCALE,\n  // how much to scale by\n  OFFSET,\n} from './Helper';\nconst n = SCALE + OFFSET;\n";
        assert_eq!(
            rewrite(src),
            "import {\n  // how much to scale by\n  SCALE,\n  // how much to scale by\n  OFFSET,\n} from 'normalized:./Helper';\nconst n = SCALE + OFFSET;\n"
        );
    }

    /// A decorator the lowering erases still keeps its import: the reference decides usage on
    /// the source, where the decorator is a reference (`fixtures/user-intent`).
    #[test]
    fn a_name_the_lowering_dropped_keeps_its_import() {
        let source =
            "import { InsightIntentLink } from '@ohos.app.ability.InsightIntentDecorator';\n\
                      @InsightIntentLink({ intentName: 'PlayMusic' })\n\
                      @Entry\n\
                      struct V { build() {} }\n";
        let lowered =
            "import { InsightIntentLink } from '@ohos.app.ability.InsightIntentDecorator';\n\
                       class V {}\n";
        // The real request pass normalizes only non-system requests (see `rewrite_spec`), so
        // the probe lets the always-on system rewrite do the work.
        let system_only = |_: &str| None;
        assert_eq!(
            rewrite_imports(lowered, Some(source), &system_only, &no_types).expect("parses"),
            "import { InsightIntentLink } from '@ohos:app.ability.InsightIntentDecorator';\n\
             class V {}\n"
        );
        // Without the source the name is unread, and the reference's elision drops it.
        assert_eq!(rewrite(lowered), "class V {}\n");
    }

    /// A field named like the import is not a read: the source's `legend: number = 1` is a
    /// property key, so the binding is unread and the statement goes (`@mcui/mccharts`'s
    /// `pageView/Charts.ets` declares five fields named after the five values it imports).
    #[test]
    fn a_property_key_does_not_keep_an_import_alive() {
        let source = "import { legend } from './opts';\n@Component\nstruct V {\n  legend: number = 1;\n  build() { this.legend = 2; }\n}\n";
        let lowered = "import { legend } from './opts';\nclass V {\n  legend: number = 1;\n  build() { this.legend = 2; }\n}\n";
        let no_requests = |_: &str| None;
        assert_eq!(
            rewrite_imports(lowered, Some(source), &no_requests, &no_types).expect("parses"),
            "class V {\n  legend: number = 1;\n  build() { this.legend = 2; }\n}\n"
        );
    }

    /// The same spelling read as a value keeps the import.
    #[test]
    fn a_value_read_keeps_the_import_the_property_key_would_have_dropped() {
        let source = "import { legend } from './opts';\nconst defaults = legend;\n@Component\nstruct V {\n  legend: number = 1;\n  build() { this.legend = 2; }\n}\n";
        let lowered = "import { legend } from './opts';\nclass V {\n  legend: number = 1;\n  build() { this.legend = 2; }\n}\n";
        assert_eq!(
            rewrite_imports(lowered, Some(source), &normalizing, &no_types).expect("parses"),
            "import { legend } from 'normalized:./opts';\nclass V {\n  legend: number = 1;\n  build() { this.legend = 2; }\n}\n"
        );
    }

    /// The lower settles the system lines it emitted: an unread kit symbol is erased and a
    /// type-only one keeps `import type` (`fixtures/kit-arkui-state`).
    #[test]
    fn settling_a_system_module_erases_the_unread_symbol() {
        let source =
            "import { AppStorageV2, PersistenceV2 } from '@kit.ArkUI';\nconst x = AppStorageV2;\n";
        let lowered = "import { AppStorageV2 } from '@ohos:arkui.StateManagement';\n\
                       import type { PersistenceV2 } from '@ohos:arkui.StateManagement';\n\
                       const x = AppStorageV2;\n";
        assert_eq!(
            settle_system_imports(lowered, source),
            "import { AppStorageV2 } from '@ohos:arkui.StateManagement';\nconst x = AppStorageV2;\n"
        );
        // A relative request is not this pass's: it stays exactly as written.
        let relative = "import { A, B } from './x';\nconst a = A;\n";
        assert_eq!(settle_system_imports(relative, relative), relative);
    }

    /// A parameter named like an import answers the references inside it, so the import is only
    /// ever read as a type — the shape `GlobalContext.ets` has (`xb_components`).
    #[test]
    fn a_parameter_shadows_the_import_it_is_spelled_like() {
        let source = "import window from '@ohos:window';\n\
                      class G {\n\
                        initWindow(window: window.Window) {\n\
                          window.getWindowProperties();\n\
                        }\n\
                      }\n";
        assert_eq!(
            settle_system_imports(source, source),
            "import type window from '@ohos:window';\n\
             class G {\n\
               initWindow(window: window.Window) {\n\
                 window.getWindowProperties();\n\
               }\n\
             }\n"
        );
    }

    /// A block binding shadows the import for the block it covers, and nothing outside it: the
    /// statement after the block still reads the import as a value.
    #[test]
    fn a_block_binding_shadows_only_its_block() {
        let source = "import window from '@ohos:window';\n\
                      {\n\
                        const window = 1;\n\
                        window;\n\
                      }\n\
                      window.getLastWindow();\n";
        assert_eq!(settle_system_imports(source, source), source);
    }

    /// A local clause whose every name is a type this file declares is printed type-only, so
    /// the program declares no runtime export for it (`chartInterface.ts`).
    #[test]
    fn a_local_export_of_types_is_printed_type_only() {
        let src = "interface A { x: number }\n\
                   type B = string;\n\
                   export { A, B };\n\
                   const v = 1;\n\
                   export { v };\n";
        assert_eq!(
            rewrite(src),
            "interface A { x: number }\n\
             type B = string;\n\
             export type { A, B };\n\
             const v = 1;\n\
             export { v };\n"
        );
    }

    /// `export * as name from …` carries a name of its own and must keep it.
    #[test]
    fn a_namespace_export_keeps_its_name() {
        assert_eq!(
            rewrite("export * as chartInterface from './chartInterface';\n"),
            "export * as chartInterface from 'normalized:./chartInterface';\n"
        );
    }

    /// A binding the source itself spelled `type` is a type even when the module reads it in a
    /// value position — the author's word wins, as with a whole `import type` clause.
    #[test]
    fn a_written_inline_type_marks_the_binding() {
        let src = "import { type Segment, Area } from './Forms';\n\
                   const area = Area;\n\
                   let segment: Segment;\n";
        assert_eq!(
            rewrite(src),
            "import { Area } from 'normalized:./Forms';\n\
             import type { Segment } from 'normalized:./Forms';\n\
             const area = Area;\n\
             let segment: Segment;\n"
        );
    }

    /// A side-effect import and an `export *` carry no name, so nothing erases them, and both
    /// keep a resolved request.
    #[test]
    fn nameless_statements_survive_and_are_resolved() {
        assert_eq!(
            rewrite("import './side-effect';\n"),
            "import 'normalized:./side-effect';\n"
        );
        assert_eq!(
            rewrite("export * from './all';\n"),
            "export * from 'normalized:./all';\n"
        );
    }

    /// A statement the source wrote `import type` is the author's decision: it is reproduced
    /// verbatim, request included (`process_kit_import.js` keeps `importClause.isTypeOnly`).
    #[test]
    fn a_written_type_statement_is_kept_verbatim() {
        let src = "import type { CustomThemeName } from 'xb_components/src/main/ets/utils/AppThemeManager';\nlet theme: CustomThemeName;\n";
        assert_eq!(rewrite(src), src);
    }

    /// Resolution is per request: a request one statement loads is loaded for every statement
    /// that names it, so a package's type-only copy takes the same url
    /// (`proxy_core/Index.ets`: the `export type` statement next to a value statement). A
    /// request nothing loads stays unresolved, and its path is left as written.
    #[test]
    fn a_request_is_loaded_where_any_statement_carries_a_value() {
        let specifiers = module_specifiers(
            "import { Helper } from '../utils/Helper';\n\
             import type { Shape } from '../utils/Helper';\n\
             import type { Local } from './Local';\n",
        )
        .expect("the probe parses");
        let loaded = loaded_requests(&specifiers);
        assert!(loaded.contains("../utils/Helper"), "{loaded:?}");
        assert!(!loaded.contains("./Local"), "{loaded:?}");
    }

    /// The flat layout's module entry re-roots what it leaves relative, because the reference's
    /// entry lives at the module root and names the source root explicitly
    /// (`xb_components/Index.ets`). A request that already names the source root is left alone,
    /// so the pass changes nothing on a source laid out the reference's way.
    #[test]
    fn a_module_entry_reroots_the_requests_it_leaves_relative() {
        let flat = "export type { A } from './controllers/A';\n\
                    export type { B } from './src/main/ets/controllers/B';\n\
                    export { C } from '@normalized:N&&&xb_components/Index&1.0.0';\n";
        assert_eq!(
            reroot_entry_requests(flat).expect("the probe parses"),
            "export type { A } from './src/main/ets/controllers/A';\n\
             export type { B } from './src/main/ets/controllers/B';\n\
             export { C } from '@normalized:N&&&xb_components/Index&1.0.0';\n"
        );
    }

    /// A name read in a value position inside a generic call keeps the value binding; a name
    /// read only in a type argument does not.
    #[test]
    fn type_arguments_do_not_keep_a_value_binding() {
        let src = "import { Group, Item } from './models';\n\
                   const list: Array<Group> = [];\n\
                   const made = new Item();\n";
        assert_eq!(
            rewrite(src),
            "import { Item } from 'normalized:./models';\n\
             import type { Group } from 'normalized:./models';\n\
             const list: Array<Group> = [];\n\
             const made = new Item();\n"
        );
    }

    /// `export { X }` re-exports the local binding, so the import behind it stays a value.
    #[test]
    fn a_local_re_export_keeps_its_import() {
        let src = "import { Thing } from './thing';\nexport { Thing };\n";
        assert_eq!(
            rewrite(src),
            "import { Thing } from 'normalized:./thing';\nexport { Thing };\n"
        );
    }

    /// One kit config with two modules: `curves` is a default export of a native module,
    /// `Second` a named export of another.
    fn kit_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("hvolt-kit-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create kit dir");
        std::fs::write(
            dir.join("@kit.Demo.json"),
            r#"{
  "symbols": {
    "curves": { "source": "@ohos.curves.d.ts", "bindings": "default" },
    "Second": { "source": "@ohos.other.d.ts", "bindings": "Second" },
    "Alias": { "source": "@ohos.other.d.ts", "bindings": "RealName" }
  }
}"#,
        )
        .expect("write kit config");
        dir
    }

    /// `ExportSpecifierKitInfo` groups the clause by module, writes the config's `bindings`
    /// as the exported name, and the star expansion lists every distinct module in config
    /// order. The statements stay where the source wrote them.
    #[test]
    fn a_kit_re_export_expands_to_its_modules() {
        crate::build::stage::process_import::load_kit_configs(&[kit_dir("re-export")])
            .expect("load kits");
        let src = "export { curves, Second, Alias } from '@kit.Demo';\nexport * from '@kit.Demo';\nclass A {}\n";
        assert_eq!(
            settle_system_imports(src, src),
            "export { default as curves } from \"@native:ohos.curves\";\n\
             export { Second, RealName as Alias } from \"@ohos:other\";\n\
             export * from \"@native:ohos.curves\";\n\
             export * from \"@ohos:other\";\n\
             class A {}\n"
        );
    }

    /// A Kit the SDK has no config for keeps its statement (the reference reports
    /// `ETS2BUNDLE_EXTERNAL_KIT_CONFIG_FILE_NOT_FOUND` beside it); a namespace export is
    /// dropped (`…_KIT_NAMESPACE_IMPORT_EXPORT_NOT_SUPPORTED`); a name the Kit does not
    /// export contributes no specifier (`…_IMPORT_NAME_NOT_EXPORTED_FROM_KIT`).
    #[test]
    fn a_kit_re_export_that_cannot_expand_keeps_or_drops_the_statement() {
        crate::build::stage::process_import::load_kit_configs(&[kit_dir("fallbacks")])
            .expect("load kits");
        let unknown = "export { X } from '@kit.Unknown';\n";
        assert_eq!(settle_system_imports(unknown, unknown), unknown);
        let namespace = "export * as NS from '@kit.Demo';\n";
        assert_eq!(settle_system_imports(namespace, namespace), "");
        let missing = "export { NotThere, Second } from '@kit.Demo';\n";
        assert_eq!(
            settle_system_imports(missing, missing),
            "export { Second } from \"@ohos:other\";\n"
        );
    }

    /// The owner argument belongs to the construction, not to the builder: the reference appends
    /// `this` to every `new CustomDialogController({ … })` written with its single object
    /// literal, and leaves a construction written any other way alone
    /// (`createCustomDialogController`, guarded by `arguments.length === 1 &&
    /// isObjectLiteralExpression(arguments[0])`).
    #[test]
    fn only_the_single_object_literal_controller_takes_an_owner() {
        let text =
            "const a = new CustomDialogController({ builder: this.Open, autoCancel: false });\n\
                    const b = new CustomDialogController(settings);\n\
                    const c = new CustomDialogController({ builder: this.Open }, this);\n";
        let sites = dialog_controller_owner_sites(text).expect("the probe parses");
        let starts: Vec<&str> = sites
            .iter()
            .map(|start| &text[*start..*start + 3])
            .collect();
        assert_eq!(starts, vec!["new"], "one construction qualifies");
        assert_eq!(sites.len(), 1);
        assert_eq!(
            sites[0],
            text.find("new CustomDialogController({").expect("first")
        );
    }
}
