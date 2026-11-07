// process stage: `process_component_member.js` — one decorated struct field becomes its
// backstore, its accessors and its lifecycle lines.
//
// The reference reads a field's decorators once (`processMemberVariableDecorators`) and routes
// to one `update*` function per decorator; the constructor's `this.__x = new <Class>(…)`
// statement, the `setInitiallyProvidedValue` / `updateStateVars` lines, the typed accessors and
// the `@CustomDialog` controller setter all come from here.
//
// Layout of what this module decides, with the reference location for each:
//
//   @State / @Provide   → ObservedPropertySimplePU / ObjectPU      process_component_member.js:736 (updateObservedPropertyPU)
//   @Prop               → SynchedPropertySimpleOneWayPU            process_component_member.js:745 (updateSynchedPropertyOneWayPU)
//   @Link               → SynchedPropertySimple/TwoWayPU           process_component_member.js:740 (updateSynchedPropertyTwoWayPU)
//   @ObjectLink         → SynchedPropertyNesedObjectPU             process_component_member.js:750
//   @StorageLink/@Prop  → createStorageLink / createStorageProp    process_component_member.js:428
//   @LocalStorage*      → createLocalStorageLink / Prop            process_component_member.js:428
//   @Consume            → initializeConsume / ObservedPropertyAbstractPU
//   @CustomDialog+CustomDialogController field → setController     process_component_member.js:184 (createControllerSet)

use crate::build::stage::pre_define;
use crate::build::stage::utils::{ComponentInfo, FieldInfo};

/// `createControllerSet` (process_component_member.js:184).
///
/// The reference keys on two things and nothing else: the enclosing struct is in
/// `componentCollection.customDialogs`, and the field's declared type is
/// `CustomDialogController`. The setter's parameter is `SET_CONTROLLER_CTR` typed
/// `SET_CONTROLLER_CTR_TYPE`, and the body assigns that parameter to the field by its own
/// name, so the field's spelling comes from the source.
pub(crate) fn emit_controller_set(out: &mut String, comp: &ComponentInfo, field: &FieldInfo) {
    if !comp.is_dialog || !is_controller_field(field) {
        return;
    }
    out.push_str(&format!(
        "    {}({}: {}) {{\n        this.{} = {};\n    }}\n",
        pre_define::SET_CONTROLLER_METHOD,
        pre_define::SET_CONTROLLER_CTR,
        pre_define::SET_CONTROLLER_CTR_TYPE,
        field.name,
        pre_define::SET_CONTROLLER_CTR,
    ));
}

/// The accessibility keyword a V1 member declaration carries, with its trailing space.
///
/// A member written with a modifier keeps it. A member written without one is declared
/// `private` — except a `readonly` member, which the reference declares with no accessibility
/// keyword at all (measured: `fixtures/readonly-member`, `readonly ro: number;` beside
/// `private plain: string;`).
fn accessibility_prefix(f: &FieldInfo) -> String {
    match f.accessibility.as_deref() {
        Some(written) => format!("{written} "),
        None if f.is_readonly => String::new(),
        None => "private ".to_string(),
    }
}

