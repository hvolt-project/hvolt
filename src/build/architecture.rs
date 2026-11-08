//! Structural invariants, checked mechanically.
//!
//! The refactor's value is not only that the output matches the reference today, but that the
//! shape that makes it match cannot quietly come apart: one printer, and both lowering paths
//! going through it. A prose note cannot hold that line — a new pass can always start printing
//! on its own — so these tests read the sources and fail when the shape changes.

#![cfg(test)]

use std::path::{Path, PathBuf};

/// Every Rust source file under `src/`.
fn sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out
}

/// The path of a source file relative to `src/`, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// oxc's code generator is configured in exactly one place.
///
/// A second configuration would decide quoting or indentation for one file and not another,
/// which is the drift the shared printer exists to prevent.
#[test]
fn the_printer_is_configured_in_one_place() {
    let mut offenders: Vec<String> = Vec::new();
    for path in sources() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Split so this file does not match itself.
        let marker = concat!("oxc_codegen", "::Codegen");
        if text.contains(marker) && relative(&path) != "build/lower/ets_output.rs" {
            offenders.push(relative(&path));
        }
    }
    assert!(
        offenders.is_empty(),
        "codegen is configured outside the printer: {offenders:?}"
    );
}

/// Both lowering paths hand their statements to the printer.
///
/// The assertion is about the wiring, not the call: a path that stops going through
/// `ets_output` is a path that lays text out by itself again.
#[test]
fn both_lowering_paths_use_the_shared_printer() {
    for path in ["build/lower/ets_plain.rs", "build/lower/ets_component.rs"] {
        let full = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(path);
        let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("read {path}: {e}"));
        assert!(
            text.contains("ets_output::"),
            "{path} does not go through the printer"
        );
    }
}

/// The component path reads its statements from a parse, not from a line scanner.
///
/// `read_component_file` is the one place that parses the prepared source; a second parse would
/// let a node span drift from the text it indexes.
#[test]
fn the_component_path_parses_once() {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/build/lower/ets_component.rs");
    let text = std::fs::read_to_string(&full).expect("read ets_component");
    let parses = text.matches("Parser::new(").count();
    assert_eq!(
        parses, 1,
        "the component path parses the source {parses} times; it must parse once"
    );
}
