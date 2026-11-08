// res_ids: assign resource ids the way restool does, then render its two id outputs.
//
// The allocator is pure: it takes the compile's module id and one item list per
// restool input directory, and returns `(type, name, id)` in allocation order.
// Rendering is separate because the two files restool writes order the same
// entries differently (`ResourceTable.txt` by type then name, `id_defined.json`
// by id).
//
// The high byte is NOT the number of modules in `moduleNames`. Measured
// 2026-11-08 by driving restool directly with only `moduleNames` changed:
// it is the 1-based rank of the module being compiled (`module.json`'s
// `module.name`) in the byte-order-sorted `moduleNames` list. `entry` alone is
// 0x01; `entry,clashbox,proxy_core,xb_components` puts `entry` second, so every
// resource gets 0x02. The earlier "high byte = module count" reading came from
// probes where the entry module happened to sort last, making rank and count
// equal. `module_id` below is that rank.
//
// The low 24 bits are one shared counter across every input directory, in the
// order applicationResource -> moduleResources -> dependencies, and inside one
// directory by (type code, name). A qualifier variant of an existing
// `(type, name)` reuses that id, but a name that exists only under a qualifier
// (`dark/media/background_white.png` with no base copy, measured on clashbox)
// is a resource of its own and does get one. A group is therefore the union of
// every qualifier directory's `(type, name)`; the caller collapses them. A
// `(type, name)` declared twice keeps its first id; restool warns about the
// duplicate and allocates nothing for the later copy.

#![allow(dead_code)] // the id allocator is not wired into res_compile yet

use std::collections::BTreeSet;

use super::res_index::ResourceType;

/// One restool input directory's role, which fixes the shared counter's order.
///
/// A dependency is one directory, not one list of all dependencies: restool
/// numbers the directories in `resConfig.dependencies` order, sorting only
/// inside each of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GroupKind {
    ApplicationResource,
    ModuleResources,
    Dependency,
}

impl GroupKind {
    fn rank(self) -> u8 {
        match self {
            Self::ApplicationResource => 0,
            Self::ModuleResources => 1,
            Self::Dependency => 2,
        }
    }
}

/// The high byte restool gives a compile of `own_module`.
///
/// This is the module's 1-based position once every `moduleNames` entry is
/// sorted by byte value (`"Entry"` sorts before `"entry"`). restool rejects a
/// manifest whose module name is not in the list; this function still returns
/// the rank it would have, so a caller can report the id before that check.
pub(crate) fn module_id(own_module: &str, module_names: &[String]) -> u32 {
    let lower = module_names
        .iter()
        .filter(|name| name.as_str() < own_module)
        .count();
    u32::try_from(lower + 1).unwrap_or(u32::MAX)
}

/// Assign ids to every resource of one restool compile.
///
/// `module_id` is the high byte from [`module_id`], not the module count.
/// A group is one input directory's whole resource set: the `(type, name)` union
/// of all its qualifier directories, since a qualifier variant of a base entry
/// does not allocate but a qualifier-only name does.
/// Groups may arrive in any order; they are numbered applicationResource first,
/// then moduleResources, then dependencies, keeping each kind's relative order.
/// Within a group, entries are sorted by `(type code, name)`, which is restool's
/// allocation order regardless of the order the JSON files list them in.
///
/// Returned in allocation order, so the ids are strictly ascending.
pub(crate) fn assign(
    module_id: u32,
    groups: &[(GroupKind, Vec<(ResourceType, String)>)],
) -> Vec<(ResourceType, String, u32)> {
    assign_with(module_id, groups, &BTreeSet::new(), module_id << 24)
}

