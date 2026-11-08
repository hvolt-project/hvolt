//! sourceMaps.map: the positions in the `.ts` this build emits, mapped back to the `.ets` source.
//!
//! The reference build writes one JSON document per build next to the compiled modules. Each entry
//! is named `<entry package>|<module>|<version>|<module-relative path>` and holds a standard source
//! map: `version`, `file`, `sources`, `names`, `mappings` (base64 VLQ) and `entry-package-info`.
//!
//! hvolt writes the same document from what it knows while lowering a file: the class that replaces
//! the struct, the fields it turns into observed properties, the `build()` body's component calls
//! (whose source line and column the UI parser records), and the lines the plain path copies
//! verbatim. Only files hvolt compiles get an entry, and every anchor points at a token boundary in
//! the source it claims — an anchor is a position someone can debug against, not filler.

use serde_json::{Map, Value};

use crate::build::ets_compile::UiItem;

/// A source position as the parser reports it: one-based line and column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// What the component parser knows about one struct, in parser coordinates (one-based).
pub(crate) struct ComponentPositions {
    pub name: String,
    /// The struct declaration: `struct <name>`.
    pub declaration: SourcePosition,
    /// The `build` keyword, when the struct declares `build()`.
    pub build: Option<SourcePosition>,
    /// Declared fields: name and the position of the field's own name.
    pub fields: Vec<(String, SourcePosition)>,
    /// Component calls in the `build()` body, in source order: component name and call position.
    pub calls: Vec<(String, SourcePosition)>,
}

/// One mapping: a zero-based generated position and the zero-based source position it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Anchor {
    pub generated_line: usize,
    pub generated_column: usize,
    pub source_line: usize,
    pub source_column: usize,
}

/// One compiled file and the anchors that describe it.
pub(crate) struct MapEntry {
    /// The module the file belongs to (`entry`).
    pub module: String,
    /// Module-relative path of the emitted file, e.g. `src/main/ets/pages/Index.ts`.
    pub module_path: String,
    /// The emitted basename, e.g. `Index.ts`; the reference names the original here (`Index.ets`).
    pub file: String,
    /// `<module>/<module_path with .ets>`, as the map's `sources` names it.
    pub source: String,
    pub anchors: Vec<Anchor>,
}

/// The one-based line and column of a byte offset.
pub(crate) fn line_column(text: &str, offset: usize) -> SourcePosition {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|s| s.chars().count())
        .unwrap_or(0)
        + 1;
    SourcePosition { line, column }
}

/// The anchors for one component file: the class, its fields, `build()` and its component calls,
/// plus the lines the plain path copies verbatim.
pub(crate) fn component_anchors(
    source: &str,
    content: &str,
    components: &[ComponentPositions],
) -> Vec<Anchor> {
    let source_lines: Vec<&str> = source.lines().collect();
    let content_lines: Vec<&str> = content.lines().collect();
    let mut anchors = Vec::new();
    let mut render_cursor = 0usize;
    for component in components {
        // The class replaces the struct: its name sits where the struct's name was.
        let class = format!("class {} extends", component.name);
        if let Some((line, column)) = find_content_line(&content_lines, 0, &class) {
            let name_offset = column + "class ".chars().count();
            if let Some(source_position) =
                resolve_token(&source_lines, component.declaration, &component.name)
            {
                anchors.push(Anchor {
                    generated_line: line,
                    generated_column: name_offset,
                    source_line: source_position.0,
                    source_column: source_position.1,
                });
            }
        }
        // `initialRender()` replaces `build()`.
        if let Some(build) = component.build {
            if let Some((line, column)) =
                find_content_line(&content_lines, render_cursor, "initialRender()")
            {
                if let Some(source_position) = resolve_token(&source_lines, build, "build") {
                    anchors.push(Anchor {
                        generated_line: line,
                        generated_column: column,
                        source_line: source_position.0,
                        source_column: source_position.1,
                    });
                }
                render_cursor = line + 1;
            }
        }
        // Every generated statement that mentions the field's backstore is a statement about that
        // field: point it at the field's declaration.
        for (name, position) in &component.fields {
            let Some(source_position) = resolve_token(&source_lines, *position, name) else {
                continue;
            };
            let patterns = [
                format!("this.__{name} = "),
                format!("this.__{name}."),
                format!("private __{name}:"),
                format!("get {name}()"),
                format!("set {name}("),
                format!("this.{name} = "),
            ];
            for pattern in patterns {
                let Some((line, column)) = find_content_line(&content_lines, 0, &pattern) else {
                    continue;
                };
                let token_offset = match pattern.as_str() {
                    _ if pattern.starts_with("private ") => "private ".chars().count(),
                    _ if pattern.starts_with("get ") => "get ".chars().count(),
                    _ if pattern.starts_with("set ") => "set ".chars().count(),
                    _ if pattern.starts_with("this.") => "this.".chars().count(),
                    _ => 0,
                };
                anchors.push(Anchor {
                    generated_line: line,
                    generated_column: column + token_offset,
                    source_line: source_position.0,
                    source_column: source_position.1,
                });
            }
        }
        // A component call is emitted as `<Component>.create(…)`; the call order is the source
        // order, so a cursor keeps the repeated components apart.
        let mut call_cursor = 0usize;
        for (name, position) in &component.calls {
            let pattern = format!("{name}.create(");
            let Some((line, column)) = find_content_line(&content_lines, call_cursor, &pattern)
            else {
                continue;
            };
            let Some(source_position) = resolve_token(&source_lines, *position, name) else {
                continue;
            };
            anchors.push(Anchor {
                generated_line: line,
                generated_column: column,
                source_line: source_position.0,
                source_column: source_position.1,
            });
            call_cursor = line + 1;
        }
    }
    anchors.extend(plain_anchors(source, content));
    anchors.sort();
    // One mapping per generated position: the segments of a line have to appear in strictly
    // increasing generated-column order, so a second claim about the same position is dropped.
    anchors.dedup_by(|left, right| {
        left.generated_line == right.generated_line
            && left.generated_column == right.generated_column
    });
    anchors
}

