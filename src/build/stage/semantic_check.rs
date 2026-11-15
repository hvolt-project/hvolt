//! The identifier pass: a reference with no binding anywhere is an error, not a warning.
//!
//! `hvolt build` used to accept a file that called a name it never imported, because the chain
//! goes straight from ArkTS spelling to TypeScript to `es2abc`, and neither step resolves names.
//! The failure then shows up as a crash on the device, or as a silent no-op, long after the build
//! said it was fine.
//!
//! What is checked, and why only this: a reference that resolves to no binding in its file, when
//! the name starts lowercase. Uppercase names are left alone on purpose — ArkUI components
//! (`Text`), types and enums all begin uppercase and are supplied ambiently by the SDK, so a check
//! that included them would report every component in every file. Lowercase free identifiers are
//! the ones that can only be a variable, a function or an import, and the case that goes wrong is
//! exactly "used but never imported".
//!
//! Files the parser rejects are skipped rather than reported: ArkTS decorator syntax is not always
//! something the TypeScript parser accepts, and a syntax complaint here would be a false alarm
//! about a file the real compiler handles.

use std::collections::HashSet;

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_ast::ast::{Class, ClassElement, MethodDefinitionKind, Program, PropertyKey};
use oxc_ast_visit::Visit;

/// One problem, in the shape the build prints.
pub(crate) struct Diagnostic {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) message: String,
}

/// Names the runtime provides without any declaration in the file.
///
/// Only lowercase entries matter here, since the check ignores uppercase names; the list is kept
/// whole so it can be read as "what the environment supplies", not "what this pass skips".
const AMBIENT: &[&str] = &[
    // JavaScript and TypeScript built-ins.
    "console", "JSON", "Math", "Date", "Array", "Object", "String", "Number", "Boolean", "RegExp",
    "Promise", "Map", "Set", "WeakMap", "WeakSet", "Symbol", "Error", "TypeError", "RangeError",
    "Function", "globalThis", "undefined", "NaN", "Infinity", "parseInt", "parseFloat", "isNaN",
    "isFinite", "encodeURIComponent", "decodeURIComponent", "encodeURI", "decodeURI",
    "setTimeout", "clearTimeout", "setInterval", "clearInterval", "queueMicrotask",
    "structuredClone", "require", "exports", "module",
    // ArkTS and ArkUI runtime entry points.
    "AppStorage", "AppStorageV2", "LocalStorage", "PersistentStorage", "PersistenceV2",
    "Environment", "animateTo", "animateToImmediately", "getContext", "getUIContext",
    "promptAction", "router", "measureText", "getInspectorByKey", "getInspectorTree",
    "sendEventByKey", "focusControl", "cursorControl", "font", "vpx2px", "px2vp", "vp2px",
    "print", "getColorSync", "getResourcePath", "getHostContext",
    // Named routes and builders the runtime registers; the SDK supplies them, not the file.
    "registerNamedRoute", "registerBuilder", "registerStyle", "registerExtend",
    "getUINativeModule", "makeBuilderParameterProxy", "canIUse", "getContext",
];

fn is_ambient(name: &str) -> bool {
    AMBIENT.iter().any(|known| *known == name)
}

/// Whether the name is one this pass reports on at all.
///
/// Lowercase start is the whole rule: see the module comment for why the uppercase half of the
/// namespace is left to the SDK.
fn is_checked(name: &str) -> bool {
    // `$r` / `$count` and `__Recycle__` / `__Common__` are spelling the lowering owns: ArkUI
    // turns them into calls and proxy parameters, so they never reach the compiler as free
    // names. Checking them reports the SDK's own machinery.
    if name.starts_with('$') || name.starts_with("__") {
        return false;
    }
    match name.chars().next() {
        Some(first) => first.is_ascii_lowercase() || first == '_',
        None => false,
    }
}

/// Checks one file's source. `file` is only used for the message.
pub(crate) fn check_file(file: &str, source: &str) -> Vec<Diagnostic> {
    check_file_with(file, source, &HashSet::new())
}

/// The same check, with names the caller knows the environment supplies.
///
/// The SDK'"'"'s kit tables are the important case: `@kit.ArkUI` expands to a list of modules, so a
/// file can use `promptAction` without importing it in its own text. Those names are ambient for
/// the same reason `console` is, and they are supplied rather than guessed at.
pub(crate) fn check_file_with(
    file: &str,
    source: &str,
    extra: &HashSet<String>,
) -> Vec<Diagnostic> {
    let allocator = Allocator::default();
    let source_type = source_type_for(file);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        // ArkTS spelling the TypeScript parser does not accept: the lowering owns this file, and
        // reporting a name from a half-parsed tree would be noise.
        return Vec::new();
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let scoping = semantic.scoping();
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<Diagnostic> = Vec::new();
    for (name, refs) in scoping.root_unresolved_references().iter() {
        if !is_checked(name) || is_ambient(name) || extra.contains(*name) || seen.contains(*name) {
            continue;
        }
        seen.insert((*name).to_string());
        let (line, column) = match refs.first() {
            Some(id) => {
                let span = semantic.reference_span(scoping.get_reference(*id));
                let offset = span.start as usize;
                let prefix = &source[..offset.min(source.len())];
                let line = prefix.matches('\n').count() + 1;
                let column = prefix.rsplit('\n').next().map(|s| s.len()).unwrap_or(0) + 1;
                (line, column)
            }
            None => (0, 0),
        };
        out.push(Diagnostic {
            file: file.to_string(),
            line,
            column,
            message: format!("{name} is not defined"),
        });
    }
    for diagnostic in member_collisions(&parsed.program, source) {
        out.push(Diagnostic {
            file: file.to_string(),
            line: diagnostic.line,
            column: diagnostic.column,
            message: diagnostic.message,
        });
    }
    out.sort_by(|a, b| a.line.cmp(&b.line).then(a.column.cmp(&b.column)));
    out
}

