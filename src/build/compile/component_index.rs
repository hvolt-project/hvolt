// component_index: which names are user components, and under which protocol.
//
// A call site cannot tell a user component from a built-in one by shape alone, and the
// instantiation differs: built-ins emit `Foo.create(…)`, user components emit an
// `observeComponentCreation2` frame that constructs the child through `ViewPU`/`ViewV2`.
// The whole project is known before any file is emitted, so the index is filled once.
//
// The declarations are read from the parsed program, not from the token stream: with `struct`
// rewritten to `class`, the decorators, the member names and the optional marker are nodes that
// already belong to their declaration. A scan that walked tokens could not tell where one
// struct's body ends, so an earlier struct inherited a later one's decorators — `theRowCustomDialog`
// took `KVCustomDialog`'s `@Link theDialogTitle` and its call site handed the child the parent's
// backstore where the reference reads the accessor (2026-11-07).
//
// The table is keyed by `(owner, name)`, not by the bare name: two modules can declare the same
// name under different protocols (`entry`'s `CrashPage` is V1, `xb_components`' is V2), and a
// flat table resolved whichever file was read last, so a call site could be emitted with the
// other module's protocol. A lookup resolves inside the owner the lowering sets for the file,
// then through that file's imports; a name that is a component of another module and is neither
// declared nor imported here is a defect and fails rather than resolving by name.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Class, ClassElement, Declaration, ExportDefaultDeclarationKind, Program, PropertyDefinition,
    PropertyKey, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::build::stage::utils::decorator_name;

/// Which protocol a component follows.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Protocol {
    #[default]
    /// `@Component` / `@CustomDialog` — a `ViewPU` subclass.
    V1,
    /// `@ComponentV2` — a `ViewV2` subclass.
    V2,
}

/// What a child component needs from its call site.
#[derive(Clone, Default, Debug)]
pub struct Meta {
    pub protocol: Protocol,
    /// `@Reusable`: the call site goes through the recycle pool instead of a plain creation.
    ///
    /// Not read yet: the recycle call site (`observeRecycleComponentCreation`) is the one part
    /// of a child's compiled shape the lowering does not produce, and `fixtures/reusable-call-site`
    /// records it. The field stays so the scanner keeps reporting the fact.
    #[allow(dead_code)]
    pub is_reusable: bool,
    /// `@ReusableV2`: the V2 recycle pool. The reference adds a file-level
    /// `__ReusableV2_Inner_Decorator__` helper, a class decorator and a
    /// `this.reuseOrCreateNewComponent({ componentClass, getParams, getReuseId, extraInfo })`
    /// call site for it (`process_struct_componentV2.js::createReusableV2ReflectFunction`).
    #[allow(dead_code)]
    pub is_reusable_v2: bool,
    /// The field declared with `@BuilderParam`, if any: a trailing child block at the call site
    /// is passed to the component as that field.
    pub builder_param: Option<String>,
    /// Fields the parent re-sends when it updates (`@Prop`/`@Link` in V1, `@Param` in V2).
    /// `updateStateVarsOfChildByElmtId` carries exactly these.
    pub state_vars: Vec<String>,
    /// Fields declared `@Link`: the constructor takes the parent's backstore for them, because
    /// a two-way link reads and writes the same observed property (measured 2026-11-06:
    /// `fixtures/link-call-site` emits `{ linked: this.__linked }` for both the `$linked` and
    /// the `this.linked` spelling).
    pub link_fields: Vec<String>,
    /// `@CustomDialog`: the call site inside a `CustomDialogController` is a dialog
    /// construction, which the reference wraps in a builder closure with a controller setter.
    pub is_dialog: bool,
}

/// The component table: `(owner, source, name)` → metadata.
///
/// `owner` is the module a source belongs to (`entry`, `xb_components`, …) or the package it
/// ships in; `source` is that source's path inside the owner, extension dropped. Both parts are
/// needed: two modules can declare the same name under different protocols (`entry`'s
/// `CrashPage` is V1, `xb_components`' is V2), and one package can declare the same name in two
/// files (`@mcui/mccharts` has a `Tooltip` with `@Prop`s and one without). A `""` source is the
/// module-level entry, used when a bare import names the module and not the file.
pub type ComponentMap = BTreeMap<(String, String, String), Meta>;

static INDEX: OnceLock<ComponentMap> = OnceLock::new();

/// `(owner, source, local name)` → the request that file imports it from.
/// A call site is reached through this table; a relative request names a file, a bare one a
/// module.
static IMPORTS: OnceLock<BTreeMap<(String, String, String), String>> = OnceLock::new();

/// Every top-level `@Builder function` in the project: a call to one is a bound call, never
/// a component (`X.bind(this)(…)`), and the name may be declared in another file.
static BUILDERS: OnceLock<std::collections::BTreeSet<String>> = OnceLock::new();

/// Publish the project-wide builder-function names.
pub fn set_builders(names: std::collections::BTreeSet<String>) {
    let _ = BUILDERS.set(names);
}

/// Whether `name` is a project-wide `@Builder function`.
pub fn is_builder(name: &str) -> bool {
    BUILDERS.get().is_some_and(|m| m.contains(name))
}