/// The anchors for a file the plain path lowers: the lines that survive verbatim. A line that
/// appears more than once in the source cannot be placed unambiguously and is left out.
pub(crate) fn plain_anchors(source: &str, content: &str) -> Vec<Anchor> {
    let source_lines: Vec<&str> = source.lines().collect();
    let mut anchors = Vec::new();
    for (generated_line, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if !is_mappable_line(trimmed) {
            continue;
        }
        let mut occurrences = source_lines
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.trim() == trimmed);
        let Some((source_index, _)) = occurrences.next() else {
            continue;
        };
        if occurrences.next().is_some() {
            continue;
        }
        let source_line = source_lines[source_index];
        anchors.push(Anchor {
            generated_line,
            generated_column: line.chars().count() - line.trim_start().chars().count(),
            source_line: source_index,
            source_column: source_line.chars().count() - source_line.trim_start().chars().count(),
        });
    }
    anchors
}

/// Whether a verbatim line is worth mapping: long enough to be distinctive, and starting with
/// something other than a closing delimiter. Import statements are rewritten before they reach
/// the emitted file, so they are not judged here.
fn is_mappable_line(trimmed: &str) -> bool {
    if trimmed.chars().count() < 4 {
        return false;
    }
    if trimmed.starts_with("import ") {
        return false;
    }
    let first = trimmed.chars().next().unwrap_or(' ');
    first.is_alphanumeric() || matches!(first, '_' | '$' | '@' | '\'' | '"')
}

/// The first content line at or after `from` that contains `needle`, with the needle's column.
fn find_content_line(lines: &[&str], from: usize, needle: &str) -> Option<(usize, usize)> {
    for (line, text) in lines.iter().enumerate().skip(from) {
        if let Some(index) = text.find(needle) {
            let column = text[..index].chars().count();
            return Some((line, column));
        }
    }
    None
}

/// Resolve a parser position to the actual source file: the parsers work on a rewritten copy, so a
/// line may have moved a little and a column may have shifted. Prefer the reported position, then
/// look nearby for the token; a position that resolves to nothing is dropped, never guessed.
fn resolve_token(
    source_lines: &[&str],
    expected: SourcePosition,
    token: &str,
) -> Option<(usize, usize)> {
    if expected.line == 0 {
        return None;
    }
    let base = expected.line - 1;
    if let Some(line) = source_lines.get(base) {
        let column = expected.column.saturating_sub(1);
        if token_at(line, column, token) {
            return Some((base, column));
        }
        // The column may have moved inside the reported line; the line itself is the better
        // hint than a neighbouring one.
        if let Some(column) = find_token(line, token) {
            return Some((base, column));
        }
    }
    for delta in [-1isize, 1, -2, 2, -3, 3] {
        let Some(index) = base.checked_add_signed(delta) else {
            continue;
        };
        let Some(line) = source_lines.get(index) else {
            continue;
        };
        if let Some(column) = find_token(line, token) {
            return Some((index, column));
        }
    }
    None
}

