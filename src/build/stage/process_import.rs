// process stage: `process_import.js` — one import declaration becomes the statements the
// runtime links.
//
// `processImportModule(importNode, sourceFile, …)` (`process_import.js:256`) rewrites a
// module specifier: an `@ohos.*` / `@kit.*` request becomes the ohm-url the runtime resolves,
// a bare package request becomes its own record, and a type-only binding is dropped when the
// module reads the name only in type position. `split_bindings` below is the clause splitter
// the rewrite works on, and `emit_import` the emitter; `value_used_names` is what decides
// whether a binding that only appears in a type can go.
//
use std::collections::BTreeMap;
use std::path::Path;

// The file's own driver (read a file, run the stages, write it) stays in
// `crate::build::lower::ets_transpile`, and the CLI addresses it there.

/// Parse the named-import list of one import line: `{ a, b as c }`.
pub(crate) fn split_bindings(spec: &str) -> Vec<SplitBinding> {
    // The list is split on top-level commas, and `as` is read from the token stream, so a
    // comma or the word `as` inside a nested expression cannot invent a binding.
    let inner = spec
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(spec)
        .trim();
    crate::build::lex::split_top_level_commas(inner)
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|item| {
            // A comment between the comma and the name is not part of it: `{ /* c */ a }`
            // names `a`, and the reference's own binding list is built from the identifiers
            // of the AST (`process_kit_import.js`: `collectSpecifier` reads `e.name`), so a
            // comment can neither name a symbol nor select an alias.
            let item = crate::build::ets_compile::strip_comments(item.trim());
            let item = item.trim();
            let tokens = crate::build::lex::tokenize(item);
            let as_index = tokens
                .iter()
                .position(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "as");
            match as_index {
                Some(index) => {
                    let symbol = item[..tokens[index].start].trim().to_string();
                    let bind = item[tokens[index].end..].trim().to_string();
                    SplitBinding { symbol, bind }
                }
                None => SplitBinding {
                    symbol: item.to_string(),
                    bind: item.to_string(),
                },
            }
        })
        .collect()
}

/// `@ohos.app.ability.AbilityConstant.d.ts` → `@ohos:app.ability.AbilityConstant`.
pub(crate) fn ohos_request_from_source(source: &str) -> String {
    let stripped = source
        .trim()
        .strip_suffix(".d.ts")
        .or_else(|| source.trim().strip_suffix(".d.ets"))
        .unwrap_or(source.trim());
    // Native system modules take the `@native:` prefix (the loader's `NATIVE_MODULE`
    // rule); everything else is the ordinary `@ohos:` form. The loader's own rule
    // (`ark_utils.js`: `getNormalizedOhmUrlByModuleRequest`) folds `@system.` into the
    // `@ohos:` namespace as well — `@system.package` becomes `@ohos:package` — while the
    // native names keep their `@native:system.…` spelling.
    let request = crate::build::ets_assemble::system_module_request(stripped);
    match request.strip_prefix("@system:") {
        Some(rest) => format!("@ohos:{rest}"),
        None => request,
    }
}

/// One kit symbol's resolution: the ohos request and the binding kind
/// ("default" or the symbol name for named imports).
pub type KitEntry = (String, String);

thread_local! {
    /// The tables of the most recent [`load_kit_configs`] call, for the passes that cannot
    /// receive the map: `ets_transform` settles re-exports after the lowering, and its call
    /// sites (`lower/…`) pass no kit configs. The build lowers one file at a time, and every
    /// lowering is preceded by a `load_kit_configs` call on the same thread.
    static KIT_TABLE: std::cell::RefCell<BTreeMap<String, BTreeMap<String, KitEntry>>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
    /// Every distinct module a kit's symbols come from, in the config file's own order
    /// (`ExportStarKitInfo` walks the `symbols` object in insertion order).
    static KIT_SOURCES: std::cell::RefCell<BTreeMap<String, Vec<String>>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
}

