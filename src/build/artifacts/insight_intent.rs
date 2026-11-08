//! The intent manifest the build generates for `@InsightIntent*` decorators.
//!
//! A class decorated with `@InsightIntentLink` (and the other six intent decorators) is
//! registered in `resources/base/profile/insight_intent.json`, which ships inside the package
//! and is how the system discovers the app's declarative intents. The loader drops the
//! decorator from the generated class, so the manifest has to be built from the original
//! sources rather than from the transpiled output.

use std::path::{Path, PathBuf};

/// One intent declaration, already in the shape the manifest records.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentEntry {
    /// The module-qualified source file, as `@normalized:N&&&<module>/src/main/ets/<path>&`.
    pub decorator_file: String,
    pub decorator_class: String,
    pub decorator_type: String,
    /// The decorator's own keys, in declaration order.
    pub info: Vec<(String, String)>,
}

impl IntentEntry {
    fn to_json(&self, bundle: &str, module: &str) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "decoratorFile".into(),
            serde_json::json!(self.decorator_file),
        );
        object.insert(
            "decoratorClass".into(),
            serde_json::json!(self.decorator_class),
        );
        for (key, value) in &self.info {
            object.insert(key.clone(), serde_json::json!(value));
        }
        object.insert("bundleName".into(), serde_json::json!(bundle));
        object.insert("moduleName".into(), serde_json::json!(module));
        object.insert(
            "decoratorType".into(),
            serde_json::json!(self.decorator_type),
        );
        serde_json::Value::Object(object)
    }
}

/// The manifest for a whole project, or `None` when the sources declare no intents.
pub fn render(
    project_dir: &Path,
    module_name: &str,
    bundle: &str,
) -> Result<Option<serde_json::Value>, String> {
    let entries = collect(project_dir, module_name)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| entry.to_json(bundle, module_name))
        .collect();
    Ok(Some(serde_json::json!({ "extractInsightIntents": items })))
}

/// Every intent declaration under `project_dir`, in file order.
pub fn collect(project_dir: &Path, module_name: &str) -> Result<Vec<IntentEntry>, String> {
    let mut files = Vec::new();
    collect_ets(project_dir, &mut files)?;
    files.sort();
    let mut entries = Vec::new();
    for file in files {
        let source =
            std::fs::read_to_string(&file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let rel = file.strip_prefix(project_dir).unwrap_or(&file);
        let rel = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches(".ets")
            .to_string();
        // The module's own source directory is the layout root; the record address is
        // `<module>/src/main/ets/<path inside the module>`.
        let rel = rel.split_once('/').map(|(_, rest)| rest).unwrap_or(&rel);
        let decorator_file = format!("@normalized:N&&&{module_name}/src/main/ets/{rel}&");
        for entry in parse_source(&source) {
            entries.push(IntentEntry {
                decorator_file: decorator_file.clone(),
                ..entry
            });
        }
    }
    Ok(entries)
}

fn collect_ets(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => return Err(format!("read {}: {err}", dir.display())),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        // `build/` holds outputs, and the staging copy of the sources would double every hit.
        if path.is_dir() {
            if name == "build" || name == "node_modules" || name == ".hvigor" {
                continue;
            }
            collect_ets(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("ets") {
            out.push(path);
        }
    }
    Ok(())
}

/// The intent declarations in one file.
///
/// The decorator, its argument list and the declaration it decorates all come from the
/// scanner: `@InsightIntent` inside a string or a comment is not a declaration, and the
/// argument list ends at the parenthesis the scanner matches.
fn parse_source(source: &str) -> Vec<IntentEntry> {
    let mut entries = Vec::new();
    let tokens = crate::build::lex::tokenize(source);
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_punct('@') {
            continue;
        }
        let Some(name) = tokens.get(index + 1) else {
            continue;
        };
        // Only the intent decorators carry declarations; `@Provide`, `@State` and the rest of
        // the component decorators are not intents.
        if name.kind != crate::build::lex::Kind::Ident || !name.text.starts_with("InsightIntent") {
            continue;
        }
        let Some(paren) = tokens.get(index + 2).filter(|t| t.is_punct('(')) else {
            continue;
        };
        let Some(close) = crate::build::lex::matching_delimiter(source, paren.start) else {
            continue;
        };
        let info = parse_info(&source[paren.end..close]);
        // The decorated declaration follows the decorator list; take the next `class`/`struct`.
        let tail_index = tokens
            .iter()
            .position(|t| t.start > close)
            .unwrap_or(tokens.len());
        let decorator_class = first_declared_type_at(&tokens, tail_index).unwrap_or_default();
        if decorator_class.is_empty() {
            continue;
        }
        entries.push(IntentEntry {
            decorator_file: String::new(),
            decorator_class,
            decorator_type: format!("@{}", name.text),
            info,
        });
    }
    entries
}

/// The first `class`/`struct`/`function` name declared after `from` in the token stream.
fn first_declared_type_at(tokens: &[crate::build::lex::Token<'_>], from: usize) -> Option<String> {
    let mut index = from;
    while index + 1 < tokens.len() {
        let token = &tokens[index];
        let is_keyword = token.kind == crate::build::lex::Kind::Ident
            && matches!(token.text, "class" | "struct" | "function");
        if is_keyword {
            let name = tokens.get(index + 1)?;
            if name.kind == crate::build::lex::Kind::Ident {
                return Some(name.text.to_string());
            }
        }
        index += 1;
    }
    None
}

/// The decorator's own keys. Only scalar entries are recorded; nested objects belong to the
/// decorator's own schema and are not part of what the manifest echoes back.
fn parse_info(args: &str) -> Vec<(String, String)> {
    // The decorator's argument list wraps one object literal; strip its braces so the split
    // below sees the members rather than a single nested object.
    let body = args.trim();
    let body = body.strip_prefix('{').unwrap_or(body);
    let body = body.strip_suffix('}').unwrap_or(body);
    let mut info = Vec::new();
    for piece in split_top_level(body) {
        let Some((key, value)) = piece.split_once(':') else {
            continue;
        };
        let key = key
            .trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .to_string();
        let value = value.trim();
        let value = value.trim_matches(|c| c == '\'' || c == '"').to_string();
        if key.is_empty() || value.is_empty() || value.starts_with('{') {
            continue;
        }
        info.push((key, value));
    }
    info
}

/// Split an object literal's body on the commas that are not inside brackets or quotes.
fn split_top_level(body: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    for ch in body.chars() {
        match quote {
            Some(q) => {
                current.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    current.push(ch);
                }
                '{' | '[' | '(' => {
                    depth += 1;
                    current.push(ch);
                }
                '}' | ']' | ')' => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    pieces.push(std::mem::take(&mut current));
                }
                _ => current.push(ch),
            },
        }
    }
    if !current.trim().is_empty() {
        pieces.push(current);
    }
    pieces
}