/// `createVariableInitStatement` for one V1 field: the constructor statement that creates its
/// backstore (`updateObservedPropertyPU`, `updateSynchedProperty*PU`,
/// `updateStoragePropAndLinkProperty`, `updateConsumeProperty`).
pub(crate) fn emit_v1_field_init(
    out: &mut String,
    f: &FieldInfo,
    facts: &crate::build::stage::process_component_class::FileFacts,
    next_comments: &[String],
) {
    if f.is_static {
        // `isStaticProperty` (`process_component_class.js`): a static member belongs to the
        // class rather than to an instance, so the reference emits no constructor line for it.
        return;
    }
    let name = &f.name;
    let init = f.initial.as_deref().unwrap_or("undefined");
    let Some(decorator) = f.decorator() else {
        // `updateNormalProperty`: a plain field keeps its initializer in the constructor.
        // A `CustomDialogController` initializer already carries the rewritten
        // `jsDialog.setController(this.<name>)`, and a plain member keeps its comment block
        // there as well (measured: `fixtures/comment-leading`, `MEMBER_PLAIN`;
        // `fixtures/configuration-page`, `/* 自定义弹框 */` + `// 配置是否保存弹框 Controller定义`).
        let init = if crate::build::stage::process_component_class::is_plain_member(f) {
            controller_comment_in_init(init, name, &f.comment)
        } else {
            init.to_string()
        };
        out.push_str(&format!("        this.{name} = {init};\n"));
        return;
    };
    // The next member's block lands in the constructor only for a field that carries its own
    // initializer: the reference appends the statement right after the initializer token. A
    // field without one seeds from `params.<name>` and takes the block inside its declared type
    // argument instead (`type_gap` below) — the two are mutually exclusive, and emitting both
    // was the double copy measured on `fixtures/proxy-group-item`
    // (`__ProxyGroupItemSelected`, `__proxyitems`).
    let construct_gap = if f.initial.is_some() {
        next_member_comments_gap(next_comments)
    } else {
        String::new()
    };
    match decorator {
        // `updateConsumeProperty`: the ancestor's provided property is looked up by key;
        // emitting a literal here would give the child a private copy.
        "Consume" => {
            let key = match (&f.key_value, &f.key_expr) {
                (Some(value), _) => format!(
                    "\"{}\"",
                    crate::build::ets_compile::escape_string_literal(value)
                ),
                (None, Some(expr)) => expr.clone(),
                (None, None) => format!("\"{name}\""),
            };
            out.push_str(&format!(
                "        this.__{name} = this.initializeConsume({key}, \"{name}\");\n"
            ));
        }
        // `updateStoragePropAndLinkProperty`: AppStorage owns the value.
        "StorageLink" | "StorageProp" => {
            let Some(key) = f.key_literal() else {
                return;
            };
            let maker = if decorator == "StorageProp" {
                "createStorageProp"
            } else {
                "createStorageLink"
            };
            out.push_str(&format!(
                "        this.__{name} = this.{maker}({key}, {init}{gap}, \"{name}\");\n",
                gap = construct_gap
            ));
        }
        // `@LocalStorage*` backstores are built on the field declaration itself
        // (`createLocalStroageCallExpression`), not in the constructor.
        "LocalStorageLink" | "LocalStorageProp" => {}
        _ => {
            // The backstore class follows the decorator *and* whether the declared type is
            // simple: an array or an object must not become an `ObservedPropertySimplePU`.
            // `createTypeReferencePU` asks `isSimpleType`, which counts an enum: a member
            // declared with one takes the same simple backstore a `number` would.
            let simple = crate::build::stage::process_component_class::is_simple_type_text(
                f.type_text.as_deref(),
                facts,
            );
            let class = crate::build::ets_compile::observed_class(decorator, simple);
            // `createInitExpressionStatementForDecorator` (`process_component_member.js:523`)
            // always passes three arguments — the seed, `this`, and the field name — and
            // `createPropertyAccessExpressionWithParams` (`:527`) makes the seed
            // `params.<name>` for `@Prop` / `@Link` / `@ObjectLink`. A two-way link therefore
            // receives the parent's observed property object, never its value; `@State` and
            // `@Provide` have no parent argument and start from their own initializer.
            let seed = match decorator {
                "Prop" | "Link" | "ObjectLink" => format!("params.{name}"),
                _ => init.to_string(),
            };
            out.push_str(&format!(
                "        this.__{name} = new {class}({seed}{gap}, this, \"{name}\");\n",
                gap = construct_gap
            ));
            if decorator == "Provide" {
                emit_add_provided_var(out, f);
            }
        }
    }
}

