// translator_resources: resource file copying and translation.
//
// Handles copy_dir_contents, copy_dir_recursive, copy_ets_files,
// and translate_resources (strings.json → string.json etc.).

use std::fs;
use std::path::Path;

use crate::translator::with_ets_suffix;
use serde_json::Value;

/// Which files count as a native module's sources when mirroring into a project.
///
/// Headers used to be skipped entirely, so any module split across .h/.cpp silently
/// failed to compile after translation; CMakeLists.txt must travel with the sources.
pub fn is_native_source_file(name: &str) -> bool {
    name.ends_with(".cpp")
        || name.ends_with(".c")
        || name.ends_with(".cc")
        || name.ends_with(".h")
        || name.ends_with(".hpp")
        || name == "CMakeLists.txt"
}

/// Top-level CMakeLists.txt that wires each native module subdirectory in.
///
/// Every native module gets its own `src/main/cpp/<module>/` directory (a flat copy made
/// same-named files like `napi_init.cpp` or `CMakeLists.txt` overwrite each other), so the
/// module root needs a CMakeLists that `add_subdirectory`s them in order.
pub fn native_cmake_lists(native_names: &[&str]) -> String {
    let mut body =
        String::from("cmake_minimum_required(VERSION 3.5.0)\nproject(emustack_native)\n\n");
    for n in native_names {
        body.push_str(&format!("add_subdirectory({})\n", n));
    }
    body
}

/// Write a native module's types bridge (`oh-package.json5` + `Index.d.ts`) into the build
/// tree. Both the full-translate and the incremental-mirror paths call this, so edits to a
/// native module's `.d.ts` always reach the build tree — the mirror path used to skip types
/// entirely, which is why "rebuild" never picked up new declarations without wiping build/.
pub fn write_native_types_bridge(
    types_dir: &Path,
    pkg_name: &str,
    version: &str,
    description: &str,
    dts_src: &Path,
) {
    if fs::create_dir_all(types_dir).is_err() {
        eprintln!("Warning: mkdir {} failed", types_dir.display());
        return;
    }
    let oh_pkg = format!(
        r#"{{
  "name": "{}",
  "types": "./Index.d.ts",
  "version": "{}",
  "description": "{}"
}}"#,
        pkg_name, version, description
    );
    if let Err(e) = fs::write(types_dir.join("oh-package.json5"), oh_pkg) {
        eprintln!(
            "Warning: write {} failed: {}",
            types_dir.join("oh-package.json5").display(),
            e
        );
    }
    if dts_src.exists() {
        hardlink_or_copy(dts_src, &types_dir.join("Index.d.ts"));
    }
}

/// Mirror a module's `libs/` into the build tree, warning about files hvigor will drop.
///
/// hvigor's ProcessLibs only packs `*.so` from the module-root `libs/`. Everything else –
/// scripts, firmware blobs, probe binaries without the `.so` extension – is silently left
/// out of the HAP while the build still reports success. That silence is the trap: a build
/// that "worked" ships a package without the file. This function copies the whole tree
/// (callers may need non-.so inputs for their own steps) and prints which files will not
/// reach the package, so the mismatch is visible in the normal build output instead of
/// buried in `.hvigor/report/`.
///
/// Returns `(packed_so_count, dropped_file_names)` for tests and callers.
pub fn mirror_libs_with_warning(src: &Path, dest: &Path, member: &str) -> (usize, Vec<String>) {
    if !src.is_dir() {
        return (0, Vec::new());
    }
    println!(
        "  libs: {} → {}/libs (packed: *.so only; else dropped by hvigor)",
        src.display(),
        member
    );
    // Walk recursively: libs/ trees are usually abi dirs (arm64-v8a/, x86_64/).
    let mut dropped: Vec<String> = Vec::new();
    let mut packed: usize = 0;
    walk_libs(src, dest, &mut dropped, &mut packed);
    if !dropped.is_empty() {
        for f in &dropped {
            eprintln!(
                "  ⚠ libs/{}: not *.so — hvigor ProcessLibs will NOT pack it (build succeeds anyway)",
                f
            );
        }
        eprintln!(
            "  ⚠ {} file(s) in libs/ will be missing from the HAP; rename to *.so if they must ship",
            dropped.len()
        );
    }
    if packed == 0 && !dropped.is_empty() {
        eprintln!("  ⚠ libs/ has no *.so at all — the package will contain nothing from libs/");
    }
    (packed, dropped)
}

/// Recursively copy `src` into `dest`, classifying files as packed (*.so) or dropped.
/// Mirrors directory structure for both.
fn walk_libs(src: &Path, dest: &Path, dropped: &mut Vec<String>, packed: &mut usize) {
    if fs::create_dir_all(dest).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            walk_libs(&path, &dest.join(&name), dropped, packed);
            continue;
        }
        if !path.is_file() {
            continue;
        }
        // Copy regardless of extension: caller may consume non-.so inputs locally; only the
        // package step filters. Record what hvigor will drop so the build is honest about it.
        hardlink_or_copy(&path, &dest.join(&name));
        if name.ends_with(".so") {
            *packed += 1;
        } else {
            dropped.push(name);
        }
    }
}