/// `@Builder function name(…)` declarations in one source.
///
/// The decorator is not in the parsed program: TypeScript has no decorator on a function
/// declaration, so the masking pass blanks it and records the name (`process_ui_syntax.rs`,
/// the `"Builder" | "LocalBuilder"` arm). That pass is the lowering's own collector of what a
/// project builder is, so the list is read from it rather than re-derived from the text.
pub fn scan_builder_functions(source: &str) -> Vec<String> {
    let (_masked, builders, _extends, _styles, _locals) =
        crate::build::ets_compile::mask_builder_decorators_with_locals(source);
    remember(&builders);
    builders
}

/// Publish the project-wide component table. Called once per build.
///
/// The table is final here, so a source whose parse needed a name from another file is retried
/// before it is stored ([`resolve_deferred`]).
pub fn set(mut map: ComponentMap) {
    resolve_deferred(&mut map);
    // The import table is frozen with the component table: both answer one question, and the
    // answer must come from the same pass over the sources.
    let imports = IMPORT_SOURCES.with(|sources| std::mem::take(&mut *sources.borrow_mut()));
    let _ = IMPORTS.set(imports);
    let _ = INDEX.set(map);
}

/// Record one source's declarations, under its own path and under the module-level key.
///
/// The module-level key answers a bare import (`from "xb_components"`): the request names the
/// module, not the file, so the file cannot be resolved from it. A relative import is answered
/// by the file-level key.
pub fn record(
    map: &mut ComponentMap,
    owner: &str,
    source: &str,
    declarations: Vec<(String, Meta)>,
) {
    for (name, meta) in declarations {
        map.insert(
            (owner.to_string(), String::new(), name.clone()),
            meta.clone(),
        );
        map.insert((owner.to_string(), source.to_string(), name), meta);
    }
}

thread_local! {
    /// The names a child block can follow — every component and `@Builder function` read so
    /// far. A file that calls an imported component with a child block (`toggleList({…}) { … }`)
    /// is not TypeScript until that name is rewritten, and the lowering parses such a file with
    /// the project's own table (`ets_assemble::project_names`); this is that table, grown one
    /// source at a time.
    static NAMES: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// The source being lowered: its owner and its path inside that owner. Set per source by the
    /// lowering (`ets_assemble`), the only place that knows which module the file belongs to.
    static CURRENT_SOURCE: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    /// The imports read from the sources scanned so far, frozen by [`set`].
    static IMPORT_SOURCES: RefCell<BTreeMap<(String, String, String), String>> =
        const { RefCell::new(BTreeMap::new()) };
    /// Sources that name something their own file does not declare, with the owner and path they
    /// belong to. The scan runs before the table is complete, so they wait for [`set`].
    static DEFERRED: RefCell<Vec<(String, String, String)>> = const { RefCell::new(Vec::new()) };
}

/// Record the names a later source may use in a child block.
fn remember<'a>(names: impl IntoIterator<Item = &'a String>) {
    NAMES.with(|known| known.borrow_mut().extend(names.into_iter().cloned()));
}

/// The names gathered from the sources read so far.
fn extra_names() -> Vec<String> {
    NAMES.with(|known| known.borrow().iter().cloned().collect())
}

/// Retry the sources whose parse needed an imported name, now that every source has been read.
///
/// The pass repeats while it makes progress: one deferred source may name a component that a
/// later deferred source declares. A source that still does not parse is left out — the
/// lowering reports it with its error context.
fn resolve_deferred(map: &mut ComponentMap) {
    let mut deferred = DEFERRED.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    while !deferred.is_empty() {
        let extra = extra_names();
        let mut remaining = Vec::new();
        let mut progress = false;
        for (owner, path, source) in deferred {
            let Some(parsed) = parse_declarations(&source, &extra) else {
                remaining.push((owner, path, source));
                continue;
            };
            progress = true;
            for (name, _) in &parsed.declarations {
                remember(std::iter::once(name));
            }
            record(map, &owner, &path, parsed.declarations);
        }
        if !progress {
            return;
        }
        deferred = remaining;
    }
}

/// Components a source declares, with their protocol.
///
/// `@Entry` is not a protocol, so a declaration carrying only `@Entry` still needs an
/// `@Component` to be one; both decorator spellings are checked independently.
pub fn scan_declarations(owner: &str, path: &str, source: &str) -> Vec<(String, Meta)> {
    let Some(parsed) = parse_declarations(source, &extra_names()) else {
        // The source names a component another file declares; the table that makes it
        // parseable is only complete at [`set`].
        DEFERRED.with(|deferred| {
            deferred
                .borrow_mut()
                .push((owner.to_string(), path.to_string(), source.to_string()))
        });
        return Vec::new();
    };
    remember(parsed.declarations.iter().map(|(name, _)| name));
    record_imports(owner, path, &parsed.imports);
    parsed.declarations
}

