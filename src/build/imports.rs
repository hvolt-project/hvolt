// imports: verify that every named import resolves to an export in its source module.
//
// The device build does not check this. A symbol moved between files keeps compiling and fails on
// the device with "the requested module ... does not provide an export name <name>", which is a
// startup crash with no build-time warning. The check runs before the compiler does, so the
// failure lands where it can be read.

use regex_lite::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE_EXTS: [&str; 2] = ["ets", "ts"];

/// Drops `.` components so keys and lookups compare equal.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SOURCE_EXTS.contains(&e))
        .unwrap_or(false)
}

/// Every source file under `root`, depth first.
fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if is_source(&path) {
            out.push(normalize(&path));
        }
    }
}

/// The names a module declares with `export`.
fn exports_of(text: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    // export function/class/interface/const/let/var/enum/type/struct, with optional `async`/`type`.
    let decl = Regex::new(
        r"(?m)^export\s+(?:declare\s+)?(?:type\s+)?(?:async\s+)?(?:abstract\s+)?(?:function|class|struct|interface|const|let|var|enum|type)\s+([A-Za-z0-9_]+)",
    )
    .expect("export declaration pattern");
    // `export const enum X` matched the `const` alternative above and captured `enum` as the
    // name. Measured on `fixtures/const-enum-import`: the file imported `ImportedKind` and this
    // pass reported it as not exported.
    let const_enum =
        Regex::new(r"(?m)^export\s+const\s+enum\s+([A-Za-z0-9_]+)").expect("const enum pattern");
    for caps in const_enum.captures_iter(text) {
        names.insert(caps[1].to_string());
    }
    for caps in decl.captures_iter(text) {
        // A keyword captured where a name belongs is the shape of the `const enum` miss above,
        // not a declaration called `enum`.
        let name: &str = &caps[1];
        if KEYWORDS.contains(&name) {
            continue;
        }
        names.insert(caps[1].to_string());
    }
    // export { a, b as c }
    let list = Regex::new(r"(?m)^export\s*\{([^}]*)\}").expect("export list pattern");
    for caps in list.captures_iter(text) {
        for part in caps[1].split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let name = part.split(" as ").last().unwrap_or(part).trim();
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    names
}

/// Words that can stand where a declared name does in the patterns above.
const KEYWORDS: &[&str] = &[
    "enum",
    "const",
    "let",
    "var",
    "function",
    "class",
    "interface",
    "type",
    "struct",
    "declare",
];

/// `(requested names, specifier)` for one `import { ... } from "..."` statement.
fn imports_of(text: &str) -> Vec<(Vec<String>, String)> {
    let pattern = Regex::new(r#"(?m)^import\s+(?:type\s+)?\{([^}]*)\}\s+from\s+['"]([^'"]+)['"]"#)
        .expect("import pattern");
    let mut out = Vec::new();
    for caps in pattern.captures_iter(text) {
        let mut names = Vec::new();
        for part in caps[1].split(',') {
            // A comment between the brace and the name is not part of the name. Measured on
            // `fixtures/import-comment`: without this the graph asked for
            // `\'// how much to scale by\n  OFFSET\'` and reported it as not exported.
            let part = crate::build::ets_compile::strip_comments(part.trim());
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            names.push(part.split(" as ").next().unwrap_or(part).trim().to_string());
        }
        out.push((names, caps[2].to_string()));
    }
    out
}

/// Resolves a relative specifier against the importing file, trying the source extensions.
fn resolve(from: &Path, specifier: &str) -> Option<PathBuf> {
    let normalised = normalize(&from.parent()?.join(specifier));
    if normalised.is_file() {
        return Some(normalised);
    }
    for ext in SOURCE_EXTS {
        // The extension is appended, not substituted: `./Registry.generated` names
        // `Registry.generated.ets`, while `with_extension` would look for `Registry.ets` and
        // report the import unresolved. `ets_assemble::resolve_source` already appends for this
        // reason; the two resolvers must agree or a file compiles into the closure yet fails
        // this check.
        let appended = PathBuf::from(format!("{}.{}", normalised.display(), ext));
        if appended.is_file() {
            return Some(appended);
        }
        let candidate = normalised.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let index_ets = normalised.join("index.ets");
    if index_ets.is_file() {
        return Some(index_ets);
    }
    let index_ts = normalised.join("index.ts");
    if index_ts.is_file() {
        return Some(index_ts);
    }
    None
}

/// Checks every relative named import under `source_root`. Returns the number of modules checked
/// or a message naming each import that cannot resolve.
pub(crate) fn check(source_root: &Path) -> Result<usize, String> {
    let mut files = Vec::new();
    collect(source_root, &mut files);
    if files.is_empty() {
        // A project with no sources is not this check's business; let the compiler speak.
        return Ok(0);
    }

    let mut declared: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for path in &files {
        let text =
            fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        declared.insert(path.clone(), exports_of(&text));
    }

    let mut problems: Vec<String> = Vec::new();
    for path in &files {
        let text =
            fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        for (names, specifier) in imports_of(&text) {
            if !specifier.starts_with('.') {
                continue; // Kit and SDK packages are resolved by the compiler, not here.
            }
            let Some(target) = resolve(path, &specifier) else {
                problems.push(format!("{}: cannot resolve '{specifier}'", path.display()));
                continue;
            };
            let exported = declared.get(&target);
            for name in names {
                let ok = exported.map(|set| set.contains(&name)).unwrap_or(false);
                if !ok {
                    problems.push(format!(
                        "{}: '{name}' is not exported by '{}'",
                        path.display(),
                        target.display()
                    ));
                }
            }
        }
    }

    if problems.is_empty() {
        return Ok(files.len());
    }
    problems.sort();
    Err(format!(
        "import graph: {} unresolved import(s)\n  {}",
        problems.len(),
        problems.join("\n  ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_declarations_and_lists() {
        let text = "export function a() {}\nexport const b = 1;\nexport type C = string;\n\
                    export interface D {}\nexport { e, f as g };\n";
        let names = exports_of(text);
        for want in ["a", "b", "C", "D", "e", "g"] {
            assert!(names.contains(want), "missing {want} in {names:?}");
        }
        assert!(
            !names.contains("f"),
            "an aliased name is exported under its alias"
        );
    }

    #[test]
    fn reads_named_imports() {
        let text = "import { a, b as c } from './x';\nimport type { D } from '../y.ts';\n";
        let found = imports_of(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(found[0].1, "./x");
        assert_eq!(found[1].0, vec!["D".to_string()]);
        assert_eq!(found[1].1, "../y.ts");
    }
    /// A specifier whose last segment already contains a dot names the appended file:
    /// `./Registry.generated` is `Registry.generated.ets`. Substituting the extension instead
    /// looked for `Registry.ets` and reported an existing file as unresolved.
    #[test]
    fn resolves_a_specifier_with_a_dot_by_appending() {
        let root = std::env::temp_dir().join(format!("hvolt-imports-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");
        fs::write(
            root.join("Registry.generated.ets"),
            "export function registerAll(): void {}\n",
        )
        .expect("write generated");
        fs::write(
            root.join("Main.ets"),
            "import { registerAll } from './Registry.generated';\n",
        )
        .expect("write main");

        let checked = check(&root).expect("dotted specifier must resolve");
        assert_eq!(checked, 2);
        let _ = fs::remove_dir_all(&root);
    }
}
