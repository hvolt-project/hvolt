// process stage: `process_component_class.js` — the class a `struct` becomes.
//
// The reference's `processComponentClass(structNode, errors, checker, …)`
// (`process_component_class.js:48`) returns `createClassDeclaration(modifiers, name,
// typeParameters, updateHeritageClauses(node, errors), members)`: the heritage clause picks
// the base class, `processMembers` orders the members, and
// `checkFinalizeConstruction` decides the file's header polyfill. `getEntryNameFunction`
// (`:222`) is the static `getEntryName()` an `@Entry` page carries.
//
// One `struct` per class: a file may declare several, and each picks its own base class
// through `protocol_for`.

use crate::build::stage::process_component_build::{
    emit_initial_render, emit_struct_builder_method,
};
use crate::build::stage::process_component_member::{
    emit_v1_field_init, emit_v1_initial_params, emit_v1_update_state_vars,
};
use crate::build::stage::process_struct_componentV2::emit_v2_class;
use crate::build::stage::utils::{ComponentInfo, Protocol};

/// What one source file says that every class of it needs, read once from its tokens.
///
/// Two facts share the pass because both are file-level and both are read while the file is
/// emitted:
///
/// * `@LocalBuilder` — `validate_ui_syntax.js` `setLocalBuilderInFile` sets
///   `storedFileInfo.hasLocalBuilderInFile` the moment a decorator named `LocalBuilder` is seen,
///   and that one flag then drives four emissions: the file header (`process_ui_syntax.js`
///   `processUISyntax` unshifts `checkContextStack()`), the `initialRender` body
///   (`process_component_build.js` `processComponentBuild`), `rerender()`
///   (`process_component_class.js` `addRerenderFunc`) and the method itself
///   (`process_component_class.js` `processComponentMethod` → `localBuilderNode`). The flag is
///   per file, so every class of the file is bracketed, while the names are kept so a `@Builder`
///   method of the same struct keeps its `parent = null` form.
/// * `enum` declarations — the reference's `simpleTypes` counts an enum as a simple type
///   (`process_component_member.js`), so a member declared with one is not a state object and a
///   read of it is never wrapped in `ObservedObject.GetRawObject`.
///
/// The enum set is the file's own declarations only. The reference resolves the name against the
/// whole program (`validate_ui_syntax.js` `enumCollection`), so an enum imported from another
/// file is not known here and its members are read as objects; that overlap is the one place this
/// rule stays wider than the reference.
pub(crate) struct FileFacts {
    /// The file declares at least one `@LocalBuilder` method.
    pub(crate) present: bool,
    /// The method names declared that way.
    pub(crate) names: std::collections::BTreeSet<String>,
    /// The `enum` names the file declares.
    pub(crate) enums: std::collections::BTreeSet<String>,
    /// The `class` names the file declares.
    ///
    /// `isRegularAttrNode` (`process_component_build.js`) reads an `X.Y` value as regular only
    /// when `X` names an enum; a class's static member, a namespace export and a module variable
    /// are all values it reads as dynamic. The reference settles that with the type checker
    /// (`getTypeAtLocation(...).flags & 1056`), hvolt with this declaration table.
    pub(crate) classes: std::collections::BTreeSet<String>,
    /// The `namespace` names the file declares (same use as `classes`).
    pub(crate) namespaces: std::collections::BTreeSet<String>,
    /// The names the file declares with a module-level `const` / `let` / `var` (same use).
    pub(crate) vars: std::collections::BTreeSet<String>,
    /// The file's `type X = …` aliases, `name → body as written`.
    ///
    /// A field whose declared type is an alias is judged by what the alias resolves to, not by
    /// the alias name: the reference asks the checker, which gives a type alias no type of its
    /// own. Measured in `fixtures/observed-property-kind`: `type L = 'a' | 'b'` → simple,
    /// `type B = boolean` → simple (the checker's `boolean` is the `true | false` union),
    /// `type E = LocalKind` → simple, `type N = number` / `type S = string` → **object**
    /// (a stand-alone primitive reached through an alias carries no union).
    pub(crate) aliases: std::collections::BTreeMap<String, String>,
    /// The file declares at least one `@ReusableV2` struct.
    ///
    /// `processStructComponentV2` sets `hasReusableV2` on the file's stored info when it meets
    /// such a struct, and `process_ui_syntax.js` unshifts the reflect helper once per file from
    /// that one flag — so a file with two of them still gets a single declaration.
    pub(crate) has_reusable_v2: bool,
    /// The object-typed member names in scope at each struct, keyed by the struct's declaration
    /// offset.
    ///
    /// `stateObjectCollection` (`process_component_member.js`) is a Set of member **names** that
    /// is filled as structs are processed and never cleared: `isNeedGetRawObject` asks whether
    /// the name is in it, not whether the reading struct declares the member that way. hvolt
    /// lowers one file at a time, so the table is this file's structs accumulated in declaration
    /// order — the closest deterministic reading of a compilation-wide, order-dependent Set
    /// (measured: clashbox `entry/components/Common/Common.ets`, where `Nothing`'s
    /// `@State name: Resource` makes `skipRowList`'s `@State name: string` read wrap).
    pub(crate) object_members:
        std::collections::BTreeMap<usize, std::collections::BTreeSet<String>>,
}

/// `createReusableV2ReflectFunction` (process_struct_componentV2.js): the helper a `@ReusableV2`
/// class is decorated with.
///
/// `process_ui_syntax.js` unshifts it once per file, after `checkContextStack()` and ahead of
/// every `_Params` interface, and `processStructComponentV2` puts the decorator on the class.
pub(crate) fn reusable_v2_reflect_function() -> &'static str {
    "function __ReusableV2_Inner_Decorator__(BaseClass) { Reflect.defineProperty(BaseClass.prototype, \"isReusable_\", { get: () => true }); }"
}

