// translator_resources: resource file copying and translation.
//
// Handles copy_dir_contents, copy_dir_recursive, copy_ets_files,
// and translate_resources (strings.json → string.json etc.).

use std::fs;
use std::path::Path;

use crate::translator::with_ets_suffix;
use serde_json::Value;

/// Copy all files in src/ to dest/ (non-recursive).
pub fn copy_dir_contents(src: &Path, dest: &Path) {
    if !src.exists() {
        return;
    }
    fs::create_dir_all(dest).expect("create dest dir");
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_file = entry.path();
            if src_file.is_file() {
                let dest_file = dest.join(src_file.file_name().unwrap());
                fs::copy(&src_file, &dest_file).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: copy {} → {}: {}",
                        src_file.display(),
                        dest_file.display(),
                        e
                    );
                    0
                });
            }
        }
    }
}

/// Recursively copy src/ to dest/.
pub fn copy_dir_recursive(src: &Path, dest: &Path) {
    if !src.exists() {
        return;
    }
    fs::create_dir_all(dest).expect("create dest dir");
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dest_path = dest.join(src_path.file_name().unwrap());
            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dest_path);
            } else {
                fs::copy(&src_path, &dest_path).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: copy {} → {}: {}",
                        src_path.display(),
                        dest_path.display(),
                        e
                    );
                    0
                });
            }
        }
    }
}

/// Copy .ets files, skipping additional directories (e.g. native module directories).
/// Copy module-root `.ets`/`.ts` files into `ets/`, skipping files that the caller has
/// already placed elsewhere (the entry ability, extension abilities) and native dirs.
pub fn copy_ets_files_skip(
    src: &Path,
    ets_dest: &Path,
    skip_files: &[String],
    skip_dirs: &[String],
) {
    if !src.exists() {
        return;
    }
    let skip_ets: Vec<String> = skip_files.iter().map(|f| with_ets_suffix(f)).collect();
    fs::create_dir_all(ets_dest).expect("create ets dir");
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if name == "hvolt.toml"
                || name == "main"
                || name == "pages"
                || name == "resources"
                || name == "native"
                || skip_dirs.contains(&name)
            {
                continue;
            }
            if skip_ets.contains(&name) {
                continue;
            }
            if path.is_dir() {
                copy_dir_recursive(&path, &ets_dest.join(&name));
            } else if name.ends_with(".ets") || name.ends_with(".ts") {
                let dest = ets_dest.join(&name);
                fs::copy(&path, &dest).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: copy {} → {}: {}",
                        path.display(),
                        dest.display(),
                        e
                    );
                    0
                });
            }
        }
    }
}
/// Translate user resource directory → HarmonyOS resource layout.
/// If src already has a `base/` subdirectory (HarmonyOS standard layout),
/// recursively copy the entire tree. Otherwise, translate flat hvolt layout.
pub fn translate_resources(src: &Path, base_dir: &Path) {
    if !src.exists() {
        return;
    }

    // If source already has HarmonyOS standard structure (base/ subdir),
    // copy only subdirectories to the parent of base_dir (not flat files at top level).
    if src.join("base").is_dir() {
        let resources_dir = base_dir.parent().unwrap();
        fs::create_dir_all(resources_dir).expect("create resources dir");
        if let Ok(entries) = fs::read_dir(src) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap();
                    let name_str = name.to_string_lossy();
                    // Skip top-level media/ — media belongs inside base/
                    if name_str == "media" {
                        continue;
                    }
                    copy_dir_recursive(&path, &resources_dir.join(name));
                }
            }
        }
        // Merge flat strings.json entries into base/element/string.json
        merge_flat_resources(src, resources_dir);
        // Deduplicate: remove from string.json any entries already in form.json
        dedup_string_against_form(&resources_dir.join("base/element"));
        return;
    }

    fs::create_dir_all(base_dir.join("element")).expect("create element");
    fs::create_dir_all(base_dir.join("media")).expect("create media");
    fs::create_dir_all(base_dir.join("profile")).expect("create profile");

    // HarmonyOS expects singular: string.json, color.json, float.json
    let element_rename: &[(&str, &str)] = &[
        ("strings.json", "string.json"),
        ("colors.json", "color.json"),
        ("floats.json", "float.json"),
    ];

    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();

            if path.is_dir() {
                match name.as_ref() {
                    "media" => copy_dir_contents(&path, &base_dir.join("media")),
                    "profiles" => copy_dir_contents(&path, &base_dir.join("profile")),
                    "dark" => {
                        let dark_dir = base_dir.parent().unwrap().join("dark/element");
                        copy_dir_contents(&path, &dark_dir);
                    }
                    _ if name.contains('_') => {
                        let locale_dir =
                            base_dir.parent().unwrap().join(format!("{}/element", name));
                        copy_dir_contents(&path, &locale_dir);
                    }
                    _ => {}
                }
            } else {
                if let Some(dest_name) = element_rename
                    .iter()
                    .find(|(src_name, _)| *src_name == name.as_ref())
                    .map(|(_, dest)| *dest)
                {
                    fs::copy(&path, base_dir.join("element").join(dest_name)).unwrap_or_else(|e| {
                        eprintln!("Warning: copy resource {}: {}", path.display(), e);
                        0
                    });
                }
            }
        }
    }
}

