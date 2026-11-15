// diagnostics: turn es2abc's printed diagnostics into the structured report.
//
// The compiler prints through `CLIDiagnosticPrinter::Print`
// (`ets2panda/util/diagnosticEngine.cpp`), which writes
//
//   <type>: <message> [<basename>:<line>:<column>]
//
// with the type string coming from `DiagnosticTypeToString` (`util/diagnostic.cpp`), and only
// the file's base name. The compiler has the absolute path; the printer drops it. Mapping is
// therefore done here: a base name that matches exactly one staged file becomes that file's
// path relative to the staged root, and anything else keeps the base name and is marked
// ambiguous so the editor can list it without marking a line.
use std::path::{Path, PathBuf};

/// One parsed diagnostic.
pub struct Row {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub severity: String,
    pub message: String,
    pub ambiguous: bool,
}

const WARNING_TYPES: [&str; 3] = ["Warning", "Plugin warning", "Declgen ets2ts warning"];

fn split_type(line: &str) -> Option<(&str, &str)> {
    let (head, rest) = line.split_once(": ")?;
    Some((head.trim(), rest))
}

fn split_location(rest: &str) -> Option<(&str, &str, u32, u32)> {
    let (message, tail) = rest.rsplit_once(" [")?;
    let inner = tail.strip_suffix(']')?;
    let mut parts = inner.rsplitn(3, ':');
    let col = parts.next()?.trim().parse::<u32>().ok()?;
    let line_no = parts.next()?.trim().parse::<u32>().ok()?;
    let file = parts.next()?.trim();
    if file.is_empty() {
        return None;
    }
    Some((message, file, line_no, col))
}

/// Parse every diagnostic line in `text`. Lines that do not match are ignored: the compiler
/// interleaves progress output with diagnostics.
pub fn parse(text: &str) -> Vec<Row> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches(['\r', ' ', '\t']);
        let Some((kind, rest)) = split_type(line) else {
            continue;
        };
        if !is_type(kind) {
            continue;
        }
        let Some((message, file, line_no, col)) = split_location(rest) else {
            continue;
        };
        out.push(Row {
            file: file.to_string(),
            line: line_no,
            col,
            severity: severity_of(kind),
            message: message.trim().to_string(),
            ambiguous: false,
        });
    }
    out
}

fn is_type(kind: &str) -> bool {
    matches!(
        kind,
        "Fatal error"
            | "SyntaxError"
            | "TypeError"
            | "Warning"
            | "Plugin warning"
            | "Plugin error"
            | "Declgen ets2ts error"
            | "Declgen ets2ts warning"
    )
}

fn severity_of(kind: &str) -> String {
    let severity = if WARNING_TYPES.contains(&kind) {
        "warning"
    } else {
        "error"
    };
    severity.to_string()
}

/// Resolve a base name against the staged tree: exactly one match becomes a relative path.
fn resolve(base: &str, sources: &[PathBuf], root: &Path) -> Option<String> {
    let matches: Vec<&PathBuf> = sources
        .iter()
        .filter(|p| p.file_name().map(|n| n == base).unwrap_or(false))
        .collect();
    if matches.len() != 1 {
        return None;
    }
    Some(
        matches[0]
            .strip_prefix(root)
            .unwrap_or(matches[0])
            .to_string_lossy()
            .to_string(),
    )
}

fn staged_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            staged_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Record a lowering failure. The Oxc message carries its own `line N` hint in the snippet;
/// the file is known to the caller, the column is not, so it is reported as 0 and the editor
/// marks the line only.
pub fn record_parse_failure(file: &str, message: &str) {
    let mut line = 0u32;
    if let Some(at) = message.find("line ") {
        let rest = &message[at + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(parsed) = digits.parse::<u32>() {
            line = parsed;
        }
    }
    let summary = message.split('\n').next().unwrap_or(message);
    crate::ffi::report::diagnostic(file, line, 0, "error", summary, false);
}

/// Parse `text` and record each diagnostic in the report, mapping base names against the
/// files staged under `ts_root`.
pub fn record(text: &str, ts_root: &Path) {
    let rows = parse(text);
    if rows.is_empty() {
        return;
    }
    let mut sources = Vec::new();
    staged_files(ts_root, &mut sources);
    for mut row in rows {
        match resolve(&row.file, &sources, ts_root) {
            Some(rel) => row.file = rel,
            None => row.ambiguous = true,
        }
        crate::ffi::report::diagnostic(
            &row.file,
            row.line,
            row.col,
            &row.severity,
            &row.message,
            row.ambiguous,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_compiler_format() {
        let text = "SyntaxError: Unexpected token [Index.ets:12:5]\n\
                    Warning: unused variable [hvolt.toml:0:0]\n";
        let rows = parse(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].file, "Index.ets");
        assert_eq!(rows[0].line, 12);
        assert_eq!(rows[0].col, 5);
        assert_eq!(rows[0].severity, "error");
        assert_eq!(rows[0].message, "Unexpected token");
        assert_eq!(rows[1].severity, "warning");
    }

    #[test]
    fn ignores_lines_that_are_not_diagnostics() {
        let text = "> hvolt build\n  rpcid.sc ./build/hvolt-out/rpcid.sc\n";
        assert!(parse(text).is_empty());
    }
}
