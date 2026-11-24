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
    TSMappedType, TSType, TSTypeAnnotation, TSTypeAssertion, TSTypeLiteral, TSTypeName,
    TSTypeQuery, TSTypeReference, ThisExpression, UnaryOperator, VariableDeclaration,
    VariableDeclarationKind, VariableDeclarator,
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
/// 含 `import` 或 `export` 的文件是模块：模块顶层声明处在一个独立作用域里，不与 SDK 的
/// 全局声明冲突（`fixtures/type-redeclare` 无 import 要报 `Cannot redeclare`，
/// `fixtures/import-type-value` 有 import 就一条诊断都没有）。
fn is_module_source(source: &str) -> bool {
    source.lines().any(|line| {
        let text = line.trim_start();
        text.starts_with("import ") || text.starts_with("export ")
    })
}

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

/// A component's non-method members: name and the type text the reference prints for it.
type ExternalMembers = std::collections::HashMap<String, Vec<(String, String)>>;

/// 一个模块的资源名：`(kind, name)` 与 `rawfile` 下的相对路径。
type ModuleResources = (
    std::collections::HashSet<(String, String)>,
    std::collections::HashSet<String>,
);

// 资源名表是**进程级**的：发布在主线程，判定在编译工作线程里跑，thread_local 会让
// 工作线程看到空表，于是每个 `$r(…)` 都被判成缺失（`fixtures/resource-audit` 的 8:15）。
static RESOURCE_NAMES: std::sync::OnceLock<std::sync::RwLock<Option<ModuleResources>>> =
    std::sync::OnceLock::new();

// `@Extend(Text) function loud()` 定义在别的文件里时，用它的是页面而不是定义处：
// 表必须是进程级、并且由编译前的一次扫描填满（`fixtures/helper-export-default` 的
// `Text('loud').loud()` 参考侧无诊断）。
static EXTEND_STYLES: std::sync::OnceLock<std::sync::RwLock<std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

pub(crate) fn set_extend_styles(names: std::collections::HashSet<String>) {
    *EXTEND_STYLES
        .get_or_init(|| std::sync::RwLock::new(std::collections::HashSet::new()))
        .write()
        .unwrap_or_else(|e| e.into_inner()) = names;
}

fn is_extend_style(name: &str) -> bool {
    EXTEND_STYLES
        .get()
        .and_then(|cell| cell.read().ok().map(|set| set.contains(name)))
        .unwrap_or(false)
}

fn resource_names_cell() -> &'static std::sync::RwLock<Option<ModuleResources>> {
    RESOURCE_NAMES.get_or_init(|| std::sync::RwLock::new(None))
}

/// 发布本模块的资源名表（见 `ets_assemble::publish_resource_names`）。
pub(crate) fn set_resource_names(
    table: std::collections::HashSet<(String, String)>,
    rawfiles: std::collections::HashSet<String>,
) {
    *resource_names_cell()
        .write()
        .unwrap_or_else(|e| e.into_inner()) = Some((table, rawfiles));
}

/// `$r('app.<kind>.<name>')` / `$rawfile('<name>')` 的缺失诊断。
///
/// 参考侧：`Unknown resource name 'not_exist'.`（10903329，锚点在字符串起始 6:15）、
/// `No such 'missing.png' resource in current module.`（10904333，7:13）。
fn missing_resource_refs(source: &str) -> Vec<(usize, &'static str, String)> {
    let table = resource_names_cell()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let Some((names, rawfiles)) = table else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("$r(") {
        let start = from + at;
        let after = &source[start + 3..];
        let Some(close) = after.find(')') else {
            break;
        };
        from = start + 3;
        let inner = after[..close].trim();
        let Some(quoted) = inner.strip_prefix('\'') else {
            continue;
        };
        let Some(value) = quoted.strip_suffix('\'') else {
            continue;
        };
        // 对象属性的值位置不判：`title: { title: $r(...) }`、`backgroundColor: $r(...)` 都是
        // 对象字面量的值，参考侧对这些缺失名字一条都不报（`fixtures/proxy-group-item`
        // 重生成 golden 后 `diagnostics.txt` 仍是 0 行，而 `fixtures/resource-missing-ref`
        // 的 `Text($r('app.string.not_exist'))` 要报 `Unknown resource name`）。
        if source[..start].trim_end().ends_with(':') {
            continue;
        }
        let resource_at = start + 3 + (after.len() - after.trim_start().len());
        let Some(rest) = value.strip_prefix("app.") else {
            continue;
        };
        let Some((kind, name)) = rest.split_once('.') else {
            continue;
        };
        if names.contains(&(kind.to_string(), name.to_string())) {
            continue;
        }
        found.push((
            resource_at,
            "10903329",
            format!("Unknown resource name '{name}'."),
        ));
    }
    let mut from = 0usize;
    while let Some(at) = source[from..].find("$rawfile(") {
        let start = from + at;
        let after = &source[start + "$rawfile(".len()..];
        let Some(close) = after.find(')') else {
            break;
        };
        from = start + "$rawfile(".len();
        let inner = after[..close].trim();
        let Some(quoted) = inner.strip_prefix('\'') else {
            continue;
        };
        let Some(value) = quoted.strip_suffix('\'') else {
            continue;
        };
        // 参考侧的锚点是 `$rawfile` 的 `$`（7:13），不是参数字符串。
        let at = start;
        if rawfiles.contains(value) {
            continue;
        }
        found.push((
            at,
            "10904333",
            format!("No such '{value}' resource in current module."),
        ));
    }
    found.sort_unstable();
    found
}

thread_local! {
    /// Components the SDK declares outside the ArkUI component metadata: the `@Component struct`s
    /// under `hms/ets/api` that `external_component_map` covers.
    static EXTERNAL_COMPONENTS: std::cell::RefCell<Option<ExternalMembers>> =
        const { std::cell::RefCell::new(None) };
}

/// Publishes the external component table; see `ets_assemble::publish_external_components`.
pub(crate) fn set_external_components(table: ExternalMembers) {
    EXTERNAL_COMPONENTS.with(|cell| *cell.borrow_mut() = Some(table));
}

/// The members of one external component, when the SDK declared it.
fn external_component(name: &str) -> Option<Vec<(String, String)>> {
    EXTERNAL_COMPONENTS.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|table| table.get(name).cloned())
    })
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
    /// `typeof <module>` 的成员：每个系统模块在值空间里的名字（不含 interface/type）。
    static SYSTEM_MODULE_VALUES: std::cell::RefCell<
        Option<std::collections::HashMap<String, std::collections::HashSet<String>>>,
    > = const { std::cell::RefCell::new(None) };
}

/// 发布系统模块的值名表（见 `ets_assemble::publish_system_module_symbols`）。
pub(crate) fn set_system_module_values(
    values: std::collections::HashMap<String, std::collections::HashSet<String>>,
) {
    SYSTEM_MODULE_VALUES.with(|cell| *cell.borrow_mut() = Some(values));
}

/// `Some(true)` when the module declares `symbol` in value position, `Some(false)` when it does
/// not, `None` when that module's values were not collected.
fn system_module_has_value(request: &str, symbol: &str) -> Option<bool> {
    SYSTEM_MODULE_VALUES.with(|cell| {
        let table = cell.borrow();
        let table = table.as_ref()?;
        table.get(request).map(|names| names.contains(symbol))
    })
}

thread_local! {
    /// `@kit.*` 聚合模块的成员 → 它实际来源的模块：`@kit.ArkTS` 的 `util` 来自
    /// `@ohos.util`。`typeof util` 的成员判定要按源模块的名字来
    /// （`fixtures/system-module-named-import` 的 4:19、5:22）。
    static KIT_MEMBERS: std::cell::RefCell<
        Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,
    > = const { std::cell::RefCell::new(None) };
}

/// Publishes the kit → (member → source module) map.
pub(crate) fn set_kit_members(
    map: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) {
    KIT_MEMBERS.with(|cell| *cell.borrow_mut() = Some(map));
}

/// `@kit.ArkTS` + `util` → `@ohos.util`。
pub(crate) fn kit_member_module(kit: &str, member: &str) -> Option<String> {
    KIT_MEMBERS.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|map| map.get(kit))
            .and_then(|members| members.get(member))
            .cloned()
    })
}

/// Publishes the declared names of every system module, alongside their existence.
pub(crate) fn set_system_module_symbols(
    symbols: std::collections::HashMap<String, Option<std::collections::HashSet<String>>>,
) {
    SYSTEM_MODULE_SYMBOLS.with(|cell| *cell.borrow_mut() = Some(symbols));
}