/// Whether `token` starts at a zero-based character column.
fn token_at(line: &str, column: usize, token: &str) -> bool {
    let start = line.char_indices().nth(column).map(|(index, _)| index);
    let Some(start) = start else {
        return false;
    };
    let Some(end) = start.checked_add(token.len()) else {
        return false;
    };
    line.get(start..end) == Some(token)
        && line[end..]
            .chars()
            .next()
            .is_none_or(|c| !is_token_character(c))
        && line[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_token_character(c))
}

/// The first occurrence of `token` as a whole identifier, with its zero-based character column.
fn find_token(line: &str, token: &str) -> Option<usize> {
    let mut from = 0usize;
    while let Some(found) = line[from..].find(token) {
        let start = from + found;
        let end = start + token.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        let left = before.is_none_or(|c| !is_token_character(c));
        let right = after.is_none_or(|c| !is_token_character(c));
        if left && right {
            return Some(line[..start].chars().count());
        }
        from = start + token.len();
        if from >= line.len() {
            return None;
        }
    }
    None
}

fn is_token_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '$')
}

/// The JSON document for one build.
pub(crate) fn document(entry_package: &str, version: &str, entries: &[MapEntry]) -> Value {
    let mut document = Map::new();
    for entry in entries {
        let key = format!(
            "{entry_package}|{}|{version}|{}",
            entry.module, entry.module_path
        );
        document.insert(
            key,
            serde_json::json!({
                "version": 3,
                "file": entry.file,
                "sourceRoot": "",
                "sources": [entry.source],
                "names": [],
                "mappings": encode_mappings(&entry.anchors),
                "entry-package-info": format!("{entry_package}|{version}"),
            }),
        );
    }
    Value::Object(document)
}

/// Encode anchors as a base64 VLQ `mappings` string.
pub(crate) fn encode_mappings(anchors: &[Anchor]) -> String {
    let mut mappings = String::new();
    let mut line = 0usize;
    let mut previous_generated = 0usize;
    let mut previous_source_line = 0i64;
    let mut previous_source_column = 0i64;
    let mut first_in_line = true;
    for anchor in anchors {
        while line < anchor.generated_line {
            mappings.push(';');
            line += 1;
            previous_generated = 0;
            first_in_line = true;
        }
        if !first_in_line {
            mappings.push(',');
        }
        first_in_line = false;
        encode_vlq(
            &mut mappings,
            anchor.generated_column as i64 - previous_generated as i64,
        );
        previous_generated = anchor.generated_column;
        encode_vlq(&mut mappings, 0);
        encode_vlq(
            &mut mappings,
            anchor.source_line as i64 - previous_source_line,
        );
        previous_source_line = anchor.source_line as i64;
        encode_vlq(
            &mut mappings,
            anchor.source_column as i64 - previous_source_column,
        );
        previous_source_column = anchor.source_column as i64;
    }
    mappings
}

fn encode_vlq(out: &mut String, value: i64) {
    const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut value = if value < 0 {
        ((-value) as u64) << 1 | 1
    } else {
        (value as u64) << 1
    };
    loop {
        let mut digit = (value & 31) as usize;
        value >>= 5;
        if value > 0 {
            digit |= 32;
        }
        out.push(BASE64[digit] as char);
        if value == 0 {
            break;
        }
    }
}