/// [`assign`], reusing the ids an earlier compile already handed out.
///
/// restool starts from `ids_map/id_defined.json` and keeps every id it finds there: an entry
/// present in the map gets its recorded id and consumes no counter slot, and a new entry is
/// numbered from the highest low-24 value the map already holds. That is why
/// `ResourceTable.txt` accumulates across builds instead of being renumbered.
pub(crate) fn assign_with(
    module_id: u32,
    groups: &[(GroupKind, Vec<(ResourceType, String)>)],
    preset: &BTreeSet<(u32, String, u32)>,
    _high_byte: u32,
) -> Vec<(ResourceType, String, u32)> {
    let existing: std::collections::BTreeMap<(u32, String), u32> = preset
        .iter()
        .map(|(code, name, id)| ((*code, name.clone()), *id))
        .collect();
    let start = preset
        .iter()
        .filter(|(_, _, id)| id >> 24 == module_id)
        .map(|(_, _, id)| (id & 0x00FF_FFFF) + 1)
        .max()
        .unwrap_or(0);
    // restool's accepted masks are [0x01000000, 0x06FFFFFF]: one byte, 1..=6.
    assert!(
        (1..=6).contains(&module_id),
        "module id {module_id} is outside restool's 0x01..=0x06 high byte"
    );

    let mut ordered: Vec<&(GroupKind, Vec<(ResourceType, String)>)> = groups.iter().collect();
    ordered.sort_by_key(|(kind, _)| kind.rank());

    let base = module_id << 24;
    let mut counter: u32 = start;
    let mut seen: BTreeSet<(u32, String)> = BTreeSet::new();
    let mut ids = Vec::new();
    for (_, items) in ordered {
        let mut sorted: Vec<&(ResourceType, String)> = items.iter().collect();
        sorted.sort_by(|left, right| {
            left.0
                .code()
                .cmp(&right.0.code())
                .then_with(|| left.1.cmp(&right.1))
        });
        for (kind, name) in sorted {
            // restool keeps the first declaration of a duplicate and warns; the
            // later one gets no id, so the counter must not advance either.
            if !seen.insert((kind.code(), name.clone())) {
                continue;
            }
            if let Some(known) = existing.get(&(kind.code(), name.clone())) {
                ids.push((*kind, name.clone(), *known));
                continue;
            }
            assert!(
                counter < 0x0100_0000,
                "resource id counter overflowed the low 24 bits"
            );
            ids.push((*kind, name.clone(), base | counter));
            counter += 1;
        }
    }
    ids
}

/// `ResourceTable.txt`: both ids and names in restool's text form.
///
/// The file lists every entry sorted by (type code, name), NOT in id order, and
/// has no trailing newline.
pub(crate) fn render_table(entries: &[(ResourceType, String, u32)]) -> String {
    let mut sorted: Vec<&(ResourceType, String, u32)> = entries.iter().collect();
    sorted.sort_by(|left, right| {
        left.0
            .code()
            .cmp(&right.0.code())
            .then_with(|| left.1.cmp(&right.1))
    });
    sorted
        .iter()
        .map(|(kind, name, id)| format!("{} {} 0x{id:08x}", type_name(*kind), name))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `id_defined.json`: restool's tab-indented record list, in ascending id order.
///
/// Ids render as uppercase hex here while `ResourceTable.txt` uses lowercase;
/// that difference is restool's, and copying it is what makes the output
/// byte-identical.
pub(crate) fn render_id_defined(entries: &[(ResourceType, String, u32)]) -> String {
    if entries.is_empty() {
        return "{\"record\":[]}".to_string();
    }
    let mut sorted: Vec<&(ResourceType, String, u32)> = entries.iter().collect();
    sorted.sort_by_key(|(_, _, id)| *id);
    let mut out = String::from("{\n\t\"record\":\t[");
    for (at, (kind, name, id)) in sorted.iter().enumerate() {
        if at > 0 {
            out.push_str(", ");
        }
        out.push_str("{\n\t\t\t\"type\":\t\"");
        out.push_str(type_name(*kind));
        out.push_str("\",\n\t\t\t\"name\":\t\"");
        push_json_string(&mut out, name);
        out.push_str("\",\n\t\t\t\"id\":\t\"0x");
        out.push_str(&format!("{id:08X}"));
        out.push_str("\"\n\t\t}");
    }
    out.push_str("]\n}");
    out
}

/// The type name restool prints in both files.
fn type_name(kind: ResourceType) -> &'static str {
    match kind {
        ResourceType::Integer => "integer",
        ResourceType::String => "string",
        ResourceType::StrArray => "strarray",
        ResourceType::IntArray => "intarray",
        ResourceType::Boolean => "boolean",
        ResourceType::Color => "color",
        ResourceType::Theme => "theme",
        ResourceType::Plural => "plural",
        ResourceType::Float => "float",
        ResourceType::Media => "media",
        ResourceType::Profile => "profile",
        // No restool spelling exists: a code this crate has never seen cannot
        // reach either renderer through `assign` unless a caller fabricates it.
        ResourceType::Other(_) => "unknown",
    }
}