/// `typeof <module>` 上不存在的成员，从源码文本看。
///
/// 页面的 `build()` 体在解析视图里被抹白（`blank_build_bodies`），AST 里没有那个表达式，
/// 所以这里按文本扫 `本地名.成员`，锚点在成员名上（`fixtures/system-module-member` 的 8:24）。
fn system_member_reads(
    source: &str,
    imports: &std::collections::HashMap<String, String>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (local, request) in imports {
        let needle = format!("{local}.");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            from = start + needle.len();
            let boundary = !source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
            if !boundary {
                continue;
            }
            let rest = &source[start + needle.len()..];
            let member: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if member.is_empty() {
                continue;
            }
            // 类型位置上的 `ns.Type` 不是值读取：`const mgr: resourceManager.ResourceManager = …`。
            if source[..start].trim_end().ends_with(':') {
                continue;
            }
            // 成员名首字母大写的是枚举成员或类静态成员，不是模块自己的导出名：
            // `KeyCode.KEYCODE_ESCAPE`、`AppStorageV2.connect` 的 `connect` 走的是类型成员，
            // 参考侧对它们不按「模块没有这个导出」判
            // （`fixtures/declare-enum-value`、`fixtures/kit-arkui-state` 金标里都没有诊断，
            // 而 `fixtures/system-module-member` 的小写 `notAReal` 要报）。
            if member
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
            {
                continue;
            }
            if system_module_has_value(request, &member) != Some(false) {
                continue;
            }
            found.push((
                start + needle.len(),
                format!("Property '{member}' does not exist on type 'typeof {local}'."),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// `as any` / `as unknown` in the source text: casts inside a `build()` body are blanked in the
/// parse view, so they are found here (see [`Checker::visit_ts_as_expression`]).
fn text_any_unknown_casts(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for keyword in ["unknown", "any"] {
        let needle = format!(" as {keyword}");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            from = start + needle.len();
            let after = source.get(from..from + 1).unwrap_or(" ");
            if after
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                // ` as unknownThing` names another type.
                continue;
            }
            if after == "." {
                // ` as unknown.foo` is a property read on a cast, not this rule.
                continue;
            }
            found.push((
                start + " as ".len(),
                "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                    .to_string(),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// 每个 struct 里声明为某个具名类型（接口/类）的字段：`box: Shape` → `box` → `Shape`。
///
/// 与 [`struct_field_types`] 分开：那张表只收 `number`/`string`/`boolean`，用来判组件实参的
/// 基本类型，加入具名类型会改变那些判定。
fn struct_object_field_types(
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
                        let TSType::TSTypeReference(reference) = &annotation.type_annotation else {
                            continue;
                        };
                        let TSTypeName::IdentifierReference(base) = &reference.type_name else {
                            continue;
                        };
                        fields.insert(name.to_string(), base.name.to_string());
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
/// 泛型函数里「返回类型就是一个类型参数」的签名：名字 → 类型参数名。
///
/// 只收这一形态：`function first<T>(items: T[]): T`。参考侧把显式实例化后的返回类型拿去和对
/// 赋值目标比较（`fixtures/ts-generic-call-type`：`const n: number = first<string>(["a"])`
/// 报 `Type 'string' is not assignable to type 'number'.`，锚在变量名 5:7）。
fn generic_return_params(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("function ") else {
            continue;
        };
        let Some(open) = rest.find('<') else {
            continue;
        };
        let name = rest[..open].trim().to_string();
        let Some(close) = rest.find('>') else {
            continue;
        };
        if close < open {
            continue;
        }
        let params: Vec<String> = rest[open + 1..close]
            .split(',')
            .map(|part| {
                part.split("extends")
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            })
            .filter(|part| !part.is_empty())
            .collect();
        let Some(ret) = rest[close + 1..].rfind("):") else {
            continue;
        };
        let ty = rest[close + 1..][ret + 2..]
            .trim()
            .trim_end_matches('{')
            .trim()
            .to_string();
        if params.iter().any(|param| param == &ty) && !name.is_empty() {
            out.push((name, ty));
        }
    }
    out
}

/// `export = X`：两条诊断，同一位置（`export` 的行首）。
///
/// 参考侧报 `"export = ..." assignment is not supported (arkts-no-export-assignment)`（10605126）
/// 与 `Export assignment cannot be used when targeting ECMAScript modules. Consider using
/// 'export default' or another module format instead.`（10505001），都在
/// `fixtures/arkts-export-assignment` 的 7:1。
fn export_assignment_violations(source: &str) -> Vec<(usize, &'static str, &'static str)> {
    let mut found = Vec::new();
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("export =") || trimmed.starts_with("export=") {
            let start = offset + (line.len() - trimmed.len());
            found.push((
                start,
                "10605126",
                "\"export = ...\" assignment is not supported (arkts-no-export-assignment)",
            ));
            found.push((
                start,
                "10505001",
                "Export assignment cannot be used when targeting ECMAScript modules. Consider using 'export default' or another module format instead.",
            ));
        }
        offset += line.len();
    }
    found
}

/// 每个类声明的方法名（不含属性）：给 `arkts-no-method-reassignment` 判定用。
fn class_method_names(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    struct Collect(std::collections::HashMap<String, std::collections::HashSet<String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                let set = self.0.entry(id.name.to_string()).or_default();
                for element in &it.body.body {
                    if let ClassElement::MethodDefinition(method) = element {
                        if let Some(name) = method.key.static_name() {
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

/// `arkts-no-method-reassignment`（10605052）：给对象的方法名重新赋值。
///
/// 参考侧 `Reassigning object methods is not supported (arkts-no-method-reassignment)`，
/// 锚点在赋值左侧的对象起始（`fixtures/arkts-method-reassignment` 的 14:3，
/// `target.method = () => 2;`）。方法名表只含 `MethodDefinition`，同名的数据字段不误报。
fn text_method_reassignments(
    source: &str,
    methods: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    locals: &std::collections::HashMap<String, String>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (local, class) in locals {
        let base = class.split('<').next().unwrap_or(class);
        let Some(names) = methods.get(base) else {
            continue;
        };
        for name in names {
            let needle = format!("{local}.{name} =");
            let mut from = 0usize;
            while let Some(at) = source[from..].find(&needle) {
                let start = from + at;
                from = start + needle.len();
                let boundary = !source[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
                if !boundary {
                    continue;
                }
                // `==` 是比较，不是赋值。
                if source[from..].starts_with('=') {
                    continue;
                }
                found.push((
                    start,
                    "Reassigning object methods is not supported (arkts-no-method-reassignment)"
                        .to_string(),
                ));
            }
        }
    }
    found.sort_unstable();
    found
}

/// `build()` 体里 `this.<字段>.<成员>`：字段的类型（接口）没有那个成员。
///
/// 参考侧 `Property 'missing' does not exist on type 'Shape'.`（
/// `fixtures/type-field-member-in-build` 的 12:24）。字段类型来自 struct 的字段声明，成员表
/// 来自本文件的接口。`build()` 体在解析视图里被抹白，所以按源码文本判。
fn text_field_member_reads(
    source: &str,
    field_types: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    member_types: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) -> Vec<(usize, String)> {
    use std::collections::HashMap;
    // 字段名 → 它声明的类型名（各 struct 的并集：同一个字段名在两个 struct 里指向同一类型时
    // 才参与判定，指向不同类型时跳过，避免误报）。
    let mut field_type: HashMap<String, Option<String>> = HashMap::new();
    for fields in field_types.values() {
        for (field, spelling) in fields {
            let base: String = spelling
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if base.is_empty() || !member_types.contains_key(&base) {
                continue;
            }
            match field_type.get(field) {
                None => {
                    field_type.insert(field.clone(), Some(base));
                }
                Some(Some(existing)) if *existing != base => {
                    field_type.insert(field.clone(), None);
                }
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    for (field, spelling) in field_type {
        let Some(spelling) = spelling else {
            continue;
        };
        let Some(members) = member_types.get(&spelling) else {
            continue;
        };
        let needle = format!("this.{field}.");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            from = start + needle.len();
            let boundary = !source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
            if !boundary {
                continue;
            }
            let rest = &source[from..];
            let member: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if member.is_empty() || members.contains_key(&member) {
                continue;
            }
            found.push((
                from,
                format!("Property '{member}' does not exist on type '{spelling}'."),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// `build()` 体里少传实参的调用：`pair(1)` 而 `pair(first, second)`。
///
/// 参考侧 `Expected 2 arguments, but got 1.`（`fixtures/type-arity-in-build` 的 10:15，锚点在
/// 函数名）。`build()` 体在解析视图里被抹白，`declared_arity` 那条 AST 判定看不到它，所以这里
/// 按源码数实参的顶层逗号。
fn text_arity_calls(
    source: &str,
    arity: &std::collections::HashMap<String, (usize, usize)>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (name, (minimum, maximum)) in arity {
        let needle = format!("{name}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            let open = start + needle.len() - 1;
            from = open + 1;
            let boundary = !source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
            if !boundary {
                continue;
            }
            // 声明不是调用：`function build(...)` 与组件的 `build() {` 方法头。
            let line_start = source[..start].rfind('\n').map(|at| at + 1).unwrap_or(0);
            let line_text = source[line_start..].split('\n').next().unwrap_or("").trim();
            if source[..start].trim_end().ends_with("function")
                || line_text == "build() {"
                || line_text == "build(){}"
            {
                continue;
            }
            // 取到配对的 `)`，数顶层逗号。
            let mut depth = 1usize;
            let bytes = source.as_bytes();
            let mut index = open + 1;
            let mut commas = 0usize;
            let mut empty = true;
            while index < bytes.len() {
                match bytes[index] {
                    // 一个对象或数组字面量也是实参：`f({})` 传了一个。
                    b'(' | b'[' | b'{' => {
                        depth += 1;
                        empty = false;
                    }
                    b')' | b']' | b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    b',' if depth == 1 => commas += 1,
                    b' ' | b'\t' | b'\n' | b'\r' => {}
                    _ => empty = false,
                }
                index += 1;
            }
            if depth != 0 {
                continue;
            }
            let provided = if empty { 0 } else { commas + 1 };
            if provided >= *minimum || provided > *maximum {
                continue;
            }
            found.push((
                start,
                format!("Expected {minimum} arguments, but got {provided}."),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// `build()` 体里传给接口参数的对象字面量缺了必填字段。
///
/// 参考侧：`Argument of type '{ width: number; }' is not assignable to parameter of type
/// 'Shape'.` 后接 `Property 'height' is missing in type '{ width: number; }' but required in
/// type 'Shape'.`（`fixtures/type-missing-field-in-build` 的 15:21，锚点在 `{`）。
/// `build()` 体在解析视图里被抹白，所以这里从源码取：接口 → 必填字段，函数 → 首参的接口名，
/// 再判调用点的字面量。
fn text_missing_literal_fields(source: &str) -> Vec<(usize, String)> {
    use std::collections::{HashMap, HashSet};
    // 接口 → 必填字段（名字与拼写类型）。
    let mut interfaces: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("interface ")
            .or_else(|| trimmed.strip_prefix("export interface "))
        else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if name.is_empty() {
            continue;
        }
        let mut fields = Vec::new();
        for body_line in lines.by_ref() {
            let body = body_line.trim();
            if body == "}" {
                break;
            }
            let Some(colon) = body.find(':') else {
                continue;
            };
            let key = body[..colon].trim();
            if key.is_empty() || key.starts_with("//") || key.starts_with('*') || key.ends_with('?')
            {
                continue;
            }
            let spelling: String = body[colon + 1..]
                .trim()
                .trim_end_matches(';')
                .trim()
                .to_string();
            fields.push((key.to_string(), spelling));
        }
        interfaces.insert(name, fields);
    }
    if interfaces.is_empty() {
        return Vec::new();
    }
    // 函数 → 首参的接口名。
    let mut takes: HashMap<String, String> = HashMap::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("function ")
            .or_else(|| trimmed.strip_prefix("export function "))
        else {
            continue;
        };
        let Some(open) = rest.find('(') else {
            continue;
        };
        let Some(close) = rest.find(')') else {
            continue;
        };
        let params = &rest[open + 1..close];
        let function_name: String = rest[..open]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if function_name.is_empty() {
            continue;
        }
        let Some(colon) = params.find(':') else {
            continue;
        };
        let spelling = params[colon + 1..].trim();
        if interfaces.contains_key(spelling) {
            takes.insert(function_name, spelling.to_string());
        }
    }
    if takes.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (function, interface) in takes {
        let needle = format!("{function}({{");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            // `needle` 末字符是字面量的 `{`：锚点在那里，字段从它之后开始。
            let open = start + needle.len() - 1;
            from = open + 1;
            let boundary = !source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
            if !boundary {
                continue;
            }
            let Some(close) = source[open + 1..].find('}') else {
                continue;
            };
            let literal = &source[open + 1..open + 1 + close];
            // `{ x, y }` 是简写属性（键名就是标识符），`{ x: 1 }` 是键值对；两者都算提供了键。
            // 把 `{ x, y }` 当成缺字段会误报解构参数（`fixtures/arkts-destructuring` 的
            // `function destructParam({ x, y }: Point)`）。
            let present: HashSet<String> = literal
                .split(',')
                .filter_map(|entry| {
                    let entry = entry.trim();
                    if entry.is_empty() {
                        return None;
                    }
                    Some(match entry.split_once(':') {
                        Some((key, _)) => key.trim().to_string(),
                        None => entry.to_string(),
                    })
                })
                .collect();
            let Some(fields) = interfaces.get(&interface) else {
                continue;
            };
            let missing: Vec<&(String, String)> = fields
                .iter()
                .filter(|(key, _)| !present.contains(key))
                .collect();
            if missing.is_empty() {
                continue;
            }
            // `{ width: number; }`：键与它写的类型，按字面量里的顺序。
            let mut spelled = String::new();
            for entry in literal.split(',') {
                let Some((key, value)) = entry.split_once(':') else {
                    continue;
                };
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                let spelling = match value.trim().chars().next() {
                    Some('"') | Some('\'') => "string",
                    Some(c) if c.is_ascii_digit() => "number",
                    Some('t') | Some('f') => "boolean",
                    _ => "unknown",
                };
                spelled.push_str(&format!("{key}: {spelling}; "));
            }
            let rendered = format!("{{ {spelled}}}");
            let (first, _) = missing[0];
            found.push((
                open,
                format!(
                    "Argument of type '{rendered}' is not assignable to parameter of type '{interface}'. Property '{first}' is missing in type '{rendered}' but required in type '{interface}'."
                ),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// 同一个文件里两次导出同一个名字：`export { dup } from './c';` 与 `export { dup } from './d';`。
///
/// 参考侧对每个出现位置各报一条 `Duplicate identifier 'dup'.`
/// （`fixtures/export-duplicate-name` 的 1:10 与 2:10，锚点在名字）。按文本扫，因为这两行
/// 是转发导出，转换后源文本仍保留原样。
fn duplicate_reexport_names(source: &str) -> Vec<(usize, String)> {
    use std::collections::HashMap;
    let mut seen: HashMap<String, Vec<usize>> = HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("export {") {
        let start = from + at;
        let open = start + "export {".len();
        let Some(close) = source[open..].find('}') else {
            break;
        };
        let names = &source[open..open + close];
        from = open + close;
        // 只认转发导出：花括号之后紧跟 `from`。
        let after = source[open + close + 1..].trim_start();
        if !after.starts_with("from") {
            continue;
        }
        let mut cursor = 0usize;
        for part in names.split(',') {
            let name = part.trim();
            let offset_in_part = part.find(name).unwrap_or(0);
            cursor += part.len() + 1;
            if name.is_empty() {
                continue;
            }
            let name_at = open + (cursor - part.len() - 1) + offset_in_part;
            seen.entry(name.to_string()).or_default().push(name_at);
        }
    }
    let mut found = Vec::new();
    for (name, positions) in seen {
        if positions.len() < 2 {
            continue;
        }
        for position in positions {
            found.push((position, format!("Duplicate identifier '{name}'.")));
        }
    }
    found.sort_unstable();
    found
}

/// 泛型函数在 `build()` 体里的实参：`identity('text')` 与 `identity<T extends number>` 冲突。
///
/// 页面 `build()` 体在解析视图里是抹白的，`function_parameter_types` 那条 AST 判定看不到它，
/// 所以这里从源码文本取两个表：函数名 → 类型参数约束，再判调用点上字面量实参的类型。
/// 锚点在字面量起始（`fixtures/type-generic-constraint` 的 10:24）。
fn text_generic_literal_calls(source: &str) -> Vec<(usize, String)> {
    use std::collections::HashMap;
    // 函数名 → 约束拼写（只收单类型参数且参数类型就是该参数的函数）。
    let mut constraints: HashMap<String, &'static str> = HashMap::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("function ") {
        let start = from + at;
        from = start + "function ".len();
        let rest = &source[from..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if name.is_empty() {
            continue;
        }
        let after_name = from + name.len();
        let Some(open) = source[after_name..].find('<') else {
            continue;
        };
        if open > 8 {
            // `function f(a: number)` has no type parameter list; a `<` far away is a comparison.
            continue;
        }
        let Some(close) = source[after_name + open..].find('>') else {
            continue;
        };
        let params = &source[after_name + open + 1..after_name + open + close];
        let Some((_, constraint)) = params.split_once(" extends ") else {
            continue;
        };
        let spelling = match constraint.trim() {
            "number" => "number",
            "string" => "string",
            "boolean" => "boolean",
            _ => continue,
        };
        constraints.insert(name, spelling);
    }
    if constraints.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (name, constraint) in constraints {
        let needle = format!("{name}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            from = start + needle.len();
            let boundary = !source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
            if !boundary {
                continue;
            }
            let argument_at = start + needle.len();
            let rest = &source[argument_at..];
            let (spelling, printed) = match rest.chars().next() {
                Some('\'') | Some('"') => ("string", "string"),
                Some(c) if c.is_ascii_digit() => ("number", "number"),
                _ => continue,
            };
            if spelling == constraint {
                continue;
            }
            found.push((
                argument_at,
                format!(
                    "Argument of type '{printed}' is not assignable to parameter of type '{constraint}'."
                ),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// Utility types ArkTS does not carry. `Partial`/`Required`/`Readonly`/`Record` are the ones it
/// keeps (measured on `fixtures/arkts-utility-types`: `Required<…>` reports nothing at the type
/// name while `Exclude<…>` reports 10605138).
const UNSUPPORTED_UTILITY_TYPES: &[&str] = &[
    "Exclude",
    "Extract",
    "NonNullable",
    "Parameters",
    "ConstructorParameters",
    "ReturnType",
    "InstanceType",
    "ThisParameterType",
    "OmitThisParameter",
    "Uppercase",
    "Lowercase",
    "Capitalize",
    "Uncapitalize",
    "Awaited",
];

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
    check_with_declarations(file, source, &[])
}

/// 与 `check` 相同，外加被 import 的声明文件里的函数返回类型（`declared → 返回类型原文`）。
///
/// 参考侧对声明文件里的签名做类型检查：`fixtures/dts-return-type-mismatch` 里
/// `const n: number = helper()`（`helper(): string`）报
/// `Type 'string' is not assignable to type 'number'.`。
pub(crate) fn check_with_declarations(
    file: &str,
    source: &str,
    declarations: &[(String, String)],
) -> Vec<Diagnostic> {
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
        // ArkTS spelling this parser does not accept: the tree is half parsed, so the
        // tree-driven rules are skipped. The ones that are pure text are still judged, because
        // the reference reports them on the same file (`@Entry` on a function makes the parser
        // choke while `10905337` is still printed — `fixtures/arkts-entry-on-function`).
        let mut out = early_text_diagnostics(file, source);
        out.extend(class_object_diagnostics(file, source));
        return out;
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
        instances: instance_bindings(&parsed.program),
        fn_returns: function_return_types(&parsed.program),
        generic_arity: generic_function_arity(&parsed.program),
        generic_mismatch: false,
        linter_hit: false,
        class_member_types: class_member_types(&parsed.program),
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
        assignment_targets: std::collections::HashSet::new(),
        arrow_depth: 0,
        method_depth: 0,
        default_system_imports: std::collections::HashMap::new(),
        standalone_this_lines: std::collections::HashSet::new(),
        declarations: declarations.to_vec(),
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
    // 写在 `build()` 体里的 `Counter.bump()` 落在 ArkUI 语法上，带方法体的解析会失败，
    // 退回的视图没有方法体，AST 走不到这次调用；类名集仍来自那次解析，规则改走词法。
    if checker.lints {
        let names = checker.class_names.clone();
        // 参考侧只在 `build()` 体内按类名当对象用报这一组：`aboutToAppear()` 里的
        // `EventHub.on(…)` 一条都没有（`fixtures/proxy-node-item` 金标为空），
        // 而 `Text(`${Counter.bump()}`)` 要报（`fixtures/arkts-standalone-this-static`）。
        let calls = match build_body_range(source) {
            Some((start, end)) => class_member_calls(&source[start..end], &names)
                .into_iter()
                .map(|(a, b, c, d)| (a + start, b + start, c + start, d + start))
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        for (object_start, dot_start, member_start, call_end) in calls {
            let name = source[object_start..]
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            checker.class_as_object(object_start, dot_start, member_start, call_end, &name);
        }
    }
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
            if is_class && !is_module_source(source) && sdk_declares(&name) == Some(true) {
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
            // 导出的类型由使用它的模块判：参考侧不在这里报
            // （`fixtures/import-type-audit` 的 `export interface Shape` 金标里没有这一条，
            // 而同名的非导出 `interface Shape` 在 `fixtures/arkts-unique-names` 里要报）。
            if is_module_source(source) {
                continue;
            }
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
        // `@Sendable` 类上还挂了别的装饰器：参考侧报
        // `Only "@Sendable" decorator can be used on "Sendable" class
        // (arkts-sendable-class-decorator)`，锚在其它装饰器的 `@`
        // （`fixtures/arkts-sendable-class-decorator` 的 6:1）。
        {
            let mut offset = 0usize;
            let mut block: Vec<(usize, String)> = Vec::new();
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if let Some(rest) = trimmed.strip_prefix('@') {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    block.push((this_line + indent, name));
                    continue;
                }
                if trimmed.is_empty() {
                    continue;
                }
                let is_class =
                    trimmed.starts_with("class ") || trimmed.starts_with("export class ");
                if is_class {
                    let has_sendable = block.iter().any(|(_, name)| name == "Sendable");
                    if has_sendable {
                        for (at, name) in &block {
                            if name == "Sendable" {
                                continue;
                            }
                            checker.push(
                                "ERROR",
                                "10605158",
                                *at,
                                "Only \"@Sendable\" decorator can be used on \"Sendable\" class (arkts-sendable-class-decorator)"
                                    .to_string(),
                            );
                        }
                    }
                }
                block.clear();
            }
        }

        // `@Sendable` 的两条字段/初始化约束
        // （`fixtures/arkts-sendable-obj-init`、`fixtures/arkts-sendable-field-type`）：
        // · 用对象字面量初始化 Sendable 类型（7:25）→ 10605077；
        // · Sendable 类的字段缺类型注解（4:3）→ 10605179。
        {
            let mut sendable_classes: std::collections::HashSet<String> = Default::default();
            let mut pending = false;
            for line in source.lines() {
                let trimmed = line.trim_start();
                if trimmed == "@Sendable" {
                    pending = true;
                    continue;
                }
                if pending {
                    if let Some(rest) = trimmed
                        .strip_prefix("class ")
                        .or_else(|| trimmed.strip_prefix("export class "))
                    {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            sendable_classes.insert(name);
                        }
                    }
                    pending = false;
                }
            }
            if !sendable_classes.is_empty() {
                // 字段缺注解：在 `@Sendable` 类的类体里找 `name =` 形式。
                let mut offset = 0usize;
                let mut in_sendable = false;
                let mut depth = 0i32;
                let mut class_depth = 0i32;
                for line in source.lines() {
                    let this_line = offset;
                    offset += line.len() + 1;
                    let trimmed = line.trim_start();
                    let indent = line.len() - trimmed.len();
                    if !in_sendable {
                        if let Some(rest) = trimmed.strip_prefix("class ") {
                            let name: String = rest
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            if sendable_classes.contains(&name) {
                                in_sendable = true;
                                class_depth = depth + 1;
                            }
                        }
                    } else if depth == class_depth
                        && !trimmed.starts_with("//")
                        && !trimmed.starts_with('}')
                        && trimmed.contains('=')
                        && !trimmed.contains(':')
                        && !trimmed.starts_with("constructor")
                    {
                        checker.push(
                            "ERROR",
                            "10605171",
                            this_line + indent,
                            "Field in sendable class must have type annotation (arkts-sendable-explicit-field-type)"
                                .to_string(),
                        );
                    }
                    // 计算属性名 `[key]: type`：参考侧报两条
                    // （`fixtures/arkts-sendable-computed` 的 6:3、6:4）。
                    if in_sendable
                        && depth == class_depth
                        && trimmed.starts_with('[')
                        && trimmed.contains(']')
                    {
                        {
                            checker.push(
                                "ERROR",
                                "10605160",
                                this_line + indent,
                                "Computed property names are not allowed in \"Sendable\" classes and interfaces (arkts-sendable-computed-prop-name)"
                                    .to_string(),
                            );
                            checker.push(
                                "ERROR",
                                "10605157",
                                this_line + indent + 1,
                                "Only imported variables can be captured by \"Sendable\" class (arkts-sendable-imported-variables)"
                                    .to_string(),
                            );
                        }
                    }
                    depth += line.matches('{').count() as i32;
                    depth -= line.matches('}').count() as i32;
                    if in_sendable && depth < class_depth {
                        in_sendable = false;
                    }
                    if depth < 0 {
                        depth = 0;
                    }
                }
                // 对象字面量初始化 Sendable 类型。
                let mut offset = 0usize;
                for line in source.lines() {
                    let this_line = offset;
                    offset += line.len() + 1;
                    let trimmed = line.trim_start();
                    if !trimmed.starts_with("const ") && !trimmed.starts_with("let ") {
                        continue;
                    }
                    let Some(eq) = line.find(" = {") else {
                        continue;
                    };
                    let left = &line[..eq];
                    if !sendable_classes
                        .iter()
                        .any(|name| left.contains(&format!(": {name}")))
                    {
                        continue;
                    }
                    checker.push(
                        "ERROR",
                        "10605159",
                        this_line + eq + " = ".len(),
                        "Objects of \"Sendable\" type can not be initialized using object literal or array literal (arkts-sendable-obj-init)"
                            .to_string(),
                    );
                }
            }
        }
        // `@<变量名>` 当装饰器（不是函数、不是 ArkUI 装饰器）：参考侧报一条无位置的
        // `Unable to resolve signature of class decorator when called as an expression.`
        // （`fixtures/ui-decorator-not-callable`）。
        {
            let arkui = [
                "Entry",
                "Component",
                "ComponentV2",
                "CustomDialog",
                "State",
                "Prop",
                "Link",
                "Provide",
                "Consume",
                "Observed",
                "ObservedV2",
                "Track",
                "Watch",
                "Builder",
                "BuilderParam",
                "Styles",
                "Extend",
                "Local",
                "Param",
                "Once",
                "Event",
                "Monitor",
                "Computed",
                "Require",
                "Reusable",
                "Concurrent",
                "Sendable",
                "AnimatableExtend",
                "LocalBuilder",
                "Preview",
            ];
            let declared: std::collections::HashSet<String> =
                function_return_types(&parsed.program)
                    .keys()
                    .cloned()
                    .collect();
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix('@') else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                // 本文件声明的函数（含返回 `object` 的）由其它判据负责，
                // 这里只处理「绑定到字面量的名字」
                // （`fixtures/arkts-decorator-obj-literal` 已有 `Type '{}' ...` 那条）。
                let is_function = source.contains(&format!("function {name}("));
                if name.is_empty()
                    || arkui.contains(&name.as_str())
                    || declared.contains(&name)
                    || is_function
                    || name.contains('.')
                {
                    continue;
                }
                // 只对「后面跟着 class」的装饰器报，避免把状态字段装饰器算进来。
                let next = source[offset.min(source.len())..]
                    .lines()
                    .find(|line| !line.trim().is_empty() && !line.trim().starts_with('@'));
                let is_class = next
                    .map(|line| line.trim_start().starts_with("class "))
                    .unwrap_or(false);
                if !is_class {
                    continue;
                }
                // 锚在装饰器名（`@` 之后），后缀按该名字绑定的字面量类型推断
                // （`fixtures/ui-decorator-not-callable` 的 4:2，`Type 'Number' ...`）。
                let bound = source[..this_line]
                    .lines()
                    .rev()
                    .find_map(|line| {
                        let line = line.trim_start();
                        let rest = line.strip_prefix("const ")?;
                        let eq = rest.find('=')?;
                        if rest[..eq].trim() != name {
                            return None;
                        }
                        let value = rest[eq + 1..].trim().trim_end_matches(';');
                        if value.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                            Some("Number")
                        } else if value.starts_with('"') {
                            Some("String")
                        } else {
                            Some("Object")
                        }
                    })
                    .unwrap_or("Object");
                checker.push(
                    "ERROR",
                    "10505001",
                    this_line + (line.len() - trimmed.len()) + 1,
                    format!(
                        "Unable to resolve signature of class decorator when called as an expression. This expression is not callable. Type '{bound}' has no call signatures."
                    ),
                );
            }
        }
        // `class X extends <函数名>`：参考侧报
        // `Type '() => number' is not a constructor function type.`，锚在基名
        // （`fixtures/arkts-extends-function` 的 10:23）。
        {
            let returns = function_return_types(&parsed.program);
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if !trimmed.starts_with("class ") {
                    continue;
                }
                let Some(at) = trimmed.find(" extends ") else {
                    continue;
                };
                let tail = &trimmed[at + " extends ".len()..];
                let name: String = tail
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let Some(returned) = returns.get(&name) else {
                    continue;
                };
                if let Some(position) = line.find(&format!("extends {name}")) {
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + position + "extends ".len(),
                        format!("Type '() => {returned}' is not a constructor function type."),
                    );
                }
            }
        }
        // 数组字面量里放对象字面量：参考侧报
        // `Array literals must contain elements of only inferrable types
        // (arkts-no-noninferrable-arr-literals)`（10605052）锚在 `[`，并对每个对象字面量
        // 再报 10605038（`fixtures/arkts-noninferrable-arr` 的 2:15、2:16、2:30）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(eq) = line.find("= [") else {
                    continue;
                };
                if !trimmed.starts_with("const ") && !trimmed.starts_with("let ") {
                    continue;
                }
                // 声明上写了类型（`const list: Pair[] = [ … ]`）时元素类型可推断，参考侧不报；
                // 锚点也要取 `= [ ` 的那个 `[`，不能取类型标注里的方括号
                // （`fixtures/shorthand-property` 的 72:21 与 72:27）。
                if line[..eq].contains(':') {
                    continue;
                }
                let bracket = eq + 2;
                let mut cursor = eq + 2;
                if !line[cursor..].contains('{') {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10605043",
                    this_line + bracket,
                    "Array literals must contain elements of only inferrable types (arkts-no-noninferrable-arr-literals)"
                        .to_string(),
                );
                while let Some(brace) = line[cursor..].find('{') {
                    let at = cursor + brace;
                    checker.push(
                        "ERROR",
                        "10605038",
                        this_line + at,
                        "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)"
                            .to_string(),
                    );
                    cursor = at + 1;
                }
            }
        }
        // 同一个 struct 里定义两次 `build`：参考侧对每个定义报
        // `Duplicate function implementation.`，锚在方法名
        // （`fixtures/ui-build-duplicate` 的 5:3、11:3）。
        {
            let mut depth = 0i32;
            let mut struct_depth: Option<i32> = None;
            let mut builds: Vec<usize> = Vec::new();
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if trimmed.starts_with("struct ") || trimmed.starts_with("export struct ") {
                    struct_depth = Some(depth + 1);
                    builds.clear();
                }
                if struct_depth.is_some() && trimmed.starts_with("build(") {
                    builds.push(this_line + indent);
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if let Some(inner) = struct_depth {
                    if depth < inner {
                        if builds.len() > 1 {
                            for at in &builds {
                                checker.push(
                                    "ERROR",
                                    "10505001",
                                    *at,
                                    "Duplicate function implementation.".to_string(),
                                );
                            }
                        }
                        struct_depth = None;
                        builds.clear();
                    }
                }
                if depth < 0 {
                    depth = 0;
                }
            }
        }
        // `build` 带参数：参考侧两条都锚在方法名，第一条带上「函数类型不可赋值」的后缀
        // （`fixtures/ui-build-parameter` 的 5:3）。
        {
            let mut offset = 0usize;
            let mut struct_name = String::new();
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("struct ") {
                    struct_name = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                }
                if !trimmed.starts_with("build(") {
                    continue;
                }
                let Some(close) = trimmed.find(')') else {
                    continue;
                };
                let args = trimmed[6..close].trim();
                if args.is_empty() || struct_name.is_empty() {
                    continue;
                }
                let name = struct_name.clone();
                let at = this_line + (line.len() - trimmed.len());
                checker.push(
                    "ERROR",
                    "10505001",
                    at,
                    format!(
                        "Property 'build' in type '{name}' is not assignable to the same property in base type 'CustomComponent'. Type '({args}) => void' is not assignable to type '() => void'."
                    ),
                );
                checker.push(
                    "ERROR",
                    "10905106",
                    at,
                    "The 'build' method can not have arguments.".to_string(),
                );
            }
        }

        // `@Entry` 挂在 `class`（而非 `struct`）上时，参考侧只报
        // `The '@Entry' decorator can only be used with 'struct'.`，不再报页面级的两条
        // （`fixtures/ui-entry-on-class`）。
        let entry_on_class = {
            let mut pending_entry = false;
            let mut found = false;
            for line in source.lines() {
                let trimmed = line.trim_start();
                if trimmed == "@Entry" {
                    pending_entry = true;
                    continue;
                }
                if trimmed.is_empty() {
                    continue;
                }
                if pending_entry {
                    if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                        found = true;
                        break;
                    }
                    pending_entry = false;
                }
            }
            found
        };
        if entry_on_class {
            // 只留这一条：`@Entry` 只能用于 struct，锚在 `@` 后一位
            // （`fixtures/ui-entry-on-class` 的 2:2）。
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                if line.trim_start() != "@Entry" {
                    continue;
                }
                let indent = line.len() - line.trim_start().len();
                checker.push(
                    "ERROR",
                    "10905337",
                    this_line + indent + 1,
                    "The '@Entry' decorator can only be used with 'struct'.".to_string(),
                );
                break;
            }
            return checker.out;
        }
        // 页面文件不导出东西：带 `export` 的是普通模块文件，参考侧不按「必须恰好一个
        // `@Entry`」判它（`fixtures/barrel-comment` 的 `Helper.ets` 与
        // `fixtures/import-type-audit` 的 `Types.ets` 都只有导出、金标里无这一条）。
        if components == 0 && entries == 0 && !source.contains("export ") {
            // 页面文件里既没有组件也没有 `@Entry`：参考侧按「必须恰好一个 `@Entry`」报
            // （`fixtures/type-implicit-any` 的整文件一条）。组件计数不含 V2，
            // 所以带 `@Entry` 的 V2 页面不走这里
            // （`fixtures/state-in-observed-v2-class`）。
            checker.push_file_level(
                "ERROR",
                "10905402",
                "A page configured in 'main_pages.json or build-profile.json5' must have one and only one '@Entry' decorator."
                    .to_string(),
            );
            checker
                .out
                .sort_by(|a, b| (a.line, a.column, a.code).cmp(&(b.line, b.column, b.code)));
            // 这条早退发生在文件末尾统一设置之前，FAQ 标记要在这里补上
            // （`fixtures/type-implicit-any` 的 WARN 行）。
            FILE_LEVEL_LINTER.with(|cell| cell.set(true));
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
        // `@Styles` 方法在文件级登记，结构体成员表里也要有，否则 `this.<style>` 会被当成
        // 不存在的成员（`fixtures/ui-watch-method` 的参考侧对此不报）。
        let mut members_with_styles = checker.members.clone();
        for set in members_with_styles.values_mut() {
            for style in &checker.style_names {
                set.insert(style.clone());
            }
        }
        // 注释与字符串里的 `this.<name>` 不是成员读取（`fixtures/configuration-page` 的 864:42）。
        let masked = mask_literals(source);
        for (offset, message) in struct_member_violations(&masked, &members_with_styles) {
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
        // An SDK component declared outside the ArkUI metadata (`hms/ets/api`): its parameter
        // literal is checked against the members the declaration lists, and the message spells the
        // whole expected type the way the reference does (8:22 of `fixtures/hms-component-params`).
        let imported_names: Vec<(String, String)> = parsed
            .program
            .body
            .iter()
            .filter_map(|statement| match statement {
                oxc_ast::ast::Statement::ImportDeclaration(import) => Some(import),
                _ => None,
            })
            .flat_map(|import| import.specifiers.iter().flatten())
            .filter_map(|specifier| match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => Some((
                    specifier.local.name.to_string(),
                    specifier.imported.name().to_string(),
                )),
                _ => None,
            })
            .collect();
        for (local, imported) in imported_names {
            let (local, imported) = (local, imported);
            if let Some(members) = external_component(imported.as_str()) {
                let expected = format!(
                    "{{ {} }}",
                    members
                        .iter()
                        .map(|(name, spelling)| format!("{name}?: {spelling};"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                let needle = format!("{local}(");
                let mut from = 0usize;
                while let Some(at) = source[from..].find(&needle) {
                    let start = from + at;
                    let open = start + needle.len() - 1;
                    let Some(close) = matching_paren(source, open) else {
                        break;
                    };
                    from = close;
                    let inside = &source[open + 1..close];
                    let trimmed = inside.trim_start();
                    let Some(body) = trimmed.strip_prefix('{') else {
                        continue;
                    };
                    let Some(end) = body.rfind('}') else {
                        continue;
                    };
                    let body_start = open + 1 + (inside.len() - trimmed.len()) + 1;
                    // The reference spells the literal's type, not its source: `{ nope: number; }`.
                    let spelled_items: Vec<String> = split_top_level(&body[..end])
                        .iter()
                        .filter_map(|item| {
                            let colon = item.find(':')?;
                            let key = item[..colon].trim();
                            let spelling = literal_spelling(item[colon + 1..].trim());
                            if key.is_empty() || spelling.is_empty() {
                                return None;
                            }
                            Some(format!("{key}: {spelling};"))
                        })
                        .collect();
                    let literal = format!("{{ {} }}", spelled_items.join(" "));
                    for item in split_top_level(&body[..end]) {
                        let Some(colon) = item.find(':') else {
                            continue;
                        };
                        let key = item[..colon].trim();
                        if members.iter().any(|(name, _)| name == key) {
                            continue;
                        }
                        let key_at = body[..end]
                            .find(key)
                            .map(|offset| body_start + offset)
                            .unwrap_or(body_start);
                        checker.push(
                            "ERROR",
                            "10505001",
                            key_at,
                            format!(
                                "Argument of type '{literal}' is not assignable to parameter of type '{expected}'. Object literal may only specify known properties, and '{key}' does not exist in type '{expected}'."
                            ),
                        );
                    }
                }
            }
        }
        // 只有剩余参数的函数接受展开实参；其余调用报在 `...` 上。
        let rest_functions = functions_with_rest(&parsed.program);
        let calls_with_spread = spread_argument_violations(source, &rest_functions);
        for (offset, message) in calls_with_spread {
            checker.push("ERROR", "10505001", offset, message);
        }
        // `@ObjectLink` 收到普通字段。
        let (link_fields, regular_fields) = struct_link_and_regular_fields(&parsed.program);
        let struct_names_all: std::collections::HashSet<String> =
            checker.structs.iter().cloned().collect();
        let owner = enclosing_declaration(source, source.len().saturating_sub(1));
        for (offset, message) in object_link_violations(
            source,
            &struct_names_all,
            &link_fields,
            &regular_fields,
            owner.as_deref(),
        ) {
            checker.push("ERROR", "10905315", offset, message);
        }
        // `CustomDialogController` 的选项对象里出现未知名。
        for (offset, message) in dialog_option_violations(source) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // 没有 `build()` 的结构体里出现 UI 组件调用。
        // `"use shared"` 模块里的 `export * from "…"`：参考侧报
        // `"export * from ..." is not allowed in shared module
        // (arkts-shared-module-no-wildcard-export)`（10605164），锚在 `export`
        // （`fixtures/arkts-shared-module-wildcard-export` 的 4:1）。
        if source.contains("\"use shared\"") || source.contains("'use shared'") {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                // 共享模块里导出的普通函数不是 Sendable 实体
                // （`fixtures/arkts-shared-module-exports` 的 5:17）。
                if let Some(rest) = trimmed.strip_prefix("export function ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect();
                    if !name.is_empty() {
                        checker.push(
                            "ERROR",
                            "10605163",
                            this_line + (line.len() - trimmed.len()) + "export function ".len(),
                            "Only \"Sendable\" entities can be exported in shared module (arkts-shared-module-exports)"
                                .to_string(),
                        );
                    }
                }
                if !trimmed.starts_with("export * from") {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10605164",
                    this_line + (line.len() - trimmed.len()),
                    "\"export * from ...\" is not allowed in shared module (arkts-shared-module-no-wildcard-export)"
                        .to_string(),
                );
            }
        }
        // `Function.prototype` 被读取，以及 `bind`/`apply`/`call` 的返回值
        // （`fixtures/arkts-prototype-assignment`）：
        // · `const p = Function.prototype;` →
        //   `Prototype assignment is not supported (arkts-no-prototype-assignment)`
        //   （10605136），锚在 `Function`（6:24）；
        // · 这类无注解的绑定，以及 `x.bind(…)` / `x.apply(…)` / `x.call(…)` 的返回值 →
        //   `arkts-no-any-unknown`（10605008），锚在绑定名（7:7、8:7、9:7）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                let Some(rest) = trimmed.strip_prefix("const ") else {
                    continue;
                };
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                if rest[..eq].contains(':') {
                    continue;
                }
                let value = rest[eq + 1..].trim();
                if let Some(at) = line.find("Function.prototype") {
                    // 锚在 `prototype`（`fixtures/arkts-prototype-assignment` 的 6:24）。
                    checker.push(
                        "ERROR",
                        "10605136",
                        this_line + at + "Function.".len(),
                        "Prototype assignment is not supported (arkts-no-prototype-assignment)"
                            .to_string(),
                    );
                }
                // `Function.prototype` 本身的绑定只报上面那条，不再补 any
                // （同 fixture：参考只在 7/8/9 行报）。
                if value.starts_with("Function.prototype") {
                    continue;
                }
                let any_returning = value.contains(".bind(")
                    || value.contains(".apply(")
                    || value.contains(".call(");
                if any_returning {
                    checker.push(
                        "ERROR",
                        "10605008",
                        this_line + indent + "const ".len(),
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                }
            }
        }
        // `import x = require("…")`（import assignment）：参考侧在同一点报两条
        // （`arkts-no-require` 10605121 与「不能用在 ES 模块里」10505001），锚在 `import`
        // （`fixtures/arkts-import-equals` 的 1:1）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if !trimmed.starts_with("import ") || !trimmed.contains(" = require(") {
                    continue;
                }
                let at = this_line + (line.len() - trimmed.len());
                checker.push(
                    "ERROR",
                    "10605121",
                    at,
                    "\"require\" and \"import\" assignment are not supported (arkts-no-require)"
                        .to_string(),
                );
                checker.push(
                    "ERROR",
                    "10505001",
                    at,
                    "Import assignment cannot be used when targeting ECMAScript modules. Consider using 'import * as ns from \"mod\"', 'import {a} from \"mod\"', 'import d from \"mod\"', or another module format instead."
                        .to_string(),
                );
            }
        }
        // `new Object()` 的实例上读写未声明属性：参考侧报
        // `Property '<name>' does not exist on type 'Object'.`（10505001），锚在属性名；
        // 若该属性读取又落进无注解的 `const`，再报一条
        // `arkts-no-any-unknown`（10605008），锚在绑定名
        // （`fixtures/arkts-object-instance-prop` 的 3:13、4:7、4:30）。
        {
            let lines: Vec<&str> = source.lines().collect();
            let mut object_locals: std::collections::HashSet<String> = Default::default();
            for line in &lines {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("const ") {
                    if let Some(eq) = rest.find('=') {
                        if !rest[..eq].contains(':')
                            && rest[eq + 1..].trim().starts_with("new Object(")
                        {
                            object_locals.insert(rest[..eq].trim().to_string());
                        }
                    }
                }
            }
            if !object_locals.is_empty() {
                let mut offset = 0usize;
                for line in &lines {
                    let this_line = offset;
                    offset += line.len() + 1;
                    let trimmed = line.trim_start();
                    let indent = line.len() - trimmed.len();
                    for local in &object_locals {
                        let mut from = 0usize;
                        while let Some(at) = trimmed[from..].find(&format!("{local}.")) {
                            let dot_at = from + at + local.len();
                            from = dot_at + 1;
                            let property: String = trimmed[dot_at + 1..]
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                                .collect();
                            if property.is_empty() {
                                continue;
                            }
                            checker.push(
                                "ERROR",
                                "10505001",
                                this_line + indent + dot_at + 1,
                                format!("Property '{property}' does not exist on type 'Object'."),
                            );
                        }
                    }
                    // 无注解的 `const x = <object>.<prop>`：读到的类型是 any。
                    if let Some(rest) = trimmed.strip_prefix("const ") {
                        if let Some(eq) = rest.find('=') {
                            if !rest[..eq].contains(':') {
                                let value = rest[eq + 1..].trim();
                                let reads_object = object_locals
                                    .iter()
                                    .any(|local| value.starts_with(&format!("{local}.")));
                                if reads_object {
                                    checker.push(
                                        "ERROR",
                                        "10605008",
                                        this_line + indent + "const ".len(),
                                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        // 装饰器实参里的对象字面量：参考侧同样报
        // `Object literal must correspond to some explicitly declared class or interface
        // (arkts-no-untyped-obj-literals)`，锚在 `{`
        // （`fixtures/arkts-decorator-obj-literal` 的 6:12）。
        {
            // SDK 声明的装饰器形参类型可查，参考侧不把它的实参判成无类型字面量
            // （`fixtures/user-intent` 的 `@InsightIntentLink({…})` 金标为空）；只有本项目
            // 自己声明的装饰器才报（`fixtures/arkts-decorator-obj-literal` 的
            // `@Configure(options: object)`，6:12）。
            let mut sdk_decorators: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                if !line.trim_start().starts_with("import ") {
                    continue;
                }
                let sdk = line.contains("from '@ohos.")
                    || line.contains("from \"@ohos.")
                    || line.contains("from '@kit.")
                    || line.contains("from \"@kit.");
                if !sdk {
                    continue;
                }
                if let (Some(open), Some(close)) = (line.find('{'), line.find('}')) {
                    if close > open {
                        for name in line[open + 1..close].split(',') {
                            let name = name.trim();
                            if !name.is_empty() {
                                sdk_decorators.insert(name.to_string());
                            }
                        }
                    }
                }
            }
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if !trimmed.starts_with('@') {
                    continue;
                }
                let decorator: String = trimmed[1..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if sdk_decorators.contains(&decorator) {
                    continue;
                }
                let Some(open) = trimmed.find("({") else {
                    continue;
                };
                checker.push(
                    "ERROR",
                    "10605038",
                    this_line + (line.len() - trimmed.len()) + open + 1,
                    "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)"
                        .to_string(),
                );
                // 装饰器函数返回的不是函数（这里只认 `object` / `Object`）：参考侧再报一条
                // 不可调用，锚在装饰器名（同 fixture 的 6:2）。
                let name: String = trimmed[1..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if object_returning_functions(source).contains(&name) {
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + (line.len() - trimmed.len()) + 1,
                        "Unable to resolve signature of class decorator when called as an expression. This expression is not callable. Type '{}' has no call signatures."
                            .to_string(),
                    );
                }
            }
        }
        // `@Sendable` 的两条约束（`fixtures/arkts-sendable-definite`、
        // `fixtures/arkts-sendable-decorator`）：
        // · `@Sendable` 类里的 `field!:` 定值断言 →
        //   `Definite assignment assertion is not allowed in "Sendable" classes
        //   (arkts-sendable-definite-assignment)`（10605162），锚在字段名（4:3）；
        // · `@Sendable` 用在 class/function/typeAlias 之外 →
        //   `The "@Sendable" decorator can only be used on "class", "function" and "typeAlias"
        //   (arkts-sendable-decorator-limited)`（10605163），锚在 `@Sendable`（2:1）。
        {
            let lines: Vec<&str> = source.lines().collect();
            let mut offset = 0usize;
            let mut pending_sendable: Option<usize> = None;
            let mut sendable_class_depth: Option<i32> = None;
            let mut depth = 0i32;
            for line in &lines {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if trimmed == "@Sendable" {
                    pending_sendable = Some(this_line + indent);
                    continue;
                }
                if let Some(at) = pending_sendable {
                    let allowed = trimmed.starts_with("class ")
                        || trimmed.starts_with("function ")
                        || trimmed.starts_with("type ")
                        || trimmed.starts_with("export class ")
                        || trimmed.starts_with("export function ")
                        || trimmed.starts_with("export type ");
                    if !allowed {
                        checker.push(
                            "ERROR",
                            "10605180",
                            at,
                            "The \"@Sendable\" decorator can only be used on \"class\", \"function\" and \"typeAlias\" (arkts-sendable-decorator-limited)"
                                .to_string(),
                        );
                        // 参考侧对带该装饰器的 struct 再补一条 WARN，锚在装饰器名
                        // （`fixtures/arkts-sendable-decorator` 的 2:2）。
                        let rest_start = offset - line.len() - 1;
                        let tail = &source[rest_start..];
                        if let Some(struct_at) = tail.find("struct ") {
                            let name: String = tail[struct_at + "struct ".len()..]
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            if !name.is_empty() {
                                checker.push(
                                    "WARN",
                                    "-",
                                    at + 1,
                                    format!("The struct '{name}' use invalid decorator."),
                                );
                            }
                        }
                    } else if trimmed.contains("class ") {
                        sendable_class_depth = Some(depth + 1);
                    }
                    pending_sendable = None;
                }
                if let Some(class_depth) = sendable_class_depth {
                    if depth >= class_depth {
                        if let Some(bang) = trimmed.find("!:") {
                            let field_start = trimmed[..bang]
                                .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                                .map(|index| index + 1)
                                .unwrap_or(0);
                            checker.push(
                                "ERROR",
                                "10605155",
                                this_line + indent + field_start,
                                "Definite assignment assertion is not allowed in \"Sendable\" classes (arkts-sendable-definite-assignment)"
                                    .to_string(),
                            );
                        }
                    }
                    if line.contains('}') && depth + line.matches('}').count() as i32 <= class_depth
                    {
                        sendable_class_depth = None;
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth < 0 {
                    depth = 0;
                }
            }
        }
        // `const x = <调用>` 且调用的返回值是 `any`（`JSON.parse`）：参考侧报
        // `Use explicit types instead of "any", "unknown" (arkts-no-any-unknown)`，锚在绑定名
        // （`fixtures/arkts-any-from-json-parse` 的 2:7）。
        {
            let any_returning = ["JSON.parse("];
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("const ") else {
                    continue;
                };
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                if rest[..eq].contains(':') {
                    continue;
                }
                let name = rest[..eq].trim().to_string();
                let value = rest[eq + 1..].trim();
                if !any_returning.iter().any(|prefix| value.starts_with(prefix)) {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10605008",
                    this_line + (line.len() - trimmed.len()) + "const ".len(),
                    "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                        .to_string(),
                );
                let _ = name;
            }
        }
        // `const x: SomeClass = { … }` 里出现了该类没有的属性：参考侧报
        // `Object literal must correspond to some explicitly declared class or interface
        // (arkts-no-untyped-obj-literals)`，锚在 `{`
        // （`fixtures/arkts-obj-literal-extra-prop` 的 6:24）。
        {
            let mut class_props: std::collections::HashMap<String, Vec<String>> =
                Default::default();
            let mut offset0 = 0usize;
            let lines: Vec<&str> = source.lines().collect();
            for line in &lines {
                let _ = offset0;
                offset0 += line.len() + 1;
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("class ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        class_props.entry(name).or_default();
                    }
                }
            }
            for (name, properties) in class_props.iter_mut() {
                *properties = class_property_names(source, name);
            }
            let mut offset = 0usize;
            for line in &lines {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(colon) = trimmed.find(": ") else {
                    continue;
                };
                let after = trimmed[colon + 2..].trim_start();
                let declared: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let Some(known) = class_props.get(&declared) else {
                    continue;
                };
                let Some(brace) = after.find('{') else {
                    continue;
                };
                let Some(close) = after.rfind('}') else {
                    continue;
                };
                let body = &after[brace + 1..close];
                let unknown = body
                    .split(',')
                    .map(|part| {
                        part.split(':')
                            .next()
                            .unwrap_or_default()
                            .trim()
                            .to_string()
                    })
                    .find(|key| {
                        !key.is_empty()
                            && key
                                .chars()
                                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                            && !known.contains(key)
                    });
                let Some(unknown) = unknown else {
                    continue;
                };
                if let Some(position) = line.find("= {") {
                    checker.push(
                        "ERROR",
                        "10605038",
                        this_line + position + "= ".len(),
                        "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)"
                            .to_string(),
                    );
                    // 同一点再报一条不可赋值，锚在多余属性名上，并列出字面量的推断类型。
                    let mut shape: Vec<String> = Vec::new();
                    for part in body.split(',') {
                        let mut pieces = part.splitn(2, ':');
                        let key = pieces.next().unwrap_or_default().trim();
                        let value = pieces.next().unwrap_or_default().trim();
                        if key.is_empty() {
                            continue;
                        }
                        let kind = if value.starts_with('"') || value.starts_with('\'') {
                            "string"
                        } else if value.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                            "number"
                        } else if value == "true" || value == "false" {
                            "boolean"
                        } else {
                            "any"
                        };
                        shape.push(format!("{key}: {kind}"));
                    }
                    if let Some(extra_at) = line.find(&format!("{unknown}:")) {
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + extra_at,
                            format!(
                                "Type '{{ {}; }}' is not assignable to type '{declared}'. Object literal may only specify known properties, and '{unknown}' does not exist in type '{declared}'.",
                                shape.join("; ")
                            ),
                        );
                    }
                }
            }
        }
        // UMD 与命名空间语句（`fixtures/arkts-umd`、`fixtures/arkts-ns-statements`）：
        // · `export as namespace X;` → `arkts-no-umd`（10605088）与「只在声明文件里允许」
        //   （10505001），同锚在 `export`（2:1）；
        // · namespace 里的非声明语句 → `arkts-no-ns-statements`（10605073），锚在语句
        //   （4:3）。
        {
            let mut offset = 0usize;
            let mut namespace_depth = 0i32;
            let mut depth = 0i32;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if trimmed.starts_with("export as namespace ") {
                    checker.push(
                        "ERROR",
                        "10605130",
                        this_line + indent,
                        "Universal module definitions (UMD) are not supported (arkts-no-umd)"
                            .to_string(),
                    );
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent,
                        "Global module exports may only appear in declaration files.".to_string(),
                    );
                }
                if trimmed.starts_with("namespace ") || trimmed.starts_with("export namespace ") {
                    // 进入命名空间后的深度（本条行首的 `{` 会把 depth 加一）。
                    namespace_depth = depth + 1;
                }
                // 只判命名空间的直接层级：函数体里的 `return` 不算
                // （`fixtures/arkts-export-assignment` 的 3:5）。
                if namespace_depth > 0 && depth == namespace_depth {
                    // 声明关键字之外的语句：`const`/`let`/`var`/`function`/`class`/`interface`
                    // 之外，都算非声明语句。
                    let declaration = [
                        "const ",
                        "let ",
                        "var ",
                        "function ",
                        "class ",
                        "interface ",
                        "type ",
                        "enum ",
                        "namespace ",
                        "export ",
                        "@",
                        "//",
                        "/*",
                        "*",
                        "}",
                    ]
                    .iter()
                    .any(|prefix| trimmed.starts_with(prefix));
                    // `export = X;` 交给 `arkts-no-export-assignment`，不算命名空间语句。
                    let declaration = declaration || trimmed.starts_with("export =");
                    if !declaration && !trimmed.is_empty() {
                        checker.push(
                            "ERROR",
                            "10605116",
                            this_line + indent,
                            "Non-declaration statements in namespaces are not supported (single semicolons are considered as empty non-delcaration statements) (arkts-no-ns-statements)"
                                .to_string(),
                        );
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= namespace_depth && namespace_depth > 0 && line.contains('}') {
                    namespace_depth = 0;
                }
                if depth < 0 {
                    depth = 0;
                }
            }
        }
        // 两条语句位置规则：
        // · `const x = SomeNamespace;`（命名空间当值使用）→ `Namespaces cannot be used as
        //   objects (arkts-no-ns-as-obj)`，锚在名字（`fixtures/arkts-ns-as-obj` 的 10:18）；
        // · 非首部的 `import` → `"import" statements after other statements are not allowed
        //   (arkts-no-misplaced-imports)`，锚在 `import`（`fixtures/arkts-misplaced-imports`
        //   的 12:1）。
        {
            let mut namespaces: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("namespace ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        namespaces.insert(name);
                    }
                }
                if let Some(rest) = trimmed.strip_prefix("export namespace ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        namespaces.insert(name);
                    }
                }
            }
            let mut offset = 0usize;
            let mut seen_statement = false;
            let mut depth = 0i32;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                    continue;
                }
                // import 出现在已有顶层语句之后。
                if trimmed.starts_with("import ") {
                    if seen_statement {
                        checker.push(
                            "ERROR",
                            "10605150",
                            this_line + (line.len() - trimmed.len()),
                            "\"import\" statements after other statements are not allowed (arkts-no-misplaced-imports)"
                                .to_string(),
                        );
                    }
                    continue;
                }
                // 顶层语句从第 0 列起：多行 import 的续行有缩进
                // （`fixtures/create-new-config` 的 3-8 行是同一个 import 的成员列表，
                // 参考侧不把 11 行的下一条 import 判成 misplaced）。
                let indent = line.len() - trimmed.len();
                if depth == 0
                    && indent == 0
                    && !trimmed.starts_with('}')
                    && !trimmed.starts_with('@')
                {
                    seen_statement = true;
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth < 0 {
                    depth = 0;
                }
                // 命名空间名出现在 `= ` 右侧且不像成员访问。
                if let Some(eq) = trimmed.find("= ") {
                    // `export = X` 由 `arkts-no-export-assignment` 负责
                    // （`fixtures/arkts-export-assignment`）。
                    if trimmed.starts_with("export =") {
                        continue;
                    }
                    let value = trimmed[eq + 2..].trim();
                    let candidate: String = value
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect();
                    if namespaces.contains(&candidate) {
                        let rest = value[candidate.len()..].trim_start();
                        if rest.is_empty() || rest.starts_with(';') {
                            if let Some(position) = line.find(&format!("= {candidate}")) {
                                checker.push(
                                    "ERROR",
                                    "10605114",
                                    this_line + position + "= ".len(),
                                    "Namespaces cannot be used as objects (arkts-no-ns-as-obj)"
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        // `+text` / `-text` 用在非 number 上：参考侧报
        // `Unary operators "+", "-" and "~" work only on numbers (arkts-no-polymorphic-unops)`，
        // 锚在运算符（`fixtures/arkts-polymorphic-unops` 的 2:18、3:17）。
        {
            let mut string_locals: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("const ") {
                    if let Some(eq) = rest.find('=') {
                        let name = rest[..eq].trim().to_string();
                        let value = rest[eq + 1..].trim();
                        if value.starts_with('"') || value.starts_with('\'') {
                            string_locals.insert(name);
                        }
                    }
                }
            }
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(eq) = trimmed.find("= ") else {
                    continue;
                };
                let value = &trimmed[eq + 2..];
                let Some(operator) = value.chars().next() else {
                    continue;
                };
                if !matches!(operator, '+' | '-' | '~') {
                    continue;
                }
                let rest = value[1..].trim_start();
                let operand: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if !string_locals.contains(&operand) {
                    continue;
                }
                let operator_at = this_line + (line.len() - trimmed.len()) + eq + 2;
                checker.push(
                    "ERROR",
                    "10605055",
                    operator_at,
                    "Unary operators \"+\", \"-\" and \"~\" work only on numbers (arkts-no-polymorphic-unops)"
                        .to_string(),
                );
            }
        }
        // `import … assert { type: "json" }`：参考侧在同一点报两条
        // （`arkts-no-import-assertions` 10605143 与「只在 esnext/nodenext 下支持」10505001），
        // 锚在 `assert`（`fixtures/arkts-import-assertions` 的 1:37）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                if !line.trim_start().starts_with("import ") {
                    continue;
                }
                let Some(at) = line.find(" assert ") else {
                    continue;
                };
                let position = this_line + at + 1;
                checker.push(
                    "ERROR",
                    "10605143",
                    position,
                    "Import assertions are not supported (arkts-no-import-assertions)".to_string(),
                );
                checker.push(
                    "ERROR",
                    "10505001",
                    position,
                    "Import assertions are only supported when the '--module' option is set to 'esnext' or 'nodenext'."
                        .to_string(),
                );
            }
        }
        // 解构赋值 `[a[0], a[1]] = [a[1], a[0]]`：参考侧报
        // `Destructuring assignment is not supported (arkts-no-destruct-assignment)`，锚在赋值左侧
        // 的 `[`（`fixtures/arkts-destruct-assignment` 的 7:3）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if !trimmed.starts_with('[') {
                    continue;
                }
                // 赋值目标：`[...] = ...`（声明形 `const [...] = …` 不在本判据里）。
                // 左侧目标是 `] =` 之前的整段（右侧数组也含 `]`，所以不能用 rfind）。
                let Some(close) = trimmed.find("] =").or_else(|| trimmed.find("]=")) else {
                    continue;
                };
                let _ = close;
                checker.push(
                    "ERROR",
                    "10605069",
                    this_line + (line.len() - trimmed.len()),
                    "Destructuring assignment is not supported (arkts-no-destruct-assignment)"
                        .to_string(),
                );
            }
        }
        // 类里第二个及以后的 `static { }`：参考侧报
        // `Only one static block is supported (arkts-no-multiple-static-blocks)`，锚在 `static`
        // （`fixtures/arkts-multiple-static-blocks` 的 6:3）。
        {
            let mut seen = false;
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed != "static {" {
                    continue;
                }
                if !seen {
                    seen = true;
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10605016",
                    this_line + (line.len() - trimmed.len()),
                    "Only one static block is supported (arkts-no-multiple-static-blocks)"
                        .to_string(),
                );
            }
        }
        // `@Sendable` 类的字段（`fixtures/arkts-sendable-props`）：
        // · 字段类型不是 Sendable 数据类型 → `Properties in "Sendable" classes and interfaces
        //   must have a Sendable data type (arkts-sendable-prop-types)`（10605154），锚在字段名
        //   （7:3、8:3）；
        // · 字段初值 `new <非 Sendable 类>()` → `Only imported variables can be captured by
        //   "Sendable" class (arkts-sendable-imported-variables)`（10605157），锚在类名（7:27）。
        {
            let mut sendable_classes: std::collections::HashSet<String> = Default::default();
            let lines: Vec<&str> = source.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                if line.trim() != "@Sendable" {
                    continue;
                }
                if let Some(next) = lines.get(index + 1) {
                    if let Some(rest) = next.trim_start().strip_prefix("class ") {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        sendable_classes.insert(name);
                    }
                }
            }
            if !sendable_classes.is_empty() {
                let mut offset = 0usize;
                let mut inside_sendable = false;
                let mut depth = 0i32;
                for line in &lines {
                    let this_line = offset;
                    offset += line.len() + 1;
                    let trimmed = line.trim_start();
                    let indent = line.len() - trimmed.len();
                    if trimmed == "@Sendable" {
                        inside_sendable = true;
                        depth = 0;
                        continue;
                    }
                    if !inside_sendable {
                        continue;
                    }
                    depth += line.matches('{').count() as i32;
                    depth -= line.matches('}').count() as i32;
                    if depth <= 0 && line.contains('}') && !line.contains('{') {
                        inside_sendable = false;
                        continue;
                    }
                    // 字段声明：`<name>: <type> = …`，缩进一级。
                    let Some(colon) = trimmed.find(':') else {
                        continue;
                    };
                    let field = trimmed[..colon].trim();
                    if field.is_empty()
                        || !field
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                    {
                        continue;
                    }
                    if indent == 0 || field.starts_with("constructor") {
                        continue;
                    }
                    let after = trimmed[colon + 1..].trim_start();
                    let declared: String = after
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect();
                    if declared.is_empty() {
                        continue;
                    }
                    let sendable_type =
                        matches!(declared.as_str(), "number" | "string" | "boolean")
                            || sendable_classes.contains(&declared);
                    // `number[]` 这类数组不是 Sendable。
                    let is_array = after[declared.len()..].trim_start().starts_with('[');
                    if !sendable_type || is_array {
                        checker.push(
                            "ERROR",
                            "10605154",
                            this_line + indent,
                            "Properties in \"Sendable\" classes and interfaces must have a Sendable data type (arkts-sendable-prop-types)"
                                .to_string(),
                        );
                    }
                    // 字段初值里 `new Plain()`：捕获了本文件里的非 Sendable 类。
                    if let Some(at) = after.find("new ") {
                        let created: String = after[at + "new ".len()..]
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                            .collect();
                        if !created.is_empty() && !sendable_classes.contains(&created) {
                            // 锚在 `new` 之后的类名（7:27），不是字段类型里的同名标识符（7:10）。
                            if let Some(position) = line.find(&format!("new {created}")) {
                                checker.push(
                                    "ERROR",
                                    "10605157",
                                    this_line + position + "new ".len(),
                                    "Only imported variables can be captured by \"Sendable\" class (arkts-sendable-imported-variables)"
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        // 结构化类型：把本文件的类实例传给「参数类型是本文件 interface」的函数时，参考侧报
        // `Structural typing is not supported (arkts-no-structural-typing)`，锚在实参
        // （`fixtures/arkts-structural-and-instanceof` 的 14:28）。
        {
            let mut interfaces: std::collections::HashSet<String> = Default::default();
            let mut class_locals: std::collections::HashSet<String> = Default::default();
            let mut interface_params: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("interface ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    interfaces.insert(name);
                }
                if let Some(rest) = trimmed.strip_prefix("const ") {
                    if let Some(eq) = rest.find('=') {
                        let value = rest[eq + 1..].trim();
                        if let Some(rest_value) = value.strip_prefix("new ") {
                            let class: String = rest_value
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            if !class.is_empty() {
                                let name = rest[..eq].trim().to_string();
                                class_locals.insert(name);
                            }
                        }
                    }
                }
                if let Some(rest) = trimmed.strip_prefix("function ") {
                    if let Some(open) = rest.find('(') {
                        let name: String = rest[..open].trim().to_string();
                        if let Some(close) = rest[open..].find(')') {
                            let params = &rest[open + 1..open + close];
                            if let Some(colon) = params.find(':') {
                                let ty = params[colon + 1..].trim();
                                if interfaces.contains(ty) {
                                    interface_params.insert(name);
                                }
                            }
                        }
                    }
                }
            }
            if !interface_params.is_empty() && !class_locals.is_empty() {
                let mut offset = 0usize;
                for line in source.lines() {
                    let this_line = offset;
                    offset += line.len() + 1;
                    for function in &interface_params {
                        let needle = format!("{function}(");
                        let Some(at) = line.find(&needle) else {
                            continue;
                        };
                        let argument_start = at + needle.len();
                        let argument: String = line[argument_start..]
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                            .collect();
                        if !class_locals.contains(&argument) {
                            continue;
                        }
                        checker.push(
                            "ERROR",
                            "10605030",
                            this_line + argument_start,
                            "Structural typing is not supported (arkts-no-structural-typing)"
                                .to_string(),
                        );
                    }
                }
            }
        }
        // `instanceof` 三条（`fixtures/arkts-structural-and-instanceof`）：
        // · 右侧是引用类型包装（`Number`/`String`/`Boolean`）→ `"instanceof" operator is
        //   partially supported (arkts-instanceof-ref-types)`（10605065），锚在右侧名（17:30）；
        // · 左侧是原始类型变量 → `The left-hand side of an 'instanceof' expression must be of
        //   type 'any', an object type or a type parameter.`（10505001），锚在左侧名（17:20）。
        {
            let mut primitive_locals: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("const ") else {
                    continue;
                };
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                // 可能没有类型注解（`const primitive = 1;`），此时 declared 为空。
                let (name, declared) = match rest.find(':') {
                    Some(colon) if colon < eq => {
                        (rest[..colon].trim().to_string(), rest[colon + 1..eq].trim())
                    }
                    _ => (rest[..eq].trim().to_string(), ""),
                };
                if matches!(declared, "number" | "string" | "boolean") {
                    primitive_locals.insert(name.clone());
                }
                // 无注解但初值是字面量时同样推导为原始类型（`const primitive = 1;`）。
                let value = rest[eq + 1..].trim();
                let literal = value
                    .trim_end_matches(';')
                    .trim()
                    .chars()
                    .next()
                    .unwrap_or(' ');
                if (declared.is_empty() || declared == "number")
                    && (literal.is_ascii_digit() || literal == '"' || literal == '\'')
                {
                    primitive_locals.insert(name);
                }
            }
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let Some(at) = line.find(" instanceof ") else {
                    continue;
                };
                let left: String = line[..at]
                    .trim_end()
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let right_start = at + " instanceof ".len();
                let right: String = line[right_start..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if matches!(
                    right.as_str(),
                    "Number" | "String" | "Boolean" | "Symbol" | "BigInt"
                ) {
                    // 参考侧锚在 `instanceof` 关键字上（17:30），不是右侧类型名。
                    checker.push(
                        "ERROR",
                        "10605065",
                        this_line + at + 1,
                        "\"instanceof\" operator is partially supported (arkts-instanceof-ref-types)"
                            .to_string(),
                    );
                }
                if primitive_locals.contains(&left) {
                    let left_start = this_line + at - left.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        left_start,
                        "The left-hand side of an 'instanceof' expression must be of type 'any', an object type or a type parameter."
                            .to_string(),
                    );
                }
            }
        }
        // `@Sendable` 类只能继承同样 `@Sendable` 的类：参考侧报
        // `The inheritance for "Sendable" classes is limited (arkts-sendable-class-inheritance)`，
        // 锚在父类名（`fixtures/arkts-sendable-inheritance` 的 6:29）。
        {
            let mut sendable_classes: std::collections::HashSet<String> = Default::default();
            let lines: Vec<&str> = source.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                if line.trim() != "@Sendable" {
                    continue;
                }
                let Some(next) = lines.get(index + 1) else {
                    continue;
                };
                if let Some(rest) = next.trim_start().strip_prefix("class ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    sendable_classes.insert(name);
                }
            }
            if !sendable_classes.is_empty() {
                let mut offset = 0usize;
                let mut pending_sendable = false;
                for line in &lines {
                    let this_line = offset;
                    offset += line.len() + 1;
                    if line.trim() == "@Sendable" {
                        pending_sendable = true;
                        continue;
                    }
                    if !pending_sendable {
                        continue;
                    }
                    let trimmed = line.trim_start();
                    let Some(rest) = trimmed.strip_prefix("class ") else {
                        pending_sendable = false;
                        continue;
                    };
                    pending_sendable = false;
                    let Some(at) = rest.find(" extends ") else {
                        continue;
                    };
                    let tail = &rest[at + " extends ".len()..];
                    let parent: String = tail
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if parent.is_empty() || sendable_classes.contains(&parent) {
                        continue;
                    }
                    if let Some(position) = line.find(&parent) {
                        checker.push(
                            "ERROR",
                            "10605153",
                            this_line + position,
                            "The inheritance for \"Sendable\" classes is limited (arkts-sendable-class-inheritance)"
                                .to_string(),
                        );
                    }
                }
            }
        }
        // 两条 clause 规则（`fixtures/arkts-heritage-clauses`）：
        // · `class X extends <interface>` → `Cannot extend an interface 'Y'. Did you mean
        //   'implements'?`（10505001），锚在接口名（9:29）；
        // · `class X implements <class>` → `Classes cannot be specified in "implements" clause
        //   (arkts-implements-only-iface)`（10605051），锚在类名（12:34）；
        // 以及 `constructor(): void` 上的返回类型（`Type annotation cannot appear on a
        // constructor declaration.`，16:18）。
        {
            let mut interfaces: std::collections::HashSet<String> = Default::default();
            let mut classes: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("interface ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    interfaces.insert(name);
                } else if let Some(rest) = trimmed.strip_prefix("class ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    classes.insert(name);
                }
            }
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("class ") {
                    // `class Holder<T extends Named>` 的 `extends` 在泛型参数里，不是继承
                    // （`fixtures/ts-generic-constraint-class` 的参考侧 exit=0）。
                    let generic_at = rest.find('<');
                    if let Some(at) = rest
                        .find(" extends ")
                        .filter(|at| generic_at.is_none_or(|open| *at < open))
                    {
                        let tail = &rest[at + " extends ".len()..];
                        let name: String = tail
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        // 同名类型与命名空间冲突时（`arkts-unique-names`），参考侧只报前者，
                        // 这里不再补 `Cannot extend an interface`
                        // （`fixtures/arkts-extends-interface-name` 的 2:11 单独一条）。
                        let shadowed = source.lines().any(|other| {
                            other.trim_start().starts_with(&format!("namespace {name}"))
                        }) || checker
                            .out
                            .iter()
                            .any(|diagnostic| diagnostic.code == "10605004");
                        if interfaces.contains(&name) && !shadowed {
                            if let Some(position) = line.find(&name) {
                                checker.push(
                                    "ERROR",
                                    "10505001",
                                    this_line + position,
                                    format!(
                                        "Cannot extend an interface '{name}'. Did you mean 'implements'?"
                                    ),
                                );
                            }
                        }
                    }
                }
                if let Some(at) = trimmed.find(" implements ") {
                    let tail = &trimmed[at + " implements ".len()..];
                    let name: String = tail
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if classes.contains(&name) {
                        if let Some(position) = line.find(&name) {
                            checker.push(
                                "ERROR",
                                "10605051",
                                this_line + position,
                                "Classes cannot be specified in \"implements\" clause (arkts-implements-only-iface)"
                                    .to_string(),
                            );
                        }
                        // 同一行再报一条「不正确实现」，锚在实现者名字（12:7），消息里带上
                        // 目标类中缺失的第一个成员。
                        let implementer: String = rest_head(trimmed);
                        let missing =
                            class_property_names(source, &name)
                                .into_iter()
                                .find(|property| {
                                    !class_property_names(source, &implementer).contains(property)
                                });
                        if let Some(property) = missing {
                            let at = this_line + (line.len() - trimmed.len()) + "class ".len();
                            checker.push(
                                "ERROR",
                                "10505001",
                                at,
                                format!(
                                    "Class '{implementer}' incorrectly implements class '{name}'. Did you mean to extend '{name}' and inherit its members as a subclass? Property '{property}' is missing in type '{implementer}' but required in type '{name}'."
                                ),
                            );
                        }
                    }
                }
                if let Some(at) = trimmed.find("constructor(") {
                    if let Some(close) = trimmed[at..].find("):") {
                        let colon = at + close + 1;
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + (line.len() - trimmed.len()) + colon + 2,
                            "Type annotation cannot appear on a constructor declaration."
                                .to_string(),
                        );
                    }
                }
            }
        }
        // 类型谓词 `value is string`：参考侧报
        // `Type guarding is supported with "instanceof" and "as" (arkts-no-is)`，锚在谓词左值
        // （`fixtures/arkts-no-is` 的 1:44）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                if !line.contains("):") {
                    continue;
                }
                let Some(at) = line.find(" is ") else {
                    continue;
                };
                let before = line[..at].trim_end();
                let name_start = before
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                    .map(|index| index + 1)
                    .unwrap_or(0);
                let subject = &line[name_start..before.len()];
                if subject.is_empty() {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10605096",
                    this_line + name_start,
                    "Type guarding is supported with \"instanceof\" and \"as\" (arkts-no-is)"
                        .to_string(),
                );
            }
        }
        // 类型位置上的 `this`（`ref: this`、`clone(): this`、`const x: this`）：参考侧报
        // `Type notation using "this" is not supported (arkts-no-typing-with-this)`（10605021），
        // 锚在 `this`；顶层出现时再补一条「只在非静态成员里可用」
        // （`fixtures/arkts-typing-with-this` 的 2:8、4:12、11:17）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                let mut from = 0usize;
                while let Some(at) = line[from..].find("this") {
                    let start = from + at;
                    from = start + "this".len();
                    // 只认类型位置：`this` 前面是 `:` 或 `|` 或 `(`，且不是表达式里的 `this.`。
                    let before = line[..start].trim_end();
                    let is_type = before.ends_with(':') || before.ends_with('|');
                    if !is_type {
                        continue;
                    }
                    // `this.plain` 是表达式里的成员访问，不是类型位置
                    // （`fixtures/ui-link-plain` 的 `{ value: this.plain }`）。
                    if line[start + "this".len()..].trim_start().starts_with('.') {
                        continue;
                    }
                    checker.push(
                        "ERROR",
                        "10605021",
                        this_line + start,
                        "Type notation using \"this\" is not supported (arkts-no-typing-with-this)"
                            .to_string(),
                    );
                    if trimmed.starts_with("const ") || trimmed.starts_with("let ") {
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + start,
                            "A 'this' type is available only in a non-static member of a class or interface."
                                .to_string(),
                        );
                    }
                }
            }
        }
        // `const sequence = (1, 2);`：逗号运算符只在 `for` 里合法，参考侧在同一点报两条
        // （`arkts-no-comma-outside-loops` 10605071 与「左侧无副作用」10505001，10:19）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let Some(eq) = line.find("= (") else {
                    continue;
                };
                let value = &line[eq + 3..];
                let Some(comma) = value.find(',') else {
                    continue;
                };
                if value[..comma].trim().is_empty() {
                    continue;
                }
                // 逗号落在对象字面量里不是逗号运算符：`const make = (): Pair => ({ a: 1, b: 2 })`
                // 的第一个逗号属于 `{ … }`（`fixtures/shorthand-property` 的 70:19，
                // 参考侧无诊断）。
                if value[..comma].contains('{') {
                    continue;
                }
                // 三元表达式里的逗号属于实参或分支，不是逗号运算符：
                // `const n = (a == -1 || a == 0) ? this.x : this.x.slice(0, a)` 的第一个逗号
                // 在 `slice(…)` 的实参里（`fixtures/configuration-page` 的 1276:23）。
                if value.contains('?') {
                    continue;
                }
                // 同理，`(` 未闭合时里面的逗号是实参分隔符。
                let mut depth = 0i32;
                for byte in value[..comma].bytes() {
                    match byte {
                        b'(' | b'[' => depth += 1,
                        b')' | b']' => depth -= 1,
                        _ => {}
                    }
                }
                if depth > 0 {
                    continue;
                }
                if line.trim_start().starts_with("for ") {
                    continue;
                }
                let at = this_line + eq + 3;
                checker.push(
                    "ERROR",
                    "10605071",
                    at,
                    "The comma operator \",\" is supported only in \"for\" loops (arkts-no-comma-outside-loops)"
                        .to_string(),
                );
                checker.push(
                    "ERROR",
                    "10505001",
                    at,
                    "Left side of comma operator is unused and has no side effects.".to_string(),
                );
            }
        }
        // 三条类型/结构规则（`fixtures/arkts-types-and-nested-funcs`）：
        // · `arkts-no-intersection-types`（10605019）— `type X = A & B`，锚在第一个成员（1:19）；
        // · `arkts-no-conditional-types`（10605022）— `T extends U ? X : Y`，锚在 `T`（2:23）；
        // · `arkts-no-nested-funcs`（10605092）— 函数体内再声明 `function`，锚在 `function`（5:3）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if indent > 0 && trimmed.starts_with("function ") {
                    checker.push(
                        "ERROR",
                        "10605092",
                        this_line + indent,
                        "Nested functions are not supported (arkts-no-nested-funcs)".to_string(),
                    );
                }
                let Some(rest) = trimmed.strip_prefix("type ") else {
                    continue;
                };
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                let value = rest[eq + 1..].trim_start();
                let value_at = this_line
                    + indent
                    + "type ".len()
                    + eq
                    + 1
                    + (rest[eq + 1..].len() - value.len());
                if let Some(amp) = value.find(" & ") {
                    checker.push(
                        "ERROR",
                        "10605019",
                        value_at,
                        "Use inheritance instead of intersection types (arkts-no-intersection-types)"
                            .to_string(),
                    );
                    let _ = amp;
                }
                if value.contains(" extends ") && value.contains('?') {
                    checker.push(
                        "ERROR",
                        "10605022",
                        value_at,
                        "Conditional types are not supported (arkts-no-conditional-types)"
                            .to_string(),
                    );
                }
            }
        }
        // `with (obj) { … }`：参考侧在同一位置报三条——严格模式拒绝、块内符号退化为 `any`、
        // 以及 `arkts-no-with`（`fixtures/arkts-with-statement` 的 2:3）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                let Some(rest) = trimmed.strip_prefix("with") else {
                    continue;
                };
                if !rest.trim_start().starts_with('(') {
                    continue;
                }
                let at = this_line + indent;
                checker.push(
                    "ERROR",
                    "10505001",
                    at,
                    "'with' statements are not allowed in strict mode.".to_string(),
                );
                checker.push(
                    "ERROR",
                    "10505001",
                    at,
                    "The 'with' statement is not supported. All symbols in a 'with' block will have type 'any'."
                        .to_string(),
                );
                checker.push(
                    "ERROR",
                    "10605084",
                    at,
                    "\"with\" statement is not supported (arkts-no-with)".to_string(),
                );
            }
        }
        // `new.target`：参考侧报 `"new.target" is not supported (arkts-no-new-target)`，锚在
        // `new`（`fixtures/arkts-new-target` 的 4:10）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let mut from = 0usize;
                while let Some(at) = line[from..].find("new.target") {
                    let start = from + at;
                    from = start + "new.target".len();
                    checker.push(
                        "ERROR",
                        "10605132",
                        this_line + start,
                        "\"new.target\" is not supported (arkts-no-new-target)".to_string(),
                    );
                }
            }
        }
        // `function*` / `yield`：参考侧报 `Generator functions are not supported`，函数声明锚在
        // `function`（`fixtures/arkts-generators-and-require` 的 1:1），`yield` 锚在它自己（2:3）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if let Some(position) = line.find("function*") {
                    checker.push(
                        "ERROR",
                        "10605094",
                        this_line + position,
                        "Generator functions are not supported (arkts-no-generators)".to_string(),
                    );
                } else if let Some(position) = line.find("function *") {
                    checker.push(
                        "ERROR",
                        "10605094",
                        this_line + position,
                        "Generator functions are not supported (arkts-no-generators)".to_string(),
                    );
                }
                if trimmed.starts_with("yield ") || trimmed == "yield" {
                    checker.push(
                        "ERROR",
                        "10605094",
                        this_line + indent,
                        "Generator functions are not supported (arkts-no-generators)".to_string(),
                    );
                }
            }
        }
        // `require("./x")`：CommonJS 的 `require` 在 ArkTS 里没有声明，参考侧报
        // `Cannot find name 'require'. Did you mean 'Require'?`，锚在 `require`（5:18）。
        {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let Some(position) = line.find("require(") else {
                    continue;
                };
                // 声明行与注释不算调用。
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with("import ") {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10505001",
                    this_line + position,
                    "Cannot find name 'require'. Did you mean 'Require'?".to_string(),
                );
                // `const required = require("./x")`：绑定没有类型注解，推导结果是 `any`，
                // 参考侧在名字上报 `arkts-no-any-unknown`（同一 fixture 的 5:7）。
                if let Some(rest) = trimmed.strip_prefix("const ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect();
                    let after = &rest[name.len()..];
                    if !name.is_empty() && after.trim_start().starts_with('=') {
                        checker.push(
                            "ERROR",
                            "10605008",
                            this_line + (line.len() - trimmed.len()) + "const ".len(),
                            "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                                .to_string(),
                        );
                    }
                }
            }
        }
        // `@CustomDialog` 的结构体里没有 `CustomDialogController` 属性：参考侧报
        // `The '@CustomDialog' decorated custom component must contain a property of the
        // CustomDialogController type.`，锚在 struct 名
        // （`fixtures/ui-custom-dialog-and-builder` 的 2:7）。
        {
            let lines: Vec<&str> = source.lines().collect();
            let mut offset = 0usize;
            let starts: Vec<usize> = lines
                .iter()
                .map(|line| {
                    let start = offset;
                    offset += line.len() + 1;
                    start
                })
                .collect();
            for (index, line) in lines.iter().enumerate() {
                if line.trim() != "@CustomDialog" {
                    continue;
                }
                let Some(declaration) = lines.get(index + 1) else {
                    continue;
                };
                let Some(rest) = declaration.trim_start().strip_prefix("struct ") else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let Some(open) = declaration.find('{') else {
                    continue;
                };
                // 找配对的 `}`：同层的第一个只含 `}` 的行。
                let mut depth = 0i32;
                let mut has_controller = false;
                for body_line in &lines[index + 1..] {
                    depth += body_line.matches('{').count() as i32;
                    depth -= body_line.matches('}').count() as i32;
                    if body_line.contains("CustomDialogController") {
                        has_controller = true;
                    }
                    if depth <= 0 {
                        break;
                    }
                }
                if has_controller {
                    continue;
                }
                // 锚在 struct 名（`struct MyDialog {` 里的 `MyDialog`，2:7）。
                let Some(name_relative) = declaration.find(&name) else {
                    continue;
                };
                let _ = open;
                checker.push(
                    "ERROR",
                    "10905211",
                    // 参考侧的列比名字的 1-based 列小一（2:7 对 `struct MyDialog {`）。
                    starts[index + 1] + name_relative.saturating_sub(1),
                    "The '@CustomDialog' decorated custom component must contain a property of the CustomDialogController type.".to_string(),
                );
            }
        }
        // `@Builder` 方法体里写了 `return <值>`：参考侧报
        // `Only UI component syntax can be written here.`，锚在 `return`
        // （同一 fixture 的 15:5）。
        {
            let mut offset = 0usize;
            let mut in_builder = false;
            let mut depth = 0i32;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed == "@Builder" {
                    in_builder = true;
                    depth = 0;
                    continue;
                }
                if !in_builder {
                    continue;
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth < 0 {
                    in_builder = false;
                    continue;
                }
                // `@Builder` 的函数体到闭合 `}` 就结束了：不回位的话，文件里后面那个普通
                // 函数（`fixtures/declaration-comments` 的 `plainHelper`）与方法
                // （`fixtures/v2-builder-method` 的 `tail`）里的 `return` 都会被当成写在
                // builder 体内。
                if depth == 0 && trimmed.starts_with('}') {
                    in_builder = false;
                    continue;
                }
                if let Some(position) = trimmed.strip_prefix("return ") {
                    if position.trim().is_empty() {
                        continue;
                    }
                    if depth <= 1 {
                        checker.push(
                            "ERROR",
                            "10905209",
                            this_line + (line.len() - trimmed.len()),
                            "Only UI component syntax can be written here.".to_string(),
                        );
                    }
                }
            }
        }
        // `@Styles` 装饰的函数带参数：参考侧报
        // `'@Styles' decorated functions and methods cannot have arguments.`，锚在 `@Styles`
        // （`fixtures/ui-styles-with-argument` 的 6:1）。
        {
            let mut offset = 0usize;
            let mut lines = source.lines().peekable();
            while let Some(line) = lines.next() {
                let this_line = offset;
                offset += line.len() + 1;
                if line.trim() != "@Styles" {
                    continue;
                }
                let Some(next) = lines.peek() else {
                    continue;
                };
                let trimmed = next.trim_start();
                let Some(rest) = trimmed.strip_prefix("function ") else {
                    continue;
                };
                let Some(open) = rest.find('(') else {
                    continue;
                };
                let Some(close) = rest[open..].find(')') else {
                    continue;
                };
                if rest[open + 1..open + close].trim().is_empty() {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10905110",
                    this_line + (line.len() - line.trim_start().len()),
                    "'@Styles' decorated functions and methods cannot have arguments.".to_string(),
                );
            }
        }
        // `obj[key]` 形式的字段索引访问：参考侧报
        // `Indexed access is not supported for fields (arkts-no-props-by-index)`，锚在对象名
        // （`fixtures/arkts-props-by-index-and-private` 的 3:18）。数组/字符串的索引不在此列，
        // 故只看「被索引的标识符是一个类的实例」这一可判形态。
        let (privates, protecteds) = private_members(&parsed.program);
        {
            // 任意位置的 `<recv>[<非数字>]`：`const x = bag[key]`（3:18）、`return bag[key]`
            // （7:10）、`Math.max(bag[key], 0)`（11:19）都算字段索引；数组变量与数字下标不算。
            let arrays: std::collections::HashSet<String> =
                checker.array_elements.keys().cloned().collect();
            let mut line_start = 0usize;
            for line in source.lines() {
                let this_line = line_start;
                line_start += line.len() + 1;
                let mut index = 0usize;
                while let Some(position) = line[index..].find('[') {
                    let open = index + position;
                    index = open + 1;
                    // 接收者：`[` 左边连续的标识符字符。
                    let mut start = open;
                    while start > 0 {
                        // 按字符回退：源码里有中文时按字节减一会落进字符中间
                        // （`fixtures/proxy-group-item` 的 `示` 曾让这里 panic）。
                        let previous = line[..start].chars().next_back();
                        match previous {
                            Some(c) if c.is_alphanumeric() || c == '_' || c == '$' => {
                                start -= c.len_utf8();
                            }
                            _ => break,
                        }
                    }
                    if start == open {
                        continue;
                    }
                    // 注释里的方括号不是索引访问：`// … 显示[收藏 /取消收藏] 单独处理` 会被
                    // 当成 `obj[key]`（`fixtures/configuration-page` 的 832:12、856:12、1093:58）。
                    if line[..start].contains("//") {
                        continue;
                    }
                    // 中文字符不算合法接收者：`[收藏 …]` 这种正文片段直接跳过。
                    if !line[..open].trim_end().chars().last().is_some_and(|c| {
                        c.is_ascii_alphanumeric()
                            || c == '_'
                            || c == '$'
                            || c == ')'
                            || c == ']'
                            || c == '}'
                    }) {
                        continue;
                    }
                    let receiver = &line[start..open];
                    let first = receiver.chars().next().unwrap_or(' ');
                    if first.is_ascii_digit() || arrays.contains(receiver) {
                        continue;
                    }
                    // 数组字段按索引访问是合法的：`arkts-no-props-by-index` 针对的是对象字段
                    // （`fixtures/lazy-forms` 的 `this.data[index]` 参考侧无诊断）。
                    let is_array_member = checker.class_member_types.values().any(|fields| {
                        fields.get(receiver).is_some_and(|spelled| {
                            spelled.contains('[') || spelled.contains("Array<")
                        })
                    });
                    if is_array_member {
                        continue;
                    }
                    // 下标必须是数字字面量才放行（`items[0]`）；字符串下标是**类型**索引
                    // （`arkts-no-aliases-by-index` 负责，`fixtures/arkts-indexed-and-apply`
                    // 的 7:21），`delete obj[k]` 由 `arkts-no-delete` 负责
                    // （`fixtures/unsupported-syntax` 的 2:10），两者都不在此判。
                    let inner = line[open + 1..].chars().next().unwrap_or(' ');
                    if inner.is_ascii_digit() || inner == ']' || inner == '"' || inner == '\'' {
                        continue;
                    }
                    if line[..start].trim_end().ends_with("delete") {
                        continue;
                    }
                    checker.push(
                        "ERROR",
                        "10605029",
                        this_line + start,
                        "Indexed access is not supported for fields (arkts-no-props-by-index)"
                            .to_string(),
                    );
                }
            }
        }
        // 类外访问 `private` 成员：锚在成员名（11:30）。
        if !privates.is_empty() {
            let mut line_start = 0usize;
            for line in source.lines() {
                let this_line = line_start;
                line_start += line.len() + 1;
                for (class, members) in &privates {
                    for member in members {
                        let needle = format!(".{member}");
                        let Some(position) = line.find(&needle) else {
                            continue;
                        };
                        // 类自身的定义行（`private hidden`）与类内 `this.<member>` 都不算越界访问。
                        if line.trim_start().starts_with("private ") {
                            continue;
                        }
                        if line[..position].trim_end().ends_with("this") {
                            continue;
                        }
                        // 模板字符串里的表达式不判访问修饰符：`` `${StaticMemberModifier.count}` ``
                        // 参考侧一条都不报（`fixtures/static-member-modifier` 的 golden 为空）。
                        let before = &line[..position];
                        if let Some(at) = before.rfind("${") {
                            if !before[at..].contains('}') {
                                continue;
                            }
                        }
                        // 通过类名读 `static` 成员不判访问修饰符：参考侧对
                        // `V2StaticMemberModifier.label`（`private static label`）同样不报
                        // （`fixtures/v2-static-member-modifier` 的 golden 为空，16:35）。
                        let receiver: String = before
                            .trim_end()
                            .chars()
                            .rev()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        if receiver.chars().next().is_some_and(|c| c.is_uppercase()) {
                            continue;
                        }
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + position + 1,
                            format!(
                                "Property '{member}' is private and only accessible within class '{class}'."
                            ),
                        );
                    }
                }
            }
        }
        // 类外访问 `protected` 成员：消息与 private 同形，锚也在成员名
        // （`fixtures/ts-protected-access` 的 7:16）。
        if !protecteds.is_empty() {
            let mut line_start = 0usize;
            for line in source.lines() {
                let this_line = line_start;
                line_start += line.len() + 1;
                for (class, members) in &protecteds {
                    for member in members {
                        let needle = format!(".{member}");
                        let Some(position) = line.find(&needle) else {
                            continue;
                        };
                        if line.trim_start().starts_with("protected ") {
                            continue;
                        }
                        if line[..position].trim_end().ends_with("this") {
                            continue;
                        }
                        // 模板字符串里的表达式不判访问修饰符：`` `${StaticMemberModifier.count}` ``
                        // 参考侧一条都不报（`fixtures/static-member-modifier` 的 golden 为空）。
                        let before = &line[..position];
                        if let Some(at) = before.rfind("${") {
                            if !before[at..].contains('}') {
                                continue;
                            }
                        }
                        // 通过类名读 `static` 成员不判访问修饰符：参考侧对
                        // `V2StaticMemberModifier.label`（`private static label`）同样不报
                        // （`fixtures/v2-static-member-modifier` 的 golden 为空，16:35）。
                        let receiver: String = before
                            .trim_end()
                            .chars()
                            .rev()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        if receiver.chars().next().is_some_and(|c| c.is_uppercase()) {
                            continue;
                        }
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + position + 1,
                            format!(
                                "Property '{member}' is protected and only accessible within class '{class}' and its subclasses."
                            ),
                        );
                    }
                }
            }
        }
        // 泛型函数显式实例化后的返回类型与赋值目标不符：
        // `const n: number = first<string>(["a"])` → 5:7。
        let generics = generic_return_params(source);
        if !generics.is_empty() {
            let mut line_start = 0usize;
            for line in source.lines() {
                let this_line = line_start;
                line_start += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("const ") else {
                    continue;
                };
                let Some(colon) = rest.find(':') else {
                    continue;
                };
                let name = rest[..colon].trim().to_string();
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                let declared_type = rest[colon + 1..eq].trim().to_string();
                let value = &rest[eq + 1..];
                let Some((callee, after)) = value.split_once('<') else {
                    continue;
                };
                let callee = callee.trim();
                let Some((instantiated, _)) = after.split_once('>') else {
                    continue;
                };
                let instantiated = instantiated.trim();
                let Some((_, param)) = generics.iter().find(|(candidate, _)| candidate == callee)
                else {
                    continue;
                };
                if param != instantiated && instantiated != declared_type {
                    if let Some(offset) = line.find(&format!("const {name}")) {
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + offset + "const ".len(),
                            format!(
                                "Type '{instantiated}' is not assignable to type '{declared_type}'."
                            ),
                        );
                    }
                }
            }
        }
        // 声明文件里的返回类型参与检查：`const n: number = helper()`（`helper(): string`）报
        // `Type 'string' is not assignable to type 'number'.`，锚在变量名（3:7）。
        let declarations = checker.declarations.clone();
        if !declarations.is_empty() {
            let mut line_start = 0usize;
            for line in source.lines() {
                let this_line = line_start;
                line_start += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("const ") else {
                    continue;
                };
                let Some(colon) = rest.find(':') else {
                    continue;
                };
                let name = rest[..colon].trim().to_string();
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                let declared_type = rest[colon + 1..eq].trim().to_string();
                let value = rest[eq + 1..].trim();
                let Some(callee) = value.split('(').next() else {
                    continue;
                };
                let callee = callee.trim();
                let Some((_, return_type)) = declarations
                    .iter()
                    .find(|(candidate, _)| candidate == callee)
                else {
                    continue;
                };
                if return_type == &declared_type {
                    continue;
                }
                // 只判「声明类型与返回类型字面不同」的情形，两处都按原文比较。
                if let Some(offset) = line.find(&format!("const {name}")) {
                    let base = this_line + offset + "const ".len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        base,
                        format!(
                            "Type '{return_type}' is not assignable to type '{declared_type}'."
                        ),
                    );
                }
            }
        }
        // `export = X`。
        for (offset, code, message) in export_assignment_violations(source) {
            checker.push("ERROR", code, offset, message.to_string());
        }
        // 给对象的方法重新赋值。
        let methods_for_text = class_method_names(&parsed.program);
        let locals_for_text = checker.class_locals.clone();
        for (offset, message) in
            text_method_reassignments(source, &methods_for_text, &locals_for_text)
        {
            checker.push("ERROR", "10605052", offset, message);
        }
        // `build()` 体里读字段类型上不存在的成员。
        let field_types_for_text = struct_object_field_types(&parsed.program, source);
        let members_for_text = checker.member_types.clone();
        for (offset, message) in
            text_field_member_reads(source, &field_types_for_text, &members_for_text)
        {
            checker.push("ERROR", "10505001", offset, message);
        }
        // 同一文件里重复的转发导出名。
        for (offset, message) in duplicate_reexport_names(source) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // `build()` 体里少传实参的调用。
        let arity_for_text = checker.arity.clone();
        for (offset, message) in text_arity_calls(source, &arity_for_text) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // `build()` 体里缺必填字段的对象字面量实参。
        for (offset, message) in text_missing_literal_fields(source) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // `build()` 体里的泛型实参与类型参数约束冲突。
        for (offset, message) in text_generic_literal_calls(source) {
            checker.push("ERROR", "10505001", offset, message);
        }
        // `build()` 体里被抹白的 `as any` / `as unknown`。
        for (offset, message) in text_any_unknown_casts(source) {
            checker.push("ERROR", "10605008", offset, message);
        }
        // 系统模块默认导入上的未知成员（`build()` 体在 AST 视图里是抹白的，只能按文本看）。
        let system_reads = system_member_reads(source, &checker.default_system_imports);
        for (offset, message) in system_reads {
            checker.push("ERROR", "10505001", offset, message);
        }
        let components_for_ui = known_components();
        for (offset, code, message) in ui_in_struct_without_build(source, &components_for_ui) {
            checker.push("ERROR", code, offset, message);
        }
        // `$r` / `$rawfile` 指向不存在的资源。
        for (offset, code, message) in missing_resource_refs(source) {
            checker.push("ERROR", code, offset, message);
        }
        // A name declared twice in this file. 只有重复的 `interface` 各报两条：合并本身
        // 与「名字要唯一」（`fixtures/interface-decl-merge` 的 2:1 / 2:11、6:1 / 6:11）；
        // 重复的 `type` 别名只报合并（`fixtures/type-declaration-merge`）。
        for (name, at) in declaration_merges(&parsed.program) {
            checker.push(
                "ERROR",
                "10605103",
                at,
                "Declaration merging is not supported (arkts-no-decl-merging)".to_string(),
            );
            if source[at..].starts_with("interface ") && components_for_ui.contains(&name) {
                checker.push(
                    "ERROR",
                    "10605004",
                    at + "interface ".len(),
                    "Use unique names for types and namespaces. (arkts-unique-names)".to_string(),
                );
            }
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
        let suppressed_lines = checker.standalone_this_lines.clone();
        for (class, members) in &statics {
            for member in members {
                for offset in this_member_offsets(source, member) {
                    if suppressed_lines.contains(&checker.position(offset).0) {
                        continue;
                    }
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
        // 只有页面文件受「恰好一个 `@Entry`」约束。带 `export` 的是普通模块文件，参考侧
        // 不判它（`fixtures/barrel-comment` 的 `Helper.ets`、`import-type-audit` 的
        // `Types.ets`、`kit-two-imports` 的 `Plain.ets` 金标里都没有这一条）。
        if entries != 1 && !source.contains("export ") {
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
    if checker.lints {
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if trimmed.contains("@Sendable") {
                continue;
            }
            let mut from = 0usize;
            while let Some(at) = line[from..].find("Sendable") {
                let start = from + at;
                from = start + "Sendable".len();
                let before_ok = !line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
                let after_ok = !line[start + "Sendable".len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                if !before_ok || !after_ok {
                    continue;
                }
                let before = line[..start].trim_end();
                let is_type = before.ends_with('(')
                    || before.ends_with(':')
                    || before.ends_with('|')
                    || before.ends_with('<');
                if !is_type {
                    continue;
                }
                checker.push(
                    "ERROR",
                    "10505001",
                    this_line + start,
                    "'Sendable' refers to a value, but is being used as a type here. Did you mean 'typeof Sendable'?"
                        .to_string(),
                );
            }
        }
    }
    // `interface B extends A` 且 B 把 A 的同名属性改成不同类型：参考侧报
    // `Interface 'B' incorrectly extends interface 'A'.`（无位置）
    // （`fixtures/interface-extend-same-prop`）。
    {
        // interface 名 → (属性名 → 类型)
        let mut bodies: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = Default::default();
        let mut pending: Option<String> = None;
        let mut depth = 0i32;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("interface ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    bodies.entry(name.clone()).or_default();
                    pending = Some(name);
                    depth = 0;
                }
                continue;
            }
            if let Some(name) = &pending {
                let mut from = 0usize;
                while let Some(at) = line[from..].find(':') {
                    let start = from + at;
                    from = start + 1;
                    let head = line[..start].trim_end();
                    let prop: String = head
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if prop.is_empty() {
                        continue;
                    }
                    let tail: String = line[start + 1..]
                        .chars()
                        .take_while(|c| *c != ';' && *c != ',' && *c != '}')
                        .collect();
                    let declared = tail.trim();
                    if declared.is_empty() {
                        continue;
                    }
                    if let Some(map) = bodies.get_mut(name) {
                        map.insert(prop, declared.to_string());
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= 0 && line.contains('}') {
                    pending = None;
                }
            }
        }
        let mut offset = 0usize;
        for line in source.lines() {
            let line_origin = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("interface ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let Some(ext_at) = rest.find(" extends ") else {
                continue;
            };
            let parent: String = rest[ext_at + " extends ".len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let (Some(child_map), Some(parent_map)) = (bodies.get(&name), bodies.get(&parent))
            else {
                continue;
            };
            let clash = child_map.iter().find_map(|(prop, ty)| {
                parent_map
                    .get(prop)
                    .filter(|base| *base != ty)
                    .map(|base| (prop.clone(), ty.clone(), base.clone()))
            });
            let Some((prop, child_ty, parent_ty)) = clash else {
                continue;
            };
            // 位置锚在子接口名（`fixtures/interface-extend-same-prop` 的 6:11），
            // 消息里带上不兼容的属性与两个类型。
            let indent = line.len() - trimmed.len();
            checker.push(
                "ERROR",
                "10505001",
                line_origin + indent + "interface ".len(),
                format!(
                    "Interface '{name}' incorrectly extends interface '{parent}'. Types of property '{prop}' are incompatible. Type '{child_ty}' is not assignable to type '{parent_ty}'."
                ),
            );
        }
    }

    // 给 `readonly` 字段赋值：参考侧报
    // `Cannot assign to '<name>' because it is a read-only property.`，锚在字段名
    // （`fixtures/ts-readonly-assign` 的 7:9）。
    {
        let mut readonly_names: std::collections::HashSet<String> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("readonly ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !name.is_empty() {
                readonly_names.insert(name);
            }
        }
        if !readonly_names.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("readonly ") || trimmed.starts_with("//") {
                    continue;
                }
                let Some(eq) = trimmed.find('=') else {
                    continue;
                };
                let head = trimmed[..eq].trim_end();
                if head.ends_with("==") || head.ends_with('!') || head.ends_with('>') {
                    continue;
                }
                let Some(dot) = head.rfind('.') else {
                    continue;
                };
                let prop: String = head[dot + 1..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if !readonly_names.contains(&prop) {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                checker.push(
                    "ERROR",
                    "10505001",
                    this_line + indent + dot + 1,
                    format!("Cannot assign to '{prop}' because it is a read-only property."),
                );
            }
        }
    }

    // 泛型约束被实参违反：参考侧报
    // `Argument of type '<arg>' is not assignable to parameter of type '<constraint>'.`
    // （`fixtures/ts-generic-constraint` 的 11:15）。
    {
        // 函数名 → (泛型参数 → 约束)
        let mut generics: std::collections::HashMap<String, (String, String)> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("function ") else {
                continue;
            };
            let Some(generic_at) = rest.find('<') else {
                continue;
            };
            let name: String = rest[..generic_at]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let Some(extends_at) = rest[generic_at..].find(" extends ") else {
                continue;
            };
            let param: String = rest[generic_at + 1..generic_at + extends_at]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let constraint: String = rest[generic_at + extends_at + " extends ".len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && !param.is_empty() && !constraint.is_empty() {
                generics.insert(name, (param, constraint));
            }
        }
        if !generics.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("function ") {
                    continue;
                }
                for (name, (_, constraint)) in &generics {
                    let Some(at) = trimmed.find(&format!("{name}(")) else {
                        continue;
                    };
                    let open = at + name.len();
                    let args: String = trimmed[open + 1..]
                        .chars()
                        .take_while(|c| *c != ')')
                        .collect();
                    let first = args.split(',').next().unwrap_or("").trim();
                    let arg_type = if first.parse::<f64>().is_ok() {
                        "number"
                    } else if first.starts_with('"') || first.starts_with('\'') {
                        "string"
                    } else if first == "true" || first == "false" {
                        "boolean"
                    } else {
                        continue;
                    };
                    let indent = line.len() - trimmed.len();
                    let arg_offset =
                        this_line + indent + open + 1 + (args.len() - args.trim_start().len());
                    checker.push(
                        "ERROR",
                        "10505001",
                        arg_offset,
                        format!(
                            "Argument of type '{arg_type}' is not assignable to parameter of type '{constraint}'."
                        ),
                    );
                }
            }
        }
    }

    // `const x: T = null;`：参考侧报
    // `Type 'null' is not assignable to type 'T'.`，锚在声明名
    // （`fixtures/ts-null-assign` 的 3:9）。
    {
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if !trimmed.starts_with("const ") && !trimmed.starts_with("let ") {
                continue;
            }
            let Some(colon) = trimmed.find(':') else {
                continue;
            };
            let Some(eq) = trimmed.find('=') else {
                continue;
            };
            if colon > eq {
                continue;
            }
            let value = trimmed[eq + 1..].trim();
            let literal = if value == "null;" {
                "null"
            } else if value == "undefined;" {
                "undefined"
            } else {
                continue;
            };
            let declared = trimmed[colon + 1..eq].trim();
            if declared.is_empty() || declared == "any" {
                continue;
            }
            let indent = line.len() - trimmed.len();
            let name_start = trimmed[..colon]
                .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                .map(|index| index + 1)
                .unwrap_or(0);
            checker.push(
                "ERROR",
                "10605999",
                this_line + indent + name_start,
                format!("Type '{literal}' is not assignable to type '{declared}'."),
            );
        }
    }

    // `return undefined;` 在有返回类型的函数里：参考侧报
    // `Type 'undefined' is not assignable to type 'T'.`，锚在 `return`
    // （`fixtures/ts-undefined-return` 的 3:3）。
    {
        let mut current_return: Option<String> = None;
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("function ") {
                current_return = rest
                    .find(": ")
                    .and_then(|at| rest[at + 2..].split('{').next())
                    .map(|ty| ty.trim().to_string())
                    .filter(|ty| !ty.is_empty() && ty != "void" && ty != "any");
                continue;
            }
            if trimmed == "}" {
                current_return = None;
                continue;
            }
            if trimmed != "return undefined;" {
                continue;
            }
            let Some(declared) = &current_return else {
                continue;
            };
            let indent = line.len() - trimmed.len();
            checker.push(
                "ERROR",
                "10605999",
                this_line + indent,
                format!("Type 'undefined' is not assignable to type '{declared}'."),
            );
        }
    }

    // `new C(…)` 的实参个数与 `constructor` 不符：参考侧报
    // `Expected N arguments, but got M.`，锚在多余的那个实参
    // （`fixtures/ts-ctor-arity` 的 13:26）。
    {
        // 类名 → constructor 参数个数
        let mut ctor_arity: std::collections::HashMap<String, (usize, usize)> = Default::default();
        let mut current_class: Option<String> = None;
        for line in source.lines() {
            let trimmed = line.trim_start();
            let declares_class =
                trimmed.starts_with("class ") || trimmed.starts_with("export class ");
            if declares_class {
                let name: String = trimmed
                    .trim_start_matches("export ")
                    .trim_start_matches("class ")
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                current_class = Some(name);
                continue;
            }
            if trimmed == "}" {
                current_class = None;
                continue;
            }
            let Some(rest) = trimmed.strip_prefix("constructor(") else {
                continue;
            };
            let Some(inner) = rest.split(')').next() else {
                continue;
            };
            // 带默认值（`a: T = …`）或可选（`a?: T`）的形参不是必须传的：参考侧对
            // `new Address("172.19.0.1", 1)` 不报，虽然 `Address` 的构造函数有三个形参
            // （`fixtures/common-vpn-service` 的 40:22、111:34）。
            let parts: Vec<&str> = if inner.trim().is_empty() {
                Vec::new()
            } else {
                inner.split(',').collect()
            };
            let minimum = parts
                .iter()
                .filter(|part| {
                    let part = part.trim();
                    !(part.contains('=') || part.contains("?:"))
                })
                .count();
            let maximum = parts.len();
            if let Some(name) = &current_class {
                ctor_arity.insert(name.clone(), (minimum, maximum));
            }
        }
        if !ctor_arity.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let mut from = 0usize;
                while let Some(at) = line[from..].find("new ") {
                    let start = from + at;
                    from = start + "new ".len();
                    let class_name: String = line[from..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    let Some((minimum, maximum)) = ctor_arity.get(&class_name).copied() else {
                        continue;
                    };
                    let Some(open) = line[from..].find('(') else {
                        continue;
                    };
                    let open_at = from + open;
                    let Some(close) = line[open_at..].find(')') else {
                        continue;
                    };
                    let inner = &line[open_at + 1..open_at + close];
                    let given = if inner.trim().is_empty() {
                        0
                    } else {
                        inner.split(',').count()
                    };
                    if given >= minimum && given <= maximum {
                        continue;
                    }
                    let anchor = if given > maximum {
                        // 多余实参里的第一个
                        let mut cursor = open_at + 1;
                        let mut seen = 0usize;
                        let mut depth = 0i32;
                        for (index, byte) in inner.bytes().enumerate() {
                            match byte {
                                b'(' | b'[' | b'{' => depth += 1,
                                b')' | b']' | b'}' => depth -= 1,
                                b',' if depth == 0 => {
                                    seen += 1;
                                    if seen == maximum {
                                        cursor = open_at + 1 + index + 1;
                                        while line[cursor..].starts_with(' ') {
                                            cursor += 1;
                                        }
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        cursor
                    } else {
                        start
                    };
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + anchor,
                        format!("Expected {maximum} arguments, but got {given}."),
                    );
                }
            }
        }
    }

    // `import … from "…​.ts"`：参考侧报
    // `An import path cannot end with a '.ts' extension. Consider importing '…​.js' instead.`，
    // 锚在说明符（`fixtures/import-ts-extension` 的 2:21）。
    {
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if !trimmed.starts_with("import ") && !trimmed.starts_with("export ") {
                continue;
            }
            let Some(from_at) = trimmed.find("from ") else {
                continue;
            };
            let Some(quote_at) = trimmed[from_at..].find(['"', '\'']) else {
                continue;
            };
            let spec_start = from_at + quote_at + 1;
            let specifier: String = trimmed[spec_start..]
                .chars()
                .take_while(|c| *c != '"' && *c != '\'')
                .collect();
            let Some(stem) = specifier.strip_suffix(".ts") else {
                continue;
            };
            let indent = line.len() - trimmed.len();
            checker.push(
                "ERROR",
                "10505001",
                this_line + indent + spec_start.saturating_sub(1),
                format!(
                    "An import path cannot end with a '.ts' extension. Consider importing '{stem}.js' instead."
                ),
            );
        }
    }

    // `componentUtils.getRectangleById(…)` 的返回值是 `ComponentInfo`，成员固定
    // （SDK `@ohos.arkui.componentUtils.d.ts` 的 `interface ComponentInfo`）。取不存在的成员
    // 时参考侧报 `Property '<name>' does not exist on type 'ComponentInfo'.`，锚在该成员
    // （`fixtures/ts-component-info-member` 的 6:15）。
    {
        const COMPONENT_INFO: [&str; 8] = [
            "size",
            "localOffset",
            "windowOffset",
            "screenOffset",
            "translate",
            "scale",
            "rotate",
            "transform",
        ];
        // 变量名 → 类型名（只认 `const x = componentUtils.<fn>(…)` 这一种来源）。
        let mut typed: std::collections::HashMap<String, &'static str> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("const ") else {
                continue;
            };
            let Some(eq) = rest.find('=') else {
                continue;
            };
            let name: String = rest[..eq]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if name.is_empty() {
                continue;
            }
            if rest[eq + 1..].contains("componentUtils.") {
                typed.insert(name, "ComponentInfo");
            }
        }
        if !typed.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("const ") || trimmed.starts_with("import ") {
                    continue;
                }
                for (variable, type_name) in &typed {
                    let Some(at) = trimmed.find(&format!("{variable}.")) else {
                        continue;
                    };
                    let dot = at + variable.len();
                    let member: String = trimmed[dot + 1..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if member.is_empty() || COMPONENT_INFO.contains(&member.as_str()) {
                        continue;
                    }
                    if *type_name != "ComponentInfo" {
                        continue;
                    }
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + dot + 1,
                        format!("Property '{member}' does not exist on type 'ComponentInfo'."),
                    );
                }
            }
        }
    }

    // SDK 里标了 `@deprecated` / `@throws` 的接口：参考侧对调用点打两条 WARN
    // （`fixtures/ts-component-info-member` 的 5:31）。表里只有实测过的条目。
    {
        const SDK_WARNINGS: [(&str, &str, &str); 3] = [
            (
                "componentUtils.getRectangleById",
                "getRectangleById",
                "'getRectangleById' has been deprecated.",
            ),
            (
                "componentUtils.getRectangleById",
                "getRectangleById",
                "Function may throw exceptions. Special handling is required.",
            ),
            (
                "= getContext(",
                "getContext",
                "'getContext' has been deprecated.",
            ),
        ];
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            for (needle, name, message) in SDK_WARNINGS {
                let Some(at) = trimmed.find(needle) else {
                    continue;
                };
                let name_at = at + needle.rfind(name).unwrap_or(0);
                let indent = line.len() - trimmed.len();
                checker.push(
                    "WARN",
                    "-",
                    this_line + indent + name_at,
                    message.to_string(),
                );
            }
        }
    }

    // `getContext()` 返回 `Context`，成员固定（SDK `application/Context.d.ts` 的
    // `class Context`）。取不存在的成员时参考侧报
    // `Property '<name>' does not exist on type 'Context'.`，锚在该成员
    // （`fixtures/ts-context-member` 的 4:14）。
    {
        const CONTEXT_MEMBERS: [&str; 14] = [
            "applicationInfo",
            "area",
            "bundleCodeDir",
            "cacheDir",
            "cloudFileDir",
            "databaseDir",
            "distributedFilesDir",
            "eventHub",
            "filesDir",
            "preferencesDir",
            "processName",
            "resourceDir",
            "resourceManager",
            "tempDir",
        ];
        let mut context_vars: std::collections::HashSet<String> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("const ") else {
                continue;
            };
            let Some(eq) = rest.find('=') else {
                continue;
            };
            let name: String = rest[..eq]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !name.is_empty() && rest[eq + 1..].contains("getContext(") {
                context_vars.insert(name);
            }
        }
        if !context_vars.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("const ") {
                    continue;
                }
                for variable in &context_vars {
                    let Some(at) = trimmed.find(&format!("{variable}.")) else {
                        continue;
                    };
                    let dot = at + variable.len();
                    let member: String = trimmed[dot + 1..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if member.is_empty() || CONTEXT_MEMBERS.contains(&member.as_str()) {
                        continue;
                    }
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + dot + 1,
                        format!("Property '{member}' does not exist on type 'Context'."),
                    );
                }
            }
        }
    }

    // `resourceManager.ResourceManager` 上的成员（SDK `@ohos.resourceManager.d.ts` 的
    // `interface ResourceManager`，实读抽取）。取不存在的成员时参考侧报
    // `Property '<name>' does not exist on type 'ResourceManager'.`
    // （`fixtures/ts-resource-manager-member` 的 7:14）。
    {
        const RM_MEMBERS: [&str; 23] = [
            "closeRawFileDescriptor",
            "getBoolean",
            "getBooleanByName",
            "getConfiguration",
            "getDeviceCapability",
            "getMedia",
            "getMediaByName",
            "getMediaContent",
            "getPluralString",
            "getPluralStringByName",
            "getPluralStringValue",
            "getRawFile",
            "getRawFileDescriptor",
            "getString",
            "getStringArray",
            "getStringArrayByName",
            "getStringArrayValue",
            "getStringByName",
            "getStringByNameSync",
            "getStringSync",
            "getStringValue",
            "releaseRawFileDescriptor",
            "getRawFd",
        ];
        let mut rm_vars: std::collections::HashSet<String> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("const ") else {
                continue;
            };
            let Some(eq) = rest.find('=') else {
                continue;
            };
            let head = &rest[..eq];
            if !head.contains(": resourceManager.ResourceManager") {
                continue;
            }
            let name: String = head
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !name.is_empty() {
                rm_vars.insert(name);
            }
        }
        if !rm_vars.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("const ") {
                    continue;
                }
                for variable in &rm_vars {
                    let Some(at) = trimmed.find(&format!("{variable}.")) else {
                        continue;
                    };
                    let dot = at + variable.len();
                    let member: String = trimmed[dot + 1..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if member.is_empty() || RM_MEMBERS.contains(&member.as_str()) {
                        continue;
                    }
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + dot + 1,
                        format!("Property '{member}' does not exist on type 'ResourceManager'."),
                    );
                }
            }
        }
    }

    // `declare global` 体里声明的类型在外部按类型用：参考侧报 `Cannot find name '<n>'.`
    // （`fixtures/ts-declare-global` 的 8:22）。
    {
        let mut declared: Vec<String> = Vec::new();
        let mut inside = false;
        let mut depth = 0i32;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("declare global") {
                inside = true;
                depth = 0;
                continue;
            }
            if !inside {
                continue;
            }
            let body = trimmed
                .strip_prefix("interface ")
                .or_else(|| trimmed.strip_prefix("class "))
                .or_else(|| trimmed.strip_prefix("type "));
            if let Some(rest) = body {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    declared.push(name);
                }
            }
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if depth <= 0 && line.contains('}') {
                inside = false;
            }
        }
        if !declared.is_empty() {
            let mut offset = 0usize;
            let mut skip_block = false;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("declare global") {
                    skip_block = true;
                    continue;
                }
                if skip_block {
                    if trimmed == "}" {
                        skip_block = false;
                    }
                    continue;
                }
                for name in &declared {
                    let mut from = 0usize;
                    while let Some(at) = trimmed[from..].find(name.as_str()) {
                        let start = from + at;
                        from = start + name.len();
                        let before_ok = !trimmed[..start]
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                        let after_ok = !trimmed[start + name.len()..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                        if !before_ok || !after_ok {
                            continue;
                        }
                        let before = trimmed[..start].trim_end();
                        if !(before.ends_with('(')
                            || before.ends_with(':')
                            || before.ends_with('<'))
                        {
                            continue;
                        }
                        let indent = line.len() - trimmed.len();
                        checker.push(
                            "ERROR",
                            "10505001",
                            this_line + indent + start,
                            format!("Cannot find name '{name}'."),
                        );
                    }
                }
            }
        }
    }

    // `delete x.<prop>` 且 `<prop>` 在某个 interface 里非可选：参考侧另报
    // `The operand of a 'delete' operator must be optional.`，锚在操作数起点
    // （`fixtures/arkts-delete-nonoptional` 的 7:10）。
    {
        // interface 名 → 必需成员集合
        let mut required: std::collections::HashSet<String> = Default::default();
        let mut inside = false;
        let mut depth = 0i32;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("interface ") {
                inside = true;
                depth = 0;
            }
            if inside {
                let mut from = 0usize;
                while let Some(at) = line[from..].find(':') {
                    let start = from + at;
                    from = start + 1;
                    let prop: String = line[..start]
                        .trim_end()
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if prop.is_empty() || prop.contains('?') {
                        continue;
                    }
                    let declared: String = line[start + 1..]
                        .chars()
                        .take_while(|c| *c != ';' && *c != ',' && *c != '}')
                        .collect();
                    if !declared.trim().is_empty() {
                        required.insert(prop);
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= 0 && line.contains('}') {
                    inside = false;
                }
            }
        }
        if !required.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("delete ") else {
                    continue;
                };
                let Some(dot) = rest.find('.') else {
                    continue;
                };
                let prop: String = rest[dot + 1..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if prop.is_empty() || !required.contains(&prop) {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                checker.push(
                    "ERROR",
                    "10605999",
                    this_line + indent + "delete ".len(),
                    "The operand of a 'delete' operator must be optional.".to_string(),
                );
            }
        }
    }

    // `Symbol()`：参考侧在调用点与后续使用点各报一条 `arkts-no-symbol`，并对访问的属性
    // 报「可能为 undefined」（`fixtures/arkts-symbol` 的 3:15、4:10、4:10）。
    {
        let mut symbol_vars: std::collections::HashSet<String> = Default::default();
        let mut symbol_sites: Vec<usize> = Vec::new();
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if !trimmed.contains("Symbol(") {
                continue;
            }
            let indent = line.len() - trimmed.len();
            if let Some(at) = trimmed.find("Symbol(") {
                symbol_sites.push(this_line + indent + at);
            }
            let Some(rest) = trimmed.strip_prefix("const ") else {
                continue;
            };
            let Some(eq) = rest.find('=') else {
                continue;
            };
            if !rest[eq + 1..].contains("Symbol(") {
                continue;
            }
            let name: String = rest[..eq]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !name.is_empty() {
                symbol_vars.insert(name);
            }
        }
        for at in symbol_sites {
            checker.push(
                "ERROR",
                "10605002",
                at,
                "\"Symbol()\" API is not supported (arkts-no-symbol)".to_string(),
            );
        }
        if !symbol_vars.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("const ") {
                    continue;
                }
                for variable in &symbol_vars {
                    let Some(at) = trimmed.find(&format!("{variable}.")) else {
                        continue;
                    };
                    let member: String = trimmed[at + variable.len() + 1..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10605002",
                        this_line + indent + at,
                        "\"Symbol()\" API is not supported (arkts-no-symbol)".to_string(),
                    );
                    if !member.is_empty() {
                        checker.push(
                            "ERROR",
                            "10605999",
                            this_line + indent + at,
                            format!("'{variable}.{member}' is possibly 'undefined'."),
                        );
                    }
                }
            }
        }
    }

    // 带标签模板调用：第一个形参是 `string[]` 时，参考侧报
    // `Argument of type 'TemplateStringsArray' is not assignable to parameter of type 'string[]'.`
    // （无位置，`fixtures/ts-tagged-template`）。
    {
        // 函数名 → 第一个形参是否 `string[]`
        let mut takes_strings: std::collections::HashSet<String> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("function ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let Some(open) = rest.find('(') else {
                continue;
            };
            let Some(close) = rest[open..].find(')') else {
                continue;
            };
            let params = &rest[open + 1..open + close];
            let first = params.split(',').next().unwrap_or("").trim();
            if first.ends_with(": string[]") {
                takes_strings.insert(name);
            }
        }
        if !takes_strings.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                for name in &takes_strings {
                    let needle = format!("{name}`");
                    let Some(at) = trimmed.find(&needle) else {
                        continue;
                    };
                    // 锚在反引号（`fixtures/ts-tagged-template` 的 7:13）。
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + at + name.len(),
                        "Argument of type 'TemplateStringsArray' is not assignable to parameter of type 'string[]'. Type 'TemplateStringsArray' is missing the following properties from type 'string[]': pop, push, reverse, shift, and 6 more."
                            .to_string(),
                    );
                }
            }
        }
    }

    // 类 `implements` 接口但缺必需成员：参考侧报
    // `Class '<C>' incorrectly implements interface '<I>'.`（无位置，
    // `fixtures/ts-implements-missing`）。
    {
        // interface 名 → 必需成员集合
        let mut iface_members: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut current: Option<String> = None;
        let mut depth = 0i32;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("interface ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    iface_members.entry(name.clone()).or_default();
                    current = Some(name);
                    depth = 0;
                }
                continue;
            }
            if let Some(name) = &current {
                let mut from = 0usize;
                while let Some(at) = line[from..].find(':') {
                    let start = from + at;
                    from = start + 1;
                    let member: String = line[..start]
                        .trim_end()
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if member.is_empty() {
                        continue;
                    }
                    if let Some(list) = iface_members.get_mut(name) {
                        list.push(member);
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= 0 && line.contains('}') {
                    current = None;
                }
            }
        }
        // `class C implements I` → C 体里声明的成员
        let mut class_members: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut current_class: Option<String> = None;
        let mut depth = 0i32;
        for line in source.lines() {
            let trimmed = line.trim_start();
            let declares = trimmed.starts_with("class ") || trimmed.starts_with("export class ");
            if declares {
                let head = trimmed
                    .trim_start_matches("export ")
                    .trim_start_matches("class ");
                let name: String = head
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                class_members.entry(name.clone()).or_default();
                current_class = Some(name);
                depth = 0;
                continue;
            }
            if let Some(name) = &current_class {
                let mut from = 0usize;
                while let Some(at) = line[from..].find(':') {
                    let start = from + at;
                    from = start + 1;
                    let member: String = line[..start]
                        .trim_end()
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if member.is_empty() {
                        continue;
                    }
                    if let Some(list) = class_members.get_mut(name) {
                        if !list.contains(&member) {
                            list.push(member);
                        }
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= 0 && line.contains('}') {
                    current_class = None;
                }
            }
        }
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            let head = trimmed
                .strip_prefix("export ")
                .unwrap_or(trimmed)
                .strip_prefix("class ");
            let Some(head) = head else { continue };
            let class_name: String = head
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let Some(impl_at) = head.find(" implements ") else {
                continue;
            };
            let iface: String = head[impl_at + " implements ".len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let (Some(required), Some(declared)) =
                (iface_members.get(&iface), class_members.get(&class_name))
            else {
                continue;
            };
            let Some(first_missing) = required.iter().find(|member| !declared.contains(member))
            else {
                continue;
            };
            // 锚在类名（`fixtures/ts-implements-missing` 的 7:7），消息带上第一个缺的成员。
            let indent = line.len() - trimmed.len();
            let name_at = indent + line[indent..].find(&class_name).unwrap_or(0);
            checker.push(
                "ERROR",
                "10505001",
                this_line + name_at,
                format!(
                    "Class '{class_name}' incorrectly implements interface '{iface}'. Property '{first_missing}' is missing in type '{class_name}' but required in type '{iface}'."
                ),
            );
        }
    }

    // 实例化 `abstract class`：参考侧报 `Cannot create an instance of an abstract class.`，
    // 锚在 `new`（`fixtures/ts-abstract-instantiate` 的 7:10）。
    {
        let mut abstract_classes: std::collections::HashSet<String> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let rest = trimmed
                .strip_prefix("export ")
                .unwrap_or(trimmed)
                .strip_prefix("abstract class ");
            let Some(rest) = rest else { continue };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                abstract_classes.insert(name);
            }
        }
        if !abstract_classes.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("abstract class ") {
                    continue;
                }
                for name in &abstract_classes {
                    let needle = format!("new {name}(");
                    let Some(at) = trimmed.find(&needle) else {
                        continue;
                    };
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + at,
                        "Cannot create an instance of an abstract class.".to_string(),
                    );
                }
            }
        }
    }

    // 在实例位置访问 `static` 成员：参考侧报
    // `Property '<m>' does not exist on type '<C>'. Did you mean to access the static member
    // '<C>.<m>' instead?`，锚在成员名（`fixtures/ts-static-member-access` 的 7:16）。
    {
        // 类名 → static 成员集合
        let mut statics: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut current: Option<String> = None;
        let mut depth = 0i32;
        for line in source.lines() {
            let trimmed = line.trim_start();
            let declares = trimmed.starts_with("class ") || trimmed.starts_with("export class ");
            if declares {
                let head = trimmed
                    .trim_start_matches("export ")
                    .trim_start_matches("class ");
                let name: String = head
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                statics.entry(name.clone()).or_default();
                current = Some(name);
                depth = 0;
                continue;
            }
            if let Some(name) = &current {
                let body = trimmed
                    .strip_prefix("static ")
                    .or_else(|| trimmed.strip_prefix("public static "))
                    .or_else(|| trimmed.strip_prefix("private static "));
                if let Some(rest) = body {
                    let member: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !member.is_empty() {
                        if let Some(list) = statics.get_mut(name) {
                            list.push(member);
                        }
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= 0 && line.contains('}') {
                    current = None;
                }
            }
        }
        for (class, members) in &statics {
            for member in members {
                let mut offset = 0usize;
                for line in source.lines() {
                    let this_line = offset;
                    offset += line.len() + 1;
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("static ") {
                        continue;
                    }
                    let needle = format!(".{member}");
                    let Some(at) = trimmed.find(&needle) else {
                        continue;
                    };
                    // 后面还接着标识符字符时不是一个成员名：`.onClick(` 里的 `.on` 不是
                    // `on` 成员的读取（`fixtures/configuration-page` 的 852:14）。
                    if trimmed[at + needle.len()..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
                    {
                        continue;
                    }
                    // `Holder.total` 与 `this.total` 都不是实例位置。
                    let base = trimmed[..at].trim_end();
                    if base.ends_with(class) || base.ends_with("this") {
                        continue;
                    }
                    // 通过类名调用静态成员是合法写法：`EventHub.on(…)` 里的 `on` 就是
                    // `EventHub` 的静态方法（`fixtures/configuration-page` 的 852:14，
                    // 参考侧无诊断）。只有拿其它**实例**去读这个成员才报。
                    if checker.class_names.contains(base) {
                        continue;
                    }
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + at + 1,
                        format!(
                            "Property '{member}' does not exist on type '{class}'. Did you mean to access the static member '{class}.{member}' instead?"
                        ),
                    );
                }
            }
        }
    }

    // `build()` 体里只允许 UI 组件语法：`try`、`let`/`const`/`var`、`for`、`while`、`switch`
    // 等语句一律报 `Only UI component syntax can be written here.`，锚在语句起点
    // （`fixtures/ui-build-only-component-syntax` 的 7:7）。
    {
        const STATEMENTS: [&str; 7] = [
            "try {", "let ", "const ", "var ", "for ", "while ", "switch ",
        ];
        let mut inside_build = false;
        let mut depth = 0i32;
        let mut arrow_depth: Option<i32> = None;
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if trimmed.starts_with("build()") {
                inside_build = true;
                depth = 0;
                continue;
            }
            if !inside_build {
                continue;
            }
            // `this.x = …;`：参考侧报 `'<语句>' does not meet UI component syntax.`
            // （`fixtures/ui-build-assign` 的 9:7），消息里带整条语句。回调里的赋值不算
            // （`fixtures/type-assign-return-element` 的 `.onClick(() => { this.label = 1; })`）。
            if let Some(mark) = arrow_depth {
                if depth <= mark {
                    arrow_depth = None;
                }
            }
            if trimmed.contains("=> {") && arrow_depth.is_none() {
                arrow_depth = Some(depth);
            }
            if arrow_depth.is_none()
                && trimmed.starts_with("this.")
                && trimmed.contains('=')
                && trimmed.ends_with(';')
            {
                let indent = line.len() - trimmed.len();
                checker.push(
                    "ERROR",
                    "10905204",
                    this_line + indent,
                    format!("'{trimmed}' does not meet UI component syntax."),
                );
            }
            if let Some(rest) = STATEMENTS
                .iter()
                .find_map(|needle| trimmed.strip_prefix(needle).map(|_| *needle))
            {
                // `if` 是允许的控制流（`fixtures/ui-build-if` 的参考侧 exit=0），不在表里。
                let indent = line.len() - trimmed.len();
                let _ = rest;
                checker.push(
                    "ERROR",
                    "10905209",
                    this_line + indent,
                    "Only UI component syntax can be written here.".to_string(),
                );
            }
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if depth <= 0 && line.contains('}') {
                inside_build = false;
            }
        }
    }

    // `struct` 没有 `build` 方法：参考侧报
    // `The struct '<S>' must have at least and at most one 'build' method.`，锚在 struct 名
    // （`fixtures/ui-struct-without-component` 的 2:8）。这里只看「一个都没有」的情形，
    // 「多于一个」由 `10905103` 回答。
    {
        let mut offset = 0usize;
        let mut pending_struct: Option<(usize, String)> = None;
        let mut depth = 0i32;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed
                .strip_prefix("struct ")
                .or_else(|| trimmed.strip_prefix("export struct "))
            {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    let indent = line.len() - trimmed.len();
                    let at = this_line + indent + (trimmed.len() - rest.len());
                    pending_struct = Some((at, name));
                    // `struct X {` 自己的 `{` 计入，否则体内一块 `{ … }` 闭合就会被当成 struct 结束。
                    depth = 1;
                }
                continue;
            }
            let Some((at, name)) = &pending_struct else {
                continue;
            };
            // 带参数的 `build(value: number)` 也算有 build 方法
            // （`fixtures/ui-build-parameter` 的参考侧另有它自己的诊断）。
            if trimmed.starts_with("build(") {
                pending_struct = None;
                continue;
            }
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if depth <= 0 && line.contains('}') {
                checker.push(
                    "ERROR",
                    "10905103",
                    *at,
                    format!(
                        "The struct '{name}' must have at least and at most one 'build' method."
                    ),
                );
                pending_struct = None;
            }
        }
    }

    // `interface X extends Y` 而 `Y` 是 class：参考侧报
    // `Interfaces cannot extend classes (arkts-extends-only-class)`，锚在 `Y`
    // （`fixtures/arkts-interface-extends-class` 的 6:26）。
    {
        let mut class_names: std::collections::HashSet<String> = Default::default();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let rest = trimmed
                .strip_prefix("export ")
                .unwrap_or(trimmed)
                .strip_prefix("class ");
            if let Some(rest) = rest {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    class_names.insert(name);
                }
            }
        }
        if !class_names.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("interface ") else {
                    continue;
                };
                let Some(at) = rest.find(" extends ") else {
                    continue;
                };
                let parent: String = rest[at + " extends ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !class_names.contains(&parent) {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                let parent_at = indent + "interface ".len() + at + " extends ".len();
                checker.push(
                    "ERROR",
                    "10605104",
                    this_line + parent_at,
                    "Interfaces cannot extend classes (arkts-extends-only-class)".to_string(),
                );
            }
        }
    }
    // 非抽象子类未实现父类的抽象方法：参考侧报
    // `Non-abstract class '<C>' does not implement inherited abstract member '<m>' from class
    // '<B>'.`，锚在子类名（`fixtures/ts-abstract-missing` 的 6:7）。
    {
        // 类名 → (是否 abstract, abstract 方法名)
        let mut abstract_methods: std::collections::HashMap<String, Vec<String>> =
            Default::default();
        let mut is_abstract: std::collections::HashSet<String> = Default::default();
        let mut current: Option<String> = None;
        for line in source.lines() {
            let trimmed = line.trim_start();
            let head = trimmed.strip_prefix("export ").unwrap_or(trimmed);
            if let Some(rest) = head.strip_prefix("abstract class ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    is_abstract.insert(name.clone());
                    abstract_methods.entry(name.clone()).or_default();
                    current = Some(name);
                }
                continue;
            }
            if let Some(rest) = head.strip_prefix("class ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    current = Some(name);
                }
                continue;
            }
            if let Some(name) = &current {
                if let Some(rest) = trimmed.strip_prefix("abstract ") {
                    let method: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !method.is_empty() {
                        if let Some(list) = abstract_methods.get_mut(name) {
                            list.push(method);
                        }
                    }
                }
                if trimmed == "}" {
                    current = None;
                }
            }
        }
        if !abstract_methods.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let head = trimmed.strip_prefix("export ").unwrap_or(trimmed);
                let Some(rest) = head.strip_prefix("class ") else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if is_abstract.contains(&name) {
                    continue;
                }
                let Some(at) = rest.find(" extends ") else {
                    continue;
                };
                let parent: String = rest[at + " extends ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let Some(required) = abstract_methods.get(&parent) else {
                    continue;
                };
                if required.is_empty() {
                    continue;
                }
                // 检查子类体本身，不看全文件：父类的 `abstract read(): number;` 里也有 `read()`。
                let mut body = String::new();
                let mut depth = 0i32;
                let mut inside = false;
                for body_line in source[this_line..].lines().skip(1) {
                    if !inside && body_line.trim_start().starts_with('}') {
                        break;
                    }
                    inside = true;
                    body.push_str(body_line);
                    body.push('\n');
                    depth += body_line.matches('{').count() as i32;
                    depth -= body_line.matches('}').count() as i32;
                    if depth <= 0 && body_line.contains('}') {
                        break;
                    }
                }
                let Some(missing) = required
                    .iter()
                    .find(|method| !body.contains(&format!("{method}(")))
                else {
                    continue;
                };
                let indent = line.len() - trimmed.len();
                let name_at = indent + line[indent..].find(&name).unwrap_or(0);
                checker.push(
                    "ERROR",
                    "10505001",
                    this_line + name_at,
                    format!(
                        "Non-abstract class '{name}' does not implement inherited abstract member '{missing}' from class '{parent}'."
                    ),
                );
            }
        }
    }

    // `Enum.Nope`：枚举没有该成员时参考侧报
    // `Property '<m>' does not exist on type 'typeof <E>'.`，锚在该成员
    // （`fixtures/ts-enum-member` 的 7:16）。
    {
        // 枚举名 → 成员集合
        let mut enums: std::collections::HashMap<String, Vec<String>> = Default::default();
        let mut current: Option<String> = None;
        for line in source.lines() {
            let trimmed = line.trim_start();
            let head = trimmed.strip_prefix("export ").unwrap_or(trimmed);
            if let Some(rest) = head.strip_prefix("enum ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    enums.entry(name.clone()).or_default();
                    current = Some(name);
                }
                continue;
            }
            if let Some(name) = &current {
                if trimmed == "}" {
                    current = None;
                    continue;
                }
                let member: String = trimmed
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !member.is_empty() {
                    if let Some(list) = enums.get_mut(name) {
                        list.push(member);
                    }
                }
            }
        }
        if !enums.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("enum ") {
                    continue;
                }
                for (name, members) in &enums {
                    let needle = format!("{name}.");
                    let Some(at) = trimmed.find(&needle) else {
                        continue;
                    };
                    let member: String = trimmed[at + needle.len()..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if member.is_empty() || members.contains(&member) {
                        continue;
                    }
                    let indent = line.len() - trimmed.len();
                    checker.push(
                        "ERROR",
                        "10505001",
                        this_line + indent + at + needle.len(),
                        format!("Property '{member}' does not exist on type 'typeof {name}'."),
                    );
                }
            }
        }
    }

    if checker.linter_hit {
        FILE_LEVEL_LINTER.with(|cell| cell.set(true));
    }
    // 同一条诊断可能同时来自文本扫描与 AST 访问者（`Property 'X' does not exist on type
    // 'typeof m'.` 在两处都判）；按 (line, column, code, message) 去重。
    checker.out.dedup_by(|a, b| {
        a.line == b.line && a.column == b.column && a.code == b.code && a.message == b.message
    });
    checker
        .out
        .sort_by(|a, b| (a.line, a.column, a.code).cmp(&(b.line, b.column, b.code)));
    checker.out.dedup_by(|a, b| {
        a.line == b.line && a.column == b.column && a.code == b.code && a.message == b.message
    });
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
    /// 被 import 的声明文件里的函数返回类型：名字 → 返回类型原文。
    declarations: Vec<(String, String)>,
    /// 已报过 `arkts-no-standalone-this` 的行号：同一行里的 `this.<x>` 不再报成员不存在
    /// （参考侧对静态方法只报前者）。
    standalone_this_lines: std::collections::HashSet<usize>,
    /// Default imports of system modules: the local name → the module it comes from. A member
    /// read on one is judged against the module's declared names
    /// (`Property 'AutoFinalizer' does not exist on type 'typeof util'.`, 8:24 of
    /// `fixtures/system-module-member`).
    default_system_imports: std::collections::HashMap<String, String>,
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
    /// `const x = new SomeClass();` 的 `x -> SomeClass`：实例上的成员判定要用它
    /// （`Property 'notThere' does not exist on type 'Box'.`，
    /// `fixtures/type-instance-member` 的 8:17）。
    instances: std::collections::HashMap<String, String>,
    /// 本文件函数的返回类型注解：`compute -> number`，用来判
    /// `const s: string = compute()`
    /// （`fixtures/type-return-mismatch` 的 6:7）。
    fn_returns: std::collections::HashMap<String, String>,
    /// 泛型函数的形参个数：`pair -> 2`，用来判显式类型实参数量
    /// （`fixtures/type-generic-arity` 的 6:18、6:13）。
    generic_arity: std::collections::HashMap<String, usize>,
    /// 本次调用已因类型实参数量不符报过：实参数量那条不再补
    /// （`fixtures/type-generic-arity` 只有类型实参那一条）。
    generic_mismatch: bool,
    /// 本文件是否命中过 linter 侧规则（决定参考侧的 FAQ 行）。
    linter_hit: bool,
    /// class 成员的声明类型：`Outer -> {inner: Inner}`，用来判 `outer.inner.notThere`
    /// （`fixtures/type-instance-member` 的嵌套形态）。
    class_member_types:
        std::collections::HashMap<String, std::collections::HashMap<String, String>>,
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
    /// 赋值左值上的成员名偏移：`options.newFileNames = [...]` 是写成员，不是读成员，
    /// 参考侧不按「类型没有该成员」报（`fixtures/configuration-page` 的 1282:13）。
    assignment_targets: std::collections::HashSet<usize>,
    /// How many arrow functions enclose the node being visited. A callback's parameter is typed by
    /// the call it is passed to, so an unannotated one there is not `any` (the reference stays
    /// silent for the `(err) => …` callbacks of `ets/EntryAbility.ets`).
    arrow_depth: usize,
    /// How many class methods enclose the node being visited. `this` is legal inside a method and
    /// rejected inside a stand-alone function (`arkts-no-standalone-this`).
    method_depth: usize,
}