/// `jsDialog.setController(this.<name>)` inside a rewritten controller initializer keeps the
/// member's comment block, printed between `this.` and the name.
///
/// The reference's printer puts the field's leading trivia there because the controller
/// expression it appends reuses the source node (measured: `fixtures/comment-leading`, where
/// `/* MEMBER_PLAIN */` appears three times — interface, this call, class body).
fn controller_comment_in_init(init: &str, name: &str, comments: &[String]) -> String {
    if comments.is_empty() {
        return init.to_string();
    }
    let needle = format!("setController(this.{name})");
    let Some(at) = init.find(&needle) else {
        return init.to_string();
    };
    let line_start = init[..at].rfind('\n').map_or(0, |previous| previous + 1);
    let indent: String = init[line_start..]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let mut block = String::new();
    for comment in comments {
        block.push_str(&format!(
            "\n{indent}{}",
            crate::build::ets::comments::reindent_block(comment, &indent)
        ));
    }
    block.push_str(&format!("\n{indent}"));
    let mut out = String::with_capacity(init.len() + block.len());
    out.push_str(&init[..at]);
    out.push_str("setController(this.");
    out.push_str(&block);
    out.push_str(&format!("{name})"));
    out.push_str(&init[at + needle.len()..]);
    out
}

/// The comment block of the member *after* this one, laid out for the inside of a backstore
/// declaration's type argument.
///
/// The reference prints the next member's trivia at the point where it stops copying the
/// declared type and closes the type argument (measured: `fixtures/configuration-page`,
/// `private __configAddPageInfos: ObservedPropertyAbstractPU<NavPathStack` /
/// `/* 主题色 */` / `>;`).
fn next_member_comments_gap_in_type(comments: &[String]) -> String {
    if comments.is_empty() {
        return String::new();
    }
    let mut gap = String::new();
    for comment in comments {
        gap.push_str(&format!(
            "\n    {}",
            crate::build::ets::comments::reindent_block(comment, "    ")
        ));
    }
    gap.push_str("\n    ");
    gap
}

/// The comment block of the member *after* this one, laid out for the inside of a generated
/// call: each line on its own row, then the indentation the appended arguments continue on.
///
/// The reference prints the next member's leading trivia at the point where it stops copying the
/// source expression and starts appending its own arguments (measured: `fixtures/comment-leading`,
/// `this.__tagged = new ObservedPropertySimplePU(1` / `/* MEMBER_READONLY */` /
/// `, this, "tagged");`, and `fixtures/configuration-page`, `this.__configStr =
/// this.createStorageLink('ConfigContent', ''` / `// 所有子组件在容器内的对齐方式` /
/// `, "configStr");`).
fn next_member_comments_gap(comments: &[String]) -> String {
    if comments.is_empty() {
        return String::new();
    }
    let mut gap = String::new();
    for comment in comments {
        gap.push_str(&format!("\n        {comment}"));
    }
    gap.push_str("\n        ");
    gap
}

/// `addAddProvidedVar`: the key the decorator carries, plus the field name when the two
/// differ; a `@Provide` without a key registers the field name only.
pub(crate) fn emit_add_provided_var(out: &mut String, f: &FieldInfo) {
    let name = &f.name;
    let alias = match (&f.key_value, &f.key_expr) {
        (Some(value), _) => Some(format!(
            "\"{}\"",
            crate::build::ets_compile::escape_string_literal(value)
        )),
        (None, Some(expr)) => Some(expr.clone()),
        (None, None) => None,
    };
    if let Some(alias) = alias {
        out.push_str(&format!(
            "        this.addProvidedVar({alias}, this.__{name}, false);\n"
        ));
        let alias_text = f
            .key_value
            .as_deref()
            .or(f.key_expr.as_deref())
            .unwrap_or_default();
        if alias_text != name {
            // `e !== r.getText()`: an aliased `@Provide` also answers to its field name.
            out.push_str(&format!(
                "        this.addProvidedVar(\"{name}\", this.__{name}, false);\n"
            ));
        }
    } else {
        out.push_str(&format!(
            "        this.addProvidedVar(\"{name}\", this.__{name}, false);\n"
        ));
    }
}

