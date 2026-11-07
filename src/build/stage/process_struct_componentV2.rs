// process stage: `process_struct_componentV2.js` — the `@ComponentV2` member shape.
//
// `StructInfo` (`process_struct_componentV2.js`, the module's export) classifies one struct's
// decorators: `isComponentV2`, `paramDecoratorMap` (`@Param`), `eventDecoratorMap` (`@Event`),
// `localDecoratorSet` (`@Local`), `providerDecoratorSet` (`@Provider`),
// `consumerDecoratorSet` (`@Consumer`), `builderParamDecoratorSet` (`@BuilderParam`). Every
// later stage reads that classification instead of re-reading the source, and
// `process_custom_component.js:546` picks the base class from it
// (`isComponentV2 ? STRUCT_PARENT : BASE_COMPONENT_NAME_PU`).
//
// The runtime method names a V2 class calls are in `constant_define.js`: `STRUCT_PARENT`
// (`ViewV2`), `INIT_PARAM` (`initParam`), `RESET_PARAM` (`resetParam`), `RESET_CONSUMER`
// (`resetConsumer`), `RESET_MONITORS_ON_REUSE` (`resetMonitorsOnReuse`). Because the base class
// does the observation, a V2 member keeps its decorators and gets no `__` backstore and no
// accessor: `resetStateVarsOnReuse` is the whole reuse hook, and `updateStateVars` exists only
// when the struct declares `@Param`.

use crate::build::stage::pre_define;
use crate::build::stage::process_component_class::{
    emit_entry_name, emit_members, emit_rerender, FieldDecl, FileFacts,
};
use crate::build::stage::utils::{ComponentInfo, FieldInfo};

pub(crate) const STRUCT_PARENT: &str = "ViewV2";
pub(crate) const INIT_PARAM: &str = "initParam";
pub(crate) const RESET_PARAM: &str = "resetParam";
pub(crate) const RESET_CONSUMER: &str = "resetConsumer";
pub(crate) const RESET_MONITORS_ON_REUSE: &str = "resetMonitorsOnReuse";
pub(crate) const RESET_COMPUTED: &str = "resetComputed";

/// V2 lowering: `@ComponentV2` struct → `ViewV2` class.
///
/// The base class does the observation, so the members keep their decorators and there is
/// no `__` backstore, no get/set accessor, no `setInitiallyProvidedValue`, no
/// `purgeVariableDependenciesOnElmtId` and no `aboutToBeDeleted` — only the constructor, the
/// reuse reset, the members themselves, `initialRender`, `updateStateVars` and `rerender`
/// (process_struct_componentV2.js `processStructMembersV2`; measured: component-v2 golden).
pub(crate) fn emit_v2_class(
    out: &mut String,
    comp: &ComponentInfo,
    local: &FileFacts,
) -> Result<(), String> {
    // `processStructComponentV2` prefixes the decorator `createReusableV2ReflectFunction`
    // defines when the struct is in `reuseComponentsV2`, i.e. when it carries `@ReusableV2`.
    // TypeScript prints a decorator ahead of the modifiers, so it stands above `export`
    // (measured: `fixtures/v2-reusable-class`).
    if crate::build::compile::component_index::meta_of(&comp.name)
        .is_some_and(|meta| meta.is_reusable_v2)
    {
        out.push_str(crate::build::stage::process_component_class::REUSABLE_V2_CLASS_DECORATOR);
        out.push('\n');
    }
    out.push_str(&format!(
        "{}class {} extends {} {{\n",
        if comp.is_exported { "export " } else { "" },
        comp.name,
        STRUCT_PARENT
    ));
    // `createConstructorParams` + `createSuperV2`: only `elmtId` keeps a default and the
    // base class is called without the LocalStorage argument.
    out.push_str(
        "    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {\n",
    );
    out.push_str("        super(parent, elmtId, extraInfo);\n");
    for f in &comp.fields {
        emit_v2_field_init(out, f);
    }
    out.push_str(&format!(
        "        this.{}();\n    }}\n",
        pre_define::FINALIZE_CONSTRUCTION
    ));
    emit_v2_reset_state_vars(out, comp);
    // The declarations are laid out by `emit_members`, among the methods: `processStructMembersV2`
    // pushes each member as it walks the struct body, so a field declared after a method is
    // emitted after it (measured: `fixtures/v2-member-order`).
    emit_members(out, comp, local, FieldDecl::V2)?;
    emit_v2_update_state_vars(out, comp);
    emit_rerender(out, local);
    emit_entry_name(out, comp);
    out.push_str("}\n");
    Ok(())
}