// ─── Hardlink-based mirror (incremental build) ──────────────────

/// Hard-link a single file, fall back to copy. Cleans stale targets.
pub fn hardlink_or_copy(src: &Path, dest: &Path) {
    if !src.exists() {
        if fs::remove_file(dest).is_err() {
            // best-effort cleanup
        }
        return;
    }
    if let Some(parent) = dest.parent() {
        if fs::create_dir_all(parent).is_err() {
            eprintln!("Warning: mkdir {} failed", parent.display());
            return;
        }
    }
    let _ = fs::remove_file(dest);
    if fs::hard_link(src, dest).is_err() && fs::copy(src, dest).is_err() {
        eprintln!(
            "Warning: copy {} → {} failed",
            src.display(),
            dest.display()
        );
    }
}

/// Mirror a directory: hardlink each file; delete stale files in dest.
pub fn mirror_dir(src: &Path, dest: &Path) {
    if !src.exists() {
        return;
    }
    if fs::create_dir_all(dest).is_err() {
        eprintln!("Warning: mkdir {} failed", dest.display());
        return;
    }

    let mut src_files = Vec::new();
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if entry.path().is_file() {
                src_files.push(name);
            } else if entry.path().is_dir() {
                mirror_dir(&src.join(&name), &dest.join(&name));
            }
        }
    }

    // Clean stale files in dest
    if let Ok(entries) = fs::read_dir(dest) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if entry.path().is_file() && !src_files.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    for name in &src_files {
        hardlink_or_copy(&src.join(name), &dest.join(name));
    }
}

pub fn mirror_ets_files_skip(
    module_root: &Path,
    ets_dest: &Path,
    skip_files: &[String],
    skip_dirs: &[String],
) {
    if !module_root.exists() {
        return;
    }
    let skip_ets: Vec<String> = skip_files.iter().map(|f| with_ets_suffix(f)).collect();
    if fs::create_dir_all(ets_dest).is_err() {
        eprintln!("Warning: mkdir {} failed", ets_dest.display());
        return;
    }

    let mut src_files = Vec::new();
    if let Ok(entries) = fs::read_dir(module_root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "hvolt.toml"
                || name == "main"
                || name == "pages"
                || name == "resources"
                || name == "native"
                || skip_dirs.contains(&name)
            {
                continue;
            }
            if skip_ets.contains(&name) {
                continue;
            }
            if entry.path().is_dir() {
                mirror_dir(&entry.path(), &ets_dest.join(&name));
            } else if name.ends_with(".ets") {
                src_files.push(entry.file_name());
            }
        }
    }

    // Clean stale .ets files
    if let Ok(entries) = fs::read_dir(ets_dest) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".ets") && !src_files.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Mirror resources with HarmonyOS renaming (strings.json → string.json, etc.)