/// The declarations of one source, or `None` when it does not parse.
///
/// `extra` widens the names a child block can follow; the program itself is read the same way
/// either way.
fn parse_declarations(source: &str, extra: &[String]) -> Option<Parsed> {
    let prepared = prepare(source, extra);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &prepared, SourceType::ts()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    Some(Parsed {
        declarations: declarations_in(&parsed.program),
        imports: imports_in(&parsed.program),
    })
}

/// What one source declares, and what it imports.
struct Parsed {
    declarations: Vec<(String, Meta)>,
    /// `(local name, request)` for every named and default import.
    imports: Vec<(String, String)>,
}

/// The import bindings a program declares: the local name, and the request it came from.
fn imports_in(program: &Program<'_>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let request = import.source.value.to_string();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers.iter() {
            let local = match specifier {
                oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    specifier.local.name.as_str()
                }
                oxc_ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    specifier.local.name.as_str()
                }
                // A namespace import is called `ns.Name`, never by a bare name.
                oxc_ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => continue,
            };
            out.push((local.to_string(), request.clone()));
        }
    }
    out
}

/// Record one source's imports under its owner and path.
fn record_imports(owner: &str, path: &str, imports: &[(String, String)]) {
    IMPORT_SOURCES.with(|sources| {
        let mut sources = sources.borrow_mut();
        for (local, request) in imports {
            sources.insert(
                (owner.to_string(), path.to_string(), local.clone()),
                request.clone(),
            );
        }
    });
}

/// The imports one owner's sources declared: `(local name, request)` pairs.
///
/// [`scan_declarations`] records them while it reads a source, because a call site in another
/// file reaches that source's declarations through the import. A caller that has to open the
/// file an import names — a module that ships only a declaration has no compiled source for
/// the scan above to read, yet it declares its components in that file — needs the same pairs,
/// and it needs the file prepared the way the scan prepares a source (`struct` rewritten, the
/// decorators a TypeScript parser rejects masked, the child blocks rewritten) before it parses.
/// Reading the recorded table gives both: one reader, one parse per source.
pub fn imports_of(owner: &str) -> Vec<(String, String)> {
    IMPORT_SOURCES.with(|sources| {
        let mut out: Vec<(String, String)> = Vec::new();
        for ((key, _, local), request) in sources.borrow().iter() {
            if key != owner {
                continue;
            }
            let pair = (local.clone(), request.clone());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
        out
    })
}

/// The source in the shape the lowering parses.
///
/// `struct` is rewritten to `class` by length-preserving replacement, the decorators a
/// TypeScript parser rejects are masked, and the ArkUI child blocks inside a `build()` body are
/// rewritten into calls; the same three steps prepare the source `ets_component::parse_components`
/// parses. The rewriting changes text but never a declaration, so the names, decorators and
/// member order read from the program are the source's.
fn prepare(source: &str, extra: &[String]) -> String {
    let (masked, mut builders, _extends, _styles, locals) =
        crate::build::ets_compile::mask_builder_decorators_with_locals(source);
    builders.extend(extra.iter().cloned());
    // A child block also follows a component this file declares, which no decorator list
    // names. `ets_assemble::struct_names` is the collector the lowering itself feeds to the
    // same rewrite (`project_names`); this is that list for the one file being read.
    builders.extend(crate::build::ets_assemble::struct_names(source));
    crate::build::ets_compile::rewrite_builders_with_locals(
        &crate::build::ets_compile::preprocess(&masked),
        &builders,
        &locals,
    )
}

/// The recorded metadata for one declared component.
pub fn meta_of(name: &str) -> Option<&'static Meta> {
    let (owner, path) = current_source()?;
    resolve(INDEX.get()?, IMPORTS.get(), &owner, &path, name)
}

/// The source the call sites being lowered belong to: its owner, and its path inside it.
///
/// The lowering sets it once per source (`ets_assemble`), because neither is in the source text.
/// A lookup without one answers `None`, like a name no source declares.
pub fn current_source() -> Option<(String, String)> {
    CURRENT_SOURCE.with(|slot| slot.borrow().clone())
}

/// Set the source the call sites being lowered belong to; the caller restores the previous one
/// with [`restore_source`] when it is done.
pub fn set_current_source(owner: &str, path: &str) {
    CURRENT_SOURCE.with(|slot| *slot.borrow_mut() = Some((owner.to_string(), path.to_string())));
}

/// Restore the source that was current before [`set_current_source`].
pub fn restore_source(previous: Option<(String, String)>) {
    CURRENT_SOURCE.with(|slot| *slot.borrow_mut() = previous);
}

