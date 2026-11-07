//! Static export check for a staged tree.
//!
//! The runtime rejects a load with `does not provide an export name '<X>'` when an import
//! names something the target module does not export at runtime.  Finding those one at a
//! time on a device costs a build, an install and a launch per name; the same question is
//! answerable from the staged sources, so this module answers it in one pass.
//!
//! What counts as a runtime export is exactly what survives compilation: classes, consts,
//! functions, enums, defaults, and named re-exports — but not `interface` / `type`, which
//! are erased.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One import that names a binding the target module does not export.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Missing {
    /// Staged file that does the importing, relative to `ts_root`.
    pub from: String,
    /// Module specifier as written.
    pub spec: String,
    /// Binding the importer expects.
    pub name: String,
}

/// Every mismatch in the staged tree, sorted for stable output.
pub fn scan(ts_root: &Path) -> Result<Vec<Missing>, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect(ts_root, &mut files)?;
    let mut exports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in &files {
        let key = record_key(ts_root, file);
        exports.insert(key, runtime_exports(file));
    }
    // Re-exports point at other modules; resolve them to a fixpoint so a barrel's names are
    // visible to its importers.
    let mut changed = true;
    let mut rounds = 0;
    while changed && rounds < 8 {
        changed = false;
        rounds += 1;
        let snapshot = exports.clone();
        for file in &files {
            let key = record_key(ts_root, file);
            for spec in reexport_specs(file) {
                let Some(target) = resolve(ts_root, file, &spec) else {
                    continue;
                };
                let Some(names) = snapshot.get(&target) else {
                    continue;
                };
                let entry = exports.entry(key.clone()).or_default();
                for name in names {
                    if entry.insert(name.clone()) {
                        changed = true;
                    }
                }
            }
        }
    }

    let mut missing: Vec<Missing> = Vec::new();
    for file in &files {
        let from = file
            .strip_prefix(ts_root)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();
        for (spec, names) in named_imports(file) {
            let Some(target) = resolve(ts_root, file, &spec) else {
                continue;
            };
            // A system or package module is not part of the staged tree: its exports are
            // whatever the SDK or the package provides, so it is out of scope here.
            let Some(exported) = exports.get(&target) else {
                continue;
            };
            for name in names {
                if !exported.contains(&name) {
                    missing.push(Missing {
                        from: from.clone(),
                        spec: spec.clone(),
                        name,
                    });
                }
            }
        }
    }
    missing.sort();
    missing.dedup();
    Ok(missing)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "ts" || e == "js") {
            out.push(path);
        }
    }
    Ok(())
}

/// Staging-tree key of a file: `<module>/<path without extension>`.
fn record_key(ts_root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(ts_root).unwrap_or(file);
    rel.with_extension("").to_string_lossy().replace('\\', "/")
}

/// The name a specifier resolves to, as a staging-tree key.
///
/// A specifier is either relative (`./x`) or the normalized url the build rewrites it to
/// (`@normalized:N&&&<pkg>/<path>&<version>`).  A url path names a source file under the
/// module source root, or a package entry; both reduce to a staged-tree key once the source
/// root and the separator convention are undone.
fn resolve(ts_root: &Path, file: &Path, spec: &str) -> Option<String> {
    if let Some(rest) = spec.strip_prefix("@normalized:N&&&") {
        let path = rest.split('&').next().unwrap_or_default();
        let stripped = path
            .strip_prefix("src/")
            .map(|p| p.to_string())
            .unwrap_or_else(|| path.to_string());
        let _ = stripped;
        let mut candidates: Vec<String> = Vec::new();
        candidates.push(path.to_string());
        candidates.push(path.replace('.', "/"));
        if let Some(idx) = path.find("/src/main/ets/") {
            candidates.push(format!(
                "{}{}",
                &path[..idx + 1],
                &path[idx + "/src/main/ets/".len()..]
            ));
        }
        for suffix in [".ts", ".js"] {
            let stripped = path.strip_suffix(suffix).unwrap_or(path);
            candidates.push(stripped.to_string());
            candidates.push(stripped.replace('.', "/"));
        }
        for candidate in candidates {
            let candidate = candidate.replace("%", "/");
            if ts_root.join(format!("{candidate}.ts")).is_file() {
                return Some(candidate);
            }
        }
        return None;
    }
    if spec.starts_with('@') || !spec.starts_with('.') {
        return None;
    }
    let dir = file.parent()?.strip_prefix(ts_root).ok()?;
    let mut parts: Vec<String> = dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    for seg in spec.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }
    let mut key = parts.join("/");
    for suffix in [".ts", ".js", ".ets"] {
        if let Some(stripped) = key.strip_suffix(suffix) {
            key = stripped.to_string();
            break;
        }
    }
    Some(key)
}