/// The inverse of [`type_name`], for reading a table back in or for a caller
/// that parses a resource description rather than the binary index.
fn type_from_name(name: &str) -> Option<ResourceType> {
    Some(match name {
        "integer" => ResourceType::Integer,
        "string" => ResourceType::String,
        "strarray" => ResourceType::StrArray,
        "intarray" => ResourceType::IntArray,
        "boolean" => ResourceType::Boolean,
        "color" => ResourceType::Color,
        "theme" => ResourceType::Theme,
        "plural" => ResourceType::Plural,
        "float" => ResourceType::Float,
        "media" => ResourceType::Media,
        "profile" => ResourceType::Profile,
        _ => return None,
    })
}

/// Append `text` as the inside of a JSON string.
///
/// Resource names are identifiers, so this only guards against a name that is
/// not one; emitting it verbatim would produce a file that no longer parses.
fn push_json_string(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: ResourceType, name: &str) -> (ResourceType, String) {
        (kind, name.to_string())
    }

    /// The measured rule: rank of the compiled module in the sorted name list.
    /// The first three cases are the ones the old "count" reading got right by
    /// construction (the entry module sorted last); the fourth shows a 4-name
    /// list whose entry module is second.
    #[test]
    fn module_id_is_the_sorted_rank_not_the_count() {
        let names = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(module_id("entry", &names(&["entry"])), 1);
        assert_eq!(module_id("entry", &names(&["entry", "dep", "dep2"])), 3);
        assert_eq!(module_id("entry", &names(&["entry", "mod_b", "mod_c"])), 1);
        assert_eq!(
            module_id(
                "entry",
                &names(&["entry", "clashbox", "proxy_core", "xb_components"])
            ),
            2
        );
        // Byte order, not case folding: 'E' (0x45) sorts before 'e' (0x65).
        assert_eq!(module_id("entry", &names(&["entry", "Entry"])), 2);
        assert_eq!(
            module_id(
                "proxy_core",
                &names(&["entry", "clashbox", "proxy_core", "xb_components"])
            ),
            3
        );
    }

    /// One counter spans all three roles; each role's own order is kept.
    #[test]
    fn assigns_one_counter_across_application_module_and_dependencies() {
        let groups = vec![
            (
                GroupKind::Dependency,
                vec![item(ResourceType::String, "dep_label")],
            ),
            (
                GroupKind::ApplicationResource,
                vec![
                    item(ResourceType::String, "app_name"),
                    item(ResourceType::Media, "startIcon"),
                ],
            ),
            (
                GroupKind::ModuleResources,
                vec![
                    item(ResourceType::String, "EntryAbility_desc"),
                    item(ResourceType::Color, "start_window_background"),
                    item(ResourceType::Profile, "main_pages"),
                ],
            ),
        ];
        let ids = assign(3, &groups);
        assert_eq!(
            ids,
            vec![
                (ResourceType::String, "app_name".into(), 0x0300_0000),
                (ResourceType::Media, "startIcon".into(), 0x0300_0001),
                (
                    ResourceType::String,
                    "EntryAbility_desc".into(),
                    0x0300_0002
                ),
                (
                    ResourceType::Color,
                    "start_window_background".into(),
                    0x0300_0003
                ),
                (ResourceType::Profile, "main_pages".into(), 0x0300_0004),
                (ResourceType::String, "dep_label".into(), 0x0300_0005),
            ]
        );
        // A second dependency directory keeps its place after the first and
        // numbers its own contents in (type, name) order.
        let groups = vec![
            (
                GroupKind::Dependency,
                vec![item(ResourceType::String, "dep_label")],
            ),
            (
                GroupKind::ApplicationResource,
                vec![
                    item(ResourceType::String, "app_name"),
                    item(ResourceType::Media, "startIcon"),
                ],
            ),
            (
                GroupKind::ModuleResources,
                vec![
                    item(ResourceType::String, "EntryAbility_desc"),
                    item(ResourceType::Color, "start_window_background"),
                    item(ResourceType::Profile, "main_pages"),
                ],
            ),
            (
                GroupKind::Dependency,
                vec![item(ResourceType::String, "dep2_label")],
            ),
        ];
        let ids = assign(3, &groups);
        assert_eq!(
            ids,
            vec![
                (ResourceType::String, "app_name".into(), 0x0300_0000),
                (ResourceType::Media, "startIcon".into(), 0x0300_0001),
                (
                    ResourceType::String,
                    "EntryAbility_desc".into(),
                    0x0300_0002
                ),
                (
                    ResourceType::Color,
                    "start_window_background".into(),
                    0x0300_0003
                ),
                (ResourceType::Profile, "main_pages".into(), 0x0300_0004),
                (ResourceType::String, "dep_label".into(), 0x0300_0005),
                (ResourceType::String, "dep2_label".into(), 0x0300_0006),
            ]
        );
    }

    /// Inside one directory restool uses (type code, name), not JSON order; and
    /// two dependency directories number their own contents but share the counter.
    #[test]
    fn sorts_each_directory_by_type_code_then_name() {
        let groups = vec![
            (
                GroupKind::ModuleResources,
                vec![
                    item(ResourceType::String, "zeta"),
                    item(ResourceType::Integer, "probe_b"),
                    item(ResourceType::String, "alpha"),
                    item(ResourceType::Integer, "probe_a"),
                    item(ResourceType::Color, "tint"),
                ],
            ),
            (
                GroupKind::Dependency,
                vec![
                    item(ResourceType::String, "dep_b"),
                    item(ResourceType::String, "dep_a"),
                ],
            ),
        ];
        let ids = assign(1, &groups);
        let names: Vec<&str> = ids.iter().map(|(_, name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            ["probe_a", "probe_b", "alpha", "zeta", "tint", "dep_a", "dep_b"]
        );
    }

    /// A duplicate `(type, name)` in a later directory gets no id and does not
    /// advance the counter (restool's "first declared" warning).
    #[test]
    fn keeps_the_first_id_of_a_duplicate_and_skips_the_later_copy() {
        let groups = vec![
            (
                GroupKind::ApplicationResource,
                vec![item(ResourceType::String, "app_name")],
            ),
            (
                GroupKind::Dependency,
                vec![
                    item(ResourceType::String, "app_name"),
                    item(ResourceType::String, "other"),
                ],
            ),
        ];
        let ids = assign(1, &groups);
        assert_eq!(
            ids,
            vec![
                (ResourceType::String, "app_name".into(), 0x0100_0000),
                (ResourceType::String, "other".into(), 0x0100_0001),
            ]
        );
    }

    /// 300 resources cross 0xFF with no carry into the high byte: the low 24
    /// bits are the counter, so id 0x0100012b is still module 1.
    #[test]
    fn the_counter_crosses_255_without_changing_the_high_byte() {
        let items: Vec<(ResourceType, String)> = (0..300)
            .map(|at| item(ResourceType::String, &format!("s_{at:03}")))
            .collect();
        let ids = assign(2, &[(GroupKind::ModuleResources, items)]);
        assert_eq!(ids.len(), 300);
        assert_eq!(ids[254].2, 0x0200_00fe);
        assert_eq!(ids[255].2, 0x0200_00ff);
        assert_eq!(ids[256].2, 0x0200_0100);
        assert_eq!(ids[299].2, 0x0200_012b);
        assert!(ids.iter().all(|(_, _, id)| id >> 24 == 2));
    }

    /// The exact text restool 6.1.0.003 wrote for the 15-resource type probe:
    /// sorted by (type, name), lowercase hex, no trailing newline.
    #[test]
    fn renders_restool_table_text() {
        let entries = vec![
            (
                ResourceType::Integer,
                "test_integer".to_string(),
                0x0100_0002,
            ),
            (
                ResourceType::String,
                "EntryAbility_desc".to_string(),
                0x0100_0003,
            ),
            (
                ResourceType::String,
                "EntryAbility_label".to_string(),
                0x0100_0004,
            ),
            (ResourceType::String, "app_name".to_string(), 0x0100_0000),
            (ResourceType::String, "module_desc".to_string(), 0x0100_0005),
            (
                ResourceType::StrArray,
                "test_strarray".to_string(),
                0x0100_0006,
            ),
            (
                ResourceType::IntArray,
                "test_intarray".to_string(),
                0x0100_0007,
            ),
            (
                ResourceType::Boolean,
                "test_boolean".to_string(),
                0x0100_0008,
            ),
            (
                ResourceType::Color,
                "start_window_background".to_string(),
                0x0100_0009,
            ),
            (ResourceType::Theme, "test_theme".to_string(), 0x0100_000a),
            (ResourceType::Plural, "test_plural".to_string(), 0x0100_000b),
            (ResourceType::Float, "test_float".to_string(), 0x0100_000c),
            (ResourceType::Media, "startIcon".to_string(), 0x0100_0001),
            (ResourceType::Profile, "main_pages".to_string(), 0x0100_000d),
            (
                ResourceType::Profile,
                "test_profile".to_string(),
                0x0100_000e,
            ),
        ];
        assert_eq!(
            render_table(&entries),
            "integer test_integer 0x01000002\n\
             string EntryAbility_desc 0x01000003\n\
             string EntryAbility_label 0x01000004\n\
             string app_name 0x01000000\n\
             string module_desc 0x01000005\n\
             strarray test_strarray 0x01000006\n\
             intarray test_intarray 0x01000007\n\
             boolean test_boolean 0x01000008\n\
             color start_window_background 0x01000009\n\
             theme test_theme 0x0100000a\n\
             plural test_plural 0x0100000b\n\
             float test_float 0x0100000c\n\
             media startIcon 0x01000001\n\
             profile main_pages 0x0100000d\n\
             profile test_profile 0x0100000e"
        );
    }

    /// The exact tab-indented record layout restool wrote, ids ascending and
    /// uppercase, no trailing newline.
    #[test]
    fn renders_restool_id_defined_json() {
        let entries = vec![
            (ResourceType::String, "app_name".to_string(), 0x0100_0000),
            (
                ResourceType::Profile,
                "test_profile".to_string(),
                0x0100_000e,
            ),
            (
                ResourceType::Color,
                "start_window_background".to_string(),
                0x0100_000a,
            ),
        ];
        assert_eq!(
            render_id_defined(&entries),
            "{\n\
             \t\"record\":\t[{\n\
             \t\t\t\"type\":\t\"string\",\n\
             \t\t\t\"name\":\t\"app_name\",\n\
             \t\t\t\"id\":\t\"0x01000000\"\n\
             \t\t}, {\n\
             \t\t\t\"type\":\t\"color\",\n\
             \t\t\t\"name\":\t\"start_window_background\",\n\
             \t\t\t\"id\":\t\"0x0100000A\"\n\
             \t\t}, {\n\
             \t\t\t\"type\":\t\"profile\",\n\
             \t\t\t\"name\":\t\"test_profile\",\n\
             \t\t\t\"id\":\t\"0x0100000E\"\n\
             \t\t}]\n\
             }"
        );
    }

    /// A name with a quote must still produce parseable JSON.
    #[test]
    fn escapes_a_name_that_is_not_an_identifier() {
        let text = render_id_defined(&[(ResourceType::String, "a\"b\\c".to_string(), 0x0100_0000)]);
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["record"][0]["name"], "a\"b\\c");
    }

    #[test]
    fn empty_input_renders_an_empty_record_list() {
        assert!(render_table(&[]).is_empty());
        assert_eq!(render_id_defined(&[]), "{\"record\":[]}");
    }

    #[test]
    fn type_names_round_trip() {
        for kind in [
            ResourceType::Integer,
            ResourceType::String,
            ResourceType::StrArray,
            ResourceType::IntArray,
            ResourceType::Boolean,
            ResourceType::Color,
            ResourceType::Theme,
            ResourceType::Plural,
            ResourceType::Float,
            ResourceType::Media,
            ResourceType::Profile,
        ] {
            assert_eq!(type_from_name(type_name(kind)), Some(kind));
        }
        assert_eq!(type_from_name("symbol"), None);
    }

    /// Reconcile the allocator with restool on the staged probe projects.
    ///
    /// `target/id-recon/manifests/*.json` is generated by
    /// `target/id-recon/tools/make_manifests.py` from each compile's actual
    /// `resConfig.json` inputs. This is an experiment, not a fixture: it is
    /// ignored by default and runs only when the caller names the directory.
    #[test]
    #[ignore = "reconciliation experiment: HVOLT_ID_RECON=<target/id-recon/manifests>"]
    fn reconciles_with_restool_outputs() {
        let Ok(dir) = std::env::var("HVOLT_ID_RECON") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let mut manifests: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        manifests.sort();
        assert!(
            !manifests.is_empty(),
            "no manifests under {}",
            dir.display()
        );

        for path in manifests {
            let text = std::fs::read_to_string(&path).expect("read manifest");
            let manifest: serde_json::Value = serde_json::from_str(&text).expect("manifest json");
            let case = manifest["case"].as_str().expect("case");
            let own = manifest["own_module"].as_str().expect("own_module");
            let names: Vec<String> = manifest["module_names"]
                .as_array()
                .expect("module_names")
                .iter()
                .map(|value| value.as_str().expect("name").to_string())
                .collect();
            let expected_module_id = manifest["module_id"].as_u64().expect("module_id") as u32;
            let computed = module_id(own, &names);
            assert_eq!(
                computed, expected_module_id,
                "{case}: module_id({own}, {names:?})"
            );

            let mut groups: Vec<(GroupKind, Vec<(ResourceType, String)>)> = Vec::new();
            for group in manifest["groups"].as_array().expect("groups") {
                let kind = match group["kind"].as_str().expect("kind") {
                    "application" => GroupKind::ApplicationResource,
                    "module" => GroupKind::ModuleResources,
                    "dependency" => GroupKind::Dependency,
                    other => panic!("{case}: unknown group kind `{other}`"),
                };
                let items = group["items"]
                    .as_array()
                    .expect("items")
                    .iter()
                    .map(|pair| {
                        let pair = pair.as_array().expect("pair");
                        let kind = type_from_name(pair[0].as_str().expect("type"))
                            .unwrap_or_else(|| panic!("{case}: unknown type {}", pair[0]));
                        (kind, pair[1].as_str().expect("name").to_string())
                    })
                    .collect();
                groups.push((kind, items));
            }

            let ids = assign(computed, &groups);
            let table_path = dir.join(manifest["table_path"].as_str().expect("table_path"));
            let defined_path = dir.join(
                manifest["id_defined_path"]
                    .as_str()
                    .expect("id_defined_path"),
            );
            let restool_table = std::fs::read_to_string(&table_path).expect("restool table");
            let restool_defined = std::fs::read_to_string(&defined_path).expect("restool ids");

            let our_table = render_table(&ids);
            let our_defined = render_id_defined(&ids);
            let table_ok = our_table == restool_table;
            let defined_ok = our_defined == restool_defined;
            println!(
                "{case}: module_id={computed} entries={} first={:#010x} table_match={table_ok} \
                 id_defined_match={defined_ok}",
                ids.len(),
                ids.first().map(|(_, _, id)| *id).unwrap_or(0)
            );
            if !table_ok {
                println!("{}", first_difference(&our_table, &restool_table));
            }
            if !defined_ok {
                println!("{}", first_difference(&our_defined, &restool_defined));
            }
            assert!(table_ok, "{case}: ResourceTable.txt differs");
            assert!(defined_ok, "{case}: id_defined.json differs");
        }
    }

    /// The first differing line (plus byte count) of two texts, for a failure
    /// message that names the resource instead of just "differs".
    fn first_difference(ours: &str, theirs: &str) -> String {
        let ours_lines: Vec<&str> = ours.lines().collect();
        let theirs_lines: Vec<&str> = theirs.lines().collect();
        let at = ours_lines
            .iter()
            .zip(theirs_lines.iter())
            .position(|(left, right)| left != right);
        match at {
            Some(index) => format!(
                "  first diff at line {}:\n    ours:   {:?}\n    restool:{:?}\n  \
                 lines ours={} restool={}",
                index + 1,
                ours_lines.get(index).unwrap_or(&"<end>"),
                theirs_lines.get(index).unwrap_or(&"<end>"),
                ours_lines.len(),
                theirs_lines.len()
            ),
            None => format!(
                "  line counts differ: ours={} restool={} (one is a prefix)",
                ours_lines.len(),
                theirs_lines.len()
            ),
        }
    }
}