/// The component `name` names at a call site in `owner`/`path`, or `None` when no source declares
/// it.
///
/// The source's own declaration wins. A name the source does not declare is looked up through
/// its import: a relative request names a file and resolves against the importing source's
/// directory (`'./Tooltip'` from `pageView/Charts` is `pageView/Tooltip`, not the `Tooltip` in
/// `components/mainpage`), a bare request names a module and answers from that module's entry.
/// A name that is a component elsewhere and is neither declared nor imported here is a defect,
/// not a built-in: resolving it by name is how two components got crossed.
///
/// Known gap: a module reached only through a re-export chain (module A re-exports module B's
/// component, and the caller imports it from A) is not resolved — `(A, name)` is not `(B, name)`
/// — so that call fails here rather than resolving the import graph.
fn resolve<'a>(
    index: &'a ComponentMap,
    imports: Option<&'a BTreeMap<(String, String, String), String>>,
    owner: &str,
    path: &str,
    name: &str,
) -> Option<&'a Meta> {
    if let Some(meta) = index.get(&(owner.to_string(), path.to_string(), name.to_string())) {
        return Some(meta);
    }
    if let Some(request) =
        imports.and_then(|map| map.get(&(owner.to_string(), path.to_string(), name.to_string())))
    {
        for (candidate_owner, candidate_path) in import_targets(request, owner, path) {
            if let Some(meta) = index.get(&(candidate_owner, candidate_path, name.to_string())) {
                return Some(meta);
            }
        }
    }
    // A bare request names the module, not the file: the module-level entry answers it.
    if let Some(meta) = index.get(&(owner.to_string(), String::new(), name.to_string())) {
        return Some(meta);
    }
    if index.keys().any(|(_, _, declared)| declared == name) {
        panic!(
            "component `{name}` is declared elsewhere and `{owner}/{path}` neither declares nor \
             imports it; the index resolves by (owner, source, name) and does not read re-export \
             chains"
        );
    }
    None
}

/// The `(owner, source)` pairs an import request can name.
///
/// A relative request resolves against the importing source's directory. A bare one names a
/// module or a package; a request written against the source layout
/// (`xb_components/src/main/ets/...`) names the module that owns it, so its prefixes are tried
/// in order, each at module granularity (an empty source path).
fn import_targets(request: &str, owner: &str, path: &str) -> Vec<(String, String)> {
    if request.starts_with('.') {
        return vec![(owner.to_string(), resolve_relative(path, request))];
    }
    request_prefixes(request)
        .into_iter()
        .map(|candidate| (candidate, String::new()))
        .collect()
}

/// The source path a relative request names, resolved against the importing source's directory.
fn resolve_relative(path: &str, request: &str) -> String {
    let mut segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    // Drop the importing file, keep its directory.
    segments.pop();
    for segment in request.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(strip_source_extension(other)),
        }
    }
    segments.join("/")
}

/// `Tooltip.ets` and `Tooltip.ts` name the source `Tooltip`, the same spelling the index keys it
/// by.
fn strip_source_extension(segment: &str) -> &str {
    segment
        .strip_suffix(".ets")
        .or_else(|| segment.strip_suffix(".ts"))
        .unwrap_or(segment)
}

/// The owners a bare import request can name, most specific first.
fn request_prefixes(request: &str) -> Vec<String> {
    let mut candidates = vec![request.to_string()];
    let mut rest = request;
    while let Some(at) = rest.rfind('/') {
        rest = &rest[..at];
        if !candidates.iter().any(|known| known == rest) {
            candidates.push(rest.to_string());
        }
    }
    candidates
}

/// The protocol one declared component lowers to.
pub fn protocol_of(name: &str) -> Option<Protocol> {
    meta_of(name).map(|meta| meta.protocol)
}

/// One component per decorated class, in source order.
fn declarations_in(program: &Program<'_>) -> Vec<(String, Meta)> {
    let mut out = Vec::new();
    for class in classes(program) {
        let Some(id) = &class.id else {
            continue;
        };
        let decorators = decorator_names(class.decorators.iter());
        let Some(protocol) = protocol_from(&decorators) else {
            continue;
        };
        out.push((
            id.name.to_string(),
            Meta {
                protocol,
                is_reusable: has(&decorators, "Reusable"),
                is_reusable_v2: has(&decorators, "ReusableV2"),
                builder_param: builder_param_of(class),
                state_vars: state_vars_of(class, protocol),
                link_fields: link_fields_of(class),
                is_dialog: has(&decorators, "CustomDialog"),
            },
        ));
    }
    out
}

/// Every class declaration at the top level, however it is exported.
fn classes<'a, 'ast>(program: &'a Program<'ast>) -> Vec<&'a Class<'ast>> {
    let mut out = Vec::new();
    for statement in &program.body {
        let class = match statement {
            Statement::ClassDeclaration(class) => Some(&**class),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::ClassDeclaration(class)) => Some(&**class),
                _ => None,
            },
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::ClassDeclaration(class) => Some(&**class),
                _ => None,
            },
            _ => None,
        };
        if let Some(class) = class {
            out.push(class);
        }
    }
    out
}

/// The names of the decorators, with their arguments dropped: `@Preview` and `@CustomDialog`
/// are what the decision needs, not what they carry.
fn decorator_names<'a, 'ast>(
    decorators: impl Iterator<Item = &'a oxc_ast::ast::Decorator<'ast>>,
) -> Vec<String>
where
    'ast: 'a,
{
    decorators
        .filter_map(|decorator| decorator_name(&decorator.expression))
        .collect()
}

/// Whether the decorator was written on the declaration.
fn has(decorators: &[String], wanted: &str) -> bool {
    decorators.iter().any(|decorator| decorator == wanted)
}