fn source_type_for(file: &str) -> SourceType {
    if file.ends_with(".ts") || file.ends_with(".ets") {
        SourceType::ts()
    } else {
        SourceType::default()
    }
}

/// Whether the pass runs. Set `HVOLT_SEMANTIC=off` to silence it for one invocation.
pub(crate) fn enabled() -> bool {
    match std::env::var("HVOLT_SEMANTIC") {
        Ok(value) => value != "off" && value != "0",
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_used_but_never_imported_name_is_reported() {
        let source = "function run(): string {\n  return missingHelper(1);\n}\n";
        let found = check_file("demo.ets", source);
        assert_eq!(found.len(), 1, "expected one diagnostic");
        assert!(found[0].message.contains("missingHelper"));
        assert_eq!(found[0].line, 2);
    }

    #[test]
    fn an_imported_name_is_quiet() {
        let source = "import { helper } from './helper';\nfunction run(): string {\n  return helper();\n}\n";
        assert!(check_file("demo.ets", source).is_empty());
    }

    #[test]
    fn a_declared_local_is_quiet() {
        let source = "function run(): number {\n  const total = 2;\n  return total;\n}\n";
        assert!(check_file("demo.ets", source).is_empty());
    }

    #[test]
    fn uppercase_names_are_left_to_the_sdk() {
        // `Text` is supplied ambiently by the SDK, so it must not be reported.
        let source = "const view = Text('hi');\n";
        assert!(check_file("demo.ets", source).is_empty());
    }

    #[test]
    fn the_environment_supplies_its_own_names() {
        let source = "function run(): void {\n  console.log(JSON.stringify({}));\n}\n";
        assert!(check_file("demo.ets", source).is_empty());
    }

    #[test]
    fn a_method_named_like_a_state_accessor_is_reported() {
        // `@State x` lowers to `get x()` / `set x()` beside a `__x` field, so a hand-written
        // method with the same name lands in the same class body. hvigor rejects that file with
        // `Duplicate identifier`; this check has to agree.
        let source = "class Page {\n  private __sdkState: string;\n  get sdkState() { return this.__sdkState; }\n  set sdkState(v: string) { this.__sdkState = v; }\n  sdkState(): string { return 'disk'; }\n}\n";
        let found = check_file("demo.ts", source);
        assert_eq!(found.len(), 1, "expected one collision"); 
        assert!(found[0].message.contains("sdkState"), "message: {}", found[0].message);
    }

    #[test]
    fn a_getter_and_setter_pair_is_quiet() {
        // One accessor pair is one member; TypeScript allows it and hvigor accepts it.
        let source = "class Page {\n  private _v: string;\n  get v() { return this._v; }\n  set v(next: string) { this._v = next; }\n}\n";
        assert!(check_file("demo.ts", source).is_empty());
    }

    #[test]
    fn overloaded_methods_are_quiet() {
        // Overload signatures share a name by design, and hvigor accepts them.
        let source = "class Page {\n  pick(v: number): number;\n  pick(v: number): number { return v; }\n}\n";
        assert!(check_file("demo.ts", source).is_empty());
    }

    #[test]
    fn a_file_the_parser_rejects_is_skipped() {
        // A decorator form the TypeScript parser does not accept must not become a name report.
        let source = "@Entry\n@Component\nstruct Page {\n  build() { nonsense() }\n}\n";
        let found = check_file("demo.ets", source);
        assert!(found.is_empty(), "unparsable files are skipped, got {found:?}", found = found.len());
    }
}

/// Runs the pass over a project tree.
///
/// Returns the diagnostics, plus how many sources were read and how many were skipped because a
/// parser could not read them. The counts are reported rather than hidden: a check that silently
/// covered half a tree would read as "clean".
pub(crate) fn check_sources(root: &std::path::Path) -> (Vec<Diagnostic>, usize, usize) {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut read = 0usize;
    let mut skipped = 0usize;
    let mut stack: Vec<std::path::PathBuf> = vec![root.join("src")];
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_source = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "ets" || e == "ts")
                .unwrap_or(false);
            if is_source {
                files.push(path);
            }
        }
    }
    files.sort();
    // Lowering needs the SDK'"'"'s kit configuration; without it, ArkTS spelling cannot be turned
    // into the TypeScript this pass reads, and the files are reported as uncovered instead of
    // silently passing.
    let kit_dirs = crate::build::ets_assemble::sdk_kit_configs().unwrap_or_default();
    let kits = if kit_dirs.is_empty() {
        Default::default()
    } else {
        crate::build::stage::process_import::load_kit_configs(&kit_dirs).unwrap_or_default()
    };
    // Every symbol a kit can expand to is ambient for the file that uses it.
    let mut kit_names: HashSet<String> = HashSet::new();
    for kit in kits.values() {
        for symbol in kit.keys() {
            kit_names.insert(symbol.clone());
        }
    }
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        read += 1;
        let lowered = if rel.ends_with(".ets") {
            match lower_source(&rel, &text, &kit_dirs, &kits) {
                Ok(lowered) => lowered,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            }
        } else {
            text.clone()
        };
        // The lowering settles system imports for the file it reads, so a name can look unbound
        // after it while the source clearly imports it. The source is the authority: a name the
        // file imports is never reported, whatever the lowered text looks like.
        for diagnostic in check_file_with(&rel, &lowered, &kit_names) {
            if imports_name(&text, &diagnostic) {
                continue;
            }
            diagnostics.push(diagnostic);
        }
    }
    (diagnostics, read, skipped)
}

