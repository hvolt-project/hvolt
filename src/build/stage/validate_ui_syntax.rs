// process stage: `validate_ui_syntax.js` — the declaration collection and its diagnostics.
//
// The reference walks the program once and fills `componentCollection` (custom components,
// custom dialogs, observed classes), `linkCollection` / `stateCollection` (a struct's decorated
// fields) and `enumCollection`; every later stage reads those collections instead of re-reading
// the source. hvolt's diagnostics for the same walk live in `ets_checker` (`1090xxxx` and the
// `10505001` family); this module reuses that entry for the diagnostic half and collects the
// declaration half, so a caller gets both from one call.
#![allow(dead_code)] // wired by `ets_checker::check` callers; kept public-shaped for the reference

use std::collections::{BTreeMap, BTreeSet};

/// `ComponentCollection`: what `validateUISyntax` records about one program.
#[derive(Default)]
pub(crate) struct ComponentCollection {
    /// `componentCollection.customComponents`.
    pub(crate) custom_components: BTreeSet<String>,
    /// `componentCollection.customDialogs`: the structs `createControllerSet` keys on.
    pub(crate) custom_dialogs: BTreeSet<String>,
    /// `observedClassCollection`.
    pub(crate) observed_classes: BTreeSet<String>,
    /// `linkCollection`: struct name → its `@Link` fields.
    pub(crate) link_collection: BTreeMap<String, BTreeSet<String>>,
    /// `stateCollection`: struct name → its `@State` fields.
    pub(crate) state_collection: BTreeMap<String, BTreeSet<String>>,
    /// `enumCollection`: every enum name, which decides `isSimpleType`.
    pub(crate) enum_collection: BTreeSet<String>,
}

/// One diagnostic the reference reports as `{type, message, pos, code}`.
pub(crate) struct TransformLog {
    pub(crate) message: String,
    pub(crate) code: String,
    pub(crate) position: usize,
}

/// `validateUISyntax(sourceText, sourceFile, fileName, errors, moduleName)`.
///
/// Fills the collection for one source file and appends a [`TransformLog`] per violation; the
/// returned collection is the one the following stages read.
pub(crate) fn validate_ui_syntax(
    source: &str,
    file_name: &std::path::Path,
    errors: &mut Vec<TransformLog>,
) -> ComponentCollection {
    let mut collection = ComponentCollection::default();
    collect_declarations(source, &mut collection);
    for diagnostic in
        crate::build::stage::ets_checker::check(&file_name.display().to_string(), source)
    {
        if diagnostic.severity != "ERROR" {
            continue;
        }
        errors.push(TransformLog {
            message: diagnostic.message.clone(),
            code: diagnostic.code.to_string(),
            position: diagnostic.line,
        });
    }
    collection
}

/// The declaration half of the walk: which structs, classes and enums the file declares.
///
/// `@Component`/`@CustomDialog`/`@Observed`/`@ObservedV2` mark the structs and classes; the
/// decorated fields of a `@Component` struct fill the link and state collections. The scan is
/// textual because the names are all the later stages need, and the reference's own walk is
/// driven by the same markers.
fn collect_declarations(source: &str, collection: &mut ComponentCollection) {
    let lines: Vec<&str> = source.lines().collect();
    let mut pending: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('@') {
            let decorator: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !decorator.is_empty() {
                pending.push(decorator);
            }
            continue;
        }
        let declares_struct =
            trimmed.starts_with("struct ") || trimmed.starts_with("export struct ");
        let declares_class = trimmed.starts_with("class ") || trimmed.starts_with("export class ");
        let declares_enum = trimmed.starts_with("enum ") || trimmed.starts_with("export enum ");
        if !(declares_struct || declares_class || declares_enum) {
            if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("/*") {
                // The marker block ends at the declaration it decorates.
                if !trimmed.starts_with('@') {
                    pending.clear();
                }
            }
            continue;
        }
        let keyword = if declares_enum {
            "enum"
        } else if declares_class {
            "class"
        } else {
            "struct"
        };
        let name: String = trimmed[keyword.len()..]
            .trim()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect();
        if name.is_empty() {
            pending.clear();
            continue;
        }
        if declares_enum {
            collection.enum_collection.insert(name.clone());
        }
        if pending.iter().any(|decorator| decorator == "Component") {
            collection.custom_components.insert(name.clone());
            collect_decorated_fields(&lines[index + 1..], &name, collection);
        }
        if pending.iter().any(|decorator| decorator == "CustomDialog") {
            collection.custom_dialogs.insert(name.clone());
        }
        if pending
            .iter()
            .any(|decorator| *decorator == "Observed" || *decorator == "ObservedV2")
        {
            collection.observed_classes.insert(name.clone());
        }
        pending.clear();
    }
}