/// Names the file exports at runtime.
fn runtime_exports(file: &Path) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return BTreeSet::new();
    };
    // Every `export` is read from the token stream: a declaration export names the declared
    // identifier, a binding-list export names the local binding of each specifier, and a
    // multi-line list needs no line bookkeeping.
    let tokens = crate::build::lex::tokenize(&text);
    let mut names = BTreeSet::new();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        let is_export = token.kind == crate::build::lex::Kind::Ident && token.text == "export";
        if !is_export {
            index += 1;
            continue;
        }
        index += 1;
        // Skip modifiers that may sit between `export` and the declaration keyword.
        while index < tokens.len()
            && tokens[index].kind == crate::build::lex::Kind::Ident
            && matches!(
                tokens[index].text,
                "abstract" | "async" | "declare" | "default"
            )
        {
            index += 1;
        }
        let Some(next) = tokens.get(index) else {
            break;
        };
        if next.is_punct('{') {
            // `export { a, b as c }` exports the local binding (`c`).
            index += 1;
            while index < tokens.len() && !tokens[index].is_punct('}') {
                let Some(specifier) = tokens.get(index) else {
                    break;
                };
                if specifier.kind != crate::build::lex::Kind::Ident
                    && !specifier.is_punct(',')
                    && !specifier.is_punct('*')
                {
                    index += 1;
                    continue;
                }
                if specifier.kind == crate::build::lex::Kind::Ident && specifier.text != "type" {
                    let is_alias_target = tokens.get(index + 1).is_some_and(|t| {
                        t.kind == crate::build::lex::Kind::Ident && t.text == "as"
                    });
                    if !is_alias_target && specifier.text != "from" {
                        names.insert(specifier.text.to_string());
                    }
                }
                // `a as b` exports b.
                if specifier.kind == crate::build::lex::Kind::Ident
                    && tokens
                        .get(index + 1)
                        .is_some_and(|t| t.kind == crate::build::lex::Kind::Ident && t.text == "as")
                {
                    if let Some(bind) = tokens.get(index + 2) {
                        if bind.kind == crate::build::lex::Kind::Ident {
                            names.insert(bind.text.to_string());
                        }
                        index += 3;
                        continue;
                    }
                }
                index += 1;
            }
            index += 1;
            continue;
        }
        if next.is_punct('*') {
            index += 1;
            continue;
        }
        // `export class X` / `export function X` / `export const X` / `export enum X`.
        let is_declaration = next.kind == crate::build::lex::Kind::Ident
            && matches!(
                next.text,
                "class" | "function" | "const" | "let" | "var" | "enum"
            );
        if is_declaration {
            if let Some(name) = tokens.get(index + 1) {
                if name.kind == crate::build::lex::Kind::Ident {
                    names.insert(name.text.to_string());
                }
            }
        }
        index += 1;
    }
    names.extend(multiline_export_names(file));
    names
}

/// Names a multi-line `export { ... }` block declares, used to keep `runtime_exports` right
/// for files that list their exports across several lines.
fn multiline_export_names(file: &Path) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return BTreeSet::new();
    };
    let mut names = BTreeSet::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(mut acc) = pending.take() {
            acc.push(' ');
            acc.push_str(t);
            if !acc.contains('}') {
                pending = Some(acc);
                continue;
            }
            collect_export_names(&acc, &mut names);
            continue;
        }
        if t.starts_with("export {") && !t.contains('}') {
            pending = Some(t.to_string());
            continue;
        }
    }
    names
}

fn collect_export_names(block: &str, names: &mut BTreeSet<String>) {
    let Some(open) = block.find('{') else {
        return;
    };
    let Some(close) = block.rfind('}') else {
        return;
    };
    for item in block[open + 1..close].split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let name = item
            .rsplit_once(" as ")
            .map(|(_, bind)| bind.trim())
            .unwrap_or(item);
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
}

/// Specifiers this file re-exports everything from (`export * from './x'`).
fn reexport_specs(file: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if !t.starts_with("export *") {
            continue;
        }
        if let Some((_, rest)) = t.split_once("from ") {
            if let Some(spec) = rest
                .trim()
                .trim_end_matches(';')
                .trim()
                .strip_prefix('\'')
                .or_else(|| rest.trim().trim_end_matches(';').trim().strip_prefix('"'))
            {
                let spec = spec.trim_end_matches(['\'', '"']);
                out.push(spec.to_string());
            }
        }
    }
    out
}

/// Named bindings this file imports: `(specifier, names)`.
fn named_imports(file: &Path) -> Vec<(String, Vec<String>)> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(mut acc) = pending.take() {
            acc.push(' ');
            acc.push_str(t);
            if !acc.contains('}') {
                pending = Some(acc);
                continue;
            }
            if let Some(parsed) = parse_named(&acc) {
                out.push(parsed);
            }
            continue;
        }
        if t.starts_with("import {") && !t.contains('}') {
            pending = Some(t.to_string());
            continue;
        }
        if t.starts_with("import {") || t.starts_with("import type {") {
            if let Some(parsed) = parse_named(t) {
                out.push(parsed);
            }
        }
    }
    out
}

fn parse_named(statement: &str) -> Option<(String, Vec<String>)> {
    let open = statement.find('{')?;
    let close = statement.rfind('}')?;
    let (_, rest) = statement.split_once("from ")?;
    let spec = rest
        .trim()
        .trim_end_matches(';')
        .trim()
        .trim_start_matches(['\'', '"']);
    let spec = spec.trim_end_matches(['\'', '"']).to_string();
    let names = statement[open + 1..close]
        .split(',')
        .filter_map(|item| {
            let item = item.trim();
            if item.is_empty() {
                return None;
            }
            Some(
                item.rsplit_once(" as ")
                    .map(|(sym, _)| sym.trim().to_string())
                    .unwrap_or_else(|| item.to_string()),
            )
        })
        .collect();
    Some((spec, names))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_exports_ignore_types() {
        let dir = std::env::temp_dir().join(format!("hvolt-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("m.ts");
        std::fs::write(
            &file,
            "export interface T {}\n\
             export type U = string\n\
             export class C {}\n\
             const x = 1\n\
             export { x }\n\
             export { a as b }\n",
        )
        .unwrap();
        let names = runtime_exports(&file);
        assert!(names.contains("C"));
        assert!(names.contains("x"));
        assert!(names.contains("b"));
        assert!(!names.contains("T"));
        assert!(!names.contains("U"));
    }
}