/// `setInitiallyProvidedValue`: the reference's `setUpdateParamsDecorators` set — `@State`,
/// `@Provide`, `@Prop`, `@ObjectLink` (process_component_member.js `createUpdateParams`).
pub(crate) fn emit_v1_initial_params(out: &mut String, comp: &ComponentInfo) {
    out.push_str(&format!(
        "    setInitiallyProvidedValue(params: {}_Params) {{\n",
        comp.name
    ));
    for f in comp.fields.iter().filter(|f| !f.is_static) {
        let name = &f.name;
        match f.decorator() {
            Some("State" | "Provide") => out.push_str(&format!(
                "        if (params.{name} !== undefined) {{\n            this.{name} = params.{name};\n        }}\n"
            )),
            Some("Prop") => {
                // A `@Prop` with no initializer keeps whatever the parent passed: the reference
                // only resets the ones the declaration gave a default to (`@Prop Icon1: Resource`
                // gets no block, measured 2026-11-06 on clashbox's `RoundButton`).
                if let Some(initial) = f.initial.as_deref() {
                    out.push_str(&format!(
                        "        if (params.{name} === undefined) {{\n            this.__{name}.set({initial});\n        }}\n"
                    ));
                }
            }
            Some("ObjectLink") => {
                out.push_str(&format!("        this.__{name}.set(params.{name});\n"))
            }
            // A plain field is copied when the parent passed one
            // (`updateNormalProperty` + `createUpdateParamsWithoutIf`).
            None => out.push_str(&format!(
                "        if (params.{name} !== undefined) {{\n            this.{name} = params.{name};\n        }}\n"
            )),
            _ => {}
        }
    }
    out.push_str("    }\n");
}

/// `updateStateVars`: the reference's `setStateVarsDecorators` set — `@ObjectLink` — plus the
/// `@Prop` reset (process_component_member.js `createUpdateParams`).
pub(crate) fn emit_v1_update_state_vars(out: &mut String, comp: &ComponentInfo) {
    out.push_str(&format!(
        "    updateStateVars(params: {}_Params) {{\n",
        comp.name
    ));
    for f in comp.fields.iter().filter(|f| !f.is_static) {
        let name = &f.name;
        match f.decorator() {
            Some("Prop") => out.push_str(&format!("        this.__{name}.reset(params.{name});\n")),
            Some("ObjectLink") => {
                out.push_str(&format!("        this.__{name}.set(params.{name});\n"))
            }
            _ => {}
        }
    }
    out.push_str("    }\n");
}

/// Whether a field declares the controller a `@CustomDialog` is driven through.
///
/// The declaration is what the setter assigns to; its name is the source's, so nothing here
/// depends on a spelling (`controller` is only a convention).
pub(crate) fn is_controller_field(field: &FieldInfo) -> bool {
    field
        .type_text
        .as_deref()
        .map(str::trim)
        .is_some_and(|text| text.trim_end_matches('?').trim() == "CustomDialogController")
}

/// `createPropertyDeclaration` + `createGetAccessor` / `createSetAccessor` for one V1 field.
pub(crate) fn emit_v1_field_declaration(
    out: &mut String,
    f: &FieldInfo,
    facts: &crate::build::stage::process_component_class::FileFacts,
    next_comments: &[String],
) {
    // The leading block goes above the declaration; a comment the source wrote after the
    // declaration on the same line stays at the end of that line
    // (`private __x; // X`, measured: `fixtures/trailing-comment`).
    for comment in &f.comment {
        // The block is re-indented to the member column: `/**` takes the member's four spaces
        // and every continuation one more (`fixtures/indent-unit`, `FIELD_BLOCK`), so a source
        // written at two spaces does not leave `/**` at four and ` * …` at three.
        let delta = 4isize - f.comment_indent as isize;
        out.push_str(&format!(
            "    {}\n",
            crate::build::ets::comments::reindent_block_with_delta(comment, "    ", delta)
        ));
    }
    let mut declaration = field_declaration_text(f, facts, next_comments);
    if !f.trailing_comment.is_empty() {
        let newline = declaration.find('\n').unwrap_or(declaration.len());
        let (line, rest) = declaration.split_at(newline);
        let trailing = f.trailing_comment.join(" ");
        declaration = format!("{line} {trailing}{rest}");
    }
    out.push_str(&declaration);
}