/// Remove destination native-source files whose source no longer exists.
///
/// The mirror path used to only add/copy files; a source file deleted from the module left
/// a stale copy in `src/main/cpp/<module>/`, so CMake kept compiling yesterday's code until
/// the build tree was wiped. `dest` must be the module's cpp mirror directory (files only —
/// subdirectories such as `types/` are left alone).
pub fn purge_stale_native_files(src: &Path, dest: &Path) {
    let Ok(entries) = fs::read_dir(dest) else {
        return;
    };
    let mut src_names: Vec<String> = Vec::new();
    if let Ok(src_entries) = fs::read_dir(src) {
        for entry in src_entries.flatten() {
            if entry.path().is_file() {
                src_names.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !src_names.contains(&name) && is_native_source_file(&name) {
            let _ = fs::remove_file(&path);
        }
    }
}

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
                // module-owned native sources are translated separately; mirroring them into
                // ets/ would ship .cpp files (and CMakeLists) inside the HAP
                || name == "cpp"
                || name == "libs"
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
                || name == "cpp"
                || name == "libs"
                || skip_dirs.contains(&name)
            {
                continue;
            }
            if skip_ets.contains(&name) {
                continue;
            }
            if entry.path().is_dir() {
                mirror_dir(&entry.path(), &ets_dest.join(&name));
            } else if name.ends_with(".ets") || name.ends_with(".ts") {
                // Recording the name is not enough: the incremental path used to skip the copy
                // and then delete anything it had not listed, so root-level edits never landed.
                hardlink_or_copy(&entry.path(), &ets_dest.join(&name));
                src_files.push(entry.file_name());
            }
        }
    }

    // Clean stale .ets files
    if let Ok(entries) = fs::read_dir(ets_dest) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let n = name.to_string_lossy();
            if (n.ends_with(".ets") || n.ends_with(".ts")) && !src_files.contains(&name) {
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

#[cfg(test)]
mod ets_mirror_tests {
    use super::copy_ets_files_skip;
    use std::fs;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hvolt-ets-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Native sources belong to the cpp/lib translation, never to ets/: mirroring them used to
    /// ship .cpp files and CMakeLists.txt inside the HAP.
    #[test]
    fn cpp_and_libs_never_reach_ets() {
        let base = scratch("skip");
        let root = base.join("module");
        let ets = base.join("ets-out"); // must live outside the mirrored tree
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("cpp")).unwrap();
        fs::create_dir_all(root.join("libs/arm64-v8a")).unwrap();
        fs::create_dir_all(root.join("common")).unwrap();
        fs::write(root.join("cpp/CMakeLists.txt"), "project(x)").unwrap();
        fs::write(root.join("cpp/napi_init.cpp"), "//").unwrap();
        fs::write(root.join("libs/arm64-v8a/libx.so"), "x").unwrap();
        fs::write(root.join("common/a.ets"), "export {}").unwrap();

        copy_ets_files_skip(&root, &ets, &[], &[]);

        assert!(ets.join("common/a.ets").exists());
        assert!(
            !ets.join("cpp").exists(),
            "cpp must not be mirrored into ets"
        );
        assert!(
            !ets.join("libs").exists(),
            "libs must not be mirrored into ets"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod incremental_mirror_tests {
    use super::mirror_ets_files_skip;
    use std::fs;

    /// The incremental path must actually copy a root-level .ets/.ts file; it used to record
    /// the name, skip the copy, then delete anything it had not listed.
    #[test]
    fn root_sources_are_copied_on_the_mirror_path() {
        let base = std::env::temp_dir().join(format!("hvolt-mirror-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("module");
        let dest = base.join("ets-out");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Engine.ets"), "export const v = 1;").unwrap();
        fs::write(src.join("bridge.d.ts"), "declare const x: number;").unwrap();

        mirror_ets_files_skip(&src, &dest, &[], &[]);

        assert_eq!(
            fs::read_to_string(dest.join("Engine.ets")).unwrap(),
            "export const v = 1;"
        );
        let _ = fs::remove_dir_all(&base);
    }

    /// A source that disappears must disappear from the translated tree as well.
    #[test]
    fn stale_root_sources_are_removed() {
        let base = std::env::temp_dir().join(format!("hvolt-mirror-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("module");
        let dest = base.join("ets-out");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("Gone.ets"), "old").unwrap();

        mirror_ets_files_skip(&src, &dest, &[], &[]);

        assert!(!dest.join("Gone.ets").exists());
        let _ = fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod native_source_tests {
    use super::{is_native_source_file, native_cmake_lists};

    /// The mirror must carry every extension a native module actually compiles: headers used
    /// to be skipped, so any module split across .h/.cpp silently failed to build.
    #[test]
    fn headers_and_sources_are_native_files() {
        for name in [
            "napi_init.cpp",
            "native.c",
            "bridge.cc",
            "header.h",
            "header.hpp",
            "CMakeLists.txt",
        ] {
            assert!(is_native_source_file(name), "{} should be mirrored", name);
        }
    }

    /// Non-source artifacts (build outputs, docs, stray files) must not land in the cpp dir.
    #[test]
    fn non_sources_are_not_native_files() {
        for name in [
            "README.md",
            "libfoo.so",
            "foo.o",
            "build.ninja",
            "something.txt",
            "CMakeLists.txt.bak",
        ] {
            assert!(
                !is_native_source_file(name),
                "{} should not be mirrored",
                name
            );
        }
    }

    /// Each native module gets its own subdirectory; the top-level CMakeLists must wire every
    /// one of them in, in declaration order.
    #[test]
    fn top_level_cmake_lists_wires_every_module() {
        let out = native_cmake_lists(&["box64", "terminal", "gom"]);
        assert!(
            out.starts_with("cmake_minimum_required(VERSION 3.5.0)\nproject(emustack_native)\n\n")
        );
        assert!(out.contains("add_subdirectory(box64)\n"), "{}", out);
        assert!(out.contains("add_subdirectory(terminal)\n"), "{}", out);
        assert!(out.contains("add_subdirectory(gom)\n"), "{}", out);
        // The old flat copy wrote a single CMakeLists per module; the aggregator must not
        // reference itself or repeat a subdirectory.
        assert!(
            !out.contains("add_subdirectory(emustack_native)"),
            "{}",
            out
        );
        assert_eq!(out.matches("add_subdirectory(").count(), 3, "{}", out);
    }

    /// An empty native set produces no aggregator (callers skip the write entirely then).
    #[test]
    fn empty_native_set_yields_just_the_header() {
        let out = native_cmake_lists(&[]);
        assert!(out.contains("project(emustack_native)"), "{}", out);
        assert!(!out.contains("add_subdirectory"), "{}", out);
    }
}

#[cfg(test)]
mod libs_warning_tests {
    use super::mirror_libs_with_warning;
    use std::fs;

    /// Non-.so files are copied (so callers can consume them) but reported as dropped:
    /// hvigor's ProcessLibs only packs *.so, and the old silent success hid missing content.
    #[test]
    fn non_so_files_are_reported_as_dropped() {
        let base = std::env::temp_dir().join(format!("hvolt-libs-mix-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src/libs");
        fs::create_dir_all(src.join("arm64-v8a")).unwrap();
        fs::write(src.join("arm64-v8a/libreal.so"), "so").unwrap();
        fs::write(src.join("arm64-v8a/pt_probe.bin"), "probe").unwrap();
        fs::write(src.join("arm64-v8a/fw.img"), "img").unwrap();
        let dest = base.join("build/libs");

        let (packed, dropped) = mirror_libs_with_warning(&src, &dest, "entry");

        assert_eq!(packed, 1, "libreal.so should count as packed");
        assert_eq!(dropped.len(), 2, "non-.so files should be reported");
        assert!(
            dropped.contains(&"pt_probe.bin".to_string()),
            "{:?}",
            dropped
        );
        assert!(dropped.contains(&"fw.img".to_string()), "{:?}", dropped);
        // Files themselves are still mirrored (sub-abi dir structure preserved).
        assert!(dest.join("arm64-v8a/libreal.so").exists());
        assert!(dest.join("arm64-v8a/pt_probe.bin").exists());

        let _ = fs::remove_dir_all(&base);
    }

    /// No *.so at all must surface prominently instead of a silent successful build.
    #[test]
    fn empty_so_set_still_mirrors_everything() {
        let base = std::env::temp_dir().join(format!("hvolt-libs-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src/libs");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("data.bin"), "x").unwrap();
        let dest = base.join("build/libs");

        let (packed, dropped) = mirror_libs_with_warning(&src, &dest, "entry");

        assert_eq!(packed, 0);
        assert_eq!(dropped, vec!["data.bin".to_string()]);
        assert!(dest.join("data.bin").exists());

        let _ = fs::remove_dir_all(&base);
    }

    /// Missing source dir is a no-op (no warning noise for modules without libs/).
    #[test]
    fn missing_libs_dir_is_a_noop() {
        let base = std::env::temp_dir().join(format!("hvolt-libs-noop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src/libs"); // never created
        let dest = base.join("build/libs");
        let (packed, dropped) = mirror_libs_with_warning(&src, &dest, "entry");
        assert_eq!(packed, 0);
        assert!(dropped.is_empty());
        let _ = fs::remove_dir_all(&base);
    }
}