/// The symbol table of one kit, or `None` when the kit has no config.
pub(crate) fn kit_table(kit: &str) -> Option<BTreeMap<String, KitEntry>> {
    KIT_TABLE.with(|cells| cells.borrow().get(kit).cloned())
}

/// The distinct module requests of one kit, in config order.
pub(crate) fn kit_sources(kit: &str) -> Option<Vec<String>> {
    KIT_SOURCES.with(|cells| cells.borrow().get(kit).cloned())
}

/// Load every `@kit.*.json` in the kit_configs dir into
/// `kit -> (symbol -> (request, bindings))`. Missing dir = empty map.
pub fn load_kit_configs(
    dirs: &[std::path::PathBuf],
) -> Result<BTreeMap<String, BTreeMap<String, KitEntry>>, String> {
    let mut kits = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for dir in dirs {
        merge_kit_configs(dir, &mut kits);
        // `getKitDefs` (`process_kit_import.js`) reads the first config file it finds for a
        // kit; the star expansion needs that file's symbol order.
        record_kit_source_order(dir, &mut sources);
    }
    KIT_TABLE.with(|cells| *cells.borrow_mut() = kits.clone());
    KIT_SOURCES.with(|cells| *cells.borrow_mut() = sources);
    Ok(kits)
}

/// Record each kit's module requests in the config file's symbol order.
///
/// The first directory that has a kit config decides the order, matching `getKitDefs`; a later
/// directory only fills kits the first one does not have.
fn record_kit_source_order(dir: &Path, sources: &mut BTreeMap<String, Vec<String>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !fname.starts_with("@kit.") || !fname.ends_with(".json") {
            continue;
        }
        let kit = fname.trim_end_matches(".json").to_string();
        if sources.contains_key(&kit) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut ordered: Vec<String> = Vec::new();
        for source in declared_source_values(&text) {
            if !ordered.contains(&source) {
                ordered.push(source);
            }
        }
        let requests = ordered
            .iter()
            .map(|source| ohos_request_from_source(source))
            .collect();
        sources.insert(kit, requests);
    }
}

/// The `"source"` values of a kit config, in file order.
///
/// The config is one `symbols` object whose entries each carry `source`; the object is read
/// with `serde_json`, whose map is ordered by key rather than by file, so the order comes from
/// the text. A value is a module path and carries no escaped quote.
fn declared_source_values(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(position) = rest.find("\"source\"") {
        rest = &rest[position + "\"source\"".len()..];
        let Some(after_colon) = rest.trim_start().strip_prefix(':') else {
            continue;
        };
        let after_colon = after_colon.trim_start();
        let Some(quote) = after_colon
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'')
        else {
            continue;
        };
        let body = &after_colon[1..];
        let Some(end) = body.find(quote) else {
            break;
        };
        out.push(body[..end].to_string());
        rest = &body[end + 1..];
    }
    out
}

/// Add one directory's kit configs to `kits`. Later directories win per symbol, which is the
/// order `sdk_kit_configs` returns: OpenHarmony first, then HMS.
pub(crate) fn merge_kit_configs(
    dir: &Path,
    kits: &mut BTreeMap<String, BTreeMap<String, KitEntry>>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !fname.starts_with("@kit.") || !fname.ends_with(".json") {
            continue;
        }
        let kit = fname.trim_end_matches(".json").to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let doc: serde_json::Value = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(_) => continue, // one bad config must not sink the build
        };
        let kit_map = kits.entry(kit).or_default();
        if let Some(symbols) = doc.get("symbols").and_then(|s| s.as_object()) {
            for (sym, info) in symbols {
                let source = info.get("source").and_then(|s| s.as_str()).unwrap_or("");
                let bindings = info
                    .get("bindings")
                    .and_then(|b| b.as_str())
                    .unwrap_or("default")
                    .to_string();
                kit_map.insert(sym.clone(), (ohos_request_from_source(source), bindings));
            }
        }
    }
}

