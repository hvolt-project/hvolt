// id_check: recompute restool's resource ids from the staged trees and compare.
//
// `res_ids::assign` reproduces restool's allocation from the input directories the
// way it was measured (2026-11-08: 5/5 samples byte-identical). Running it after a
// compile turns that measurement into a tripwire: if hvolt's own allocator and
// restool's output disagree, the build stops instead of shipping an index whose ids
// came from a model nobody re-checked.
//
// The inputs are read from `resConfig.json`, which restool consumed on this run, so
// the comparison sees exactly the directories restool saw rather than a second,
// possibly diverging scan.

use std::collections::BTreeSet;
use std::path::Path;

use super::res_ids::{self, GroupKind};
use super::res_index::ResourceType;

/// Recompute the ids of the compile in `out_dir` and compare with `ResourceTable.txt`.
///
/// Errors when the two disagree; the message names the first few mismatching entries.
pub(crate) fn verify(out_dir: &Path, own_module: &str) -> Result<usize, String> {
    let config_path = out_dir.join("resConfig.json");
    let text = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("read {}: {e}", config_path.display()))?;
    let config: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", config_path.display()))?;

    let names: Vec<String> = config
        .get("moduleNames")
        .and_then(|value| value.as_str())
        .map(|list| {
            list.split(',')
                .map(|name| name.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() {
        return Err("resConfig.json carries no moduleNames".to_string());
    }
    let module_id = res_ids::module_id(own_module, &names);

    let mut groups: Vec<(GroupKind, Vec<(ResourceType, String)>)> = Vec::new();
    if let Some(dir) = config.get("applicationResource").and_then(|v| v.as_str()) {
        groups.push((GroupKind::ApplicationResource, collect(Path::new(dir))));
    }
    if let Some(list) = config.get("moduleResources").and_then(|v| v.as_array()) {
        for dir in list.iter().filter_map(|v| v.as_str()) {
            groups.push((GroupKind::ModuleResources, collect(Path::new(dir))));
        }
    }
    if let Some(list) = config.get("dependencies").and_then(|v| v.as_array()) {
        for dir in list.iter().filter_map(|v| v.as_str()) {
            groups.push((GroupKind::Dependency, collect(Path::new(dir))));
        }
    }

    let preset = read_id_map(&out_dir.join("ids_map/id_defined.json"));
    let ours = res_ids::assign_with(module_id, &groups, &preset, module_id << 24);
    let theirs = read_table(&out_dir.join("ResourceTable.txt"))?;
    compare(&ours, &theirs)?;
    Ok(ours.len())
}

/// Every `(type, name)` one input directory declares, qualifier directories included.
fn collect(dir: &Path) -> Vec<(ResourceType, String)> {
    let mut out: BTreeSet<(u32, String)> = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "ResourceTable.txt" {
            continue;
        }
        // `base`, `en_US`, `dark`, … — each holds the same four subdirectories.
        collect_qualifier(&path, &mut out);
    }
    out.into_iter()
        .map(|(code, name)| (ResourceType::from_code(code), name))
        .collect()
}

fn collect_qualifier(dir: &Path, out: &mut BTreeSet<(u32, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let kind = entry.file_name().to_string_lossy().to_string();
        match kind.as_str() {
            "element" => collect_elements(&path, out),
            "media" => collect_files(&path, ResourceType::Media.code(), out),
            "profile" => collect_files(&path, ResourceType::Profile.code(), out),
            // rawfile carries no id; the rest of the tree is not a resource table.
            _ => {}
        }
    }
}

fn collect_elements(dir: &Path, out: &mut BTreeSet<(u32, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(map) = value.as_object() else {
            continue;
        };
        // An element file is `{ "<type>": [ { "name": …, "value": … }, … ] }`; the type comes
        // from the key, never from the file name (README element files disagree on it).
        for (type_name, items) in map {
            let Some(kind) = type_named(type_name) else {
                continue;
            };
            let Some(list) = items.as_array() else {
                continue;
            };
            for item in list {
                if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
                    out.insert((kind.code(), name.to_string()));
                }
            }
        }
    }
}

fn collect_files(dir: &Path, code: u32, out: &mut BTreeSet<(u32, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        out.insert((code, stem.to_string()));
    }
}

