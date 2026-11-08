// res_index_build: turn staged resource trees into a `ResourceIndex`, in restool's V1 shape.
//
// `res_index_write` knows the bytes and `res_ids` knows the ids; this module knows the entry
// list: which config each resource belongs to, what its value bytes are, and the order restool
// emits records in.
//
// Value encoding (measured on real indexes, 2026-11-08):
//   string / color / float / integer / boolean   the value text, NUL-terminated
//   media / profile                              `<sources>/<qualifier>/<kind>/<file>`, NUL-terminated
//   strarray / intarray / plural / theme          nested length-prefixed table (not handled here)
//
// `<sources>` is the directory the resource came from as restool saw it: `entry/resources` for a
// module, and the dependency's own `resources` for a package. That string is reproduced from the
// staged tree, so the caller passes the prefix for each input directory.

#![allow(dead_code)] // the builder is exercised from a comparison test; res_compile still
                     // calls restool for the index until the config ordering is reproduced.

use std::path::Path;

use super::res_ids::GroupKind;
use super::res_index::{ConfigIndex, IndexEntry, Qualifier, ResourceIndex, ResourceType};

/// One resource already claimed in a config: the qualifier tuple, its type, and its name.
type SeenEntry = (Vec<(u32, u32)>, u32, String);

/// One staged input directory: where its resources live on disk and the prefix its value bytes
/// carry.
pub(crate) struct Input<'a> {
    pub kind: GroupKind,
    /// Directory as staged, holding one subdirectory per qualifier.
    pub dir: &'a Path,
    /// Prefix of the value bytes, e.g. `entry/resources`.
    pub sources: String,
}

/// One resource found in a staged tree.
struct Found {
    /// Directory the resource came from, matched against `Input::sources`.
    sources: String,
    qualifiers: Vec<Qualifier>,
    kind: ResourceType,
    name: String,
    value: Vec<u8>,
}