/// The protocol a class's decorators name, or `None` when the class is not a component.
fn protocol_from(decorators: &[String]) -> Option<Protocol> {
    if has(decorators, "ComponentV2") {
        Some(Protocol::V2)
    } else if has(decorators, "Component") || has(decorators, "CustomDialog") {
        Some(Protocol::V1)
    } else {
        None
    }
}

/// The property members of one class, in declaration order.
fn properties<'a, 'ast>(class: &'a Class<'ast>) -> Vec<&'a PropertyDefinition<'ast>> {
    class
        .body
        .body
        .iter()
        .filter_map(|element| match element {
            ClassElement::PropertyDefinition(property) => Some(&**property),
            _ => None,
        })
        .collect()
}

/// The name a property declares, which is the source's spelling. A key written as a string or
/// a number is not an ArkTS field, so it names nothing here.
fn property_name<'a, 'ast>(property: &'a PropertyDefinition<'ast>) -> Option<&'a str> {
    match &property.key {
        PropertyKey::StaticIdentifier(key) => Some(key.name.as_str()),
        _ => None,
    }
}

/// The `@BuilderParam` field of one class, if it declares one.
fn builder_param_of(class: &Class<'_>) -> Option<String> {
    properties(class).into_iter().find_map(|property| {
        let decorators = decorator_names(property.decorators.iter());
        has(&decorators, "BuilderParam")
            .then(|| property_name(property).map(str::to_string))
            .flatten()
    })
}

fn state_vars_of(class: &Class<'_>, protocol: Protocol) -> Vec<String> {
    let wanted: &[&str] = match protocol {
        // `@Prop` and `@ObjectLink` are re-sent; a `@Link` child already holds the parent's
        // backstore, so the compiler omits it (measured: fixtures/link emits an empty object,
        // fixtures/state-decorators re-sends the `@ObjectLink`).
        Protocol::V1 => &["Prop", "ObjectLink"],
        Protocol::V2 => &["Param"],
    };
    fields_with(class, wanted)
}

/// The `@Link` fields of one class, in source order.
///
/// A two-way link is constructed from the parent's observed property, not from its value, so
/// the call site needs the list before it can write the params object (measured 2026-11-06:
/// `fixtures/link-call-site`).
fn link_fields_of(class: &Class<'_>) -> Vec<String> {
    fields_with(class, &["Link"])
}