/// `setPropertyStatement` for one V2 field: constructor init statements.
pub(crate) fn emit_v2_field_init(out: &mut String, f: &FieldInfo) {
    if f.is_static || f.has("Env") {
        // `traverseStructInfo` walks `propertiesMap` but skips `staticPropertySet` and
        // `envDecoratorSet`, so neither reaches the constructor (measured for `static`:
        // `fixtures/v2-static-member`; the `@Env` half is source-only — a `@Env` field needs a
        // `SystemProperties` key and a `WindowSizeLayoutBreakpointInfo`-family type, and the
        // compiler rejects the fixtures this repository can express).
        return;
    }
    let name = &f.name;
    let init = f.initial.as_deref().unwrap_or("undefined");
    if f.has("BuilderParam") {
        // `setPropertyStatement` puts a `@BuilderParam` in the `builderParamDecoratorSet`
        // branch: the parent's builder wins, the declared initializer is the fallback
        // (measured: `Xb_SideBar.ts`, `this.customList = "customList" in params ? …`).
        out.push_str(&format!(
            "        this.{name} = \"{name}\" in params ? params.{name} : {init};\n"
        ));
    } else if f.has("Param") {
        // `createInitOrUpdateParam(…, initParam)`: the parent's value wins, the declared
        // default is the fallback.
        out.push_str(&format!(
            "        this.{}(\"{name}\", (params && \"{name}\" in params) ? params.{name} : {init});\n",
            INIT_PARAM
        ));
    } else if f.has("Event") {
        // `createPropertyAssignNode(…, true)` over `getDefaultValueForEvent(info, false)`: the
        // constructor's arrow takes no parameters.
        let default = f.initial.as_deref().unwrap_or("() => {}");
        out.push_str(&format!(
            "        this.{name} = \"{name}\" in params ? params.{name} : {default};\n"
        ));
    } else {
        // `@Local`, `@Provider`, `@Consumer` and regular fields read the initializer directly.
        out.push_str(&format!("        this.{name} = {init};\n"));
    }
}

/// `resetStateVarsOnReuse`: every V2 struct gets one; the statements mirror the constructor
/// per decorator, and `@Computed` / `@Monitor` members reset by name
/// (process_struct_componentV2.js `createResetStateVarsOnReuse`).
///
/// `traverseStructInfo` builds the body by walking `propertiesMap`, the map `parsePropertyDecorator`
/// and `parseGetAccessor` fill in source order. A `@Computed` getter therefore contributes its
/// `resetComputed` call where the getter stands, not at the end (measured: `fixtures/v2-reset-order`,
/// `this.resetComputed("doubled")` between `this.before` and `this.after`). `resetMonitorsOnReuse`
/// is appended afterwards by `createResetStateVarsOnReuse` itself, so it stays last.
pub(crate) fn emit_v2_reset_state_vars(out: &mut String, comp: &ComponentInfo) {
    out.push_str("    public resetStateVarsOnReuse(params: Object): void {\n");
    let mut statements: Vec<(usize, String)> = Vec::new();
    for f in &comp.fields {
        if let Some((offset, statement)) = v2_reset_statement(f) {
            statements.push((offset, statement));
        }
    }
    for name in &comp.computed {
        // A getter the method list already knows keeps its place; one that cannot be located
        // falls back to the end of the body, ahead of the monitor reset.
        let offset = computed_getter_offset(comp, name).unwrap_or(usize::MAX);
        statements.push((
            offset,
            format!("        this.{}(\"{name}\");\n", RESET_COMPUTED),
        ));
    }
    statements.sort_by_key(|(offset, _)| *offset);
    for (_, statement) in statements {
        out.push_str(&statement);
    }
    if comp.has_monitor {
        out.push_str(&format!("        this.{}();\n", RESET_MONITORS_ON_REUSE));
    }
    out.push_str("    }\n");
}

/// One field's line in `resetStateVarsOnReuse`, with the offset it sorts at.
///
/// `setPropertyStatement` decides the shape per decorator. A member with no decorator is not
/// state: `parsePropertyDecorator` puts it in `regularSet`, and that branch writes the
/// constructor only (measured: `Xb_Studio.ts`, whose `private thePageInfo = new NavPathStack()`
/// is initialized once and never reset).
fn v2_reset_statement(f: &FieldInfo) -> Option<(usize, String)> {
    if f.is_static || f.has("Env") {
        // Same two exclusions as the constructor (`traverseStructInfo`).
        return None;
    }
    let name = &f.name;
    let init = f.initial.as_deref().unwrap_or("undefined");
    let statement = if f.has("BuilderParam") {
        // The same branch feeds the reuse hook: a builder parameter is re-read from the new
        // parent rather than reset to its default.
        format!("        this.{name} = \"{name}\" in params ? params.{name} : {init};\n")
    } else if f.has("Param") {
        format!(
            "        this.{}(\"{name}\", (params && \"{name}\" in params) ? params.{name} : {init});\n",
            RESET_PARAM
        )
    } else if f.has("Event") {
        // `getDefaultValueForEvent(info, true)`: the reuse hook's arrow carries the declared
        // function type's parameter list (measured: `fixtures/v2-event-default` resets
        // `onChange` to `(value: string) => { }` where the constructor writes `() => { }`).
        let default = f
            .initial
            .clone()
            .unwrap_or_else(|| event_reset_default(f.type_text.as_deref()));
        format!("        this.{name} = \"{name}\" in params ? params.{name} : {default};\n")
    } else if f.has("Consumer") {
        format!("        this.{}(\"{name}\", {init});\n", RESET_CONSUMER)
    } else if !f.decorators.is_empty() {
        format!("        this.{name} = {init};\n")
    } else {
        return None;
    };
    Some((f.offset, statement))
}