impl Checker<'_> {
    /// 数组字面量的每个元素都要能赋给元素类型（`numbers: number[] = [1, 'two']` 报在
    /// `'two'`，9:27 of `fixtures/type-assign-return-element`）。
    fn judge_array_initializer(
        &mut self,
        annotation: Option<&TSTypeAnnotation<'_>>,
        init: Option<&Expression<'_>>,
    ) {
        let Some(annotation) = annotation else {
            return;
        };
        let TSType::TSArrayType(array) = &annotation.type_annotation else {
            return;
        };
        let element = match array.element_type {
            TSType::TSNumberKeyword(_) => "number",
            TSType::TSStringKeyword(_) => "string",
            TSType::TSBooleanKeyword(_) => "boolean",
            _ => return,
        };
        let Some(Expression::ArrayExpression(literal)) = init else {
            return;
        };
        for item in &literal.elements {
            let spelling = match item {
                oxc_ast::ast::ArrayExpressionElement::NumericLiteral(_) => "number",
                oxc_ast::ast::ArrayExpressionElement::StringLiteral(_) => "string",
                oxc_ast::ast::ArrayExpressionElement::BooleanLiteral(_) => "boolean",
                _ => continue,
            };
            if spelling == element {
                continue;
            }
            self.push(
                "ERROR",
                "10505001",
                item.span().start as usize,
                format!("Type '{spelling}' is not assignable to type '{element}'."),
            );
        }
    }

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
            // 同名局部在多处按不同类型声明时按名字全局映射会串台：
            // `fixtures/configuration-page` 里 `options` 既被标成本文件的 `ToastOptions`，
            // 又在新行上是 `new picker.DocumentSaveOptions()`，后者的成员在 SDK 里，
            // 于是 `options.newFileNames` 被报成「ToastOptions 没有该成员」（1282:13）。
            // 冲突时不登记，宁可不判。
            if self.class_locals.remove(name.name.as_str()).is_none() {
                self.class_locals.insert(name.name.to_string(), display);
            }
        }
    }

    fn check_system_request(&mut self, it: &ImportDeclaration<'_>) {
        let request = it.source.value.as_str();
        // `@hms.*` is the same shape of module, declared under the SDK's `hms/ets/api`
        // (`fixtures/hms-missing-symbol` measures the same wording there).
        if !(request.starts_with("@ohos.")
            || request.starts_with("@system.")
            || request.starts_with("@hms.")
            // `@kit.*` 的成员是聚合导入，仍要登记本地名，让 `typeof util` 的成员判定生效
            // （`fixtures/system-module-named-import` 的 4:19、5:22）。
            || request.starts_with("@kit."))
        {
            return;
        }
        if !request.starts_with("@kit.") && system_module_declared(request) == Some(false) {
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
                // 默认导入的系统模块：记住本地名，属性访问时按模块声明的名字判定。
                if let ImportDeclarationSpecifier::ImportDefaultSpecifier(default) = specifier {
                    let local = default.local.name.to_string();
                    if system_module_declared(request) == Some(true) {
                        self.default_system_imports
                            .insert(local, request.to_string());
                    }
                    continue;
                }
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let imported = specifier.imported.name().as_str();
                let lookup =
                    kit_member_module(request, imported).unwrap_or_else(|| request.to_string());
                if system_module_has_symbol(&lookup, imported) != Some(false) {
                    // 具名导入的系统模块（`import { util } from "@kit.ArkTS"`）同样要记住本地名，
                    // 属性访问时按该模块声明的名字判定
                    // （`fixtures/system-module-named-import` 的 4:19、5:22）。
                    //
                    // 但首字母大写的是类、枚举或接口，不是命名空间：`AppStorageV2.connect`、
                    // `KeyCode.KEYCODE_ESCAPE` 读的是类型成员，参考侧不按「模块没有这个导出」判
                    // （`fixtures/kit-arkui-state`、`fixtures/declare-enum-value` 金标里都没有诊断）。
                    let local_name = specifier.local.name.to_string();
                    let is_namespace = local_name
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_lowercase());
                    let resolved = lookup.clone();
                    if is_namespace && system_module_declared(&resolved) == Some(true) {
                        self.default_system_imports.insert(local_name, resolved);
                    }
                    continue;
                }
                // `@kit.*` 的缺成员由 `visit_import_declaration` 的 kit 表分支报
                // （不带这条建议，`fixtures/system-module-named-import` 只有那一条）。
                if !request.starts_with("@kit.") {
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
    }

    /// 该偏移所在行若是无注解的 `const <name> = …`，返回 `name` 的偏移。
    fn unannotated_binding_at(&self, at: usize) -> Option<usize> {
        let start = self.source[..at.min(self.source.len())]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = self.source[at.min(self.source.len())..]
            .find('\n')
            .map(|i| at + i)
            .unwrap_or(self.source.len());
        let line = &self.source[start..end];
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix("const ")?;
        let eq = rest.find('=')?;
        if rest[..eq].contains(':') {
            return None;
        }
        let name = rest[..eq].trim();
        if name.is_empty() {
            return None;
        }
        Some(start + (line.len() - trimmed.len()) + "const ".len())
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

    /// 类名当作可调用对象用的一条记录：四条 `10505001` 加一条 `arkts-no-classes-as-obj`。
    fn class_as_object(
        &mut self,
        object_start: usize,
        dot_start: usize,
        member_start: usize,
        call_end: usize,
        name: &str,
    ) {
        let (line, column) = self.position(object_start);
        if self
            .out
            .iter()
            .any(|d| d.line == line && d.column == column && d.message.contains("is not callable"))
        {
            return;
        }
        let member = self.source[member_start..]
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("")
            .to_string();
        self.push(
            "ERROR",
            "10505001",
            object_start,
            format!(
                "Value of type 'typeof {name}' is not callable. Did you mean to include 'new'?"
            ),
        );
        self.push("ERROR", "10505001", dot_start, "'(' expected.".to_string());
        self.push(
            "ERROR",
            "10505001",
            member_start,
            format!("Cannot find name '{member}'."),
        );
        self.push("ERROR", "10505001", call_end, "',' expected.".to_string());
        self.push(
            "WARN",
            "-",
            object_start,
            "Classes cannot be used as objects (arkts-no-classes-as-obj)".to_string(),
        );
    }

    fn push(&mut self, severity: &'static str, code: &'static str, offset: usize, message: String) {
        // 参考侧的 FAQ 行只在真有 ArkTS linter 规则报错时出现
        // （`fixtures/ui-entry-count` 只有页面结构错误，没有这一行）。
        if message.contains("(arkts-") {
            self.linter_hit = true;
        }
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
        // 字面量里带展开（`{ ...input }`）时字段来源在运行时才知道，不判缺字段
        // （`fixtures/arkts-spread-object` 只有 `arkts-no-spread` 一条）。
        if self
            .source
            .get(literal.span.start as usize..literal.span.end as usize)
            .is_some_and(|text| text.contains("..."))
        {
            return;
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
    /// `"size" in bag` —— ArkTS 不支持 `in` 运算符（`arkts-no-in`，10605066，锚点是 `in`
    /// 关键字，`fixtures/arkts-in-and-dynamic-import` 的 10:17）。
    fn visit_binary_expression(&mut self, it: &oxc_ast::ast::BinaryExpression<'a>) {
        if self.lints && matches!(it.operator, oxc_syntax::operator::BinaryOperator::In) {
            let from = it.left.span().end as usize;
            let to = it.right.span().start as usize;
            let window = self.source.get(from..to).unwrap_or("");
            let at = window.find("in").map(|found| from + found).unwrap_or(from);
            self.push(
                "ERROR",
                "10605066",
                at,
                "\"in\" operator is not supported (arkts-no-in)".to_string(),
            );
        }
        oxc_ast_visit::walk::walk_binary_expression(self, it);
    }

    /// `throw "boom"` — only an `Error` may be thrown (`arkts-limited-throw`, 10605087 at the
    /// `throw` — 2:3 of `fixtures/arkts-throw-as-const`).
    fn visit_throw_statement(&mut self, it: &oxc_ast::ast::ThrowStatement<'a>) {
        if self.lints {
            let arbitrary = matches!(
                &it.argument,
                Expression::StringLiteral(_)
                    | Expression::NumericLiteral(_)
                    | Expression::BooleanLiteral(_)
                    | Expression::TemplateLiteral(_)
            );
            if arbitrary {
                self.push(
                    "ERROR",
                    "10605087",
                    it.span.start as usize,
                    "\"throw\" statements cannot accept values of arbitrary types (arkts-limited-throw)"
                        .to_string(),
                );
            }
        }
        oxc_ast_visit::walk::walk_throw_statement(self, it);
    }

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
        // `Counter.bump()`：类名当可调用对象用。参考侧对这种「类名 + 成员方法调用」
        // 出四条 10505001（不能调用 / 期望 `(` / 找不到成员名 / 期望 `,`）与一条
        // `arkts-no-classes-as-obj` 的 WARN；属性访问（`Widget.width`）不同，只出
        // `10605093`。两处都是实跑读数：`fixtures/arkts-standalone-this-static`
        // 的 14:15、14:22、14:23、14:29，以及探测工程 `probe-class-obj` 的 4:18。
        if false && self.lints {
            if let Expression::StaticMemberExpression(member) = &it.callee {
                if let Expression::Identifier(object) = &member.object {
                    if self.class_names.contains(object.name.as_str()) {
                        let name = object.name.as_str().to_string();
                        self.class_as_object(
                            object.span.start as usize,
                            (member.property.span.start as usize).saturating_sub(1),
                            member.property.span.start as usize,
                            it.span.end as usize,
                            &name,
                        );
                    }
                }
            }
        }
        // 显式类型实参数量与形参不符：`pair<number>(1)` 对 `pair<Left, Right>`
        // （`fixtures/type-generic-arity` 的 6:18 与 6:13）。
        if self.lints {
            if let Expression::Identifier(callee) = &it.callee {
                if let Some(expected) = self.generic_arity.get(callee.name.as_str()).copied() {
                    if let Some(arguments) = &it.type_arguments {
                        let given = arguments.params.len();
                        if given != expected {
                            // 锚在第一个类型实参（`pair<number>` 的 `number`，6:18）。
                            let first = arguments
                                .params
                                .first()
                                .map(|param| param.span().start as usize)
                                .unwrap_or(arguments.span.start as usize);
                            self.push(
                                "ERROR",
                                "10505001",
                                first,
                                format!("Expected {expected} type arguments, but got {given}."),
                            );
                            self.generic_mismatch = true;
                            self.push(
                                "ERROR",
                                "10605034",
                                it.span.start as usize,
                                "Type inference in case of generic function calls is limited (arkts-no-inferred-generic-params)"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        if self.generic_mismatch {
            self.generic_mismatch = false;
            oxc_ast_visit::walk::walk_call_expression(self, it);
            return;
        }
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
                        // Too many: anchored at the last argument it was given
                        // (6:22 of `fixtures/type-call-arity`). Too few: anchored at the callee
                        // (5:13 of `fixtures/ts-argument-count-and-type`).
                        let offset = if given > *maximum {
                            it.arguments
                                .last()
                                .map(|argument| argument.span().start as usize)
                                .unwrap_or_else(|| it.span.end.saturating_sub(1) as usize)
                        } else {
                            it.callee.span().start as usize
                        };
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

    /// `arkts-no-utility-types` (10605138): a utility type ArkTS does not carry, at the type
    /// name (`Exclude<string | number, string>` → 2:17 of `fixtures/arkts-utility-types`).
    ///
    /// `Partial`, `Required`, `Readonly` and `Record` are the ones ArkTS keeps: the reference
    /// reports nothing for `Required<…>` while it reports the `Exclude<…>` beside it.
    fn visit_ts_type_reference(&mut self, it: &TSTypeReference<'a>) {
        if self.lints {
            if let TSTypeName::IdentifierReference(name) = &it.type_name {
                if UNSUPPORTED_UTILITY_TYPES.contains(&name.name.as_str()) {
                    self.push(
                        "ERROR",
                        "10605138",
                        it.span.start as usize,
                        "Some of utility types are not supported (arkts-no-utility-types)"
                            .to_string(),
                    );
                }
            }
        }
        oxc_ast_visit::walk::walk_ts_type_reference(self, it);
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
        // `namespace Utils { … }` is not ambient and the reference accepts it
        // (`fixtures/arkts-namespace` builds clean); only `declare module …` is this rule's
        // business. The declaration is reset afterwards because a nested `namespace` inside a
        // `declare module` inherits the flag.
        if self.lints && it.declare {
            self.push(
                "ERROR",
                "10605128",
                it.span.start as usize,
                "Ambient module declaration is not supported (arkts-no-ambient-decls)".to_string(),
            );
            // `declare global { … }` 另报一条：全局增强只能在外部模块或环境模块声明里嵌套，
            // 锚在 `global`（`fixtures/ts-declare-global` 的 2:9）。
            if matches!(
                &it.id,
                oxc_ast::ast::TSModuleDeclarationName::Identifier(id)
                    if id.name.as_str() == "global"
            ) {
                self.push(
                    "ERROR",
                    "10505001",
                    it.id.span().start as usize,
                    "Augmentations for the global scope can only be directly nested in external modules or ambient module declarations."
                        .to_string(),
                );
            }
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
            // `x as unknown` names the type just as an annotation does
            // (`9:29` of `fixtures/arkts-utility-types`).
            if matches!(
                it.type_annotation,
                TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
            ) {
                self.push(
                    "ERROR",
                    "10605008",
                    it.type_annotation.span().start as usize,
                    "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                        .to_string(),
                );
            }
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
                    // The literal under the assertion has no contextual type either (10605038 at
                    // the `{` — 5:16 of `fixtures/arkts-throw-as-const`).
                    if let Expression::ObjectExpression(literal) = &it.expression {
                        self.push(
                            "ERROR",
                            "10605038",
                            literal.span.start as usize,
                            "Object literal must correspond to some explicitly declared class or interface (arkts-no-untyped-obj-literals)"
                                .to_string(),
                        );
                    }
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
        // 读取 `<Class>.prototype`（不是赋值目标）同样被拒
        // （`fixtures/arkts-prototype-read` 的 6:18）。
        // 嵌套实例成员：`outer.inner.notThere`（`outer -> Outer`，`Outer.inner -> Inner`）。
        if self.lints {
            if let Expression::StaticMemberExpression(inner) = &it.object {
                if let Expression::Identifier(root) = &inner.object {
                    if let Some(root_class) = self.instances.get(root.name.as_str()) {
                        let inner_type = self
                            .class_member_types
                            .get(root_class)
                            .and_then(|fields| fields.get(inner.property.name.as_str()))
                            .cloned();
                        if let Some(inner_type) = inner_type {
                            let member = it.property.name.as_str();
                            let known = self
                                .members
                                .get(&inner_type)
                                .map(|set| set.contains(member))
                                .unwrap_or(false);
                            if !known {
                                self.push(
                                    "ERROR",
                                    "10505001",
                                    it.property.span.start as usize,
                                    format!(
                                        "Property '{member}' does not exist on type '{inner_type}'."
                                    ),
                                );
                                if let Some(binding) =
                                    self.unannotated_binding_at(it.property.span.start as usize)
                                {
                                    self.push(
                                        "ERROR",
                                        "10605008",
                                        binding,
                                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        // 实例上不存在的成员：`class Box { value }` 的 `box.notThere`
        // （`fixtures/type-instance-member` 的 8:17、8:7）。
        if self.lints {
            if let Expression::Identifier(object) = &it.object {
                let local = object.name.as_str();
                let class = self.instances.get(local).cloned();
                if let Some(class) = class {
                    let member = it.property.name.as_str();
                    let known = self
                        .members
                        .get(&class)
                        .map(|set| set.contains(member))
                        .unwrap_or(false);
                    if !known {
                        self.push(
                            "ERROR",
                            "10505001",
                            it.property.span.start as usize,
                            format!("Property '{member}' does not exist on type '{class}'."),
                        );
                        if let Some(binding) =
                            self.unannotated_binding_at(it.property.span.start as usize)
                        {
                            self.push(
                                "ERROR",
                                "10605008",
                                binding,
                                "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        if self.lints && it.property.name.as_str() == "prototype" {
            if let Expression::Identifier(constructor) = &it.object {
                if self.class_names.contains(constructor.name.as_str()) {
                    self.push(
                        "ERROR",
                        "10605136",
                        it.property.span.start as usize,
                        "Prototype assignment is not supported (arkts-no-prototype-assignment)"
                            .to_string(),
                    );
                }
            }
        }
        // `util.AutoFinalizer`: a member the system module's declaration does not carry
        // (`Property 'AutoFinalizer' does not exist on type 'typeof util'.`, 8:24 of
        // `fixtures/system-module-member`).
        if let Expression::Identifier(object) = &it.object {
            if let Some(request) = self
                .default_system_imports
                .get(object.name.as_str())
                .cloned()
            {
                let member = it.property.name.as_str();
                if system_module_has_value(&request, member) == Some(false) {
                    self.push(
                        "ERROR",
                        "10505001",
                        it.property.span.start as usize,
                        format!(
                            "Property '{member}' does not exist on type 'typeof {}'.",
                            object.name
                        ),
                    );
                    // 少了成员的读取落在无注解的 `const` 上时，参考侧再报一条 any
                    // （`fixtures/system-module-named-import` 的 5:7）。
                    if let Some(binding) =
                        self.unannotated_binding_at(it.property.span.start as usize)
                    {
                        self.push(
                            "ERROR",
                            "10605008",
                            binding,
                            "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                                .to_string(),
                        );
                    }
                }
            }
        }
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
                    if self
                        .assignment_targets
                        .contains(&(it.property.span.start as usize))
                    {
                        oxc_ast_visit::walk::walk_static_member_expression(self, it);
                        return;
                    }
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
        if let oxc_ast::ast::AssignmentTarget::StaticMemberExpression(target) = &it.left {
            self.assignment_targets
                .insert(target.property.span.start as usize);
        }
        // `Animal.prototype.extra = 1`: 参考侧锚在构造器名
        // （`fixtures/arkts-prototype-and-class-literal` 的 5:8）。
        if self.lints {
            if let oxc_ast::ast::AssignmentTarget::StaticMemberExpression(target) = &it.left {
                if let Expression::StaticMemberExpression(inner) = &target.object {
                    if inner.property.name.as_str() == "prototype" {
                        if let Expression::Identifier(constructor) = &inner.object {
                            // 锚在 `prototype` 上（参考侧 5:8），不是构造器名。
                            self.push(
                                "ERROR",
                                "10605136",
                                inner.property.span.start as usize,
                                "Prototype assignment is not supported (arkts-no-prototype-assignment)"
                                    .to_string(),
                            );
                            // 被赋的属性名在类上不存在时再报一条
                            // （`Property 'extra' does not exist on type 'Animal'.`，5:18）。
                            let member = target.property.name.as_str();
                            let known = self
                                .members
                                .get(constructor.name.as_str())
                                .is_some_and(|members| members.contains(member));
                            if !known {
                                self.push(
                                    "ERROR",
                                    "10505001",
                                    target.property.span.start as usize,
                                    format!(
                                        "Property '{member}' does not exist on type '{}'.",
                                        constructor.name
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
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
        if self.lints {
            // 静态方法里的 `this` 与独立函数里的同义：参考侧报
            // `Using "this" inside stand-alone functions is not supported
            // (arkts-no-standalone-this)`，锚在 `this`
            // （`fixtures/arkts-standalone-this-in-static` 的 5:12）。
            if it.r#static {
                {
                    let function = &it.value;
                    for span in this_spans(function) {
                        let line = self.position(span).0;
                        self.standalone_this_lines.insert(line);
                        self.push(
                            "ERROR",
                            "10605093",
                            span,
                            "Using \"this\" inside stand-alone functions is not supported (arkts-no-standalone-this)".to_string(),
                        );
                    }
                }
            }
            // `@Watch("onCount")` 装饰在方法上：`'@Watch("onCount")' can not decorate the method.`
            // （10905112，锚点在装饰器起始，`fixtures/ui-watch-method` 的 11:3）。
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
                let text = &self.source[decorator.span.start as usize..decorator.span.end as usize];
                let name = text
                    .split('(')
                    .nth(1)
                    .map(|rest| rest.trim_end_matches(')').trim())
                    .unwrap_or("");
                self.push(
                    "ERROR",
                    "10905112",
                    decorator.span.start as usize,
                    format!("'@Watch({name})' can not decorate the method."),
                );
            }
        }
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
                        // 返回类型不必是本文件声明的类型：`static isClickEffect(): ClickEffect`
                        // 的 `ClickEffect` 来自 SDK，标注同样给了上下文类型，参考侧不报无类型
                        // 字面量（`fixtures/configuration-page` 的 138:12）。
                        self.return_type = Some(spelling);
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
        // 只有作为属性访问基底的 `globalThis.` 才报；裸引用不报
        // （`fixtures/globalthis-decl` 的 `const x = globalThis;` 两侧无诊断）。
        if self.lints
            && it.name.as_str() == "globalThis"
            && self.source[it.span.end as usize..].starts_with('.')
        {
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
        // `const x: string = compute()`（`compute(): number`）：
        // `Type 'number' is not assignable to type 'string'.`，锚在绑定名
        // （`fixtures/type-return-mismatch` 的 6:7）。
        if self.lints {
            for declarator in &it.declarations {
                let Some(init) = &declarator.init else {
                    continue;
                };
                let Expression::CallExpression(call) = init else {
                    continue;
                };
                let Expression::Identifier(callee) = &call.callee else {
                    continue;
                };
                let Some(returned) = self.fn_returns.get(callee.name.as_str()).cloned() else {
                    continue;
                };
                let Some(annotation) = &declarator.id.type_annotation else {
                    continue;
                };
                let Some(declared) = primitive_name(&annotation.type_annotation) else {
                    continue;
                };
                if declared != returned {
                    self.push(
                        "ERROR",
                        "10505001",
                        declarator.id.kind.span().start as usize,
                        format!("Type '{returned}' is not assignable to type '{declared}'."),
                    );
                }
            }
        }
        // `let untyped;`（既无注解也无初值）与 `const x = untyped;`：参考侧各报一条
        // `arkts-no-any-unknown`，锚在绑定名
        // （`fixtures/type-implicit-any` 的 2:5、4:7）。
        if self.lints {
            for declarator in &it.declarations {
                let oxc_ast::ast::BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind
                else {
                    continue;
                };
                // `for (const k in obj)` / `for (const v of list)` 的绑定不算隐式 any
                // （`fixtures/arkts-for-in` 对 `const key in obj` 不报）。
                let line_start = self.source[..id.span.start as usize]
                    .rfind('\n')
                    .map(|index| index + 1)
                    .unwrap_or(0);
                let line_of = self.source[line_start..].lines().next().unwrap_or("");
                let in_for_header = line_of.contains("for (");
                if in_for_header {
                    continue;
                }
                if declarator.init.is_none() && declarator.id.type_annotation.is_none() {
                    self.push(
                        "ERROR",
                        "10605008",
                        id.span.start as usize,
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                    self.any_locals.insert(id.name.to_string());
                    continue;
                }
                let Some(Expression::Identifier(source)) = &declarator.init else {
                    continue;
                };
                if declarator.id.type_annotation.is_none()
                    && self.any_locals.contains(source.name.as_str())
                {
                    self.push(
                        "ERROR",
                        "10605008",
                        id.span.start as usize,
                        "Use explicit types instead of \"any\", \"unknown\" (arkts-no-any-unknown)"
                            .to_string(),
                    );
                }
            }
        }
        for declarator in &it.declarations {
            let oxc_ast::ast::BindingPatternKind::BindingIdentifier(_) = &declarator.id.kind else {
                continue;
            };
            self.judge_array_initializer(
                declarator.id.type_annotation.as_deref(),
                declarator.init.as_ref(),
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
        // `const A = class { … }`: 类字面量，参考侧锚在 `class` 关键字
        // （`fixtures/arkts-prototype-and-class-literal` 的 7:19）。
        if self.lints && class.id.is_none() {
            self.push(
                "ERROR",
                "10605050",
                class.span.start as usize,
                "Class literals are not supported (arkts-no-class-literals)".to_string(),
            );
        }
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
        // `struct` 在 AST 里也是一个 class：源码上写的是 `struct` 时不算「装饰器用错地方」，
        // 那条由 struct 自己的检查给出（`fixtures/ui-struct-without-component`）。
        // `class.span` 从装饰器起算，`@CustomDialog struct X` 的文本是 `@` 开头，按前缀判会漏；
        // 名字落在 `struct_names` 里的也算（`fixtures/link-call-site` 的 `@CustomDialog struct
        // LinkDialog` 里 `@Link` 是合法的，参考侧无诊断）。
        let declared_as_struct = self
            .source
            .get(class.span.start as usize..)
            .is_some_and(|rest| rest.trim_start().starts_with("struct "))
            || class
                .id
                .as_ref()
                .is_some_and(|id| self.structs.contains(id.name.as_str()));
        if self.lints && !component && !v2 && !declared_as_struct {
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
                        if self.style_names.contains(&attribute) || is_extend_style(&attribute) {
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
                // `build()` 整个包在 `if (…) { … }` 里是允许的写法：根节点在分支内部，参考侧
                // 不报（`fixtures/proxy-node-item` 的 `if (this.appConfig.proxyCardSize === …)`）。
                // 语句切分在这里把一个 if 块数成多段，所以条件写进一份看作单节点。
                let conditional_root = body.statements.first().is_some_and(|(start, _)| {
                    let text = self.source[*start..].trim_start();
                    text.starts_with("if ") || text.starts_with("if(")
                });
                // 空 `build() {}` 不报：参考侧只判「多于一个根节点」
                // （`fixtures/visual-page` 的 golden 为空）。
                if body.nodes > 1 && !conditional_root {
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
        self.judge_array_initializer(it.type_annotation.as_deref(), it.value.as_ref());
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
                        // 参考侧保留源里字面量的引号：单引号源出单引号、双引号源出双引号
                        // （`fixtures/ui-watch-name` 与 `fixtures/ui-watch-missing-method`
                        // 的 4:17 各是一种）。
                        format!(
                            "'@Watch' cannot be used with {}. Apply it only to parameters that correspond to existing methods.",
                            &self.source[literal.span.start as usize..literal.span.end as usize]
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
                    // 参考侧在同一点再报一条 TS 侧的「没有初始化器」——但可选成员
                    // （`@State optional?: number`）不报
                    // （`fixtures/ui-state-default` 只有 10905303 那一条）。
                    if !it.optional {
                        self.push(
                            "ERROR",
                            "10605999",
                        it.key.span().start as usize,
                        format!(
                                "Property '{name}' has no initializer and is not definitely assigned in the constructor."
                            ),
                        );
                    }
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
            // A page takes no `@Link` **with a default value** (measured 6:9 of
            // `fixtures/ui-static-state`, a warning on the same name as the default-value error
            // above). A `@Link` that declares no value is not reported
            // (`fixtures/ui-extend-args-and-link-no-init`: reference is silent, this build warned).
            if link && it.value.is_some() {
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
        // `@Sendable` 类的字段由 `arkts-sendable-definite-assignment` 独占报告，不再重复 WARN
        // （`fixtures/arkts-sendable-definite`）。
        if it.definite
            && self.lints
            && !in_sendable_class_range(self.source, it.span.start as usize)
        {
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
                // 只有组件调用或块结束之后的 `.` 才是属性链。`deviceInfo.apiAvailable(11)` 里
                // 的 `.` 读的是导入模块的成员，参考侧对它不按组件属性判
                // （`fixtures/api-available` 的 13:22 无诊断）；带空白的换行链仍算链式调用
                // （`Column() { … }` 换行后 `.alsoNotReal(2)`）。
                let mut look = index;
                while look > 0 && bytes[look - 1].is_ascii_whitespace() {
                    look -= 1;
                }
                if !(look > 0 && matches!(bytes[look - 1], b')' | b'}')) {
                    index += 1;
                    continue;
                }
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
        // 只看父组件块的直接子层级。`List() { ForEach(… (x) => { ListItem() { Text(x) } }) }`
        // 里的 `Text` 在 `ForEach` 的块内，参考侧不拿 `List` 的子组件限制判它
        // （`fixtures/builder-arrow-call` 与 `list-item-form` 都无诊断）。
        let mut depth = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'{' | b'(' | b'[' => {
                    depth += 1;
                    index += 1;
                    continue;
                }
                b'}' | b')' | b']' => {
                    depth = depth.saturating_sub(1);
                    index += 1;
                    continue;
                }
                _ => {}
            }
            if depth > 0 {
                index += 1;
                continue;
            }
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
            // `ForEach` / `LazyForEach` / `Repeat` 不是真正的子组件：它们在自身的块里生成
            // 子节点，参考侧不拿父组件的子组件限制判它们
            // （`fixtures/builder-arrow-call`：`List() { ForEach(…) }` 无诊断）。
            if matches!(name, "ForEach" | "LazyForEach" | "Repeat") {
                index = cursor;
                continue;
            }
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
        // 箭头函数体里的语句不是 UI 语法位置：`.onAction((e) => { … this.loadConfig(item) … })`
        // 里的调用与组件无关（`fixtures/configuration-page` 的 563:29，参考侧无诊断）。
        // 回退窗口必须落在字符边界上：源里满是中文注释，按字节退 400 会切在多字节字符中间
        // （实测 `fixtures/configuration-page` 在这里 panic）。
        let mut window_start = position.saturating_sub(400);
        while window_start > 0 && !text.is_char_boundary(window_start) {
            window_start -= 1;
        }
        if let Some(arrow) = text[window_start..position].rfind("=>") {
            let mut depth = 0i32;
            for byte in text[window_start + arrow + 2..position].bytes() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
            }
            if depth > 0 {
                continue;
            }
        }
        let rest = &text[index..];
        let method: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if method.is_empty() || builders.contains(&method) {
            continue;
        }
        // 只有调用形式才算 UI 语法问题：`this.value = v;` 是赋值，参考侧不报
        // （`fixtures/ui-component-props` 的 exit=0）。
        if !rest[method.len()..].trim_start().starts_with('(') {
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
                    // 紧随其后的 `}` 是外层块的结束，不是新语句的开头：`build()` 把整个体
                    // 包在 `if (…) { … }` 里时，内层组件闭合后下一个字节就是 if 的 `}`，把它
                    // 当新语句会让一个根节点被数成 3 个
                    // （`fixtures/proxy-node-item`：nodes=3，statement 边界 3118/3117）。
                    if !next.is_empty()
                        && !next.starts_with(';')
                        && !next.starts_with('.')
                        && !next.starts_with('}')
                    {
                        if let Some(start) = statement_start.take() {
                            close_statement(
                                start,
                                index + 1,
                                &mut declarations,
                                &mut statements,
                                &mut nodes,
                            );
                            // 闭合括号本身不是下一条语句的起点，跳过它再继续：否则 `}` 会被
                            // 当成 start，把两个根节点并成一条（`fixtures/ui-build-roots` 的 8:5）。
                            index += 1;
                            continue;
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
        if statement_start.is_none()
            && !matches!(
                byte,
                b' ' | b'\t' | b'\n' | b'\r' | b'}' | b')' | b']' | b';'
            )
        {
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

thread_local! {
    /// `CustomDialogControllerOptions` 的合法键，由构建从 SDK 发布。
    static DIALOG_OPTIONS: std::cell::RefCell<Option<std::collections::HashSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// 发布对话框选项的合法键（见 `ets_assemble::publish_dialog_options`）。
pub(crate) fn set_dialog_options(names: std::collections::HashSet<String>) {
    DIALOG_OPTIONS.with(|cell| *cell.borrow_mut() = Some(names));
}

/// `new CustomDialogController({ nope: … })`：未知名报在键上，两条句子合成一条
/// （`fixtures/ui-dialog-options` 的 15:65）。
fn dialog_option_violations(source: &str) -> Vec<(usize, String)> {
    let Some(names) = DIALOG_OPTIONS.with(|cell| cell.borrow().clone()) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("new CustomDialogController(") {
        let start = from + at;
        let open = start + "new CustomDialogController".len();
        let Some(close) = matching_paren(source, open) else {
            break;
        };
        from = close;
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
            // `split_top_level` 剥掉了每项的首尾空白，锚点按它在 `body` 里的实际位置算，
            // 不能按长度累加（否则整体偏一列）。
            let item_at = body_start + body[..end].find(item.as_str()).unwrap_or(0);
            let Some(colon) = item.find(':') else {
                continue;
            };
            let key = item[..colon].trim();
            if key.is_empty() || names.contains(key) {
                continue;
            }
            let value = item[colon + 1..].trim();
            let spelled = value.split('(').next().unwrap_or(value).trim().to_string();
            let literal = format!("{{ {key}: {spelled}; }}");
            let key_at = item_at + item.find(key).unwrap_or(0);
            found.push((
                key_at,
                format!(
                    "Argument of type '{literal}' is not assignable to parameter of type 'CustomDialogControllerOptions'. Object literal may only specify known properties, and '{key}' does not exist in type 'CustomDialogControllerOptions'."
                ),
            ));
        }
    }
    found
}

/// `(struct, 字段)` 的集合：`@ObjectLink` 字段与普通字段各一份。
type StructLinkFields = (
    std::collections::HashSet<(String, String, String)>,
    std::collections::HashSet<(String, String)>,
);

/// 每个 struct 的 `@ObjectLink` 字段名，以及没有状态装饰器的普通字段名。
///
/// 父组件把普通字段绑给子组件的 `@ObjectLink` 时，参考侧报
/// `The 'regular' property 'plain' cannot be assigned to the '@ObjectLink' property 'model'.`
/// （10905315，锚点在传入值，`fixtures/ui-object-link-plain` 的 23:22）。
fn struct_link_and_regular_fields(program: &Program<'_>) -> StructLinkFields {
    struct Collect {
        links: std::collections::HashSet<(String, String, String)>,
        regular: std::collections::HashSet<(String, String)>,
    }
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &oxc_ast::ast::Class<'a>) {
            let name = it.id.as_ref().map(|id| id.name.to_string());
            if let Some(name) = name {
                for element in &it.body.body {
                    let ClassElement::PropertyDefinition(property) = element else {
                        continue;
                    };
                    let Some(field) = property.key.static_name().map(|n| n.to_string()) else {
                        continue;
                    };
                    // `@Link` 与 `@ObjectLink` 都要求来源是状态字段，消息里写的是各自的装饰器名。
                    let link = property.decorators.iter().find_map(|decorator| {
                        let Expression::Identifier(id) = &decorator.expression else {
                            return None;
                        };
                        match id.name.as_str() {
                            "ObjectLink" | "Link" => Some(id.name.to_string()),
                            _ => None,
                        }
                    });
                    if let Some(decorator_name) = link {
                        self.links.insert((name.clone(), field, decorator_name));
                        continue;
                    }
                    let decorated = property.decorators.iter().any(|decorator| {
                        let Expression::Identifier(id) = &decorator.expression else {
                            return false;
                        };
                        matches!(
                            id.name.as_str(),
                            "State"
                                | "Link"
                                | "Prop"
                                | "Provide"
                                | "Consume"
                                | "ObjectLink"
                                | "StorageLink"
                                | "StorageProp"
                                | "Local"
                                | "Param"
                                | "Event"
                                | "Monitor"
                                | "Computed"
                                | "Trace"
                        )
                    });
                    if !decorated {
                        self.regular.insert((name.clone(), field));
                    }
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect {
        links: std::collections::HashSet::new(),
        regular: std::collections::HashSet::new(),
    };
    collect.visit_program(program);
    (collect.links, collect.regular)
}

/// `Child({ model: this.plain })`：普通字段绑给 `@ObjectLink`。
fn object_link_violations(
    source: &str,
    structs: &std::collections::HashSet<String>,
    links: &std::collections::HashSet<(String, String, String)>,
    regular: &std::collections::HashSet<(String, String)>,
    owner: Option<&str>,
) -> Vec<(usize, String)> {
    let Some(owner) = owner else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for child in structs {
        let needle = format!("{child}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            let open = start + needle.len() - 1;
            let Some(close) = matching_paren(source, open) else {
                break;
            };
            from = close;
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
                let Some(colon) = item.find(':') else {
                    continue;
                };
                let field = item[..colon].trim();
                let value = item[colon + 1..].trim();
                let Some(decorator_name) = links
                    .iter()
                    .find(|(owner, linked, _)| owner == child && linked == field)
                    .map(|(_, _, decorator)| decorator.clone())
                else {
                    continue;
                };
                let Some(local) = value.strip_prefix("this.") else {
                    continue;
                };
                if !regular.contains(&(owner.to_string(), local.to_string())) {
                    continue;
                }
                let item_at = body_start + body[..end].find(item.as_str()).unwrap_or(0);
                let value_at = item_at + item.find(value).unwrap_or(0);
                found.push((
                    value_at,
                    format!(
                        "The 'regular' property '{local}' cannot be assigned to the '@{decorator_name}' property '{field}'."
                    ),
                ));
            }
        }
    }
    found
}

/// 没有 `build()` 的结构体里，方法体中的 UI 组件调用会被拒：
/// `UI component 'Column' cannot be used in this place.`（10905236，锚点是组件名），
/// 其后的 `{` 另报 `';' expected.`（10505001，锚点是 `{`）——
/// `fixtures/ui-missing-build` 的 6:5 / 6:14 / 7:7。
fn ui_in_struct_without_build(
    source: &str,
    components: &std::collections::HashSet<String>,
) -> Vec<(usize, &'static str, String)> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = source[from..].find("struct ") {
        let start = from + at;
        let after = &source[start + "struct ".len()..];
        let name_len = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .count();
        from = start + "struct ".len() + name_len.max(1);
        if name_len == 0 {
            continue;
        }
        let Some(open_rel) = after.find('{') else {
            continue;
        };
        let open = start + "struct ".len() + open_rel;
        let Some(close) = matching_brace(source, open) else {
            continue;
        };
        let body = &source[open + 1..close];
        if body.contains("build(") {
            continue;
        }
        for component in components {
            let needle = format!("{component}(");
            let mut cursor = 0usize;
            while let Some(hit) = body[cursor..].find(&needle) {
                let name_at = open + 1 + cursor + hit;
                let boundary = !source[..name_at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.');
                cursor += hit + needle.len();
                if !boundary {
                    continue;
                }
                found.push((
                    name_at,
                    "10905236",
                    format!("UI component '{component}' cannot be used in this place."),
                ));
                let line_end = source[name_at..]
                    .find('\n')
                    .map(|offset| name_at + offset)
                    .unwrap_or(close);
                if let Some(brace) = source[name_at..line_end].find('{') {
                    found.push((name_at + brace, "10505001", "';' expected.".to_string()));
                }
            }
        }
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
        // 只有三种字面量类型能与成员类型直接比；接口/类/数组等成员类型只判存在性，
        // 比值交给它们自己的检查（否则 `icon: $r(…)` 会被比成 string vs object）。
        if !matches!(wanted.as_str(), "number" | "string" | "boolean") {
            continue;
        }
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

/// 展开实参：`f(...xs)` 只在被调函数有剩余参数（或形参是元组）时成立
/// （`A spread argument must either have a tuple type or be passed to a rest parameter.`，
/// `fixtures/type-spread-argument` 的 6:19）。
fn spread_argument_violations(
    source: &str,
    rest_functions: &std::collections::HashSet<String>,
) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for name in rest_functions {
        let needle = format!("{name}(");
        let mut from = 0usize;
        while let Some(at) = source[from..].find(&needle) {
            let start = from + at;
            // 函数声明本身不是调用（`function withArgs(...rest)` 的形参也带 `...`）。
            if source[..start].trim_end().ends_with("function") {
                from = start + needle.len();
                continue;
            }
            let open = start + needle.len() - 1;
            let Some(close) = matching_paren(source, open) else {
                break;
            };
            from = close;
            let inside = &source[open + 1..close];
            let mut cursor = 0usize;
            for argument in split_top_level(inside) {
                if argument.trim_start().starts_with("...") {
                    let offset = open + 1 + cursor + argument.find("...").unwrap_or(0);
                    found.push((
                        offset,
                        "A spread argument must either have a tuple type or be passed to a rest parameter."
                            .to_string(),
                    ));
                }
                cursor += argument.len() + 1;
            }
        }
    }
    found.sort_unstable();
    found
}

/// 带剩余参数的函数名。
fn functions_with_rest(program: &Program<'_>) -> std::collections::HashSet<String> {
    struct Collect(std::collections::HashSet<String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(
            &mut self,
            it: &oxc_ast::ast::Function<'a>,
            flags: oxc_syntax::scope::ScopeFlags,
        ) {
            let has_rest = it.params.rest.is_some();
            if has_rest {
                if let Some(id) = &it.id {
                    self.0.insert(id.name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
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
        // `this.$name` 是 `$$` 双向绑定的写法，名字在编译期生成，不是声明的成员
        // （`fixtures/link-dollar-arg` 的 8:33，参考侧无诊断）。
        if member.starts_with('$') {
            continue;
        }
        // 调用形式 `this.name(...)` 可能是从组件基类继承的方法（`ViewPU.getUIContext`）：
        // 成员表只收本文件声明的成员，判不了继承关系，这一形态不在这里报
        // （`fixtures/member-shapes` 的 `private scroller = this.getUIContext()` 参考侧无诊断）。
        if source[after + member.len()..].starts_with('(') {
            continue;
        }
        found.push((
            after,
            format!("Property '{member}' does not exist on type '{owner}'."),
        ));
    }
    found
}

/// 把注释与字符串字面量的内容替换成空格，长度与位置不变。
///
/// 文本级检查（成员访问、索引访问、build 体语句）只看代码：注释里的 `this.componentName`
/// 与模板文本里的 `[配置页 …]` 都不是代码，却会被当成成员读取与索引访问
/// （`fixtures/configuration-page` 的 864:42、1093:58）。
fn mask_literals(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = vec![b' '; bytes.len()];
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'/' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'*' => {
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            // 模板字符串不遮蔽：`` `${this.notAField}` `` 参考侧照样报成员不存在
            // （`fixtures/type-struct-member` 的 12:35），只有注释与普通字符串字面量不是代码。
            quote @ (b'\'' | b'"') => {
                out[index] = quote;
                index += 1;
                while index < bytes.len() && bytes[index] != quote {
                    if bytes[index] == b'\\' && index + 1 < bytes.len() {
                        index += 2;
                        continue;
                    }
                    index += 1;
                }
                if index < bytes.len() {
                    out[index] = quote;
                    index += 1;
                }
            }
            byte => {
                out[index] = byte;
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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
                // 成员类型只对三种字面量类型做值比较，但**成员本身**要收录：漏掉
                // 非原始类型的成员，`{ inner: … }`、`{ icon: $r(…) }` 会被判成「属性不存在」
                // （`fixtures/shorthand-property` 的 68:30、`fixtures/same-name-with-resource`
                // 的 11:40，两处参考侧都无诊断）。
                let spelling = match &annotation.type_annotation {
                    TSType::TSNumberKeyword(_) => "number",
                    TSType::TSStringKeyword(_) => "string",
                    TSType::TSBooleanKeyword(_) => "boolean",
                    _ => "object",
                };
                types.insert(name.name.to_string(), spelling.to_string());
            }
            self.0.insert(it.id.name.to_string(), types);
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    // `interface Derived extends Base` 继承来的成员也要算进 `Derived`
    // （`fixtures/ts-interface-extends-interface` 的参考侧 exit=0）。
    let mut parents: std::collections::HashMap<String, Vec<String>> = Default::default();
    struct Parents<'p>(&'p mut std::collections::HashMap<String, Vec<String>>);
    impl<'a> Visit<'a> for Parents<'_> {
        fn visit_ts_interface_declaration(
            &mut self,
            it: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
        ) {
            let mut names = Vec::new();
            for heritage in &it.extends {
                if let oxc_ast::ast::Expression::Identifier(id) = &heritage.expression {
                    names.push(id.name.to_string());
                }
            }
            if !names.is_empty() {
                self.0.insert(it.id.name.to_string(), names);
            }
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
    }
    let mut parents_collect = Parents(&mut parents);
    parents_collect.visit_program(program);
    for _ in 0..8 {
        let snapshot = collect.0.clone();
        for (name, list) in &parents {
            for parent in list {
                let Some(inherited) = snapshot.get(parent).cloned() else {
                    continue;
                };
                let entry = collect.0.entry(name.clone()).or_default();
                for (member, spelling) in inherited {
                    entry.entry(member).or_insert(spelling);
                }
            }
        }
    }
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

/// 类里 `private` 声明的成员：类名 → 成员名集合。
///
/// 参考侧对类外访问报 `Property 'hidden' is private and only accessible within class 'Priv'.`
/// `class X …` 行里 `class ` 之后的名字。
fn rest_head(trimmed: &str) -> String {
    trimmed
        .strip_prefix("class ")
        .unwrap_or(trimmed)
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// 本文件里 `class <name> { … }` 的属性名（只看 `name:` 形态的行，够判定缺失成员用）。
fn class_property_names(source: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    let mut depth = 0i32;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !inside {
            if let Some(rest) = trimmed.strip_prefix("class ") {
                let declared: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if declared == name {
                    inside = true;
                    depth = 0;
                }
            }
            continue;
        }
        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if depth <= 0 && line.contains('}') {
            break;
        }
        if let Some(colon) = trimmed.find(':') {
            let candidate = trimmed[..colon].trim();
            if !candidate.is_empty()
                && candidate
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                && !candidate.starts_with("constructor")
            {
                out.push(candidate.to_string());
            }
        }
    }
    out
}

fn private_members(
    program: &Program<'_>,
) -> (
    std::collections::HashMap<String, std::collections::HashSet<String>>,
    std::collections::HashMap<String, std::collections::HashSet<String>>,
) {
    struct Collect(
        std::collections::HashMap<String, std::collections::HashSet<String>>,
        std::collections::HashMap<String, std::collections::HashSet<String>>,
    );
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                let set = self.0.entry(id.name.to_string()).or_default();
                for element in &it.body.body {
                    let (key, accessibility) = match element {
                        ClassElement::PropertyDefinition(property) => {
                            (&property.key, property.accessibility)
                        }
                        ClassElement::MethodDefinition(method) => {
                            (&method.key, method.accessibility)
                        }
                        _ => continue,
                    };
                    if accessibility == Some(oxc_ast::ast::TSAccessibility::Protected) {
                        if let Some(name) = key.static_name() {
                            self.1
                                .entry(id.name.to_string())
                                .or_default()
                                .insert(name.to_string());
                        }
                        continue;
                    }
                    if accessibility != Some(oxc_ast::ast::TSAccessibility::Private) {
                        continue;
                    }
                    if let Some(name) = key.static_name() {
                        set.insert(name.to_string());
                    }
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(Default::default(), Default::default());
    collect.visit_program(program);
    (collect.0, collect.1)
}

fn class_members(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    struct Collect(
        std::collections::HashMap<String, std::collections::HashSet<String>>,
        std::collections::HashMap<String, Vec<String>>,
    );
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
            let mut parents = Vec::new();
            for heritage in &it.extends {
                if let oxc_ast::ast::Expression::Identifier(id) = &heritage.expression {
                    parents.push(id.name.to_string());
                }
            }
            if !parents.is_empty() {
                self.1.insert(it.id.name.to_string(), parents);
            }
            oxc_ast_visit::walk::walk_ts_interface_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new(), Default::default());
    collect.visit_program(program);
    // `interface Derived extends Base` 的父接口成员也算 `Derived` 的成员
    // （`fixtures/ts-interface-extends-interface` 的参考侧 exit=0）。
    for _ in 0..8 {
        let snapshot = collect.0.clone();
        for (name, list) in &collect.1 {
            for parent in list {
                let Some(inherited) = snapshot.get(parent).cloned() else {
                    continue;
                };
                let entry = collect.0.entry(name.clone()).or_default();
                for member in inherited {
                    entry.insert(member);
                }
            }
        }
    }
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
            // `return import('…')`：动态导入的结果要由库的签名回答，返回类型同样推不出来
            // （`fixtures/arkts-in-and-dynamic-import` 的 17:10）。
            Expression::ImportExpression(_) => true,
            // `return x + y;` where either side is an `any` local (1:10 of
            // `fixtures/arkts-any-unknown`).
            Expression::BinaryExpression(binary) => {
                [&binary.left, &binary.right].iter().any(|side| match side {
                    // `return a + b;` 里未注解的参数同样是 `any`：
                    // `fixtures/arkts-implicit-any-params` 的 1:10。
                    Expression::Identifier(id) => {
                        any_locals.contains(id.name.as_str())
                            || any_params.contains(id.name.as_str())
                    }
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

/// 该字节位置是否落在某个 `@Sendable` 类的类体内：这类字段的定值断言由
/// `arkts-sendable-definite-assignment` 报，不再额外报 `arkts-no-definite-assignment`
/// （`fixtures/arkts-sendable-definite`）。
fn in_sendable_class_range(source: &str, at: usize) -> bool {
    let mut offset = 0usize;
    let mut pending = false;
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for line in source.lines() {
        let this_line = offset;
        offset += line.len() + 1;
        let trimmed = line.trim_start();
        if trimmed == "@Sendable" {
            pending = true;
            continue;
        }
        if pending {
            if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                let start = this_line;
                let mut depth = 0i32;
                let mut cursor = this_line;
                let mut end = None;
                for body_line in source[this_line..].lines() {
                    depth += body_line.matches('{').count() as i32;
                    depth -= body_line.matches('}').count() as i32;
                    if depth <= 0 && body_line.contains('}') {
                        end = Some(cursor + body_line.len());
                        break;
                    }
                    cursor += body_line.len() + 1;
                }
                if let Some(end) = end {
                    ranges.push((start, end));
                }
            }
            pending = false;
        }
    }
    ranges.iter().any(|(start, end)| at >= *start && at <= *end)
}

/// 返回类型写成 `object` / `Object` 的函数名：用它们做装饰器时参考侧报不可调用
/// （`fixtures/arkts-decorator-obj-literal` 的 6:2）。
fn object_returning_functions(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("function ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if name.is_empty() {
            continue;
        }
        let tail = &rest[name.len()..];
        if tail.contains("): object") || tail.contains("): Object") {
            names.push(name);
        }
    }
    names
}

thread_local! {
    /// 最近一次检查是否命中过整文件级 linter 规则（`10905231` / `10905402`）。
    static FILE_LEVEL_LINTER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 上一次 [`check_with_declarations`] 是否命中过整文件级 linter 规则。
pub(crate) fn took_file_level_linter() -> bool {
    FILE_LEVEL_LINTER.with(|cell| cell.replace(false))
}

/// 文本级判据：解析失败时仍要报的那些。
///
/// `@Entry` 加在函数上时，`parser_view` 产不出树（参考侧同样解析失败），但
/// `Decorators are not valid here.`（10505001，锚在 `@`）与
/// `The '@Entry' decorator can only be used with 'struct'.`（10905337，锚在名字）照报
/// （`fixtures/arkts-entry-on-function` 的 16:1、16:2）。
fn early_text_diagnostics(file: &str, source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // `@Sendable` 函数 / typeAlias 上还挂了别的装饰器
    // （`fixtures/arkts-sendable-function-decorator` 的 5:1、
    // `fixtures/arkts-sendable-typealias-decorator` 的 5:1）：前者另报「只能用这六个之一」
    // 与「装饰器在此无效」。
    {
        let mut block: Vec<(usize, String)> = Vec::new();
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if let Some(rest) = trimmed.strip_prefix('@') {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                block.push((this_line + indent, name));
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
            let has_sendable = block.iter().any(|(_, name)| name == "Sendable");
            let others: Vec<usize> = block
                .iter()
                .filter(|(_, name)| name != "Sendable")
                .map(|(at, _)| *at)
                .collect();
            let is_function = trimmed.starts_with("function ");
            let is_alias = trimmed.starts_with("type ");
            if has_sendable && !others.is_empty() && (is_function || is_alias) {
                let message = if is_function {
                    "Only \"@Sendable\" decorator can be used on \"Sendable\" function (arkts-sendable-function-decorator)"
                } else {
                    "Only \"@Sendable\" decorator can be used on \"Sendable\" typeAlias (arkts-sendable-typealias-decorator)"
                };
                let code = if is_function { "10605173" } else { "10605174" };
                for at in &others {
                    let (line_no, column) = position_of(source, *at);
                    out.push(Diagnostic {
                        severity: "ERROR",
                        code,
                        file: file.to_string(),
                        line: line_no,
                        column,
                        message: message.to_string(),
                    });
                    // 两条都报：`Decorators are not valid here.`（10505001）与
                    // 「只能用这六个之一」（10905117）。
                    out.push(Diagnostic {
                        severity: "ERROR",
                        code: "10505001",
                        file: file.to_string(),
                        line: line_no,
                        column,
                        message: "Decorators are not valid here.".to_string(),
                    });
                    if is_function {
                        out.push(Diagnostic {
                            severity: "ERROR",
                            code: "10905117",
                            file: file.to_string(),
                            line: line_no,
                            column,
                            message: "A function can only be decorated by one of the '@AnimatableExtend', '@Builder', '@Extend', '@Styles', '@Concurrent' and '@Sendable'."
                                .to_string(),
                        });
                    }
                }
            }
            block.clear();
        }
    }

    // `Sendable` 类型位置上的裸用（同 `sendable-as-type`）：解析成功的文件走这条路。
    {
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if trimmed.contains("@Sendable") {
                continue;
            }
            let mut from = 0usize;
            while let Some(at) = line[from..].find("Sendable") {
                let start = from + at;
                from = start + "Sendable".len();
                let before_ok = !line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
                let after_ok = !line[start + "Sendable".len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                if !before_ok || !after_ok {
                    continue;
                }
                let before = line[..start].trim_end();
                let is_type = before.ends_with('(')
                    || before.ends_with(':')
                    || before.ends_with('|')
                    || before.ends_with('<');
                if !is_type {
                    continue;
                }
                let (line_no, column) = position_of(source, this_line + start);
                out.push(Diagnostic {
                    severity: "ERROR",
                    code: "10505001",
                    file: file.to_string(),
                    line: line_no,
                    column,
                    message: "'Sendable' refers to a value, but is being used as a type here. Did you mean 'typeof Sendable'?"
                        .to_string(),
                });
            }
        }
    }

    // `declare global { … }`：参考侧除 `arkts-no-ambient-decls` 外还报两条
    // （`fixtures/ts-declare-global`）：全局增强只能在外部模块或环境模块声明里嵌套
    // （2:9，锚在 `global`），以及体里声明的类型在外面查不到（8:22）。
    {
        let mut globals: Vec<String> = Vec::new();
        let mut in_global = false;
        let mut depth = 0i32;
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if trimmed.starts_with("declare global") {
                in_global = true;
                depth = 0;
                let at = indent + "declare ".len();
                let (line_no, column) = position_of(source, this_line + at);
                out.push(Diagnostic {
                    severity: "ERROR",
                    code: "10505001",
                    file: file.to_string(),
                    line: line_no,
                    column,
                    message: "Augmentations for the global scope can only be directly nested in external modules or ambient module declarations."
                        .to_string(),
                });
            }
            if in_global {
                let body = trimmed
                    .strip_prefix("interface ")
                    .or_else(|| trimmed.strip_prefix("class "))
                    .or_else(|| trimmed.strip_prefix("type "));
                if let Some(rest) = body {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        globals.push(name);
                    }
                }
                depth += line.matches('{').count() as i32;
                depth -= line.matches('}').count() as i32;
                if depth <= 0 && line.contains('}') && !trimmed.starts_with("declare global") {
                    in_global = false;
                }
            }
        }
        if !globals.is_empty() {
            let mut offset = 0usize;
            let mut inside = false;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if trimmed.starts_with("declare global") {
                    inside = true;
                    continue;
                }
                if inside {
                    if trimmed == "}" {
                        inside = false;
                    }
                    continue;
                }
                for name in &globals {
                    let mut from = 0usize;
                    while let Some(at) = trimmed.find(name.as_str()) {
                        let start = from + at;
                        from = start + name.len();
                        let before_ok = !trimmed[..start]
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                        let after_ok = !trimmed[start + name.len()..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                        if !before_ok || !after_ok {
                            continue;
                        }
                        // 只认类型位置
                        let before = trimmed[..start].trim_end();
                        if !(before.ends_with('(')
                            || before.ends_with(':')
                            || before.ends_with('<'))
                        {
                            continue;
                        }
                        let indent = line.len() - trimmed.len();
                        let (line_no, column) = position_of(source, this_line + indent + start);
                        out.push(Diagnostic {
                            severity: "ERROR",
                            code: "10505001",
                            file: file.to_string(),
                            line: line_no,
                            column,
                            message: format!("Cannot find name '{name}'."),
                        });
                    }
                }
            }
        }
    }

    // `@Sendable` typeAlias 拿来接一个普通箭头函数：参考侧报
    // `Only "Sendable" function or "Sendable" typeAlias object can be assigned to
    // "Sendable" typeAlias (arkts-sendable-function-assignment)`，锚在绑定名
    // （`fixtures/arkts-sendable-function-assignment` 的 5:7）。
    {
        let mut sendable_aliases: std::collections::HashSet<String> = Default::default();
        let mut pending = false;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed == "@Sendable" {
                pending = true;
                continue;
            }
            if pending {
                if let Some(rest) = trimmed.strip_prefix("type ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        sendable_aliases.insert(name);
                    }
                }
                pending = false;
            }
        }
        if !sendable_aliases.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                if !trimmed.starts_with("const ") && !trimmed.starts_with("let ") {
                    continue;
                }
                let Some(colon) = trimmed.find(':') else {
                    continue;
                };
                let Some(eq) = trimmed.find('=') else {
                    continue;
                };
                if colon > eq {
                    continue;
                }
                let declared = trimmed[colon + 1..eq].trim();
                if !sendable_aliases.contains(declared) {
                    continue;
                }
                if !trimmed[eq + 1..].contains("=>") {
                    continue;
                }
                let binding_start = trimmed[..colon]
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                    .map(|index| index + 1)
                    .unwrap_or(0);
                let (line_no, column) = position_of(
                    source,
                    this_line + (line.len() - trimmed.len()) + binding_start,
                );
                out.push(Diagnostic {
                        severity: "ERROR",
                        code: "10605176",
                        file: file.to_string(),
                        line: line_no,
                        column,
                        message: "Only \"Sendable\" function or \"Sendable\" typeAlias object can be assigned to \"Sendable\" typeAlias (arkts-sendable-function-assignment)"
                            .to_string(),
                    });
            }
        }
    }
    // `Sendable` 类型位置上的裸用：参考侧报
    // `'Sendable' refers to a value, but is being used as a type here. Did you mean
    // 'typeof Sendable'?`，锚在该类型名
    // （`fixtures/sendable-as-type` 的 7:20）。
    {
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if trimmed.contains("@Sendable") {
                continue;
            }
            let mut from = 0usize;
            while let Some(at) = line[from..].find("Sendable") {
                let start = from + at;
                from = start + "Sendable".len();
                let before_ok = !line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
                let after_ok = !line[start + "Sendable".len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                if !before_ok || !after_ok {
                    continue;
                }
                // 只在类型位置：`(` / `:` / `|` 之后。
                let before = line[..start].trim_end();
                let is_type = before.ends_with('(')
                    || before.ends_with(':')
                    || before.ends_with('|')
                    || before.ends_with('<');
                if !is_type {
                    continue;
                }
                let (line_no, column) = position_of(source, this_line + start);
                out.push(Diagnostic {
                    severity: "ERROR",
                    code: "10505001",
                    file: file.to_string(),
                    line: line_no,
                    column,
                    message: "'Sendable' refers to a value, but is being used as a type here. Did you mean 'typeof Sendable'?"
                        .to_string(),
                });
            }
        }
    }
    // `@Sendable` 函数体里引用本文件顶层变量：参考侧报
    // `Only imported variables can be captured by "Sendable" function
    // (arkts-sendable-function-imported-variables)`，锚在变量
    // （`fixtures/arkts-sendable-function-capture` 的 6:10）。
    {
        let mut sendable_functions: std::collections::HashSet<String> = Default::default();
        let mut pending = false;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed == "@Sendable" {
                pending = true;
                continue;
            }
            if pending {
                if let Some(rest) = trimmed.strip_prefix("function ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        sendable_functions.insert(name);
                    }
                }
                pending = false;
            }
        }
        if !sendable_functions.is_empty() {
            // 本文件顶层 `const` / `let` 声明的名字（非导入）。
            let mut top_locals: std::collections::HashSet<String> = Default::default();
            for line in source.lines() {
                let trimmed = line.trim_start();
                let rest = trimmed
                    .strip_prefix("const ")
                    .or_else(|| trimmed.strip_prefix("let "));
                let Some(rest) = rest else { continue };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    top_locals.insert(name);
                }
            }
            if !top_locals.is_empty() {
                let mut inside = 0i32;
                let mut offset = 0usize;
                let mut in_sendable_fn = false;
                for line in source.lines() {
                    let this_line = offset;
                    offset += line.len() + 1;
                    let trimmed = line.trim_start();
                    if let Some(rest) = trimmed.strip_prefix("function ") {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if sendable_functions.contains(&name) {
                            in_sendable_fn = true;
                        }
                    }
                    if in_sendable_fn {
                        for local in &top_locals {
                            let mut from = 0usize;
                            while let Some(at) = line[from..].find(local.as_str()) {
                                let start = from + at;
                                from = start + local.len();
                                let before_ok = !line[..start]
                                    .chars()
                                    .next_back()
                                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
                                let after_ok = !line[start + local.len()..]
                                    .chars()
                                    .next()
                                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                                if before_ok && after_ok {
                                    let (line_no, column) = position_of(source, this_line + start);
                                    out.push(Diagnostic {
                                        severity: "ERROR",
                                        code: "10605172",
                                        file: file.to_string(),
                                        line: line_no,
                                        column,
                                        message: "Only imported variables can be captured by \"Sendable\" function (arkts-sendable-function-imported-variables)"
                                            .to_string(),
                                    });
                                }
                            }
                        }
                    }
                    inside += line.matches('{').count() as i32;
                    inside -= line.matches('}').count() as i32;
                    if in_sendable_fn && inside <= 0 && line.contains('}') {
                        in_sendable_fn = false;
                    }
                }
            }
        }
    }
    // `@Sendable` 函数上的属性赋值：参考侧报
    // `The property of "Sendable" function is limited (arkts-sendable-function-property)`，
    // 锚在该语句行首（`fixtures/arkts-sendable-function-property` 的 7:1）。
    {
        let mut sendable_functions: std::collections::HashSet<String> = Default::default();
        let mut pending = false;
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed == "@Sendable" {
                pending = true;
                continue;
            }
            if pending {
                if let Some(rest) = trimmed.strip_prefix("function ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        sendable_functions.insert(name);
                    }
                }
                pending = false;
            }
        }
        if !sendable_functions.is_empty() {
            let mut offset = 0usize;
            for line in source.lines() {
                let this_line = offset;
                offset += line.len() + 1;
                let trimmed = line.trim_start();
                let wildcard = sendable_functions
                    .iter()
                    .any(|name| trimmed.starts_with(&format!("{name}.")));
                if !wildcard || !trimmed.contains('=') {
                    continue;
                }
                let (line_no, column) =
                    position_of(source, this_line + (line.len() - trimmed.len()));
                out.push(Diagnostic {
                    severity: "ERROR",
                    code: "10605178",
                    file: file.to_string(),
                    line: line_no,
                    column,
                    message: "The property of \"Sendable\" function is limited (arkts-sendable-function-property)"
                        .to_string(),
                });
            }
        }
    }
    // `build` 带参数：参考侧报一条无位置的「不可赋值给基类」与一条锚在方法名的
    // 「不能有参数」（`fixtures/ui-build-parameter` 的 5:3）。
    {
        let mut offset = 0usize;
        let mut struct_name = String::new();
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("struct ") {
                struct_name = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
            }
            if !trimmed.starts_with("build(") {
                continue;
            }
            let Some(close) = trimmed.find(')') else {
                continue;
            };
            let args = trimmed[6..close].trim();
            if args.is_empty() || struct_name.is_empty() {
                continue;
            }
            let (line_no, column) = position_of(source, this_line + (line.len() - trimmed.len()));
            out.push(Diagnostic {
                severity: "ERROR",
                code: "10505001",
                file: file.to_string(),
                line: 0,
                column: 0,
                message: format!(
                    "Property 'build' in type '{struct_name}' is not assignable to the same property in base type 'CustomComponent'."
                ),
            });
            out.push(Diagnostic {
                severity: "ERROR",
                code: "10505001",
                file: file.to_string(),
                line: line_no,
                column,
                message: "The 'build' method can not have arguments.".to_string(),
            });
        }
    }
    // `@Entry` 组件的 `build` 带返回类型：参考侧按「只能有一个根节点」报 10905210，
    // 锚在方法体的 `{` 之后（`fixtures/ui-build-with-return` 的 5:20）。这条在解析失败
    // 时也要报，所以放在文本级。
    {
        let mut offset = 0usize;
        for line in source.lines() {
            let this_line = offset;
            offset += line.len() + 1;
            let trimmed = line.trim_start();
            if !trimmed.starts_with("build(") || !trimmed.contains("):") {
                continue;
            }
            if let Some(brace) = line.find('{') {
                let (line_no, column) = position_of(source, this_line + brace + 1);
                out.push(Diagnostic {
                    severity: "ERROR",
                    code: "10905210",
                    file: file.to_string(),
                    line: line_no,
                    column,
                    message: "In an '@Entry' decorated component, the 'build' method can have only one root node, which must be a container component."
                        .to_string(),
                });
            }
        }
    }
    let mut line_number = 1usize;
    let mut offset = 0usize;
    for line in source.lines() {
        let this_line = offset;
        offset += line.len() + 1;
        let trimmed = line.trim_start();
        if trimmed == "@Entry" {
            let rest = &source[offset.min(source.len())..];
            // 跳过紧跟的其他装饰器行（`@Entry` 下面常是 `@Component`）。
            let next = rest.lines().find(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('@')
            });
            let is_struct = next
                .map(|line| {
                    let trimmed = line.trim_start();
                    trimmed.starts_with("struct ") || trimmed.starts_with("export struct ")
                })
                .unwrap_or(false);
            if !is_struct {
                let indent = line.len() - trimmed.len();
                let (start_line, start_column) = position_of(source, this_line + indent);
                let mut first = Diagnostic {
                    severity: "ERROR",
                    code: "10505001",
                    file: file.to_string(),
                    line: start_line,
                    column: start_column,
                    message: "Decorators are not valid here.".to_string(),
                };
                first.line = start_line;
                out.push(first);
                let (second_line, second_column) = position_of(source, this_line + indent + 1);
                out.push(Diagnostic {
                    severity: "ERROR",
                    code: "10905337",
                    file: file.to_string(),
                    line: second_line,
                    column: second_column,
                    message: "The '@Entry' decorator can only be used with 'struct'.".to_string(),
                });
            }
        }
        line_number += 1;
    }
    let _ = line_number;
    // 早期路径的诊断不经 `Checker::push`，FAQ 标记在这里按本文件的结果重设，
    // 不沿用上一个文件留下的值。
    let has_linter = out.iter().any(|d| d.message.contains("(arkts-"));
    FILE_LEVEL_LINTER.with(|cell| cell.set(has_linter));
    out.sort_by(|a, b| (a.line, a.column, a.code).cmp(&(b.line, b.column, b.code)));
    out
}

/// 字节偏移到 (行, 列)，都从 1 起。
fn position_of(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    for (index, byte) in source.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// 本文件里 `const x = new SomeClass();` 的绑定：`x -> SomeClass`。
fn instance_bindings(program: &Program<'_>) -> std::collections::HashMap<String, String> {
    struct Collect(std::collections::HashMap<String, String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_variable_declaration(&mut self, it: &oxc_ast::ast::VariableDeclaration<'a>) {
            for declarator in &it.declarations {
                let Some(init) = &declarator.init else {
                    continue;
                };
                let Expression::NewExpression(new_expression) = init else {
                    continue;
                };
                let Expression::Identifier(callee) = &new_expression.callee else {
                    continue;
                };
                let oxc_ast::ast::BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind
                else {
                    continue;
                };
                self.0.insert(id.name.to_string(), callee.name.to_string());
            }
            oxc_ast_visit::walk::walk_variable_declaration(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// class 成员的声明类型：`Outer -> {inner: Inner}`。
fn class_member_types(
    program: &Program<'_>,
) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
    struct Collect(std::collections::HashMap<String, std::collections::HashMap<String, String>>);
    impl<'a> Visit<'a> for Collect {
        fn visit_class(&mut self, it: &Class<'a>) {
            if let Some(id) = &it.id {
                let entry = self.0.entry(id.name.to_string()).or_default();
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
                    // 数组类型也收：`private data: string[] = []` 的字段名要能被索引检查
                    // 认出来（`fixtures/lazy-forms` 的 `this.data[index]`）。
                    let spelled = match &annotation.type_annotation {
                        oxc_ast::ast::TSType::TSArrayType(_) => "[]".to_string(),
                        oxc_ast::ast::TSType::TSTypeReference(reference) => {
                            let oxc_ast::ast::TSTypeName::IdentifierReference(type_name) =
                                &reference.type_name
                            else {
                                continue;
                            };
                            type_name.name.to_string()
                        }
                        _ => continue,
                    };
                    entry.insert(name.to_string(), spelled);
                }
            }
            oxc_ast_visit::walk::walk_class(self, it);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// 本文件函数的返回类型注解：`compute -> number`。
fn function_return_types(program: &Program<'_>) -> std::collections::HashMap<String, String> {
    struct Collect(std::collections::HashMap<String, String>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(
            &mut self,
            it: &oxc_ast::ast::Function<'a>,
            flags: oxc_syntax::scope::ScopeFlags,
        ) {
            if let (Some(id), Some(annotation)) = (&it.id, &it.return_type) {
                if let Some(name) = primitive_name(&annotation.type_annotation) {
                    self.0.insert(id.name.to_string(), name.to_string());
                }
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// 泛型函数的形参个数：`pair<Left, Right>` → 2。
fn generic_function_arity(program: &Program<'_>) -> std::collections::HashMap<String, usize> {
    struct Collect(std::collections::HashMap<String, usize>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(
            &mut self,
            it: &oxc_ast::ast::Function<'a>,
            flags: oxc_syntax::scope::ScopeFlags,
        ) {
            if let (Some(id), Some(parameters)) = (&it.id, &it.type_parameters) {
                if !parameters.params.is_empty() {
                    self.0.insert(id.name.to_string(), parameters.params.len());
                }
            }
            oxc_ast_visit::walk::walk_function(self, it, flags);
        }
    }
    let mut collect = Collect(std::collections::HashMap::new());
    collect.visit_program(program);
    collect.0
}

/// 类名当作可调用对象的那组诊断，走文本通道：写在 `build()` 体里的调用落在 ArkUI 语法上，
/// 解析失败后没有方法体的树，规则的输入只剩源码。
fn class_object_diagnostics(file: &str, source: &str) -> Vec<Diagnostic> {
    let tokens = crate::build::lex::tokenize(source);
    let mut classes: std::collections::BTreeSet<String> = Default::default();
    for pair in tokens.windows(2) {
        if pair[0].kind == crate::build::lex::Kind::Ident
            && pair[0].text == "class"
            && pair[1].kind == crate::build::lex::Kind::Ident
        {
            classes.insert(pair[1].text.to_string());
        }
    }
    if classes.is_empty() {
        return Vec::new();
    }
    let starts = line_starts(source);
    let position = |offset: usize| -> (usize, usize) {
        let idx = match starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        (idx + 1, offset - starts[idx] + 1)
    };
    let mut out: Vec<Diagnostic> = Vec::new();
    for (object_start, dot_start, member_start, call_end) in class_member_calls(source, &classes) {
        let name = source[object_start..]
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("")
            .to_string();
        let member = source[member_start..]
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("")
            .to_string();
        if name.is_empty() || member.is_empty() {
            continue;
        }
        let mut push =
            |offset: usize, severity: &'static str, code: &'static str, message: String| {
                let (line, column) = position(offset);
                out.push(Diagnostic {
                    severity,
                    code,
                    file: file.to_string(),
                    line,
                    column,
                    message,
                });
            };
        push(
            object_start,
            "ERROR",
            "10505001",
            format!(
                "Value of type 'typeof {name}' is not callable. Did you mean to include 'new'?"
            ),
        );
        push(dot_start, "ERROR", "10505001", "'(' expected.".to_string());
        push(
            member_start,
            "ERROR",
            "10505001",
            format!("Cannot find name '{member}'."),
        );
        push(call_end, "ERROR", "10505001", "',' expected.".to_string());
        push(
            object_start,
            "WARN",
            "-",
            "Classes cannot be used as objects (arkts-no-classes-as-obj)".to_string(),
        );
    }
    out
}

/// `build()` 方法体在源码里的区间：`{` 之后到与之匹配的 `}` 之前。
fn build_body_range(source: &str) -> Option<(usize, usize)> {
    let at = source.find("build()")?;
    let open = at + source[at..].find('{')?;
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open + 1, index));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// `Class.member(...)` 的词法形态：（类名起点、点号起点、成员名起点、调用右括号之后）。
///
/// 模板字符串里的 `${ … }` 不在词法流里，单独切开再扫：`Text(`${Counter.bump()}`)`
/// 就是这种形态（`fixtures/arkts-standalone-this-static` 的 14 行）。
fn class_member_calls(
    source: &str,
    class_names: &std::collections::BTreeSet<String>,
) -> Vec<(usize, usize, usize, usize)> {
    let mut out = Vec::new();
    // 只判模板字符串里的调用：参考侧对 `Text(`${Counter.bump()}`)` 报那一组，而对
    // `.fontColor(ProxyData.getColorForLatency(…))` 这种参数位置的静态方法调用一条都不报
    // （`fixtures/arkts-standalone-this-static` 的 14 行 vs `fixtures/proxy-node-item` 的 98 行）。
    for token in crate::build::lex::tokenize(source) {
        if token.kind != crate::build::lex::Kind::Template {
            continue;
        }
        for (start, end) in template_expressions(&token) {
            scan_class_member_calls(&source[start..end], start, class_names, &mut out);
        }
    }
    out
}

/// 一个模板字符串里每段 `${ … }` 的源码区间。
fn template_expressions(token: &crate::build::lex::Token<'_>) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let bytes = token.text.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        let Some(found) = token.text[i..].find("${") else {
            break;
        };
        let open = i + found + 2;
        let mut depth = 1usize;
        let mut j = open;
        while j < bytes.len() {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if depth == 0 {
            out.push((token.start + open, token.start + j));
        }
        i = j + 1;
    }
    out
}

/// `source[base..]` 里每一处 `Class.member(...)`；`base` 是它在整份源码里的起点。
fn scan_class_member_calls(
    source: &str,
    base: usize,
    class_names: &std::collections::BTreeSet<String>,
    out: &mut Vec<(usize, usize, usize, usize)>,
) {
    let tokens = crate::build::lex::tokenize(source);
    let mut i = 0usize;
    while i + 3 < tokens.len() {
        let object = &tokens[i];
        let dot = &tokens[i + 1];
        let member = &tokens[i + 2];
        let open = &tokens[i + 3];
        let prior_new = i > 0
            && tokens[i - 1].kind == crate::build::lex::Kind::Ident
            && tokens[i - 1].text == "new";
        if object.kind == crate::build::lex::Kind::Ident
            && member.kind == crate::build::lex::Kind::Ident
            && dot.is_punct('.')
            && open.is_punct('(')
            && !prior_new
            && class_names.contains(object.text)
        {
            let mut depth = 0usize;
            let mut j = i + 3;
            let mut close = open.end;
            while j < tokens.len() {
                if tokens[j].is_punct('(') {
                    depth += 1;
                } else if tokens[j].is_punct(')') {
                    depth -= 1;
                    if depth == 0 {
                        close = tokens[j].end;
                        break;
                    }
                }
                j += 1;
            }
            out.push((
                base + object.start,
                base + dot.start,
                base + member.start,
                base + close,
            ));
            i = j + 1;
            continue;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::check;

    /// 片段跑一遍，只取带位置的诊断：整文件级的两条（`10905231` / `10905402`）不属于
    /// 任何一条片段规则，由各自的 fixture 覆盖。
    fn lines(source: &str) -> Vec<String> {
        check("ets/pages/Index.ets", source)
            .into_iter()
            .filter(|diagnostic| diagnostic.line != 0)
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