/// The decorator `createReusableV2ReflectFunction` defines, as it is written above a class.
pub(crate) const REUSABLE_V2_CLASS_DECORATOR: &str = "@__ReusableV2_Inner_Decorator__";

/// `contextStackPushOrPop(PUSH, [this])` (process_component_class.js).
pub(crate) const CONTEXT_STACK_PUSH: &str =
    "PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.push(this);";

/// `contextStackPushOrPop(POP, [])` (process_component_class.js).
pub(crate) const CONTEXT_STACK_POP: &str =
    "PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.pop();";

/// `globalBuilderParamAssignment()` (process_ui_syntax.js): the parent a local builder renders
/// into, `null` when no component is being rendered.
pub(crate) const BUILDER_PARENT: &str = "const parent = PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.length ? PUV2ViewBase.contextStack[PUV2ViewBase.contextStack.length - 1] : null;";

/// `parentConditionalExpression()` (process_component_build.js): the receiver a child
/// construction takes inside a builder body.
pub(crate) const BUILDER_PARENT_EXPR: &str = "parent ? parent : this";

/// Whether a declared type is one the reference's `getDeclarationType`
/// (`process_component_member.js`) calls simple.
///
/// The two shapes are judged by different rules. A single type node is simple when it is the
/// `string`/`number`/`boolean` keyword (`simpleTypes`) or a name `enumCollection` holds; the
/// reference then asks the checker, whose only remaining positive is the `EnumLike` flag, so a
/// bare literal type (`'a'`) is not simple. A union is simple when, after `findNonNullType`
/// drops `undefined`/`null`, one survivor is left (judged as a stand-alone node, so `E |
/// undefined` with `E` an enum is simple and `number | undefined` is not) or every survivor is
/// a primitive (`string | number` is simple). The enum half is the file's own declarations
/// only: the reference resolves the name against the whole program, so an enum imported from
/// another file is still read as an object here.
pub(crate) fn is_simple_type_text(text: Option<&str>, facts: &FileFacts) -> bool {
    let Some(text) = text else {
        return false;
    };
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    let parts: Vec<&str> = text
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        // One type node: `simpleTypes` holds `string`, `number`, `boolean` and an enum
        // declaration. A literal type (`'a'`) is not one of them, and the checker path that
        // follows rejects it as well (its type carries no `EnumLike` flag), so it lowers to the
        // object property. A declared alias is read through its body, see [`alias_is_simple`].
        return matches!(text, "string" | "number" | "boolean") || decl_is_simple(text, facts, 0);
    }
    // A union: `findNonNullType` drops `undefined`/`null` first, and one survivor is then read
    // as a stand-alone type node would be.
    let kept: Vec<&str> = parts
        .iter()
        .copied()
        .filter(|part| !matches!(*part, "undefined" | "null"))
        .collect();
    if kept.len() == 1 {
        // One survivor is judged as a stand-alone node, and the only stand-alone node that
        // resolves to a union is `boolean` (`true | false`): `boolean | undefined` is simple,
        // `string | undefined` and `number | undefined` are objects (measured 2026-11-07,
        // `fixtures/observed-property-kind` and `ProxyArrangement.ts`).
        return kept[0] == "boolean" || decl_is_simple(kept[0], facts, 0);
    }
    kept.iter().all(|part| is_basic_type_text(part, facts, 0))
}

/// The name and body of the `type X = …` whose `type` keyword sits at `index`.
///
/// The body runs to the end of the line or a `;`, whichever comes first: a declaration is never
/// continued on the next line without a `|` continuation, and the aliases in play are one-liners.
fn alias_declaration(
    tokens: &[crate::build::lex::Token],
    index: usize,
    source: &str,
) -> Option<(String, String)> {
    let name = tokens
        .get(index + 1)
        .filter(|next| next.kind == crate::build::lex::Kind::Ident)?;
    let equals = tokens.get(index + 2).filter(|next| next.is_punct('='))?;
    let mut breadth = 0usize;
    let mut end = equals.end;
    for next in tokens.iter().skip(index + 3) {
        if next.is_punct('(') || next.is_punct('[') || next.is_punct('{') || next.is_punct('<') {
            breadth += 1;
        } else if next.is_punct(')')
            || next.is_punct(']')
            || next.is_punct('}')
            || next.is_punct('>')
        {
            breadth = breadth.saturating_sub(1);
        }
        if breadth == 0 && (next.is_punct(';') || source[equals.end..next.start].contains('\n')) {
            break;
        }
        end = next.end;
    }
    let body = source[equals.end..end].trim();
    if body.is_empty() {
        return None;
    }
    Some((name.text.to_string(), body.to_string()))
}

/// What a type name denotes, as far as the backstore choice is concerned.
///
/// The distinction is the same one the checker makes for the reference: an enum's type is a
/// union of enum literals, a class or interface is an object, and an alias resolves to whatever
/// its body is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum TypeDecl {
    /// `enum X` / `const enum X`: the checker's type is the union of its members.
    Enum,
    /// `class X` / `namespace X`: an object type.
    Class,
    /// `interface X`: an object type.
    Interface,
    /// `type X = body`, with the body kept as written.
    Alias(String),
}