fn field_declaration_text(
    f: &FieldInfo,
    facts: &crate::build::stage::process_component_class::FileFacts,
    next_comments: &[String],
) -> String {
    let mut out = String::new();
    let name = &f.name;
    let ty = f.type_text.as_deref().unwrap_or("Object");
    let simple = crate::build::stage::process_component_class::is_simple_type_text(
        f.type_text.as_deref(),
        facts,
    );
    if f.is_static {
        // `isStaticProperty` (`process_component_class.js`): the declaration is pushed through
        // as written — no backstore, no accessors, and the initializer stays on the
        // declaration. "As written" includes the accessibility modifier: the reference emits
        // whatever the source wrote and adds no default of its own, unlike the non-static
        // branch below, which defaults to `private` (measured:
        // `fixtures/static-member-modifier/golden/Index.ts:30-32`, where the three static
        // declarations come out `public static …` / `private static …` / `static …`, matching
        // the source word for word).
        let access = f
            .accessibility
            .as_deref()
            .map(|access| format!("{access} "))
            .unwrap_or_default();
        out.push_str(&format!(
            "    {access}static {}{name}{}: {ty} = {init};\n",
            if f.is_readonly { "readonly " } else { "" },
            if f.is_optional { "?" } else { "" },
            init = f.initial.as_deref().unwrap_or("undefined"),
        ));
        return out;
    }
    if f.has("BuilderParam") {
        // A builder parameter is a callable the parent supplies. The reference declares it as a
        // bare `__<name>` member with no type and no initializer, and calls it bound; a `?` on
        // the declaration survives (`@BuilderParam extraBuilder?: () => void` becomes
        // `private __extraBuilder?;`).
        out.push_str(&format!(
            "    private __{name}{};\n",
            if f.is_optional { "?" } else { "" }
        ));
        return out;
    }
    let Some(decorator) = f.decorator() else {
        // An ordinary field is declared without an initializer; the constructor sets it, and
        // the declaration keeps the type, the `?` and the `readonly` the source wrote
        // (`private readonly ANGLE_0: number` stays readonly — measured: ControlDirection.ts).
        let access = accessibility_prefix(f);
        out.push_str(&format!(
            "    {access}{}{name}{}{};\n",
            if f.is_readonly { "readonly " } else { "" },
            if f.is_optional { "?" } else { "" },
            f.type_annotation()
        ));
        return out;
    };
    let class = v1_backstore_type(decorator, simple);
    // The backstore carries the source's own modifier list and gets `private` only when the
    // source wrote none: `createPropertyDeclaration` (`process_component_class.js`) re-modifies
    // the declaration with `concatenateDecoratorsAndModifiers(void 0, getModifiers(source) ||
    // [private])`, so `@State readonly st` becomes `readonly __st` and `@State public x` becomes
    // `public __x` (measured: `fixtures/readonly-member`, and clashbox
    // `entry/components/Common/Common.ts:3364` `public __providerListData` for
    // `@State public providerListData: Array<Provider>`). `accessibility_prefix` already carries
    // the accessibility half of that rule for the plain-member branch, and `readonly` follows it
    // because TypeScript only accepts the modifier in that order.
    let backstore_access = format!(
        "{}{}",
        accessibility_prefix(f),
        if f.is_readonly { "readonly " } else { "" }
    );
    // A declaration takes the next member's block inside its type argument
    // (`ObservedPropertyAbstractPU<X` + the comment + `>`) exactly when the field itself has no
    // initializer: with one, the type text is followed by the initializer's own token and the
    // comment lands elsewhere; without one the type is the last real token before the `>` the
    // reference appends. Measured across `fixtures/comment-leading` (`MAT_*`, `NEXT_*`),
    // `fixtures/configuration-page` and `fixtures/proxy-group-item`: `@Consume` without an
    // initializer and `@Prop ProxyGroupItemSelected: ProxyCardSize` carry, while
    // `@StorageProp('bottomRectHeight') bottomRectHeight: number = 0` and every `= value`
    // member does not.
    // Only a `@Consume` declaration takes the next member's block inside its type argument
    // (`ObservedPropertyAbstractPU<X` + the comment + `>`), measured on
    // `fixtures/configuration-page`: `__stackAlignContentAlignment`,
    // `__configAddPageInfos` and `__icon_emphasize` carry, `__bottomRectHeight` (a
    // `@StorageProp`) and the `@State` backstores do not.
    let type_gap = if f.initial.is_none() {
        next_member_comments_gap_in_type(next_comments)
    } else {
        String::new()
    };
    // A declaration written `?` keeps the marker on its backstore too
    // (`@Prop buttonColor?: ResourceColor` → `private __buttonColor?: …`).
    let optional = if f.is_optional { "?" } else { "" };
    if matches!(decorator, "LocalStorageLink" | "LocalStorageProp") {
        // `createLocalStroageCallExpression`: key and default live on the declaration, and
        // the call carries the declared type as its generic argument.
        if let Some(key) = f.key_literal() {
            let maker = if decorator == "LocalStorageProp" {
                "createLocalStorageProp"
            } else {
                "createLocalStorageLink"
            };
            let generic = f
                .type_text
                .as_deref()
                .map(|t| format!("<{t}>"))
                .unwrap_or_default();
            out.push_str(&format!(
                "    {backstore_access}__{name}{optional}: {class}<{ty}{type_gap}> = this.{maker}{generic}({key}, {}, \"{name}\");\n",
                f.initial.as_deref().unwrap_or("undefined")
            ));
        } else {
            out.push_str(&format!(
                "    {backstore_access}__{name}{optional}: {class}<{ty}{type_gap}>;\n"
            ));
        }
    } else {
        out.push_str(&format!(
            "    {backstore_access}__{name}{optional}: {class}<{ty}{type_gap}>;\n"
        ));
    }
    out.push_str(&format!(
        "    get {name}() {{\n        return this.__{name}.get();\n    }}\n"
    ));
    // `immutableDecorators` (`@ObjectLink`, `@BuilderParam`, `@Env`) get no setter.
    if !f.is_immutable() {
        out.push_str(&format!(
            "    set {name}(newValue: {ty}) {{\n        this.__{name}.set(newValue);\n    }}\n"
        ));
    }
    out
}