/// A parsed import line, kept until the type-position analysis is done.
pub(crate) struct ParsedImport {
    kind: ImportKind,
    /// `import type { A } from '…'`: every line the statement expands into keeps the
    /// `type` qualifier.
    type_only: bool,
}

pub(crate) enum ImportKind {
    Kit {
        kit: String,
        bindings: Vec<SplitBinding>,
    },
    So {
        request: String,
    },
    Plain(String),
}

pub(crate) fn classify_import(line: &str) -> Option<ParsedImport> {
    // A leading comment (`// Index`) is part of the node span, so the keyword itself is located
    // by the scanner and everything from it onward is analysed.
    let keyword = crate::build::lex::tokenize(line)
        .into_iter()
        .find(|t| t.kind != crate::build::lex::Kind::Comment)?;
    if keyword.kind != crate::build::lex::Kind::Ident || keyword.text != "import" {
        return None;
    }
    let trimmed = &line[keyword.start..];
    let (spec, request) = split_import(trimmed)?;
    // `import type { A } from '@kit.X'` qualifies the whole clause: the reference builds the
    // emitted clause from `o.importClause.isTypeOnly` (`process_kit_import.js`,
    // `ImportSpecifierKitInfo.transform`), so the keyword has to survive the expansion.
    let (type_only, spec) = match strip_leading_type(spec) {
        Some(rest) => (true, rest),
        None => (false, spec),
    };
    if let Some(kit) = request.strip_prefix("@kit.") {
        return Some(ParsedImport {
            type_only,
            kind: ImportKind::Kit {
                kit: format!("@kit.{kit}"),
                bindings: split_bindings(spec),
            },
        });
    }
    if request.ends_with(".so") {
        return Some(ParsedImport {
            type_only: false,
            kind: ImportKind::So {
                request: request.to_string(),
            },
        });
    }
    Some(ParsedImport {
        type_only,
        kind: ImportKind::Plain(trimmed.to_string()),
    })
}

/// `spec` without its leading `type` keyword, when it carries one.
pub(crate) fn strip_leading_type(spec: &str) -> Option<&str> {
    let tokens = crate::build::lex::tokenize(spec);
    let first = tokens
        .iter()
        .find(|t| t.kind != crate::build::lex::Kind::Comment)?;
    if first.kind != crate::build::lex::Kind::Ident || first.text != "type" {
        return None;
    }
    let rest = spec[first.end..].trim();
    // A default import of the symbol literally named `type` is not a type-only clause.
    (!rest.is_empty()).then_some(rest)
}

/// Split `import <spec> from '<request>'` into (spec, request).
pub(crate) fn split_import(line: &str) -> Option<(&str, &str)> {
    // A re-export carries the same binding list and specifier shape. The scanner locates the
    // `from` keyword and the module literal, so a `from` inside a comment, a string or a
    // nested brace cannot split the statement in the wrong place.
    let tokens = crate::build::lex::tokenize(line);
    let first = tokens.iter().position(|t| {
        t.kind == crate::build::lex::Kind::Ident && (t.text == "import" || t.text == "export")
    })?;
    let mut depth = 0i32;
    let mut from = None;
    for (index, token) in tokens.iter().enumerate().skip(first + 1) {
        if token.is_punct('{') {
            depth += 1;
            continue;
        }
        if token.is_punct('}') {
            depth -= 1;
            continue;
        }
        if depth == 0 && token.kind == crate::build::lex::Kind::Ident && token.text == "from" {
            from = Some(index);
            break;
        }
    }
    let from = from?;
    let module = tokens.get(from + 1)?;
    if module.kind != crate::build::lex::Kind::Str {
        return None;
    }
    let spec = line[tokens[first].end..tokens[from].start].trim();
    let request = module.text.trim_matches(|c| c == '\'' || c == '"');
    Some((spec, request))
}