thread_local! {
    /// The type names every file of the project can see, gathered before any file is lowered
    /// (`fixtures/observed-property-kind-import`). A file cannot answer this alone: the
    /// declaration sits in another module. Kept apart from the `const enum` table — that one
    /// answers "what value does `A.B` render as", this one answers "what kind of type is `X`",
    /// and the two collections only share the pre-scan pass.
    static PROJECT_TYPES: std::cell::RefCell<Vec<(String, TypeDecl)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Publish the project's exported type declarations for the files lowered after this call.
pub(crate) fn set_project_types(entries: &[(String, TypeDecl)]) {
    PROJECT_TYPES.with(|cell| {
        let mut table = cell.borrow_mut();
        table.clear();
        table.extend_from_slice(entries);
    });
}

fn project_type(name: &str) -> Option<TypeDecl> {
    PROJECT_TYPES.with(|cell| {
        cell.borrow()
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, decl)| decl.clone())
    })
}

/// Every type name the given sources export, with what it denotes.
///
/// Only `export`ed declarations are recorded: a name another module cannot import is not a name
/// a field of that module can be declared with. A name declared twice with different
/// declarations is dropped, the same choice `collect_const_enums` makes — at a use site there is
/// no way to tell which declaration is meant, and a wrong kind is worse than none.
pub(crate) fn collect_type_declarations(sources: &[String]) -> Vec<(String, TypeDecl)> {
    let mut table: Vec<(String, TypeDecl)> = Vec::new();
    let mut ambiguous: Vec<String> = Vec::new();
    for source in sources {
        for (name, decl) in types_in(source, false) {
            if ambiguous.contains(&name) {
                continue;
            }
            match table.iter().position(|(known, _)| *known == name) {
                Some(index) if table[index].1 == decl => {}
                Some(index) => {
                    table.remove(index);
                    ambiguous.push(name);
                }
                None => table.push((name, decl)),
            }
        }
    }
    table
}

/// The enum names the SDK's `*.d.ts` files declare (`declare enum FontWeight`).
///
/// The SDK is not part of the project's sources, but a field may name one of its enums
/// (`@State fontWeight: FontWeight` in `ConfigInfo.ets`), and the reference's checker resolves
/// it. Only enums are kept: every SDK interface and class is an object, which is the answer the
/// table already gives by not finding the name.
pub(crate) fn collect_sdk_enums(sources: &[String]) -> Vec<(String, TypeDecl)> {
    let mut names: Vec<(String, TypeDecl)> = Vec::new();
    for source in sources {
        for (name, decl) in types_in(source, true) {
            if decl == TypeDecl::Enum && !names.iter().any(|(known, _)| *known == name) {
                names.push((name, decl));
            }
        }
    }
    names
}

/// The `export`ed (or, for an ambient `*.d.ts`, `declare`d) type declarations of one source.
fn types_in(source: &str, ambient: bool) -> Vec<(String, TypeDecl)> {
    let tokens = crate::build::lex::tokenize(source);
    let mut out = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Ident {
            continue;
        }
        if !matches!(token.text, "enum" | "class" | "interface" | "type") {
            continue;
        }
        // `export enum` / `export default class`: an `export` sits one or two identifiers
        // back. `export const enum` puts `const` in between, so two is the window.
        let exported = tokens[..index].iter().rev().take(2).any(|previous| {
            previous.kind == crate::build::lex::Kind::Ident
                && (previous.text == "export" || (ambient && previous.text == "declare"))
        });
        if !exported {
            continue;
        }
        let decl = match token.text {
            "enum" => TypeDecl::Enum,
            "class" => TypeDecl::Class,
            "interface" => TypeDecl::Interface,
            _ => {
                if let Some((name, body)) = alias_declaration(&tokens, index, source) {
                    out.push((name, TypeDecl::Alias(body)));
                }
                continue;
            }
        };
        let Some(name) = tokens
            .get(index + 1)
            .filter(|next| next.kind == crate::build::lex::Kind::Ident)
        else {
            continue;
        };
        out.push((name.text.to_string(), decl));
    }
    out
}

/// What a type name denotes, the file's own declarations first.
///
/// A file that declares the name decides: its `enum`, `type` or `class` is the one the field in
/// that file means, and the project table only answers for names the file does not declare.
fn declared_type(name: &str, facts: &FileFacts) -> Option<TypeDecl> {
    if facts.enums.contains(name) {
        return Some(TypeDecl::Enum);
    }
    if let Some(body) = facts.aliases.get(name) {
        return Some(TypeDecl::Alias(body.clone()));
    }
    if facts.classes.contains(name) || facts.namespaces.contains(name) {
        return Some(TypeDecl::Class);
    }
    project_type(name)
}

/// Whether a type name resolves to something the reference calls simple.
fn decl_is_simple(name: &str, facts: &FileFacts, depth: usize) -> bool {
    match declared_type(name, facts) {
        Some(TypeDecl::Enum) => true,
        Some(TypeDecl::Alias(body)) if depth < 2 => alias_body_is_simple(&body, facts, depth + 1),
        _ => false,
    }
}

/// Whether an alias body resolves to a simple type.
///
/// The body must be `boolean` (the checker's `true | false` union), a union, or an enum name.
/// A body that is a stand-alone `number` or `string` is **not** simple: a primitive reached
/// through an alias carries no union, which is the asymmetry `fixtures/observed-property-kind`
/// measures (`numberAlias` object, `boolAlias` simple).
fn alias_body_is_simple(body: &str, facts: &FileFacts, depth: usize) -> bool {
    let body = body.trim();
    if body == "boolean" {
        return true;
    }
    let members: Vec<&str> = body.split('|').map(str::trim).collect();
    if members.len() >= 2 {
        let kept: Vec<&str> = members
            .iter()
            .copied()
            .filter(|part| !matches!(*part, "undefined" | "null"))
            .collect();
        if kept.len() == 1 {
            return kept[0] == "boolean" || decl_is_simple(kept[0], facts, depth);
        }
        return kept
            .iter()
            .all(|part| is_basic_type_text(part, facts, depth));
    }
    matches!(declared_type(body, facts), Some(TypeDecl::Enum))
}