/// A struct body's `@Link` and `@State` fields, up to the matching closing brace.
fn collect_decorated_fields(
    body: &[&str],
    struct_name: &str,
    collection: &mut ComponentCollection,
) {
    let mut decorator: Option<&str> = None;
    let mut depth = 0usize;
    for line in body {
        let trimmed = line.trim();
        depth += trimmed.matches('{').count();
        depth = depth.saturating_sub(trimmed.matches('}').count());
        if depth == 0 && trimmed == "}" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix('@') {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let kind = match name.as_str() {
                "Link" => Some("Link"),
                "State" => Some("State"),
                _ => None,
            };
            // 装饰器与字段写在同一行时（`@Link value: number;`），剩余部分就是字段声明。
            let remainder = rest[name.len()..].trim();
            if remainder.is_empty() {
                decorator = kind;
                continue;
            }
            if let (Some(kind), Some((field, _))) = (kind, remainder.split_once(':')) {
                let field = field.trim();
                if !field.is_empty() {
                    match kind {
                        "Link" => collection
                            .link_collection
                            .entry(struct_name.to_string())
                            .or_default()
                            .insert(field.to_string()),
                        _ => collection
                            .state_collection
                            .entry(struct_name.to_string())
                            .or_default()
                            .insert(field.to_string()),
                    };
                }
            }
            decorator = None;
            continue;
        }
        let Some(kind) = decorator else {
            continue;
        };
        let Some((field, _)) = trimmed.split_once(':') else {
            decorator = None;
            continue;
        };
        let field = field.trim();
        if field.is_empty() {
            decorator = None;
            continue;
        }
        match kind {
            "Link" => collection
                .link_collection
                .entry(struct_name.to_string())
                .or_default()
                .insert(field.to_string()),
            _ => collection
                .state_collection
                .entry(struct_name.to_string())
                .or_default()
                .insert(field.to_string()),
        };
        decorator = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "@Observed\nclass Model {\n  size: number = 1;\n}\n\n@Component\nstruct Child {\n  @Link value: number;\n  @State own: string = \"\";\n\n  build() {\n    Column() {\n      Text(this.own)\n    }\n  }\n}\n\n@CustomDialog\nstruct Dialog {\n  build() {\n    Column() {}\n  }\n}\n\nenum Kind {\n  A,\n}\n";

    /// The declarations the walk records: components, dialogs, observed classes, enums.
    #[test]
    fn collection_records_declarations() {
        let mut errors = Vec::new();
        let collection = validate_ui_syntax(SOURCE, std::path::Path::new("Index.ets"), &mut errors);
        assert!(collection.custom_components.contains("Child"));
        assert!(collection.custom_dialogs.contains("Dialog"));
        assert!(collection.observed_classes.contains("Model"));
        assert!(collection.enum_collection.contains("Kind"));
        assert!(errors.is_empty(), "a clean file reports nothing");
    }

    /// A struct's decorated fields reach the link and state collections.
    #[test]
    fn collection_records_fields() {
        let mut errors = Vec::new();
        let collection = validate_ui_syntax(SOURCE, std::path::Path::new("Index.ets"), &mut errors);
        assert!(collection
            .link_collection
            .get("Child")
            .is_some_and(|fields| fields.contains("value")));
        assert!(collection
            .state_collection
            .get("Child")
            .is_some_and(|fields| fields.contains("own")));
    }
}