/// The property names of one class that carry one of the wanted decorators, in source order.
///
/// A member may carry several decorators (`@Param @Once x: number`), so any one of the wanted
/// names selects it. `name?: T` declares the same member as `name: T`: the optional marker is
/// on the property node and does not change which set the field belongs to (measured:
/// `Xb_SideBar` re-sends `icon1?`, `icon2?` and `isBackPress?`).
fn fields_with(class: &Class<'_>, wanted: &[&str]) -> Vec<String> {
    properties(class)
        .into_iter()
        .filter(|property| {
            let decorators = decorator_names(property.decorators.iter());
            wanted.iter().any(|wanted| has(&decorators, wanted))
        })
        .filter_map(|property| property_name(property).map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_params_are_collected() {
        let src = r#"
/**通用圆形按钮*/
@ComponentV2
struct Xb_CircleButton {

  @Param iconSize: number = 21
  // @Param isShowMenu: boolean = false
  @Param needClick: boolean = true

  build() { Button() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        assert_eq!(decls.len(), 1, "{decls:?}");
        assert_eq!(decls[0].0, "Xb_CircleButton");
        assert_eq!(decls[0].1.protocol, Protocol::V2);
        assert_eq!(decls[0].1.state_vars, vec!["iconSize", "needClick"]);
    }

    #[test]
    fn v1_collects_the_fields_the_compiler_resends() {
        let src = r#"
@Component
export struct Card {
  @Prop title: string = ''
  @Link selected: boolean
  @State local: number = 0
  build() { Column() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        assert_eq!(decls[0].1.protocol, Protocol::V1);
        // Only `@Prop` is re-sent by id; a `@Link` child already shares the parent's
        // backstore, so the compiler omits it (fixtures/link emits an empty object).
        assert_eq!(decls[0].1.state_vars, vec!["title"]);
    }
    #[test]
    fn optional_members_are_collected() {
        // `name?: T` writes the `?` between the name and the `:`. Every optional member used
        // to be dropped from the update object, which is what `Xb_SideBar` measured: the three
        // re-sent members it lost (`icon1`, `icon2`, `isBackPress`) are exactly the optional
        // ones, and the five it kept carry no `?`.
        let src = r#"
@ComponentV2
struct Xb_TopTitleBar {
  @Param icon1?: Resource = undefined
  @Param isBackPress?: boolean = false
  @Param name: string = ''
  build() { Row() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        assert_eq!(decls[0].1.state_vars, vec!["icon1", "isBackPress", "name"]);
    }

    #[test]
    fn a_plain_optional_field_is_not_a_state_var() {
        // The `?` makes the member optional, not decorated: a bare `x?: T` is nobody's state
        // var, and collecting it would re-send it on every update.
        let src = r#"
@ComponentV2
struct Bare {
  @Param icon1?: Resource = undefined
  plain?: number
  other: string = ''
  build() { Row() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        assert_eq!(decls[0].1.state_vars, vec!["icon1"]);
    }

    #[test]
    fn real_file_scan() {
        let text = std::fs::read_to_string(
        "/Users/hongyuncong/DshHome/clashbox-tailscale/clashbox-flat/xb_components/components/Xb_Button.ets",
    )
    .unwrap();
        let decls = crate::build::compile::component_index::scan_declarations(
            "entry",
            "test/Source",
            &text,
        );
        for (name, meta) in &decls {
            eprintln!("{name} {:?} {:?}", meta.protocol, meta.state_vars);
        }
        assert!(!decls.is_empty());
    }
}

#[cfg(test)]
mod link_tests {
    use super::scan_declarations;

    /// A `@Link` member is recorded by its decorator, and the field survives beside the ones
    /// the parent re-sends.
    #[test]
    fn link_members_are_recorded_apart_from_the_resend_set() {
        let src = r#"
@Component
struct Card {
  @Link linked: number
  @Prop copied: boolean = false
  build() { Column() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        assert_eq!(decls[0].1.link_fields, vec!["linked"]);
        assert_eq!(decls[0].1.state_vars, vec!["copied"]);
        assert!(!decls[0].1.is_dialog);
    }

    /// `@CustomDialog` is a protocol of its own for the call site, and it stays V1.
    #[test]
    fn a_custom_dialog_is_flagged() {
        let src = r#"
@CustomDialog
struct Dialog {
  controller?: CustomDialogController
  @Link linked: number
  build() { Column() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        assert_eq!(decls[0].1.protocol, super::Protocol::V1);
        assert!(decls[0].1.is_dialog);
        assert_eq!(decls[0].1.link_fields, vec!["linked"]);
    }

    /// A struct's members end at its own closing brace: a `@Link` a later struct in the same
    /// module declares is not a member of the earlier one. The scan used to run to the end of
    /// the file, so `theRowCustomDialog`'s record inherited `KVCustomDialog`'s `@Link
    /// theDialogTitle` and `theCustomDialog`'s `@Link theDialogTitle`.
    #[test]
    fn a_later_struct_does_not_leak_its_link_fields() {
        let src = r#"
@CustomDialog
export struct TheRowDialog {
  @State theDialogTitle: string = ''
  controller?: CustomDialogController
  build() { Column() {} }
}

@CustomDialog
export struct KVDialog {
  @Link theDialogTitle: string
  build() { Column() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        let row = decls
            .iter()
            .find(|(name, _)| name == "TheRowDialog")
            .expect("TheRowDialog is declared");
        assert!(row.1.link_fields.is_empty(), "{:?}", row.1.link_fields);
        let kv = decls
            .iter()
            .find(|(name, _)| name == "KVDialog")
            .expect("KVDialog is declared");
        assert_eq!(kv.1.link_fields, vec!["theDialogTitle"]);
    }

    /// The same bound for the fields the parent re-sends: a `@Prop` written by a later struct
    /// used to land in the earlier struct's `state_vars`, and `updateStateVarsOfChildByElmtId`
    /// then carried a member the child does not declare.
    #[test]
    fn a_later_struct_does_not_leak_its_state_vars() {
        let src = r#"
@CustomDialog
export struct TheRowDialog {
  @State theDialogTitle: string = ''
  build() { Column() {} }
}

@CustomDialog
export struct KVDialog {
  @Prop value: string = ''
  build() { Column() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        let row = decls
            .iter()
            .find(|(name, _)| name == "TheRowDialog")
            .expect("TheRowDialog is declared");
        assert!(row.1.state_vars.is_empty(), "{:?}", row.1.state_vars);
        let kv = decls
            .iter()
            .find(|(name, _)| name == "KVDialog")
            .expect("KVDialog is declared");
        assert_eq!(kv.1.state_vars, vec!["value"]);
    }

    /// A later struct's `@BuilderParam` is not the earlier struct's trailing child slot.
    #[test]
    fn a_later_struct_does_not_leak_its_builder_param() {
        let src = r#"
@CustomDialog
export struct TheRowDialog {
  @State theDialogTitle: string = ''
  build() { Column() {} }
}

@CustomDialog
export struct TailDialog {
  @BuilderParam content: () => void
  build() { Column() {} }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        let row = decls
            .iter()
            .find(|(name, _)| name == "TheRowDialog")
            .expect("TheRowDialog is declared");
        assert_eq!(row.1.builder_param, None);
        let tail = decls
            .iter()
            .find(|(name, _)| name == "TailDialog")
            .expect("TailDialog is declared");
        assert_eq!(tail.1.builder_param.as_deref(), Some("content"));
    }

    /// The value a dialog call site passes for a member that is not a `@Link` stays the parent's
    /// accessor: `theDialogTitle: this.theResetDialogTitle`, never the backstore
    /// `this.__theResetDialogTitle` (the reference reads the getter; the bytecode differs only
    /// in the `ldobjbyname` property name).
    #[test]
    fn a_dialog_state_read_at_the_call_site_keeps_the_accessor() {
        let src = r#"
@CustomDialog
export struct TheRowDialog {
  @State theDialogTitle: string = ''
  controller?: CustomDialogController
  build() { Column() {} }
}

@CustomDialog
export struct KVDialog {
  @Link theDialogTitle: string
  build() { Column() {} }
}
"#;
        let row = scan_declarations("entry", "test/Source", src)
            .into_iter()
            .find(|(name, _)| name == "TheRowDialog")
            .expect("TheRowDialog is declared");
        let params = crate::build::stage::process_custom_component::construct_params(
            "{ theDialogTitle: this.theResetDialogTitle }",
            &row.1.link_fields,
        );
        assert_eq!(
            params, "{ theDialogTitle: this.theResetDialogTitle }",
            "the state read must keep the getter name"
        );
    }

    /// A child block also follows a component the same file declares; the scan has to know that
    /// name before it parses, or the file is read as having no components at all.
    #[test]
    fn a_source_that_calls_its_own_component_with_a_child_block_is_read() {
        let src = r#"
@Component
struct Row {
  @BuilderParam content: () => void
  build() { Column() { this.content() } }
}

@CustomDialog
export struct Caller {
  @Prop title: string = ''
  build() {
    Column() {
      Row({ a: this.title }) {
        Text('x')
      }
    }
  }
}
"#;
        let decls = scan_declarations("entry", "test/Source", src);
        let names: Vec<&str> = decls.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, vec!["Row", "Caller"], "{names:?}");
        let caller = decls.iter().find(|(name, _)| name == "Caller").unwrap();
        assert!(caller.1.is_dialog);
        assert_eq!(caller.1.state_vars, vec!["title"]);
    }

    /// A file that calls a component another file declares parses only once that name is
    /// known: the child-block rewrite needs it, exactly as the lowering's own parse needs the
    /// project table. The scan runs first, so the source waits for [`super::resolve_deferred`].
    #[test]
    fn a_source_naming_an_imported_component_is_retried_when_the_table_is_complete() {
        let caller = r#"
@CustomDialog
export struct Caller {
  @State title: string = ''
  build() {
    Column() {
      SomeImported({ a: this.title }) {
        Text('x')
      }
    }
  }
}
"#;
        let imported = r#"
@Component
export struct SomeImported {
  @BuilderParam content: () => void
  build() { Column() { this.content() } }
}
"#;
        assert!(
            scan_declarations("entry", "pages/Caller", caller).is_empty(),
            "the caller parses only once SomeImported is known"
        );
        assert_eq!(
            scan_declarations("xb_components", "view/Imported", imported).len(),
            1
        );
        let mut map = std::collections::BTreeMap::new();
        super::resolve_deferred(&mut map);
        assert!(
            map.contains_key(&(
                "entry".to_string(),
                "pages/Caller".to_string(),
                "Caller".to_string()
            )),
            "the deferred source is retried: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }

    /// Two modules can declare the same name under different protocols. The table keyed by
    /// the bare name resolved whichever file was read last, so `entry`'s `CrashPage` was
    /// emitted as a V2 component (`ViewV2.create`) where the reference emits `ViewPU.create`.
    #[test]
    fn two_modules_keep_their_own_component_of_the_same_name() {
        let entry = r#"
@Component
export struct CrashPage {
  @State message: string = ''
  build() { Column() { Text(this.message) } }
}
"#;
        let xb = r#"
@ComponentV2
export struct CrashPage {
  @Param message: string = ''
  build() { Column() { Text(this.message) } }
}
"#;
        let mut index = std::collections::BTreeMap::new();
        for (owner, path, src) in [
            ("entry", "components/Start/CrashPage", entry),
            ("xb_components", "view/Xb_CrashPage", xb),
        ] {
            let parsed = super::parse_declarations(src, &[]).expect("the declaration parses");
            super::record(&mut index, owner, path, parsed.declarations);
        }
        assert_eq!(
            super::resolve(&index, None, "entry", "pages/Index", "CrashPage")
                .expect("entry declares CrashPage")
                .protocol,
            super::Protocol::V1
        );
        assert_eq!(
            super::resolve(
                &index,
                None,
                "xb_components",
                "view/Xb_CrashPage",
                "CrashPage"
            )
            .expect("xb_components declares CrashPage")
            .protocol,
            super::Protocol::V2
        );
    }

    /// A component another module declares is reached through the importing file's import, not
    /// by its bare name.
    #[test]
    fn a_component_of_another_module_is_reached_through_the_import() {
        let xb = r#"
@ComponentV2
export struct Xb_Studio {
  build() { Column() { Text('x') } }
}
"#;
        let parsed = super::parse_declarations(xb, &[]).expect("the declaration parses");
        let mut index = std::collections::BTreeMap::new();
        super::record(
            &mut index,
            "xb_components",
            "view/Xb_Studio",
            parsed.declarations,
        );
        let mut imports = std::collections::BTreeMap::new();
        imports.insert(
            (
                "entry".to_string(),
                "pages/Index".to_string(),
                "Xb_Studio".to_string(),
            ),
            "xb_components".to_string(),
        );
        assert!(
            super::resolve(&index, Some(&imports), "entry", "pages/Index", "Xb_Studio").is_some()
        );
        // A request written against the source layout names the module that owns it.
        imports.insert(
            (
                "entry".to_string(),
                "pages/Index".to_string(),
                "Xb_Studio".to_string(),
            ),
            "xb_components/src/main/ets/view/Xb_Studio".to_string(),
        );
        assert!(
            super::resolve(&index, Some(&imports), "entry", "pages/Index", "Xb_Studio").is_some()
        );
    }

    /// A component of another module that the caller neither declares nor imports is a defect,
    /// not a built-in: resolving it by name is how the two `CrashPage`s got crossed.
    #[test]
    #[should_panic(expected = "declared elsewhere")]
    fn a_component_of_another_module_is_not_resolved_by_name() {
        let xb = r#"
@ComponentV2
export struct CrashPage {
  build() { Column() { Text('x') } }
}
"#;
        let parsed = super::parse_declarations(xb, &[]).expect("the declaration parses");
        let mut index = std::collections::BTreeMap::new();
        super::record(
            &mut index,
            "xb_components",
            "view/Xb_CrashPage",
            parsed.declarations,
        );
        let _ = super::resolve(&index, None, "entry", "pages/Index", "CrashPage");
    }

    /// The import table is readable per owner: the declaration scan of a module that ships only
    /// a `.d.ets` needs the request its callers import the name from, which is the owner the
    /// declaration has to be keyed by.
    #[test]
    fn the_import_table_answers_per_owner() {
        let src = r#"
import { SegmentButton, SegmentButtonOptions } from '@kit.ArkUI'
import { Empty } from './Sibling'

@Component
struct Card {
    build() { Column() {} }
}
"#;
        assert_eq!(scan_declarations("entry", "test/Source", src).len(), 1);
        let mut imports = super::imports_of("entry");
        imports.sort();
        assert_eq!(
            imports,
            vec![
                ("Empty".to_string(), "./Sibling".to_string()),
                ("SegmentButton".to_string(), "@kit.ArkUI".to_string()),
                ("SegmentButtonOptions".to_string(), "@kit.ArkUI".to_string()),
            ]
        );
        assert!(super::imports_of("xb_components").is_empty());
    }

    /// One package can declare the same name in two files. A relative import names a file, so a
    /// call site takes the declaration beside it: `@mcui/mccharts` has a `Tooltip` with
    /// `@Prop ctxWidth` / `@Prop ctxHeight` in `pageView/` and one without in
    /// `components/mainpage/`, and keying by the package alone let the wrong one answer
    /// (`updateStateVarsOfChildByElmtId(…, {})` where the reference re-sends the two props).
    #[test]
    fn a_relative_import_selects_the_file_beside_it() {
        let sibling = r#"
@CustomDialog
export struct Tooltip {
  @Link tooltipInfo: string
  @Prop ctxWidth: number = 0
  @Prop ctxHeight: number = 0
  @BuilderParam customTooltip: () => void
  build() { Column() {} }
}
"#;
        let other = r#"
@CustomDialog
export struct Tooltip {
  @Link tooltipInfo: string
  @BuilderParam customTooltip: () => void
  build() { Column() {} }
}
"#;
        let mut index = std::collections::BTreeMap::new();
        for (path, src) in [
            ("pageView/Tooltip", sibling),
            ("components/mainpage/Tooltip", other),
        ] {
            let parsed = super::parse_declarations(src, &[]).expect("the declaration parses");
            super::record(&mut index, "mccharts", path, parsed.declarations);
        }
        let mut imports = std::collections::BTreeMap::new();
        imports.insert(
            (
                "mccharts".to_string(),
                "pageView/Charts".to_string(),
                "Tooltip".to_string(),
            ),
            "./Tooltip".to_string(),
        );
        let meta = super::resolve(
            &index,
            Some(&imports),
            "mccharts",
            "pageView/Charts",
            "Tooltip",
        )
        .expect("the sibling file declares Tooltip");
        assert_eq!(meta.state_vars, vec!["ctxWidth", "ctxHeight"]);
        // The same request written in another directory names the other file.
        imports.insert(
            (
                "mccharts".to_string(),
                "components/mainpage/Other".to_string(),
                "Tooltip".to_string(),
            ),
            "./Tooltip".to_string(),
        );
        let meta = super::resolve(
            &index,
            Some(&imports),
            "mccharts",
            "components/mainpage/Other",
            "Tooltip",
        )
        .expect("the other file declares Tooltip");
        assert!(meta.state_vars.is_empty(), "{:?}", meta.state_vars);
    }

    /// `@ObjectLink` and `@Param` are not links: only `@Link` needs the parent's backstore.
    #[test]
    fn other_decorators_are_not_links() {
        let src = r#"
@Observed
class Model { value: number = 0 }

@Component
struct Holder {
  @ObjectLink item: Model
  build() { Column() {} }
}

@ComponentV2
struct Params {
  @Param value: string = ''
  build() { Column() {} }
}
"#;
        for (name, meta) in scan_declarations("entry", "test/Source", src) {
            assert!(meta.link_fields.is_empty(), "{name} {:?}", meta.link_fields);
        }
    }
}