/// One union member the reference's `isBasicType` accepts: a primitive, an enum, or a literal
/// (`TypeFlags` 4092 — the string/number/boolean/bigint/symbol/enum families).
///
/// The reference reaches this predicate only from the union path of `getDeclarationType`; a
/// stand-alone literal type is judged by [`is_simple_type_text`]'s single-part branch instead.
fn is_basic_type_text(part: &str, facts: &FileFacts, depth: usize) -> bool {
    crate::build::ets_compile::is_simple_type(Some(part))
        || matches!(part, "bigint" | "symbol")
        || decl_is_simple(part, facts, depth)
}

/// The header `checkContextStack()` writes (process_component_class.js).
pub(crate) fn context_stack_header() -> &'static str {
    "if (PUV2ViewBase.contextStack === undefined) {\n    Reflect.set(PUV2ViewBase, \"contextStack\", []);\n}"
}

/// Read the file's `@LocalBuilder` declarations from its tokens.
///
/// The scan is token-level because the same characters inside a comment or a string are not a
/// declaration, and the lexer already separates those (the reference reads the decorator node,
/// `process_component_class.js` `isBuilderOrLocalBuilder`, never the text).
pub(crate) fn file_facts(source: &str) -> FileFacts {
    let mut present = false;
    let mut names = std::collections::BTreeSet::new();
    let mut enums = std::collections::BTreeSet::new();
    let mut classes = std::collections::BTreeSet::new();
    let mut namespaces = std::collections::BTreeSet::new();
    let mut vars = std::collections::BTreeSet::new();
    let mut aliases = std::collections::BTreeMap::new();
    let tokens = crate::build::lex::tokenize(source);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Ident || token.text != "type" {
            continue;
        }
        if let Some((name, body)) = alias_declaration(&tokens, index, source) {
            aliases.insert(name, body);
        }
    }
    // `class` / `namespace` / `const|let|var` declarations: the name is the next identifier.
    // `const enum X` names an enum, which the loop below already records, so the keyword is
    // skipped rather than taken for a declarator.
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Ident {
            continue;
        }
        let set = match token.text {
            "class" => &mut classes,
            "namespace" => &mut namespaces,
            "const" | "let" | "var" => &mut vars,
            _ => continue,
        };
        let Some(name) = tokens.get(index + 1) else {
            continue;
        };
        if name.kind == crate::build::lex::Kind::Ident
            && !matches!(name.text, "enum" | "namespace" | "class" | "function")
        {
            set.insert(name.text.to_string());
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Ident || token.text != "enum" {
            continue;
        }
        if let Some(name) = tokens.get(index + 1) {
            if name.kind == crate::build::lex::Kind::Ident {
                enums.insert(name.text.to_string());
            }
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_punct('@') {
            continue;
        }
        let Some(decorator) = tokens.get(index + 1) else {
            continue;
        };
        if decorator.kind != crate::build::lex::Kind::Ident || decorator.text != "LocalBuilder" {
            continue;
        }
        present = true;
        // The declaration's name follows the decorator, past any modifier.
        for next in tokens.iter().skip(index + 2) {
            if next.kind != crate::build::lex::Kind::Ident {
                break;
            }
            if matches!(
                next.text,
                "private" | "public" | "protected" | "static" | "async"
            ) {
                continue;
            }
            names.insert(next.text.to_string());
            break;
        }
    }
    // `@ReusableV2` names a struct, so the decorator run that follows it ends at `struct`.
    let mut has_reusable_v2 = false;
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_punct('@') {
            continue;
        }
        let Some(decorator) = tokens.get(index + 1) else {
            continue;
        };
        if decorator.kind != crate::build::lex::Kind::Ident || decorator.text != "ReusableV2" {
            continue;
        }
        // Walk the rest of the decorator run: another decorator (`@ComponentV2`) is skipped, a
        // `struct` settles it, and anything else (a `class`, a `{`, a `;`) means the decorator
        // is not on a struct at all.
        let mut next = index + 2;
        while let Some(token) = tokens.get(next) {
            if token.is_punct('@') {
                next += 2;
                continue;
            }
            if token.kind == crate::build::lex::Kind::Ident && token.text == "struct" {
                has_reusable_v2 = true;
                break;
            }
            if token.is_punct('(') || token.is_punct('{') || token.is_punct(';') {
                break;
            }
            next += 1;
        }
        if has_reusable_v2 {
            break;
        }
    }
    FileFacts {
        present,
        names,
        enums,
        classes,
        namespaces,
        vars,
        aliases,
        has_reusable_v2,
        // Filled by the caller that owns the parsed structs (`expand_component_file_with`),
        // the only place that sees every struct of the file at once.
        object_members: std::collections::BTreeMap::new(),
    }
}

