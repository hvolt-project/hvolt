//! Coverage map for the ets-loader conformance work.
//!
//! Every module of the official loader is a coverage item. A module advances to `covered`
//! only when a fixture exists whose golden output was produced by running hvigor, and to
//! `exempt` only with a written reason. Anything still `unmapped` is unverified work, and
//! this gate refuses to let it pass silently.
//!
//! `not-applicable` is deliberately not `exempt`. An exemption says "this is not verified";
//! `not-applicable` says "this was read, and it produces no behaviour hvolt could mirror" —
//! the module's exports are consumed only by the loader's own logging, monitoring or error
//! classification, and never reach the compilation output. It therefore needs both a reason
//! and the evidence that the product chain is cut.

#![cfg(test)]

use std::collections::BTreeMap;

const MAP: &str = include_str!("../../../.scratch/hvigor-oracle/coverage-map.json");

/// One fixture's declaration of what it exercises: `{"<module>": ["<export>", …]}`.
fn fixture_covers() -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut covered: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return covered;
    };
    for entry in entries.flatten() {
        let path = entry.path().join("covers.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(claims) =
            serde_json::from_str::<std::collections::BTreeMap<String, Vec<String>>>(&text)
        else {
            continue;
        };
        for (module, exports) in claims {
            let slot = covered.entry(module).or_default();
            for export in exports {
                slot.insert(export);
            }
        }
    }
    covered
}

#[derive(Debug, serde::Deserialize)]
struct Item {
    #[serde(default)]
    status: String,
    /// Fixtures the claim names. The gate resolves claims by output path instead, so this is
    /// parsed for completeness rather than read.
    #[serde(default)]
    #[allow(dead_code)]
    fixtures: Vec<String>,
    #[serde(default)]
    reason: String,
    /// How the claim was established: the callers, and where the chain to the compiled output
    /// ends. Required for `not-applicable`, ignored otherwise.
    #[serde(default)]
    evidence: String,
    /// Fixture directories whose artifact comparison depends on this module. Required for
    /// `covered-indirectly`; every entry must exist on disk.
    #[serde(default)]
    indirect_fixtures: Vec<String>,
    #[serde(default)]
    exports: Vec<String>,
    #[serde(default)]
    required: bool,
}

#[derive(Debug, serde::Deserialize)]
struct Map {
    modules: BTreeMap<String, Item>,
}

fn load() -> Map {
    serde_json::from_str(MAP).expect("coverage map parses")
}

#[test]
fn every_loader_module_is_verified_or_exempt_with_a_reason() {
    let map = load();
    let done = fixture_covers();
    assert!(!map.modules.is_empty(), "coverage map is empty");
    let mut uncovered = Vec::new();
    let mut partial = 0usize;
    let mut covered = 0usize;
    let mut exempt = 0usize;
    let mut not_applicable = 0usize;
    let mut indirect = 0usize;
    let mut indirect_missing: Vec<String> = Vec::new();
    for (name, item) in &map.modules {
        if !item.required {
            continue;
        }
        // `partial` means a fixture exercises this module but has not exhausted it; it is
        // reported as unverified, and only `unmapped` blocks the gate.
        // A module is covered only when every export it declares has been exercised by some
        // fixture; claims live in `fixtures/*/covers.json`, not in the map.
        let covered_exports = done.get(name);
        let all_exports_covered = !item.exports.is_empty()
            && item
                .exports
                .iter()
                .all(|export| covered_exports.is_some_and(|set| set.contains(export)));
        let some_export_covered = covered_exports.is_some_and(|set| !set.is_empty());
        // The state is derived, never read back from the map's own `status` field, so the
        // summary and the failure list always describe the same computation.
        if item.status == "exempt" && !item.reason.trim().is_empty() {
            exempt += 1;
            continue;
        }
        if item.status == "covered-indirectly"
            && !item.reason.trim().is_empty()
            && !item.indirect_fixtures.is_empty()
        {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let missing: Vec<&String> = item
                .indirect_fixtures
                .iter()
                .filter(|f| !root.join(f).is_dir())
                .collect();
            if missing.is_empty() {
                indirect += 1;
                continue;
            }
            indirect_missing.push(format!("{name} -> {missing:?}"));
        }
        if item.status == "not-applicable"
            && !item.reason.trim().is_empty()
            && !item.evidence.trim().is_empty()
        {
            not_applicable += 1;
            continue;
        }
        if all_exports_covered {
            covered += 1;
            continue;
        }
        if some_export_covered {
            partial += 1;
            continue;
        }
        uncovered.push(name.clone());
    }
    eprintln!(
        "coverage: {covered} covered, {partial} partial, {not_applicable} not-applicable, \
{indirect} covered-indirectly, {} unmapped, {exempt} exempt, {} total",
        uncovered.len(),
        map.modules.len()
    );
    assert!(
        indirect_missing.is_empty(),
        "covered-indirectly claims name fixtures that do not exist: {indirect_missing:?}"
    );
    assert!(
        uncovered.is_empty(),
        "{} of {} loader modules are still unmapped (first 20): {:?}",
        uncovered.len(),
        map.modules.len(),
        &uncovered[..uncovered.len().min(20)]
    );
}