/// If src already has a `base/` subdirectory, recursively mirror the whole tree.
pub fn mirror_resources(src: &Path, base_dir: &Path) {
    if !src.exists() {
        return;
    }

    // If source already has HarmonyOS standard structure, mirror only subdirs
    if src.join("base").is_dir() {
        let resources_dir = base_dir.parent().unwrap();
        if let Ok(entries) = fs::read_dir(src) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap();
                    let name_str = name.to_string_lossy();
                    // Skip top-level media/ — media belongs inside base/
                    if name_str == "media" {
                        continue;
                    }
                    mirror_dir(&path, &resources_dir.join(name));
                }
            }
        }
        // Merge flat strings.json entries into base/element/string.json
        merge_flat_resources(src, resources_dir);
        // Deduplicate: remove from string.json any entries already in form.json
        dedup_string_against_form(&resources_dir.join("base/element"));
        return;
    }

    let element_dir = base_dir.join("element");
    let media_dir = base_dir.join("media");
    let profile_dir = base_dir.join("profile");
    if fs::create_dir_all(&element_dir).is_err()
        || fs::create_dir_all(&media_dir).is_err()
        || fs::create_dir_all(&profile_dir).is_err()
    {
        return;
    }

    let element_rename: &[(&str, &str)] = &[
        ("strings.json", "string.json"),
        ("colors.json", "color.json"),
        ("floats.json", "float.json"),
    ];

    let src_media = src.join("media");
    if src_media.exists() {
        mirror_dir(&src_media, &media_dir);
    }

    for &(src_name, dest_name) in element_rename {
        hardlink_or_copy(&src.join(src_name), &element_dir.join(dest_name));
    }

    let src_profiles = src.join("profiles");
    if src_profiles.exists() {
        mirror_dir(&src_profiles, &profile_dir);
    }
}

/// Remove from string.json any entries whose names already appear in form.json.
/// This avoids "Resource conflict" errors from hvigor's restool.
fn dedup_string_against_form(element_dir: &Path) {
    let form_path = element_dir.join("form.json");
    let string_path = element_dir.join("string.json");
    if !form_path.exists() || !string_path.exists() {
        return;
    }

    let form_content = match fs::read_to_string(&form_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let string_content = match fs::read_to_string(&string_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Parse string.json as JSON, collect names from form.json
    let mut form_names: Vec<String> = Vec::new();
    for line in form_content.lines() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("\"name\":") {
            let rest = &trimmed[start + 7..].trim();
            if let Some(quoted) = rest.strip_prefix('"').and_then(|s| s.split('"').next()) {
                form_names.push(quoted.to_string());
            }
        }
    }

    if form_names.is_empty() {
        return;
    }

    // Parse string.json value, filter entries, re-serialize
    let mut parsed: Value = match serde_json::from_str(&string_content) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(arr) = parsed["string"].as_array_mut() {
        arr.retain(|entry| {
            let name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("");
            !form_names.contains(&name.to_string())
        });
    }

    let cleaned = serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| string_content.clone());
    if cleaned != string_content {
        // host fn returns () — full Err threading deferred; silence was the
        // defect (claimed done, not on disk)
        if let Err(e) = fs::write(&string_path, &cleaned) {
            eprintln!("✗ dedup write failed {}: {}", string_path.display(), e);
        }
    }
}

/// Merge entries from flat strings.json/colors.json/floats.json into base/element/.
/// Only adds entries whose names don't already exist in the target.
fn merge_flat_resources(src: &Path, resources_dir: &Path) {
    let mappings: &[(&str, &str, &str)] = &[
        ("strings.json", "string.json", "string"),
        ("colors.json", "color.json", "color"),
        ("floats.json", "float.json", "float"),
    ];

    for &(flat_name, target_name, array_key) in mappings {
        let flat_path = src.join(flat_name);
        if !flat_path.exists() {
            continue;
        }
        let target = resources_dir.join("base/element").join(target_name);
        if !target.exists() {
            continue;
        }

        let flat: Value = match fs::read_to_string(&flat_path) {
            Ok(c) => match serde_json::from_str(&c) {
                Ok(v) => v,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        let target_content = match fs::read_to_string(&target) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut target_json: Value = match serde_json::from_str(&target_content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let flat_entries = flat[array_key].as_array().cloned().unwrap_or_default();
        let target_arr = match target_json[array_key].as_array_mut() {
            Some(a) => a,
            None => continue,
        };

        let existing: Vec<String> = target_arr
            .iter()
            .filter_map(|e| {
                e.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        for entry in &flat_entries {
            let name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if !existing.contains(&name.to_string()) {
                target_arr.push(entry.clone());
            }
        }

        let merged = serde_json::to_string_pretty(&target_json).unwrap_or(target_content);
        if let Err(e) = fs::write(&target, &merged) {
            eprintln!("✗ merge write failed {}: {}", target.display(), e);
        }
    }
}