/// The `<Component>_Params` interface a component's constructor takes.
///
/// Only V1 structs get one: `ViewV2` construction reads the raw `params` object itself
/// (`initParam` / `"x" in params`), so the reference emits no interface for a V2 struct
/// (measured: component-v2 golden, 2026-11-05).
/// The comment block the member at `index` carries from the member after it.
///
/// The reference prints the next member's leading trivia inside the statement or declaration it
/// synthesizes for this member — but only when the two declarations are adjacent in the source.
/// A method in between stops it (measured: `fixtures/comment-leading`, marker
/// `GAP_AFTER_METHODS`, which the golden keeps out of the constructor when a method separates
/// the two fields; the same shape is why `fixtures/configuration-page` does not carry
/// `// ContextMenu 列表项 动态替换用` into `currentRadioConfigData`).
fn carried_comments(comp: &ComponentInfo, index: usize) -> Vec<String> {
    let Some(current) = comp.fields.get(index) else {
        return Vec::new();
    };
    // The next member decides what is carried: a plain field hands over its own block, and a
    // method hands over the block written above it — the reference moves that block into the
    // declaring field's type argument as well (measured: `fixtures/carry-gate`, whose fourth
    // group carries a method's doc block). `build` is not in `method_offsets` (it is emitted
    // as `initialRender`), so its own offset is checked too and stops the carry.
    let next_field = comp.fields.get(index + 1).map(|field| field.offset);
    let next_method = comp
        .method_offsets
        .iter()
        .copied()
        .chain(
            comp.struct_builder_methods
                .iter()
                .map(|method| method.offset),
        )
        .filter(|offset| *offset > current.offset)
        .min();
    let next_build = (comp.build_offset > current.offset).then_some(comp.build_offset);
    match (next_field, next_method, next_build) {
        (Some(field), method, build)
            if method.is_none_or(|m| field <= m) && build.is_none_or(|b| field <= b) =>
        {
            comp.fields[index + 1].comment.clone()
        }
        // `build()` is the nearest following member: the field carries the block written above
        // it. Without this arm the block fell through to `Vec::new()` and the constructor lost
        // the copy the reference prints there (`Index.ts:523`, `this.addProvidedVar(…)` then
        // `// TODO 6.0(20)光效` and `// @Provide lightSourceType …`). A method written below
        // `build()` must not take it: `build` comes first in that file, and the old guard let
        // the method arm claim the block or drop it entirely.
        (_, method, Some(build)) if method.is_none_or(|m| build < m) => comp.build_comment.clone(),
        (_, Some(method), build) if build.is_none_or(|b| method <= b) => {
            let which = comp
                .method_offsets
                .iter()
                .position(|offset| *offset == method);
            let text = which
                .map(|position| comp.methods[position].as_str())
                .or_else(|| {
                    comp.struct_builder_methods
                        .iter()
                        .find(|builder| builder.offset == method)
                        .map(|_| "")
                });
            text.map(leading_comment_block).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// The comment block a method's text starts with, as lines ready for the type argument.
///
/// A block comment spans lines and ends at `*/`; a line comment ends at its own newline. The
/// cut stops at the first line that is neither, so the method's signature and body never
/// enter the type argument.
fn leading_comment_block(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if in_block {
            lines.push(trimmed.to_string());
            if trimmed.contains("*/") {
                in_block = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            lines.push(trimmed.to_string());
            in_block = !trimmed.contains("*/");
            continue;
        }
        if trimmed.starts_with("//") {
            lines.push(trimmed.to_string());
            continue;
        }
        break;
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

/// Whether the member is written without a decorator, without an accessibility keyword and
/// without `readonly` — the shape whose source node the reference keeps reusing in the
/// statements it synthesizes.
pub(crate) fn is_plain_member(f: &crate::build::stage::utils::FieldInfo) -> bool {
    f.decorator().is_none() && f.accessibility.is_none() && !f.is_readonly
}

pub(crate) fn emit_params(out: &mut String, comp: &ComponentInfo) {
    // Every non-static field, in source order, optional (process_component_class.js
    // `processMembers`). A static member is not part of the constructor contract, so the
    // reference leaves it out of the interface (measured: fixtures/_static-member golden).
    out.push_str(&format!("interface {}_Params {{\n", comp.name));
    for f in comp.fields.iter().filter(|f| !f.is_static) {
        // A plain member — no decorator, no accessibility keyword, not `readonly` — keeps its
        // comment block here as well, because the reference's synthesized interface entry
        // reuses the source node (`fixtures/comment-leading`: `/* MEMBER_PLAIN */` appears
        // three times, once of them above `dlg2?:`). A decorated, `public` or `readonly`
        // member gets no copy here (markers `MEMBER_PUBLIC`, `MEMBER_STATE`, measured
        // 2026-11-07).
        if is_plain_member(f) {
            for comment in &f.comment {
                out.push_str(&format!(
                    "    {}\n",
                    crate::build::ets::comments::reindent_block(comment, "    ")
                ));
            }
        }
        out.push_str(&format!("    {}?{};\n", f.name, f.type_annotation()));
    }
    out.push_str("}\n");
}

/// The generated component class for one `struct`.
pub(crate) fn emit_class(
    out: &mut String,
    comp: &ComponentInfo,
    protocol: Protocol,
    local: &FileFacts,
) -> Result<(), String> {
    if protocol.v2 {
        emit_v2_class(out, comp, local)?;
    } else {
        emit_v1_class(out, comp, local)?;
    }
    Ok(())
}

/// `addRerenderFunc` (process_component_class.js): the dirty-element pass, bracketed by the
/// context stack when the file declares a `@LocalBuilder`.
pub(crate) fn emit_rerender(out: &mut String, local: &FileFacts) {
    out.push_str("    rerender() {\n");
    if local.present {
        out.push_str(&format!("        {CONTEXT_STACK_PUSH}\n"));
    }
    out.push_str("        this.updateDirtyElements();\n");
    if local.present {
        out.push_str(&format!("        {CONTEXT_STACK_POP}\n"));
    }
    out.push_str("    }\n");
}

/// V1 lowering: `@Component` struct → `ViewPU` class.
///
/// Member order follows process_component_class.js `processMembers`: constructor, then
/// `setInitiallyProvidedValue` / `updateStateVars` / `purgeVariableDependenciesOnElmtId` /
/// `aboutToBeDeleted`, then the backstore declarations with their accessors, then methods.
pub(crate) fn emit_v1_class(
    out: &mut String,
    comp: &ComponentInfo,
    local: &FileFacts,
) -> Result<(), String> {
    // A child's constructor arguments read this struct's backstores, so the emitter needs the
    // names before any call site inside `build()` is rendered.
    let backstores: Vec<String> = comp
        .fields
        .iter()
        .filter(|field| field.decorator().is_some())
        .map(|field| field.name.clone())
        .collect();
    crate::build::ets_compile::set_v1_fields(&backstores);
    out.push_str(&format!(
        "{}class {} extends ViewPU {{\n",
        if comp.is_exported { "export " } else { "" },
        comp.name
    ));
    // `initConstructorParams` (partial-update mode) + `addConstructor`.
    out.push_str("    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {\n");
    out.push_str("        super(parent, __localStorage, elmtId, extraInfo);\n");
    out.push_str("        if (typeof paramsLambda === \"function\") {\n            this.paramsGenerator_ = paramsLambda;\n        }\n");
    for (index, f) in comp.fields.iter().enumerate() {
        // The constructor statement of one member carries the comment block of the member
        // after it, right before the arguments the reference appends (measured:
        // `fixtures/comment-leading`, `new ObservedPropertySimplePU(1` + the next member's
        // `/* MEMBER_READONLY */` + `, this, "tagged");`).
        let carried = carried_comments(comp, index);
        emit_v1_field_init(out, f, local, &carried);
    }
    out.push_str("        this.setInitiallyProvidedValue(params);\n");
    for f in &comp.fields {
        if let Some(watcher) = &f.watch_method {
            out.push_str(&format!(
                "        this.declareWatch(\"{}\", this.{});\n",
                f.name, watcher
            ));
        }
    }
    out.push_str("        this.finalizeConstruction();\n    }\n");
    emit_v1_initial_params(out, comp);
    emit_v1_update_state_vars(out, comp);
    // `addDeleteParamsFunc`: every field with a backstore is purged and deleted.
    out.push_str("    purgeVariableDependenciesOnElmtId(rmElmtId) {\n");
    for f in comp.fields.iter().filter(|f| f.decorator().is_some()) {
        out.push_str(&format!(
            "        this.__{}.purgeDependencyOnElmtId(rmElmtId);\n",
            f.name
        ));
    }
    out.push_str("    }\n");
    out.push_str("    aboutToBeDeleted() {\n");
    for f in comp.fields.iter().filter(|f| f.decorator().is_some()) {
        out.push_str(&format!("        this.__{}.aboutToBeDeleted();\n", f.name));
    }
    out.push_str("        SubscriberManager.Get().delete(this.id__());\n        this.aboutToBeDeletedInternal();\n    }\n");
    // A `@Reusable` component is bound to a new element id when it leaves the reuse pool:
    // `addDeleteParamsFunc(…, true)` → `createRecycleElmt` (process_component_class.js).
    if comp.is_reusable {
        out.push_str("    updateRecycleElmtId(oldElmtId, newElmtId) {\n");
        for f in comp.fields.iter().filter(|f| f.decorator().is_some()) {
            out.push_str(&format!(
                "        this.__{}.updateElmtId(oldElmtId, newElmtId);\n",
                f.name
            ));
        }
        out.push_str("    }\n");
    }
    emit_members(out, comp, local, FieldDecl::V1)?;
    emit_rerender(out, local);
    // `createResetStateVarsOnReuseForV1` only runs when the reference sees
    // `compatibleSdkVersion >= 26`; the default project targets API 17, where
    // `updateRecycleElmtId` above is the whole reuse hook (measured: reusable golden).
    emit_entry_name(out, comp);
    out.push_str("}\n");
    Ok(())
}

/// One slot of the generated class body, in the order the struct wrote it.
pub(crate) enum Member {
    /// An index into `ComponentInfo::methods`.
    Method(usize),
    /// An index into `ComponentInfo::struct_builder_methods`.
    Builder(usize),
    /// `build()`: emitted as `initialRender()` where the struct declares it.
    Render,
    /// An index into `ComponentInfo::fields`: the declaration takes the field's own slot, because
    /// both `processMembers` and `processStructMembersV2` push it where the struct wrote it,
    /// inside the same loop that pushes the methods.
    Field(usize),
}

/// Which declaration a field slot takes.
///
/// The two protocols put a member in different places. A V1 member becomes a `__name` backstore
/// with accessors (or a plain declaration plus the `@CustomDialog` controller setter); a V2
/// member keeps its decorators and has no backstore at all.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum FieldDecl {
    V1,
    V2,
}

/// Emit the struct's members in declaration order, with `build()` become `initialRender()`.
///
/// The reference lays the class out the way the struct was written: a method declared after
/// `build()` lands after `initialRender()`, and a struct-level `@Builder` keeps its place among
/// the ordinary methods (measured: `fixtures/member-shapes`, `Xb_GeneralListItem` in clashbox,
/// 2026-11-06). Emitting every method first and the render last moved both. The same holds for
/// V2, where a field declared after a method is emitted after it (measured:
/// `fixtures/v2-member-order`).
///
/// The struct's `@Reusable` fact is established here, not in `initialRender`: the members keep
/// declaration order, so a `@Builder` declared above `build()` is rendered before the render
/// body, and a flag written only there would leave that builder reading the previous struct's
/// value (measured: clashbox `StylesEdit.ets`, whose `EditTitleBuilder` inherited `ProxyNode`'s
/// `@Reusable` — five writes wrapped in `if (isInitialRender)` where the reference writes them
/// flat).
pub(crate) fn emit_members(
    out: &mut String,
    comp: &ComponentInfo,
    local: &FileFacts,
    fields: FieldDecl,
) -> Result<(), String> {
    // Each struct establishes its own flag; without this write a `@Builder` body inherits the
    // residue of the struct emitted before it.
    crate::build::ets_compile::set_reusable_component(comp.is_reusable);
    let mut slots: Vec<(usize, Member)> = Vec::new();
    for (index, offset) in comp.method_offsets.iter().enumerate() {
        slots.push((*offset, Member::Method(index)));
    }
    for (index, method) in comp.struct_builder_methods.iter().enumerate() {
        slots.push((method.offset, Member::Builder(index)));
    }
    // `processMembers` and `processStructMembersV2` both walk the struct body once and push each
    // member as they meet it, so a field lands where it was written — after the methods declared
    // above it (measured: `Shortcuts.ts`, whose `private ShortcutsPromptAction` follows the
    // methods). Emitting the fields as a block ahead of the methods moves every member after it.
    for (index, field) in comp.fields.iter().enumerate() {
        slots.push((field.offset, Member::Field(index)));
    }
    slots.push((comp.build_offset, Member::Render));
    slots.sort_by_key(|(offset, _)| *offset);
    for (_, member) in slots {
        match member {
            Member::Method(index) => {
                // The method text is handed over as written: the printer lays it out, so no
                // re-indentation happens here.
                out.push_str(&comp.methods[index]);
                out.push('\n');
            }
            Member::Builder(index) => {
                emit_struct_builder_method(out, comp, &comp.struct_builder_methods[index], local)
            }
            Member::Render => emit_initial_render(out, comp, local)?,
            Member::Field(index) => match fields {
                FieldDecl::V1 => {
                    crate::build::stage::process_component_member::emit_v1_field_declaration(
                        out,
                        &comp.fields[index],
                        local,
                        // The reference carries whatever comment block the next member
                        // declares, decoration or not: a `@Consume` before a plain field, a
                        // `static` field or a method prints that member's block inside the
                        // type argument (measured: `fixtures/carry-gate`, where all four
                        // successors carry).
                        &carried_comments(comp, index),
                    );
                    crate::build::stage::process_component_member::emit_controller_set(
                        out,
                        comp,
                        &comp.fields[index],
                    );
                }
                FieldDecl::V2 => {
                    crate::build::stage::process_struct_componentV2::emit_v2_field_declaration(
                        out,
                        &comp.fields[index],
                    );
                }
            },
        }
    }
    Ok(())
}

/// Only an `@Entry` component carries the entry name.
pub(crate) fn emit_entry_name(out: &mut String, comp: &ComponentInfo) {
    if comp.is_entry {
        out.push_str(&format!(
            "    static getEntryName(): string {{\n        return \"{}\";\n    }}\n",
            comp.name
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{context_stack_header, file_facts, is_simple_type_text};

    /// The file-level flag and the declared names come from the decorator, whatever modifier
    /// the declaration carries (`process_component_class.js` `isBuilderOrLocalBuilder`).
    #[test]
    fn a_local_builder_declaration_names_its_method() {
        let source = "@ComponentV2\nstruct X {\n  @LocalBuilder\n  private title(g: string) {\n    Text(g)\n  }\n}\n";
        let local = file_facts(source);
        assert!(local.present);
        assert!(local.names.contains("title"), "{:?}", local.names);
    }

    /// `@Builder` is the other decorator: it keeps the `parent = null` method form, so it must
    /// not turn the file flag on (`validate_ui_syntax.js` `setLocalBuilderInFile` compares the
    /// decorator name, not a substring).
    #[test]
    fn a_plain_builder_is_not_a_local_builder() {
        let source =
            "@Component\nstruct X {\n  @Builder\n  badge(t: string) {\n    Text(t)\n  }\n}\n";
        let local = file_facts(source);
        assert!(!local.present);
        assert!(local.names.is_empty());
    }

    /// The same characters in a comment or a string are not a declaration.
    #[test]
    fn a_comment_or_string_is_not_a_declaration() {
        let source = "// @LocalBuilder\nconst s = \"@LocalBuilder\\n  ghost()\";\n";
        let local = file_facts(source);
        assert!(!local.present);
        assert!(local.names.is_empty());
    }

    /// `@ReusableV2` names a struct, in either decorator order; the flag drives the one
    /// per-file reflect helper (`process_ui_syntax.js` `hasReusableV2`).
    #[test]
    fn a_reusable_v2_struct_sets_the_file_flag() {
        assert!(file_facts("@ComponentV2\n@ReusableV2\nstruct X {}\n").has_reusable_v2);
        assert!(file_facts("@ReusableV2\n@ComponentV2\nstruct X {}\n").has_reusable_v2);
        assert!(!file_facts("@ComponentV2\nstruct X {}\n").has_reusable_v2);
    }

    /// The decorator only counts on a struct: the same characters in a comment, a string or on
    /// a class are not a declaration.
    #[test]
    fn a_reusable_v2_decorator_elsewhere_is_not_a_struct() {
        assert!(!file_facts("// @ReusableV2\nstruct X {}\n").has_reusable_v2);
        assert!(!file_facts("const s = \"@ReusableV2 struct X\";\n").has_reusable_v2);
        assert!(!file_facts("@ReusableV2\nclass X {}\n").has_reusable_v2);
    }

    /// A field whose declared type is a local alias is judged by the alias body, not by the
    /// name (`fixtures/observed-property-kind`, measured 2026-11-07): a string-literal union, a
    /// `boolean` (the checker's `true | false` union) and an enum are simple; a stand-alone
    /// `number`/`string` reached through the alias is not.
    #[test]
    fn an_alias_is_judged_by_its_body() {
        let source = "type L = 'a' | 'b'\ntype B = boolean\ntype E = LocalKind\ntype N = number\ntype S = string\ntype T = string | number\nenum LocalKind {\n  A = 1\n}\n";
        let facts = file_facts(source);
        assert!(
            is_simple_type_text(Some("L"), &facts),
            "string-literal union alias"
        );
        assert!(is_simple_type_text(Some("B"), &facts), "boolean alias");
        assert!(is_simple_type_text(Some("E"), &facts), "enum alias");
        assert!(
            is_simple_type_text(Some("T"), &facts),
            "primitive union alias"
        );
        assert!(!is_simple_type_text(Some("N"), &facts), "number alias");
        assert!(!is_simple_type_text(Some("S"), &facts), "string alias");
    }

    /// An alias body that is not a union, a `boolean` or an enum names an object.
    #[test]
    fn an_object_alias_is_not_simple() {
        let source = "class C {}\ntype O = C\ntype A = C[]\n";
        let facts = file_facts(source);
        assert!(!is_simple_type_text(Some("O"), &facts));
        assert!(!is_simple_type_text(Some("A"), &facts));
    }

    /// A type name the file does not declare is read from the project table, and a name the
    /// file does declare wins over it (`fixtures/observed-property-kind-import`).
    #[test]
    fn the_project_table_answers_for_imported_names() {
        let table = super::collect_type_declarations(&[
            "export enum Kind {\n  A = 1\n}\nexport type Alias = 'x' | 'y';\nexport type Num = number;\nexport class Obj {}\nexport interface Shape {\n  a: string\n}\n".to_string(),
        ]);
        super::set_project_types(&table);
        let facts = file_facts("struct X {}\n");
        assert!(
            super::is_simple_type_text(Some("Kind"), &facts),
            "imported enum"
        );
        assert!(
            super::is_simple_type_text(Some("Alias"), &facts),
            "imported alias union"
        );
        assert!(
            super::is_simple_type_text(Some("Alias | undefined"), &facts),
            "nullable imported alias"
        );
        assert!(
            !super::is_simple_type_text(Some("Num"), &facts),
            "imported number alias"
        );
        assert!(
            !super::is_simple_type_text(Some("Obj"), &facts),
            "imported class"
        );
        assert!(
            !super::is_simple_type_text(Some("Shape"), &facts),
            "imported interface"
        );
        // 文件自己声明的名字优先：同名类是对象，即使项目表里是 enum。
        let local = file_facts("class Kind {}\nstruct X {}\n");
        assert!(
            !super::is_simple_type_text(Some("Kind"), &local),
            "local class wins"
        );
        super::set_project_types(&[]);
    }

    /// 同名但声明不同时整条丢：用点处无法判断指哪一个。
    #[test]
    fn a_name_declared_twice_is_dropped() {
        let table = super::collect_type_declarations(&[
            "export enum Dup {\n  A = 1\n}\n".to_string(),
            "export type Dup = 'x' | 'y';\n".to_string(),
        ]);
        assert!(table.is_empty(), "{table:?}");
        let same = super::collect_type_declarations(&[
            "export type Same = 'x' | 'y';\n".to_string(),
            "export type Same = 'x' | 'y';\n".to_string(),
        ]);
        assert_eq!(same.len(), 1, "同一个声明出现两次不算冲突");
    }

    /// The header is the reference's `checkContextStack()` text.
    #[test]
    fn the_header_is_the_reference_statement() {
        assert_eq!(
            context_stack_header(),
            "if (PUV2ViewBase.contextStack === undefined) {\n    Reflect.set(PUV2ViewBase, \"contextStack\", []);\n}"
        );
    }

    /// A union of primitives is simple while a union with `undefined` is not: `getDeclarationType`
    /// drops the nullable members first and then judges the survivors, so `string | number` keeps
    /// two primitive members (`isBasicType`) and `number | undefined` is left with one type node
    /// that carries no `EnumLike` flag (measured 2026-11-07, `fixtures/state-type-shapes`).
    #[test]
    fn a_union_of_primitives_is_simple_and_a_nullable_one_is_not() {
        let facts = file_facts("enum Kind { A }\n");
        assert!(is_simple_type_text(Some("string | number"), &facts));
        assert!(!is_simple_type_text(Some("number | undefined"), &facts));
    }

    /// A stand-alone literal type is not one of `simpleTypes` and the checker path rejects it;
    /// the same literals in a union are simple because each member is a basic type.
    #[test]
    fn a_bare_literal_type_is_not_simple_but_a_literal_union_is() {
        let facts = file_facts("");
        assert!(!is_simple_type_text(Some("'a'"), &facts));
        assert!(is_simple_type_text(Some("'a' | 'b'"), &facts));
    }

    /// One survivor after dropping `undefined` is read as a stand-alone type node: an enum is
    /// simple, a class is not.
    #[test]
    fn a_nullable_enum_is_simple_and_a_nullable_class_is_not() {
        let facts = file_facts("enum Kind { A }\n");
        assert!(is_simple_type_text(Some("Kind | undefined"), &facts));
        assert!(is_simple_type_text(Some("Kind"), &facts));
        assert!(!is_simple_type_text(Some("Model | undefined"), &facts));
    }
}