/// Turns ArkTS into the TypeScript the parser accepts, following the same route the build takes:
/// a component file through the full transpile pass, everything else through the plain lowering.
fn lower_source(
    file: &str,
    source: &str,
    kit_dirs: &[std::path::PathBuf],
    kits: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, crate::build::stage::process_import::KitEntry>>,
) -> Result<String, String> {
    if kit_dirs.is_empty() {
        return Err("no kit configuration".to_string());
    }
    let is_component = crate::build::lex::has_decorator(
        source,
        &["Component", "ComponentV2", "Entry", "CustomDialog"],
    );
    if is_component {
        crate::build::lower::ets_transpile::transpile(
            source,
            &crate::build::lower::ets_transpile::TranspileOptions {
                kit_config_dirs: kit_dirs,
                project_builders: &[],
                route: None,
            },
        )
        .map_err(|e| format!("{file}: {e}"))
    } else {
        crate::build::lower::ets_plain::lower(source, kits).map_err(|e| format!("{file}: {e}"))
    }
}

/// 同类内两个成员用同一个名字：ArkTS 的 `@State x` 会转译成 `get x()` / `set x()` 加一个
/// `__x` 字段，所以它和手写的同名方法落进同一个类体。运行时字段压过方法，调用点拿到的是
/// 值而不是函数（`sdkState is not callable`，2026-11-12 的应用闪退）。
///
/// hvigor 在 CompileArkTS 阶段以 `Duplicate identifier '<name>'` 拒绝这种文件，这里对齐它。
///
/// 两种合法形态必须放过：`get x()` / `set x()` 成对出现算同一个成员；方法重载是同名的多个
/// 方法声明。冲突只发生在「访问器」与「普通方法」之间。
struct MemberCollisions {
    found: Vec<(String, u32)>,
}

impl<'a> Visit<'a> for MemberCollisions {
    fn visit_class(&mut self, class: &Class<'a>) {
        let mut accessors: HashSet<String> = HashSet::new();
        let mut methods: Vec<(String, u32)> = Vec::new();
        for element in &class.body.body {
            if let ClassElement::MethodDefinition(method) = element {
                let name = match &method.key {
                    PropertyKey::StaticIdentifier(ident) => ident.name.to_string(),
                    _ => continue,
                };
                match method.kind {
                    MethodDefinitionKind::Get | MethodDefinitionKind::Set => {
                        accessors.insert(name);
                    }
                    MethodDefinitionKind::Method => methods.push((name, method.span.start)),
                    _ => {}
                }
            }
        }
        for (name, start) in methods {
            if accessors.contains(&name) {
                self.found.push((name, start));
            }
        }
        oxc_ast_visit::walk::walk_class(self, class);
    }
}

fn member_collisions(program: &Program, source: &str) -> Vec<Diagnostic> {
    let mut visit = MemberCollisions { found: Vec::new() };
    visit.visit_program(program);
    let mut out: Vec<Diagnostic> = Vec::new();
    for (name, start) in visit.found {
        let offset = (start as usize).min(source.len());
        let prefix = &source[..offset];
        let line = prefix.matches('\n').count() + 1;
        let column = prefix.rsplit('\n').next().map(|s| s.len()).unwrap_or(0) + 1;
        out.push(Diagnostic {
            file: String::new(),
            line,
            column,
            message: format!("Duplicate identifier '{}'", name),
        });
    }
    out
}

/// Whether the file's own import statements mention the reported name.
fn imports_name(source: &str, diagnostic: &Diagnostic) -> bool {
    let name = diagnostic
        .message
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return false;
    }
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("import ") && !trimmed.starts_with("export ") {
            continue;
        }
        for token in trimmed.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$') {
            if token == name {
                return true;
            }
        }
    }
    false
}
