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
/// `export * from './x'` 的目标：转发导出的模块本身不写名字，要看目标文件有什么。
fn star_export_targets(text: &str) -> Vec<String> {
    let pattern =
        Regex::new(r#"(?m)^export\s+\*\s+from\s+['"]([^'"]+)['"]"#).expect("star export pattern");
    pattern
        .captures_iter(text)
        .map(|caps| caps[1].to_string())
        .collect()
}

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
    // `export * from './x'` and `export { a } from './x'` name a module too. They are not imports,
    // but they cross the same file boundary, and a TS file reaching an ArkTS one through them is
    // refused the same way (`fixtures/import-ts-to-ets`: the barrel is a `.ts`).
    let reexport = Regex::new(r#"(?m)^export\s+(?:\*|\{[^}]*\})\s+from\s+['"]([^'"]+)['"]"#)
        .expect("re-export pattern");
    for caps in reexport.captures_iter(text) {
        out.push((Vec::new(), caps[1].to_string()));
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

/// The 1-based line and column of `'<specifier>'` inside `text`, when it appears quoted.
fn locate_specifier(text: &str, specifier: &str) -> Option<(usize, usize)> {
    for quote in ['\'', '"'] {
        let needle = format!("{quote}{specifier}{quote}");
        if let Some(at) = text.find(&needle) {
            let before = &text[..at];
            let line = before.matches('\n').count() + 1;
            let column = before
                .rsplit('\n')
                .next()
                .map(|l| l.chars().count())
                .unwrap_or(0)
                + 1;
            return Some((line, column));
        }
    }
    None
}

/// The path of `path` under `root`, with `/` separators.
fn relative_to(root: &Path, path: &Path) -> String {
    let normalised = normalize(path);
    let base = normalize(root);
    normalised
        .strip_prefix(&base)
        .unwrap_or(&normalised)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The packages installed under `root/oh_modules`: name -> its entry file.
///
/// A bare specifier that names one of them is resolved here rather than by the compiler, so that a
/// named import the package does not export is caught at build time
/// (`Module '"mylib"' has no exported member 'nope'.`, 1:18 of `fixtures/import-package-symbol`).
fn packages_under(root: &Path) -> HashMap<String, PathBuf> {
    let mut found = HashMap::new();
    let Ok(entries) = fs::read_dir(root.join("oh_modules")) else {
        return found;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name = name.trim_start_matches('@').to_string();
        // `oh-package.json5` names the entry; without one the usual file names are tried.
        let mut entry_file = None;
        if let Ok(text) = fs::read_to_string(dir.join("oh-package.json5")) {
            if let Some(main) = manifest_value(&text, "main") {
                let candidate = dir.join(&main);
                if candidate.is_file() {
                    entry_file = Some(candidate);
                }
            }
        }
        if entry_file.is_none() {
            for candidate in ["index.ets", "Index.ets", "index.ts", "Index.ts"] {
                let path = dir.join(candidate);
                if path.is_file() {
                    entry_file = Some(path);
                    break;
                }
            }
        }
        if let Some(entry_file) = entry_file {
            found.insert(name, entry_file);
        }
    }
    found
}

/// The string value of `"<key>": "<value>"` in a JSON-with-comments manifest.
fn manifest_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let at = text.find(&needle)?;
    let after = &text[at + needle.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The 1-based line and column of a bare identifier inside an import list.
fn locate_name(text: &str, name: &str) -> Option<(usize, usize)> {
    let at = text.find(name)?;
    let before = &text[..at];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|l| l.chars().count())
        .unwrap_or(0)
        + 1;
    Some((line, column))
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

    let packages = packages_under(source_root);
    // `export * from './x'`：转发文件自己不写名字，把目标文件的导出并进来
    // （`fixtures/export-star-ets` 里 `barrel.ets` 只写 `export * from './utils'`）。
    let mut stars: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for (path, names) in &declared {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let targets = star_export_targets(&text);
        if !targets.is_empty() {
            stars.insert(path.clone(), targets);
        }
        let _ = names;
    }
    for _ in 0..8 {
        let mut changed = false;
        for (path, targets) in &stars {
            let mut added: Vec<String> = Vec::new();
            for specifier in targets {
                let Some(target) = resolve(path, specifier) else {
                    continue;
                };
                if let Some(names) = declared.get(&target) {
                    added.extend(names.iter().cloned());
                }
            }
            if let Some(own) = declared.get_mut(path) {
                for name in added {
                    if own.insert(name) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut problems: Vec<String> = Vec::new();
    for path in &files {
        let text =
            fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        for (names, specifier) in imports_of(&text) {
            if !specifier.starts_with('.') {
                // A package installed under `oh_modules` is resolved here: its named exports are
                // known from the entry file.
                let package = specifier.split('/').next().unwrap_or(&specifier);
                let package = package.trim_start_matches('@');
                // A bare specifier that no installed package answers: the reference reports it the
                // same way it reports a missing relative module (`fixtures/import-file-dependency`:
                // `file:` in `oh-package.json5` installs nothing, so `Cannot find module 'lib'`).
                // `@`-scoped names are the SDK's own (kit/system/hms) and stay with the compiler.
                // A member of the same project (`shared/` next to the importing module's `src/`)
                // is reached through the module manifest, not through `oh_modules`
                // (`fixtures/hsp-member-import`).
                let sibling_member = source_root
                    .parent()
                    .map(|project| project.join(package).is_dir())
                    .unwrap_or(false);
                if !packages.contains_key(package) && !sibling_member && !specifier.starts_with('@')
                {
                    if let Some((line, column)) = locate_specifier(&text, &specifier) {
                        let file = format!("ets/{}", relative_to(source_root, path));
                        eprintln!(
                            "ERROR 10505001 {file}:{line}:{column} Cannot find module '{specifier}' or its corresponding type declarations."
                        );
                    }
                    continue;
                }
                if let Some(entry) = packages.get(package) {
                    // The package directory is not walked as part of the module's own sources, so
                    // its entry file is read here when the collected set has no record of it.
                    let exported = declared.get(entry).cloned().unwrap_or_else(|| {
                        fs::read_to_string(entry)
                            .map(|text| exports_of(&text))
                            .unwrap_or_default()
                    });
                    for name in names {
                        if !exported.contains(&name) {
                            if let Some((line, column)) = locate_name(&text, &name) {
                                let file = format!("ets/{}", relative_to(source_root, path));
                                eprintln!(
                                    "ERROR 10505001 {file}:{line}:{column} Module '\"{specifier}\"' has no exported member '{name}'."
                                );
                            }
                        }
                    }
                }
                continue; // Kit and SDK packages are resolved by the compiler, not here.
            }
            if let Some(target) = resolve(path, &specifier) {
                // A `.ts` file may not pull in a `.ets` one: `Importing ArkTS files in JS and TS
                // files is forbidden.` (10605999, at the specifier — 1:15 of
                // `fixtures/import-ts-to-ets`).
                let into_ets = path.extension().and_then(|e| e.to_str()) == Some("ts")
                    && target.extension().and_then(|e| e.to_str()) == Some("ets");
                if into_ets {
                    if let Some((line, column)) = locate_specifier(&text, &specifier) {
                        let file = format!("ets/{}", relative_to(source_root, path));
                        eprintln!(
                            "ERROR 10605999 {file}:{line}:{column} Importing ArkTS files in JS and TS files is forbidden."
                        );
                    }
                }
            }
            let Some(target) = resolve(path, &specifier) else {
                // The reference reports this as a type-level diagnostic against the specifier
                // (`Cannot find module '../utils/types' or its corresponding type declarations.`,
                // 1:24 of `fixtures/import-missing-module`), so the console line is printed here
                // too, in the shape `scripts/oracle/diagnostics.py` reads.
                if let Some((line, column)) = locate_specifier(&text, &specifier) {
                    let file = format!("ets/{}", relative_to(source_root, path));
                    eprintln!(
                        "ERROR 10505001 {file}:{line}:{column} Cannot find module '{specifier}' or its corresponding type declarations."
                    );
                }
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