/// The type annotation a V1 backstore declares.
///
/// `process_component_class.js:609-610` (`createTypeReferencePU`) is the source, and it carries
/// an asymmetry this function reproduces rather than smooths over: a `@Link` whose declared
/// type is not simple still declares `SYNCHED_PROPERTY_SIMPLE_ONE_WAY_PU`, while
/// `updateSynchedPropertyTwoWayPU` (`process_component_member.js:740`) constructs
/// `SYNCHED_PROPERTY_OBJECT_TWO_WAY_PU` for the same field. The declaration and the
/// construction name different classes on purpose.
pub(crate) fn v1_backstore_type(decorator: &str, simple: bool) -> &'static str {
    match decorator {
        "State" | "Provide" => {
            if simple {
                pre_define::OBSERVED_PROPERTY_SIMPLE_PU
            } else {
                pre_define::OBSERVED_PROPERTY_OBJECT_PU
            }
        }
        "Link" => {
            if simple {
                pre_define::SYNCHED_PROPERTY_SIMPLE_TWO_WAY_PU
            } else {
                pre_define::SYNCHED_PROPERTY_SIMPLE_ONE_WAY_PU
            }
        }
        "Prop" => pre_define::SYNCHED_PROPERTY_SIMPLE_ONE_WAY_PU,
        "ObjectLink" => pre_define::SYNCHED_PROPERTY_NESED_OBJECT_PU,
        "Consume" | "StorageLink" | "StorageProp" | "LocalStorageLink" | "LocalStorageProp" => {
            pre_define::OBSERVED_PROPERTY_ABSTRACT_PU
        }
        _ => pre_define::OBSERVED_PROPERTY_SIMPLE_PU,
    }
}