/// Collect the staged trees, assign ids the way restool would, and build the index.
///
/// This is the whole compile minus the file write, so `res_compile` can replace restool with
/// one call. `preset` carries `ids_map/id_defined.json` so an incremental build keeps the ids
/// it already handed out.
pub(crate) fn compile(
    version: &str,
    module_id: u32,
    inputs: &[Input<'_>],
    preset: &std::collections::BTreeSet<(u32, String, u32)>,
) -> Result<ResourceIndex, String> {
    let mut found: Vec<Found> = Vec::new();
    for input in inputs {
        collect(input, &mut found)?;
    }
    // Group by input directory for the allocator: restool numbers directories in
    // `applicationResource -> moduleResources -> dependencies` order.
    let mut groups: Vec<(GroupKind, Vec<(ResourceType, String)>)> = Vec::new();
    for input in inputs {
        groups.push((input.kind, Vec::new()));
    }
    let mut by_input: Vec<std::collections::BTreeSet<(u32, String)>> =
        vec![Default::default(); inputs.len()];
    let mut only: Vec<Found> = Vec::new();
    for entry in found {
        only.push(entry);
    }
    let _ = &mut by_input;
    for entry in &only {
        let _ = entry;
    }
    let assigned = super::res_ids::assign_with(
        module_id,
        &groups_of(&only, inputs),
        preset,
        module_id << 24,
    );
    let mut ids: std::collections::BTreeMap<(u32, String), u32> = Default::default();
    for (kind, name, id) in &assigned {
        ids.insert((kind.code(), name.clone()), *id);
    }
    build(version, inputs, &ids)
}

/// The allocator's view: one entry list per input directory, in input order.
fn groups_of(
    found: &[Found],
    inputs: &[Input<'_>],
) -> Vec<(GroupKind, Vec<(ResourceType, String)>)> {
    inputs
        .iter()
        .map(|input| {
            let mut items: Vec<(ResourceType, String)> = found
                .iter()
                .filter(|entry| entry.sources == input.sources)
                .map(|entry| (entry.kind, entry.name.clone()))
                .collect();
            items.sort_by_key(|(kind, name)| (kind.code(), name.clone()));
            items.dedup();
            (input.kind, items)
        })
        .collect()
}

/// Build the index for one compile from the staged trees and the id table.
///
/// `ids` maps `(type, name)` to the id restool would hand out; it comes from `res_ids`, which
/// reproduces restool's allocation. A resource with no id is an error rather than a guessed one.
pub(crate) fn build(
    version: &str,
    inputs: &[Input<'_>],
    ids: &std::collections::BTreeMap<(u32, String), u32>,
) -> Result<ResourceIndex, String> {
    let mut found: Vec<Found> = Vec::new();
    for input in inputs {
        let before = found.len();
        collect(input, &mut found)?;
        eprintln!(
            "index-build: {} -> {} entries (from {})",
            input.sources.len(),
            found.len() - before,
            input.dir.display()
        );
    }

    // Config order: the qualifier-free base first, then every other qualifier tuple in the
    // order restool emits them (by qualifier kind, then value).
    // A repeated `(type, name)` inside one config keeps its first declaration — a qualifier
    // variant is a *different* config and carries its own record (the en_US `app_name` of the
    // 641-byte sample has its own offset), so the dedup key includes the qualifier tuple.
    let mut seen: std::collections::BTreeSet<SeenEntry> = std::collections::BTreeSet::new();
    let mut groups: std::collections::BTreeMap<Vec<(u32, u32)>, Vec<Found>> =
        std::collections::BTreeMap::new();
    for entry in found {
        let key = entry
            .qualifiers
            .iter()
            .map(|q| (q.kind, q.value))
            .collect::<Vec<_>>();
        if !seen.insert((key.clone(), entry.kind.code(), entry.name.clone())) {
            continue;
        }
        groups.entry(key).or_default().push(entry);
    }

    let mut configs: Vec<ConfigIndex> = Vec::new();
    let mut keys: Vec<Vec<(u32, u32)>> = groups.keys().cloned().collect();
    // restool orders configs by their directory name, not by qualifier kind: the measured
    // sequence for one project is `base, dark, en, en_US, zh_CN, zh_HK, zh_TW`. Sorting by
    // `(kind, value)` puts `dark` (kind 6) last, which is where the first byte difference was.
    keys.sort_by_key(|qualifiers| qualifier_name(qualifiers));

    for key in keys {
        let mut entries: Vec<IndexEntry> = Vec::new();
        let members = groups.remove(&key).unwrap_or_default();
        // A config's entries are written in id order, not in (type, name) order: restool's
        // first base row is `app_name` (0x01000000), not the first integer.
        let mut numbered: Vec<(u32, Found)> = Vec::with_capacity(members.len());
        for member in members {
            let Some(id) = ids.get(&(member.kind.code(), member.name.clone())) else {
                return Err(format!(
                    "{} {} has no id: the allocation table is incomplete",
                    member.kind.code(),
                    member.name
                ));
            };
            numbered.push((*id, member));
        }
        numbered.sort_by_key(|(id, _)| *id);
        for (id, member) in numbered {
            entries.push(IndexEntry {
                id,
                kind: member.kind,
                name: member.name,
                value: member.value,
            });
        }
        configs.push(ConfigIndex {
            qualifiers: key
                .into_iter()
                .map(|(kind, value)| Qualifier { kind, value })
                .collect(),
            entries,
        });
    }

    Ok(ResourceIndex {
        version: version.to_string(),
        configs,
    })
}

fn collect(input: &Input<'_>, out: &mut Vec<Found>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(input.dir).map_err(|e| format!("read {}: {e}", input.dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let qualifier = entry.file_name().to_string_lossy().to_string();
        let qualifiers = parse_qualifier(&qualifier);
        for sub in std::fs::read_dir(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?
            .flatten()
        {
            let sub_path = sub.path();
            if !sub_path.is_dir() {
                continue;
            }
            let kind_name = sub.file_name().to_string_lossy().to_string();
            match kind_name.as_str() {
                "element" => collect_elements(input, &qualifier, &qualifiers, &sub_path, out)?,
                "media" | "profile" => {
                    let kind = if kind_name == "media" {
                        ResourceType::Media
                    } else {
                        ResourceType::Profile
                    };
                    collect_files(
                        input,
                        &qualifier,
                        &qualifiers,
                        kind,
                        &kind_name,
                        &sub_path,
                        out,
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn collect_elements(
    input: &Input<'_>,
    qualifier: &str,
    qualifiers: &[Qualifier],
    dir: &Path,
    out: &mut Vec<Found>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
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
        for (type_name, items) in map {
            let Some(kind) = type_named(type_name) else {
                continue;
            };
            let Some(list) = items.as_array() else {
                continue;
            };
            for item in list {
                let Some(name) = item.get("name").and_then(|value| value.as_str()) else {
                    continue;
                };
                let Some(value) = element_value(kind, item) else {
                    continue;
                };
                out.push(Found {
                    sources: input.sources.clone(),
                    qualifiers: qualifiers.to_vec(),
                    kind,
                    name: name.to_string(),
                    value,
                });
            }
        }
    }
    let _ = (input, qualifier);
    Ok(())
}

/// The record value bytes one element contributes.
///
/// Scalars are their text plus a NUL. An array is a run of `u16le length + bytes + NUL` entries
/// closed by a zero length (measured: `config_auto_update_text_picker` is
/// `06 00 <6 bytes> 00 06 00 … 00 00`).
fn element_value(kind: ResourceType, item: &serde_json::Value) -> Option<Vec<u8>> {
    let value = item.get("value")?;
    let text = match kind {
        ResourceType::String | ResourceType::Color => value.as_str()?.to_string(),
        ResourceType::Float => format_float(value),
        ResourceType::Integer => value.as_i64()?.to_string(),
        ResourceType::Boolean => {
            if value.as_bool()? {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        ResourceType::StrArray => {
            let mut bytes = Vec::new();
            for item in value.as_array()? {
                // An array element is `{ "value": "…" }`, not a bare string.
                let text = item
                    .as_str()
                    .or_else(|| item.get("value").and_then(|inner| inner.as_str()))?;
                bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
                bytes.push(0);
            }
            // The run closes with a single zero byte, not a zero length: the measured payload
            // ends `…天\0 0`.
            bytes.push(0);
            return Some(bytes);
        }
        // The remaining nested tables (intarray, plural, theme) are not modelled yet; leaving
        // them out keeps the build honest instead of guessing a payload.
        _ => return None,
    };
    let mut bytes = text.into_bytes();
    bytes.push(0);
    Some(bytes)
}

/// restool prints floats with one fractional digit minimum (`1.0`, not `1`).
fn format_float(value: &serde_json::Value) -> String {
    // A float resource may be written as a bare number or as a dimension string ("14fp");
    // restool keeps the text as written, so a string is used verbatim.
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(number) = value.as_f64() {
        let text = format!("{number}");
        return if text.contains('.') {
            text
        } else {
            format!("{text}.0")
        };
    }
    value.to_string()
}

fn collect_files(
    input: &Input<'_>,
    qualifier: &str,
    qualifiers: &[Qualifier],
    kind: ResourceType,
    kind_dir: &str,
    dir: &Path,
    out: &mut Vec<Found>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        let value = format!("{}/{qualifier}/{kind_dir}/{file}", input.sources);
        let mut bytes = value.into_bytes();
        bytes.push(0);
        out.push(Found {
            sources: input.sources.clone(),
            qualifiers: qualifiers.to_vec(),
            kind,
            name: stem.to_string(),
            value: bytes,
        });
    }
}

/// The qualifier tuple a staged directory name names (`en_US`, `dark`, `base`).
fn parse_qualifier(name: &str) -> Vec<Qualifier> {
    if name == "base" {
        return Vec::new();
    }
    if name == "dark" {
        return vec![Qualifier { kind: 6, value: 0 }];
    }
    let mut out = Vec::new();
    let mut parts = name.split('_');
    if let Some(language) = parts.next() {
        if !language.is_empty() {
            out.push(Qualifier {
                kind: 0,
                value: pack(language),
            });
        }
    }
    if let Some(region) = parts.next() {
        if !region.is_empty() {
            out.push(Qualifier {
                kind: 1,
                value: pack(region),
            });
        }
    }
    out
}

/// Up to four ASCII characters right-aligned in a u32 (`en` → `0x0000656e`).
/// The directory name a qualifier tuple came from (`base`, `dark`, `en_US`, `zh_CN`).
fn qualifier_name(qualifiers: &[(u32, u32)]) -> String {
    if qualifiers.is_empty() {
        return "base".to_string();
    }
    // A color-mode qualifier has no name of its own in the directory form restool accepts.
    if qualifiers.iter().any(|(kind, _)| *kind == 6) {
        return "dark".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for (_, value) in qualifiers {
        let bytes = value.to_be_bytes();
        let text: String = bytes
            .iter()
            .filter(|byte| **byte != 0)
            .map(|byte| *byte as char)
            .collect();
        parts.push(text);
    }
    parts.join("_")
}

fn pack(text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut value = 0u32;
    for byte in bytes.iter().take(4) {
        value = (value << 8) | u32::from(*byte);
    }
    value
}

pub(crate) fn type_named(name: &str) -> Option<ResourceType> {
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
#[cfg(test)]
mod tests {
    use super::*;

    /// Compare a generated index with restool's for the same compile.
    ///
    /// `HVOLT_INDEX_BUILD` names the compile's output directory (`build/hvolt-out/res`),
    /// `HVOLT_STAGE` its staged module tree, `HVOLT_MODULE` the module name. The comparison
    /// covers the entry kinds this builder models; anything it cannot encode yet is counted
    /// and reported instead of being silently ignored.
    #[test]
    fn compares_with_restool_when_the_environment_names_a_compile() {
        let (Ok(out_dir), Ok(stage), Ok(module)) = (
            std::env::var("HVOLT_INDEX_BUILD"),
            std::env::var("HVOLT_STAGE"),
            std::env::var("HVOLT_MODULE"),
        ) else {
            return;
        };
        let table =
            std::fs::read_to_string(std::path::Path::new(&out_dir).join("ResourceTable.txt"))
                .expect("read ResourceTable.txt");
        let mut ids: std::collections::BTreeMap<(u32, String), u32> = Default::default();
        for line in table.lines() {
            let mut parts = line.split_whitespace();
            let (Some(kind), Some(name), Some(id)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let Some(kind) = type_code(kind) else {
                continue;
            };
            let id = u32::from_str_radix(id.trim_start_matches("0x"), 16).expect("id");
            ids.insert((kind, name.to_string()), id);
        }

        // Every input restool saw, with the value prefix each one's records carry.
        let project = std::env::var("HVOLT_PROJECT").unwrap_or_default();
        let config_path = std::path::Path::new(&out_dir).join("resConfig.json");
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&config_path).expect("read resConfig.json"),
        )
        .expect("parse resConfig.json");
        // `resConfig.json` spells staged paths relative to the project root; the test runs from
        // the repository, so they are resolved here.
        let resolve = |path: &str| -> String {
            if path.starts_with('/') || project.is_empty() {
                return path.to_string();
            }
            format!("{project}/{}", path.trim_start_matches("./"))
        };
        let mut owned: Vec<(GroupKind, String)> = Vec::new();
        if let Some(dir) = config.get("applicationResource").and_then(|v| v.as_str()) {
            owned.push((GroupKind::ApplicationResource, resolve(dir)));
        }
        for dir in config
            .get("moduleResources")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
        {
            let _ = &stage;
            owned.push((GroupKind::ModuleResources, resolve(dir)));
        }
        for dir in config
            .get("dependencies")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
        {
            owned.push((GroupKind::Dependency, resolve(dir)));
        }
        let inputs: Vec<Input<'_>> = owned
            .iter()
            .map(|(kind, dir)| Input {
                kind: *kind,
                dir: std::path::Path::new(dir),
                // The record names the directory as restool saw it; the module's own tree is
                // `<module>/resources`, not the staged copy's path.
                // Every input's records name the module's own tree: `app-stage` is the
                // app-scope slice, and restool copies dependency resources into the module
                // before writing the index, so a dependency's media is recorded as
                // `<module>/resources/...` too.
                sources: format!("{module}/resources"),
            })
            .collect();
        let built = build("Restool 6.1.0.003", &inputs, &ids).expect("build the index");

        let theirs = std::fs::read(std::path::Path::new(&out_dir).join("resources.index"))
            .expect("read restool's index");
        let theirs = super::super::res_index::parse(&theirs).expect("parse restool's index");

        let mut same = 0usize;
        let mut differing: Vec<String> = Vec::new();
        for reference in theirs.configs.iter() {
            // Configs are matched by their qualifier tuple: restool's order is not ours.
            let key: Vec<(u32, u32)> = reference
                .qualifiers
                .iter()
                .map(|q| (q.kind, q.value))
                .collect();
            let Some(ours) = built.configs.iter().find(|candidate| {
                candidate
                    .qualifiers
                    .iter()
                    .map(|q| (q.kind, q.value))
                    .collect::<Vec<_>>()
                    == key
            }) else {
                differing.push(format!("config {key:?} missing from ours"));
                continue;
            };
            for entry in &reference.entries {
                match ours.entries.iter().find(|candidate| {
                    candidate.name == entry.name && candidate.kind.code() == entry.kind.code()
                }) {
                    Some(candidate)
                        if candidate.id == entry.id && candidate.value == entry.value =>
                    {
                        same += 1
                    }
                    Some(candidate) => differing.push(format!(
                        "{} {}: restool {:08x}/{:?}, ours {:08x}/{:?}",
                        entry.kind.code(),
                        entry.name,
                        entry.id,
                        String::from_utf8_lossy(&entry.value),
                        candidate.id,
                        String::from_utf8_lossy(&candidate.value)
                    )),
                    None => differing.push(format!(
                        "{} {}: missing from ours",
                        entry.kind.code(),
                        entry.name
                    )),
                }
            }
        }
        println!(
            "index build: configs ours/theirs = {}/{}, entries equal = {}, differing = {}",
            built.configs.len(),
            theirs.configs.len(),
            same,
            differing.len()
        );
        for line in differing.iter().take(8) {
            println!("  {line}");
        }
        assert!(
            differing.is_empty(),
            "{} entries differ between the generated index and restool's",
            differing.len()
        );

        // The bytes, not just the entries. Every entry already matches; what is still open is
        // the config ordering, so this reports the difference instead of asserting on it.
        let raw = std::fs::read(std::path::Path::new(&out_dir).join("resources.index"))
            .expect("read restool's index bytes");
        let ours = super::super::res_index_write::write(&built);
        for (position, config) in built.configs.iter().enumerate().take(2) {
            println!(
                "ours config {position} first: {:?}",
                config
                    .entries
                    .iter()
                    .take(4)
                    .map(|e| format!("{}:{:08x}", e.name, e.id))
                    .collect::<Vec<_>>()
            );
        }
        for (position, config) in theirs.configs.iter().enumerate().take(2) {
            println!(
                "restool config {position} first: {:?}",
                config
                    .entries
                    .iter()
                    .take(4)
                    .map(|e| format!("{}:{:08x}", e.name, e.id))
                    .collect::<Vec<_>>()
            );
        }
        if ours == raw {
            println!("index bytes: identical ({} bytes)", raw.len());
        } else {
            let at = ours
                .iter()
                .zip(raw.iter())
                .position(|(left, right)| left != right)
                .unwrap_or(ours.len().min(raw.len()));
            println!(
                "index bytes: ours {} restool {}, first difference at {at:#x}",
                ours.len(),
                raw.len()
            );
        }
    }

    fn type_code(name: &str) -> Option<u32> {
        Some(match name {
            "string" => 9,
            "color" => 14,
            "float" => 18,
            "integer" => 8,
            "boolean" => 12,
            "strarray" => 10,
            "intarray" => 11,
            "plural" => 17,
            "theme" => 16,
            "media" => 19,
            "profile" => 20,
            _ => return None,
        })
    }
}