/// `getDefaultValueForEvent(info, true)`: an empty arrow whose parameter list is the declared
/// function type's own (`(value: string) => void` becomes `(value: string) => { }`).
fn event_reset_default(type_text: Option<&str>) -> String {
    match function_type_parameters(type_text) {
        Some(parameters) if !parameters.is_empty() => format!("({parameters}) => {{}}"),
        _ => "() => {}".to_string(),
    }
}

/// The parameter list of a function type written `(…) => …`, verbatim.
///
/// `getDefaultValueForEvent` reads `e.type.parameters` and only for a `FunctionTypeNode`, so a
/// named type alias (`OnChange`) or a union contributes nothing.
fn function_type_parameters(type_text: Option<&str>) -> Option<String> {
    let text = type_text?.trim();
    if !text.starts_with('(') {
        return None;
    }
    let close = crate::build::lex::matching_delimiter(text, 0)?;
    let rest = text[close + 1..].trim_start();
    if !rest.starts_with("=>") {
        return None;
    }
    Some(text[1..close].trim().to_string())
}

/// Where the `@Computed` getter for `name` stands in the source.
///
/// The getter is one of the struct's method texts; `method_offsets` gives it the same coordinate
/// the fields carry, so both kinds of reset statement sort against one another.
fn computed_getter_offset(comp: &ComponentInfo, name: &str) -> Option<usize> {
    for (index, text) in comp.methods.iter().enumerate() {
        if !is_getter_for(text, name) {
            continue;
        }
        if let Some(offset) = comp.method_offsets.get(index) {
            return Some(*offset);
        }
    }
    None
}

/// Whether this method text declares `get <name>(`, past any decorators and modifiers.
fn is_getter_for(text: &str, name: &str) -> bool {
    let tokens = crate::build::lex::tokenize(text);
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        let token = &tokens[index];
        if token.is_punct('@') {
            index += 2;
            continue;
        }
        if token.kind == crate::build::lex::Kind::Ident
            && matches!(
                token.text,
                "private" | "public" | "protected" | "static" | "async"
            )
        {
            index += 1;
            continue;
        }
        break;
    }
    let is_get = tokens
        .get(index)
        .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "get");
    let is_name = tokens
        .get(index + 1)
        .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == name);
    let opens = tokens.get(index + 2).is_some_and(|t| t.is_punct('('));
    is_get && is_name && opens
}