/// Collect the component calls of a `build()` body, in the order they are emitted.
pub(crate) fn collect_calls(items: &[UiItem], calls: &mut Vec<(String, SourcePosition)>) {
    for item in items {
        match item {
            UiItem::Node(node) => {
                calls.push((
                    node.component.clone(),
                    SourcePosition {
                        line: node.line,
                        column: node.col,
                    },
                ));
                collect_calls(&node.children, calls);
            }
            UiItem::If(branches) => {
                for branch in branches {
                    collect_calls(&branch.body, calls);
                }
            }
            UiItem::ForEach(node) | UiItem::LazyForEach(node) => {
                collect_calls(&node.body, calls);
            }
            UiItem::Repeat(repeat_) => {
                for link in &repeat_.links {
                    if let Some((_, body)) = &link.generator {
                        collect_calls(body, calls);
                    }
                }
            }
            UiItem::Raw(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_encode_generated_and_source_deltas() {
        let anchors = [
            Anchor {
                generated_line: 0,
                generated_column: 0,
                source_line: 0,
                source_column: 0,
            },
            Anchor {
                generated_line: 0,
                generated_column: 4,
                source_line: 0,
                source_column: 4,
            },
        ];
        assert_eq!(encode_mappings(&anchors), "AAAA,IAAI");
    }

    #[test]
    fn mappings_skip_unmapped_generated_lines() {
        let anchors = [Anchor {
            generated_line: 2,
            generated_column: 4,
            source_line: 1,
            source_column: 8,
        }];
        assert_eq!(encode_mappings(&anchors), ";;IACQ");
    }

    #[test]
    fn a_document_names_the_entry_and_its_source() {
        let entries = [MapEntry {
            module: "entry".to_string(),
            module_path: "src/main/ets/pages/Index.ts".to_string(),
            file: "Index.ets".to_string(),
            source: "entry/src/main/ets/pages/Index.ets".to_string(),
            anchors: vec![Anchor {
                generated_line: 0,
                generated_column: 0,
                source_line: 0,
                source_column: 0,
            }],
        }];
        let document = document("entry", "1.0.0", &entries);
        let object = document.as_object().expect("a map is an object");
        let entry = object
            .get("entry|entry|1.0.0|src/main/ets/pages/Index.ts")
            .expect("the entry key");
        assert_eq!(entry["version"], 3);
        assert_eq!(entry["file"], "Index.ets");
        assert_eq!(entry["sources"][0], "entry/src/main/ets/pages/Index.ets");
        assert_eq!(entry["entry-package-info"], "entry|1.0.0");
        assert_eq!(entry["mappings"], "AAAA");
    }

    #[test]
    fn verbatim_lines_map_and_duplicates_are_left_out() {
        let source = "const a = 1;\nconst b = 2;\nreturn x;\n}\nreturn x;\n";
        let content = "    const a = 1;\n    const b = 2;\n    return x;\n";
        let anchors = plain_anchors(source, content);
        // `return x;` appears twice in the source, so it is ambiguous and not mapped.
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].generated_line, 0);
        assert_eq!(anchors[0].generated_column, 4);
        assert_eq!(anchors[0].source_line, 0);
        assert_eq!(anchors[0].source_column, 0);
    }

    #[test]
    fn parser_positions_resolve_to_the_original_token() {
        let source_lines = ["@State count: number = 0", "  build() {", "    Text('a')"];
        let resolved = resolve_token(
            &source_lines,
            SourcePosition { line: 1, column: 1 },
            "count",
        );
        assert_eq!(resolved, Some((0, 7)));
        let resolved = resolve_token(
            &source_lines,
            SourcePosition { line: 2, column: 3 },
            "build",
        );
        assert_eq!(resolved, Some((1, 2)));
        let missing = resolve_token(
            &source_lines,
            SourcePosition { line: 1, column: 1 },
            "absent",
        );
        assert_eq!(missing, None);
    }

    #[test]
    fn a_component_call_maps_to_its_source_position() {
        let source = "@Entry\n@Component\nstruct Page {\n  build() {\n    Text('a')\n  }\n}\n";
        let content = "class Page extends ViewPU {\n    initialRender() {\n        this.observeComponentCreation2((elmtId, isInitialRender) => {\n            Text.create('a');\n        }, Text);\n    }\n}\n";
        let components = [ComponentPositions {
            name: "Page".to_string(),
            declaration: SourcePosition { line: 3, column: 8 },
            build: Some(SourcePosition { line: 4, column: 3 }),
            fields: Vec::new(),
            calls: vec![("Text".to_string(), SourcePosition { line: 5, column: 5 })],
        }];
        let anchors = component_anchors(source, content, &components);
        let call = anchors
            .iter()
            .find(|anchor| anchor.generated_line == 3)
            .expect("the component creation line");
        assert_eq!(call.generated_column, 12);
        assert_eq!(call.source_line, 4);
        assert_eq!(call.source_column, 4);
        let render = anchors
            .iter()
            .find(|anchor| anchor.generated_line == 1)
            .expect("the initialRender line");
        assert_eq!(render.source_line, 3);
        assert_eq!(render.source_column, 2);
    }
}