/// `ids_map/id_defined.json`: the ids earlier compiles handed out, as restool re-reads them.
fn read_id_map(path: &Path) -> std::collections::BTreeSet<(u32, String, u32)> {
    let mut out = std::collections::BTreeSet::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return out;
    };
    let Some(records) = value.get("record").and_then(|records| records.as_array()) else {
        return out;
    };
    for record in records {
        let (Some(kind), Some(name), Some(id)) = (
            record.get("type").and_then(|v| v.as_str()),
            record.get("name").and_then(|v| v.as_str()),
            record.get("id").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        let (Some(kind), Ok(id)) = (
            type_named(kind),
            u32::from_str_radix(id.trim_start_matches("0x"), 16),
        ) else {
            continue;
        };
        out.insert((kind.code(), name.to_string(), id));
    }
    out
}

/// `ResourceTable.txt` as restool writes it: `type name 0xID` per line, no trailing newline.
fn read_table(path: &Path) -> Result<Vec<(ResourceType, String, u32)>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(kind), Some(name), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let Some(kind) = type_named(kind) else {
            continue;
        };
        let id = u32::from_str_radix(id.trim_start_matches("0x"), 16)
            .map_err(|e| format!("resource id '{id}' in {}: {e}", path.display()))?;
        out.push((kind, name.to_string(), id));
    }
    Ok(out)
}

fn type_named(name: &str) -> Option<ResourceType> {
    Some(match name {
        "string" => ResourceType::String,
        "color" => ResourceType::Color,
        "float" => ResourceType::Float,
        "integer" => ResourceType::Integer,
        "boolean" => ResourceType::Boolean,
        "strarray" => ResourceType::StrArray,
        "intarray" => ResourceType::IntArray,
        "plural" => ResourceType::Plural,
        "theme" => ResourceType::Theme,
        "media" => ResourceType::Media,
        "profile" => ResourceType::Profile,
        _ => return None,
    })
}

/// Compare two allocation results by `(type, name) → id`.
fn compare(
    ours: &[(ResourceType, String, u32)],
    theirs: &[(ResourceType, String, u32)],
) -> Result<(), String> {
    // `ids_map/id_defined.json` carries ids from earlier runs, and restool keeps every one of
    // them: `ResourceTable.txt` is the union of this compile's resources and that history.
    // Only what this compile contributes can be checked against the allocator, so an entry
    // restool has but we do not is history, not a disagreement.
    let mut mine: std::collections::BTreeMap<(u32, String), u32> =
        std::collections::BTreeMap::new();
    for (kind, name, id) in ours {
        mine.insert((kind.code(), name.clone()), *id);
    }
    let mut assigned: std::collections::BTreeMap<(u32, String), u32> =
        std::collections::BTreeMap::new();
    for (kind, name, id) in theirs {
        assigned.insert((kind.code(), name.clone()), *id);
    }
    let mut diff: Vec<String> = Vec::new();
    for ((code, name), id) in &mine {
        match assigned.get(&(*code, name.clone())) {
            Some(theirs) if theirs == id => {}
            Some(theirs) => diff.push(format!(
                "{} {name}: restool 0x{theirs:08x}, ours 0x{id:08x}",
                name_of(ResourceType::from_code(*code))
            )),
            None => diff.push(format!(
                "{} {name}: restool missing, ours 0x{id:08x}",
                name_of(ResourceType::from_code(*code))
            )),
        }
    }
    if diff.is_empty() {
        return Ok(());
    }
    diff.sort();
    let shown: Vec<String> = diff.iter().take(5).cloned().collect();
    Err(format!(
        "resource ids disagree with restool on {} entries: {}",
        diff.len(),
        shown.join("; ")
    ))
}

fn name_of(kind: ResourceType) -> &'static str {
    match kind {
        ResourceType::String => "string",
        ResourceType::Color => "color",
        ResourceType::Float => "float",
        ResourceType::Integer => "integer",
        ResourceType::Boolean => "boolean",
        ResourceType::StrArray => "strarray",
        ResourceType::IntArray => "intarray",
        ResourceType::Plural => "plural",
        ResourceType::Theme => "theme",
        ResourceType::Media => "media",
        ResourceType::Profile => "profile",
        ResourceType::Other(_) => "other",
    }
}