/// One V2 field declaration: decorators, `readonly` for a `@Param` without `@Once`, the type.
pub(crate) fn emit_v2_field_declaration(out: &mut String, f: &FieldInfo) {
    for comment in &f.comment {
        out.push_str(&format!("    {comment}\n"));
    }
    if f.is_static {
        // `processComponentProperty` hands the initializer back to the declaration for a
        // `staticPropertySet` member (`t.staticPropertySet.has(n)&&(p=e.initializer)`) and the
        // final `updatePropertyDeclaration` keeps the source's modifiers, so the member is
        // emitted as written and the constructor never touches it. "As written" includes the
        // accessibility modifier, and no default is added — the same rule as V1's static branch
        // in `process_component_member.rs` (measured:
        // `fixtures/v2-static-member-modifier/golden/Index.ts:13-15`, where the three static
        // declarations come out `public static …` / `private static …` / `static …`, matching
        // the source word for word; `fixtures/v2-static-member` covers the bare
        // `static readonly TAG` form).
        let access = f
            .accessibility
            .as_deref()
            .map(|access| format!("{access} "))
            .unwrap_or_default();
        out.push_str(&format!(
            "    {access}static {}{}{}{} = {};\n",
            if f.is_readonly { "readonly " } else { "" },
            f.name,
            if f.is_optional { "?" } else { "" },
            f.type_annotation(),
            f.initial.as_deref().unwrap_or("undefined"),
        ));
        return;
    }
    // `@Once` is the only way a child may write a `@Param` back, so the plain form is
    // emitted `readonly` (measured: component-v2 golden). A member the source declares
    // `readonly` keeps the keyword for the same reason every other modifier survives: the
    // reference re-modifies the declaration from `getModifiers(source)`
    // (`process_component_class.js::createPropertyDeclaration`), so `private readonly ANGLE_0`
    // stays `private readonly` — clashbox `ControlDirection.ets:320` (measured 2026-11-07).
    let readonly = if f.is_readonly || (f.has("Param") && !f.has("Once")) {
        "readonly "
    } else {
        ""
    };
    // Every decorator stays on the declaration — es2abc reads the file in ArkTS mode
    // (`lang = "ets"`), where they are compiler annotations rather than runtime calls — except
    // `@BuilderParam`. That one is not a V2 annotation: in a `@ComponentV2` class it survives
    // into the module and is evaluated as a call to an undefined `BuilderParam`, which is the
    // `ReferenceError` clashbox died on at startup. The reference replaces it with a plain
    // parent-supplied member and drops the text (process_component_member_for_v2.js).
    let kept: Vec<&str> = f
        .decorator_texts
        .iter()
        .filter(|text| !text.contains("BuilderParam"))
        .map(String::as_str)
        .collect();
    // A bare declaration carries the source's own modifiers and nothing else: `processComponentProperty`
    // ends with `updatePropertyDeclaration(e, concatenateDecoratorsAndModifiers(s, getModifiers(e)), …)`,
    // and a field written without a modifier therefore stays `plain: number;` — no `private` is
    // added the way the V1 `createPropertyDeclaration` adds it (measured: `fixtures/v2-field-modifiers`).
    // `concatenateDecoratorsAndModifiers`: the decorators come first because TypeScript only
    // accepts them ahead of the modifiers (`@Local private x`, never `private @Local x`); a
    // field written with neither stays bare.
    let access = f.accessibility.as_deref().unwrap_or("");
    let mut prefix = String::new();
    if !kept.is_empty() {
        prefix.push_str(&kept.join(" "));
        prefix.push(' ');
    }
    if !access.is_empty() {
        prefix.push_str(access);
        prefix.push(' ');
    }
    out.push_str(&format!(
        "    {prefix}{readonly}{}{}{};\n",
        f.name,
        if f.is_optional { "?" } else { "" },
        f.type_annotation()
    ));
}

/// `updateStateVarsMethodNode`: emitted only when the struct declares `@Param` fields.
pub(crate) fn emit_v2_update_state_vars(out: &mut String, comp: &ComponentInfo) {
    let params: Vec<&FieldInfo> = comp.fields.iter().filter(|f| f.has("Param")).collect();
    if params.is_empty() {
        return;
    }
    out.push_str("    public updateStateVars(params) {\n");
    out.push_str("        if (params === undefined) {\n            return;\n        }\n");
    for f in params {
        out.push_str(&format!(
            "        if (\"{}\" in params) {{\n            this.updateParam(\"{}\", params.{});\n        }}\n",
            f.name, f.name, f.name
        ));
    }
    out.push_str("    }\n");
}

#[cfg(test)]
mod tests {
    use super::{event_reset_default, is_getter_for};

    /// `getDefaultValueForEvent(info, true)` builds its arrow from the declared function type's
    /// own parameter list, so the reset default is callable with the same arguments.
    #[test]
    fn an_event_reset_default_carries_the_declared_parameters() {
        assert_eq!(
            event_reset_default(Some("(value: string) => void")),
            "(value: string) => {}"
        );
        assert_eq!(
            event_reset_default(Some("(a: number, b: string) => void")),
            "(a: number, b: string) => {}"
        );
    }

    /// Only a function type node contributes parameters: a named alias, a union and a
    /// parameterless function all fall back to `() => {}`.
    #[test]
    fn a_non_function_type_gets_a_parameterless_default() {
        assert_eq!(event_reset_default(Some("OnChange")), "() => {}");
        assert_eq!(
            event_reset_default(Some("(() => void) | undefined")),
            "() => {}"
        );
        assert_eq!(event_reset_default(Some("() => void")), "() => {}");
        assert_eq!(event_reset_default(None), "() => {}");
    }

    /// The `@Computed` getter is located in the method list, past decorators and modifiers, so
    /// its `resetComputed` sorts at the getter's own source offset.
    #[test]
    fn a_getter_is_recognised_by_name() {
        assert!(is_getter_for(
            "@Computed\n  get doubled(): number {\n    return 1;\n  }",
            "doubled"
        ));
        assert!(is_getter_for(
            "private get total(): number { return 1; }",
            "total"
        ));
        assert!(!is_getter_for(
            "get doubled(): number { return 1; }",
            "other"
        ));
        assert!(!is_getter_for("helper(): number { return 1; }", "doubled"));
    }
}