/// Expand one import line into the rewritten lines.
/// Every name the program reads as a value.
///
/// Decided on the tree: an identifier reference is a use, except inside a type — annotations,
/// type arguments, interface and alias bodies — because those are erased before the module
/// runs.  Reading tokens instead made the answer depend on where the previous statement ended;
/// a file written without semicolons swallowed the statement body (`di.ets` in clashbox: the
/// kit import of `PersistenceV2` disappeared because the scan ran past its usage).
pub(crate) fn value_used_names(
    program: &oxc_ast::ast::Program<'_>,
) -> std::collections::HashSet<String> {
    use oxc_ast_visit::{walk, Visit};

    struct Finder {
        names: std::collections::HashSet<String>,
    }

    impl<'a> Visit<'a> for Finder {
        fn visit_identifier_reference(&mut self, ident: &oxc_ast::ast::IdentifierReference<'a>) {
            self.names.insert(ident.name.to_string());
        }

        // A type is erased at run time; nothing inside it keeps a binding alive.
        fn visit_ts_type(&mut self, _ty: &oxc_ast::ast::TSType<'a>) {}
        fn visit_ts_type_annotation(&mut self, _annotation: &oxc_ast::ast::TSTypeAnnotation<'a>) {}
        // `class A implements B` is erased with the class: the clause names a type even
        // though the parser spells it as a type name rather than a `TSType`
        // (`TSClassImplements.expression`). Counting it as a value use kept a kit binding a
        // value import: `implements rpc.Parcelable` alone turned `import type rpc` into
        // `import rpc`, and es2abc then put `@ohos:rpc` into the module's request array — 27
        // entries where the reference has 26 (`EntryAbility`, measured 2026-11-07).
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
        names: std::collections::HashSet::new(),
    };
    finder.visit_program(program);
    finder.names
}

pub fn emit_import(
    line: &str,
    value_used: &std::collections::HashSet<String>,
    kits: &BTreeMap<String, BTreeMap<String, KitEntry>>,
) -> Vec<String> {
    // Which bindings survive, and which of those are types, is decided once for the whole
    // module by `ets_transform::rewrite_imports`: it needs the file after lowering, and a
    // statement here only knows its own line. The kit expansion therefore emits every symbol
    // the source named, and the module pass erases the unread ones and marks the type-only
    // ones — the reference does the same, from one `EmitResolver` over the module.
    let _ = value_used;
    // A re-export is not an import. The reference rewrites the two through separate visitors
    // — `processKitImport`'s visitor branches on `ts.isImportDeclaration(e)` /
    // `ts.isExportDeclaration(e)` and keeps a separate node list per kind
    // (`process_kit_import.js`: `ExportSpecifierKitInfo`) — so one statement is emitted once.
    // Here the component expander already emits every export from its own top-level statement
    // list, in source order and once each (`ets_component.rs`: `ordered_pieces` sorts the
    // statements, the helpers and the classes by source offset, and nothing is moved
    // afterwards); emitting it here as well produced the statement twice, and es2abc rejects
    // the second one as a duplicate export.
    let first = crate::build::lex::tokenize(line)
        .into_iter()
        .find(|t| t.kind != crate::build::lex::Kind::Comment);
    if first.is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "export") {
        return Vec::new();
    }
    let Some(parsed) = classify_import(line) else {
        return vec![line.to_string()];
    };
    match parsed.kind {
        ImportKind::So { request, .. } => {
            let spec = split_import(line)
                .map(|(s, _)| s.trim().to_string())
                .unwrap_or_default();
            vec![format!(
                "import {spec} from \"@normalized:Y&&&{request}&\";"
            )]
        }
        ImportKind::Kit { kit, bindings } => {
            // A Kit with no config keeps its statement: the reference reports
            // `ETS2BUNDLE_EXTERNAL_KIT_CONFIG_FILE_NOT_FOUND` and returns the node untouched,
            // rather than inventing a request the runtime cannot resolve.
            let Some(symbols) = kits.get(&kit) else {
                return vec![line.to_string()];
            };
            let mut out = Vec::new();
            for b in &bindings {
                // A name the Kit does not export is an error in the reference
                // (`ETS2BUNDLE_EXTERNAL_IMPORT_NAME_NOT_EXPORTED_FROM_KIT`) and contributes
                // no statement; a namespace import lands here too (`* as NS` is not a symbol
                // name), and the reference drops that statement with
                // `ETS2BUNDLE_EXTERNAL_KIT_NAMESPACE_IMPORT_EXPORT_NOT_SUPPORTED`.
                let Some((request, bkind)) = symbols.get(&b.symbol).cloned() else {
                    continue;
                };
                let request = crate::build::ets_assemble::system_module_request(&request);
                // A `type` written on the clause survives the expansion and qualifies every
                // line it becomes (`process_kit_import.js`:
                // `createImportClause(o.importClause.isTypeOnly, …)`). A symbol the file only
                // names in a type position keeps its line as `import type` as well: the
                // reference emits `import type AbilityConstant from
                // "@ohos:app.ability.AbilityConstant"`, and es2abc erases such a line instead
                // of adding it to the module's request array. Deciding it here — not in the
                // assembly pass — is what keeps the *written* `type` and the *inferred* one
                // apart: the assembly pass normalizes the paths of the statements it infers
                // and leaves the written ones alone (`fixtures/import-type-audit` vs
                // `fixtures/import-forms`).
                let kw = if parsed.type_only || !value_used.contains(&b.bind) {
                    "import type "
                } else {
                    "import "
                };
                if bkind == "default" {
                    // default binding: alias (fileIo as fs) becomes the local name
                    out.push(format!("{kw}{} from \"{request}\";", b.bind));
                } else {
                    // Named binding. The kit config's `bindings` field is the name the module
                    // really exports, which may differ from the kit's public name: `KeyAction`
                    // is exported as `Action` (`@kit.InputKit.json`). The reference builds the
                    // specifier from that field and marks it renamed whenever the local name
                    // differs (`process_kit_import.js`: `createImportSpecifier(…, isRenamed()
                    // ? getBindings() : undefined, getLocalName())`).
                    let named = if bkind == b.bind {
                        b.bind.clone()
                    } else {
                        format!("{} as {}", bkind, b.bind)
                    };
                    out.push(format!("{kw}{{ {named} }} from \"{request}\";"));
                }
            }
            out
        }
        ImportKind::Plain(s) => vec![s],
    }
}

/// One import binding after splitting a `@kit.*` import.
pub(crate) struct SplitBinding {
    /// Imported symbol name.
    pub(crate) symbol: String,
    /// Local binding (after `as`).
    pub(crate) bind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_used(source: &str) -> std::collections::HashSet<String> {
        let allocator = oxc_allocator::Allocator::default();
        let ret = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::ts()).parse();
        assert!(!ret.panicked && ret.errors.is_empty(), "{:?}", ret.errors);
        value_used_names(&ret.program)
    }

    /// `class A implements B` is erased with the class, so `B` is named in a type position.
    ///
    /// Counted as a value use, the kit expansion kept `rpc` a value import
    /// (`EntryAbility.ets` writes `import { rpc } from '@kit.IPCKit'` and only
    /// `rpc.MessageSequence` / `implements rpc.Parcelable` after it), so the statement stayed
    /// `import rpc from "@ohos:rpc"` where the reference emits `import type rpc`; es2abc then
    /// kept `@ohos:rpc` in the module's request array — 27 entries against the reference's 26,
    /// which is the whole of that file's bytecode divergence (18 mnemonic blocks, 2 lines).
    #[test]
    fn implements_clause_is_not_a_value_use() {
        let used = value_used(
            "class P implements rpc.Parcelable {\n\
                 marshalling(dataOut: rpc.MessageSequence): boolean { return true; }\n\
             }\n",
        );
        assert!(!used.contains("rpc"), "{used:?}");
    }

    /// `extends` is read at run time (the base class object), so it keeps a value import.
    #[test]
    fn extends_clause_is_a_value_use() {
        let used = value_used("class A extends Base {}\n");
        assert!(used.contains("Base"), "{used:?}");
    }
}
