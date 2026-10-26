// Assemble: translate user structure → HarmonyOS standard layout
// with hard‑link mirroring and toml‑hash manifest for incremental rebuilds.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;

use crate::config::AppConfig;
use crate::translator;
use crate::translator_resources;

/// Run the assemble step: check manifest, translate if needed, hardlink source files.
pub fn assemble(project_dir: &Path) {
    let root = project_dir;
    let build_dir = root.join("build");

    // 1. Parse root config
    let root_toml = root.join("hvolt.toml");
    if !root_toml.exists() {
        eprintln!("Error: hvolt.toml not found in {}", root.display());
        std::process::exit(1);
    }
    let app_config = AppConfig::from_file(&root_toml).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    let (modules, native_modules) = match translator::discover_modules(root, &app_config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("✗ assemble: {}", e);
            std::process::exit(1);
        }
    };

    // 3. Compute toml hash
    let current_hash = compute_toml_hash(root, &modules, &native_modules);

    // 4. Check manifest
    let manifest_path = build_dir.join(".hvolt_manifest");
    let needs_full = needs_full_rebuild(
        manifest_path.exists(),
        read_manifest_hash(&manifest_path),
        current_hash,
    );

    if needs_full {
        // Full rebuild
        if build_dir.exists() {
            fs::remove_dir_all(&build_dir).expect("clean build dir");
        }
        fs::create_dir_all(&build_dir).expect("create build dir");

        translator::full_translate(root, &build_dir, &app_config, &modules, &native_modules);

        write_manifest(&manifest_path, current_hash);
        println!("✓ Assemble (full): {} modules", modules.len());
    } else {
        // Incremental: mirror source files only
        mirror_source_files(root, &build_dir, &app_config, &modules, &native_modules);
        println!("✓ Assemble (mirror): {} modules", modules.len());
    }
}

/// Compute a hash covering root hvolt.toml + all module hvolt.toml contents.
fn compute_toml_hash(
    root: &Path,
    modules: &[(String, translator::ModuleConfig)],
    native_modules: &[(String, translator::NativeModuleConfig)],
) -> u64 {
    let mut hasher = DefaultHasher::new();

    // Root toml
    if let Ok(content) = fs::read_to_string(root.join("hvolt.toml")) {
        content.hash(&mut hasher);
    }

    // Module tomls — use sorted dir names for deterministic hash
    let mut dirs: Vec<&String> = modules.iter().map(|(d, _)| d).collect();
    dirs.sort();
    for dir in dirs {
        if let Ok(content) = fs::read_to_string(root.join(dir).join("hvolt.toml")) {
            content.hash(&mut hasher);
        }
    }

    // Native module tomls
    let mut native_dirs: Vec<&String> = native_modules.iter().map(|(d, _)| d).collect();
    native_dirs.sort();
    for dir in native_dirs {
        if let Ok(content) = fs::read_to_string(root.join(dir).join("hvolt.toml")) {
            content.hash(&mut hasher);
        }
    }

    hasher.finish()
}

/// Full vs incremental: a missing manifest, an unreadable one, or a different toml hash all
/// force a full translate. Pure so the incremental contract is testable without a filesystem.
pub(crate) fn needs_full_rebuild(
    manifest_exists: bool,
    manifest_hash: Option<u64>,
    current_hash: u64,
) -> bool {
    !manifest_exists || manifest_hash != Some(current_hash)
}

fn read_manifest_hash(path: &Path) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

fn write_manifest(path: &Path, hash: u64) {
    let mut f = fs::File::create(path).expect("write manifest");
    writeln!(f, "{}", hash).expect("write manifest");
}

/// Mirror source files into build/ using hard links (fallback: copy).
fn mirror_source_files(
    root: &Path,
    build_dir: &Path,
    _app_config: &AppConfig,
    modules: &[(String, translator::ModuleConfig)],
    native_modules: &[(String, translator::NativeModuleConfig)],
) {
    // AppScope resources — try resources/media/ (flat) first, then resources/base/media/ (standard)
    let project_media = root.join("resources/media");
    let effective_media = if project_media.exists() {
        &project_media
    } else {
        &root.join("resources/base/media")
    };
    let appscope_media = build_dir.join("AppScope/resources/base/media");
    if effective_media.exists() {
        translator_resources::mirror_dir(effective_media, &appscope_media);
    }

    // Collect native module directory names to skip during ETS file mirroring
    let native_skip_dirs: Vec<String> = native_modules
        .iter()
        .map(|(dir_name, _)| {
            // Use dependency directory basename (last component of the dep path)
            let dep_path = &_app_config
                .dependencies
                .get(dir_name)
                .map(|d| d.path.as_str())
                .unwrap_or(dir_name);
            std::path::Path::new(dep_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| dir_name.clone())
        })
        .collect();

    for (dir_name, module) in modules {
        let module_root = root.join(dir_name);
        let module_build = build_dir.join(dir_name);
        let module_ets = module_build.join("src/main/ets");
        let module_res = module_build.join("src/main/resources/base");
        let is_hsp = module.module.module_type == "hsp" || module.module.module_type == "shared";
        // HAR keeps its entry at src/main/ too: oh-package.json5's `main` is resolved from there.
        let entry_at_src_main = is_hsp || module.module.module_type == "har";

        // Mirror pages/ → ets/pages/
        translator_resources::mirror_dir(&module_root.join("pages"), &module_ets.join("pages"));

        // Mirror entry file
        let entry_file = translator::with_ets_suffix(&module.module.entry);
        let entry_src = module_root.join(&entry_file);
        if entry_at_src_main {
            // src/main/: module.json5 resolves srcEntry (and oh-package `main`) from there.
            let entry_dest = module_build.join("src/main").join(&entry_file);
            translator_resources::hardlink_or_copy(&entry_src, &entry_dest);
        } else {
            let entry_dest = module_ets.join("entryability").join(&entry_file);
            translator_resources::hardlink_or_copy(&entry_src, &entry_dest);
        }

        // Mirror abilities (entry only, skip if same as entry ability)
        for ability in &module.module.abilities {
            let entry_file_name = &ability.entry;
            let ability_src = module_root.join(entry_file_name);
            let is_ext = !ability.ability_type.is_empty();
            let subdir = if is_ext {
                if ability.dir.is_empty() {
                    "entryformability"
                } else {
                    ability.dir.as_str()
                }
            } else {
                "entryability"
            };
            let ability_dest = module_ets.join(subdir).join(entry_file_name);
            translator_resources::hardlink_or_copy(&ability_src, &ability_dest);
        }

        let placed: Vec<String> = std::iter::once(module.module.entry.clone())
            .chain(module.module.abilities.iter().map(|a| a.entry.clone()))
            .collect();
        translator_resources::mirror_ets_files_skip(
            &module_root,
            &module_ets,
            &placed,
            &native_skip_dirs,
        );

        // Mirror resources
        let module_res_src = module_root.join("resources");
        let project_res = root.join("resources");
        if is_hsp {
            let src = if module_res_src.exists() {
                &module_res_src
            } else {
                &project_res
            };
            translator_resources::mirror_resources(src, &module_res);
        } else if project_res.exists() {
            translator_resources::mirror_resources(&project_res, &module_res);
        } else if module_res_src.exists() {
            translator_resources::mirror_resources(&module_res_src, &module_res);
        }

        // Module-owned native code is translated separately from ets/ (never mirrored into it);
        // the incremental path has to carry it too, or cmake keeps building yesterday's tree.
        let inline_cpp = module_root.join("cpp/CMakeLists.txt").is_file();
        if inline_cpp {
            translator_resources::mirror_dir(
                &module_root.join("cpp"),
                &module_build.join("src/main/cpp"),
            );
        }
        let module_libs = module_root.join("libs");
        if module_libs.is_dir() {
            // hvigor's ProcessLibs reads the module-root libs/, not src/main/libs/ — mirroring
            // there left prebuilt .so out of the package while the build stayed green.
            translator_resources::mirror_dir(&module_libs, &module_build.join("libs"));
        }

        // Re-inject form/ability strings after resource mirroring
        // Always: the entry ability's generated label/desc strings are referenced by
        // module.json5 even when the project declares no extra abilities.
        if true {
            let element_dir = module_res.join("element");
            fs::create_dir_all(&element_dir).expect("create element dir");
            let string_json_path = element_dir.join("string.json");
            if let Err(e) = translator::inject_form_strings(&string_json_path, module) {
                eprintln!("✗ {}", e);
                std::process::exit(1); // propagate, don't continue on a half-translated tree
            }
        }
    }
    // Mirror native module C++ sources into src/main/cpp/<module>/ (one dir per module).
    // Kept in sync with the full-translate path in translator::mod.
    for (dir_name, _cfg) in native_modules {
        if let Some(dep) = _app_config.dependencies.get(dir_name) {
            let native_src = root.join(&dep.path);
            if native_src.exists() {
                for (dir_name_m, _module) in modules {
                    let cpp_dest = build_dir
                        .join(dir_name_m)
                        .join("src/main/cpp")
                        .join(dir_name);
                    fs::create_dir_all(&cpp_dest).expect("create cpp dir");
                    if let Ok(entries) = fs::read_dir(&native_src) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() {
                                let name = entry.file_name();
                                let name_str = name.to_string_lossy();
                                if translator_resources::is_native_source_file(&name_str) {
                                    translator_resources::hardlink_or_copy(
                                        &path,
                                        &cpp_dest.join(name),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Top-level CMakeLists per module, wiring the native subdirectories together.
    for (dir_name_m, _module) in modules {
        let cpp_root = build_dir.join(dir_name_m).join("src/main/cpp");
        let native_names: Vec<&str> = native_modules.iter().map(|(n, _)| n.as_str()).collect();
        if native_names.is_empty() {
            continue;
        }
        fs::create_dir_all(&cpp_root).expect("create cpp root");
        let body = translator_resources::native_cmake_lists(&native_names);
        let top = cpp_root.join("CMakeLists.txt");
        if !top.exists() {
            if let Err(e) = fs::write(&top, body) {
                eprintln!("Warning: write top-level CMakeLists.txt: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod rebuild_tests {
    use super::needs_full_rebuild;

    /// First run: no manifest, full translate.
    #[test]
    fn missing_manifest_forces_full() {
        assert!(needs_full_rebuild(false, None, 42));
    }

    /// Same toml hash: mirror only.
    #[test]
    fn matching_hash_mirrors_only() {
        assert!(!needs_full_rebuild(true, Some(42), 42));
    }

    /// Edited hvolt.toml changes the hash and must force a full translate.
    #[test]
    fn changed_hash_forces_full() {
        assert!(needs_full_rebuild(true, Some(41), 42));
    }

    /// A manifest that exists but cannot be parsed is not trusted.
    #[test]
    fn unreadable_manifest_forces_full() {
        assert!(needs_full_rebuild(true, None, 42));
    }
}

#[cfg(test)]
mod native_mirror_tests {
    use super::mirror_source_files;
    use crate::config::{AppConfig, Dependency, ModuleConfig, NativeModuleConfig};
    use std::collections::BTreeMap;
    use std::fs;

    fn native(name: &str) -> (String, NativeModuleConfig) {
        (
            name.to_string(),
            NativeModuleConfig {
                name: name.to_string(),
                module_type: "native".to_string(),
                types: "Index.d.ts".to_string(),
                version: "1.0.0".to_string(),
                version_code: 0,
                build_version: String::new(),
                description: String::new(),
            },
        )
    }

    /// Two native modules with the same source filenames must land in separate
    /// src/main/cpp/<module>/ directories. The old flat copy made napi_init.cpp and
    /// CMakeLists.txt overwrite each other, so only the last module compiled.
    #[test]
    fn same_named_sources_do_not_overwrite_across_modules() {
        let base = std::env::temp_dir().join(format!("hvolt-native-same-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("root");
        let build = base.join("build");

        // Two native sources, each with a same-named napi_init.cpp plus a header.
        for name in ["box64", "gom"] {
            let src = root.join("native").join(name);
            fs::create_dir_all(&src).unwrap();
            fs::write(src.join("napi_init.cpp"), format!("// {} impl", name)).unwrap();
            fs::write(src.join("bridge.h"), format!("// {} header", name)).unwrap();
            fs::write(src.join("README.md"), "not a source").unwrap();
        }

        let mut deps = BTreeMap::new();
        deps.insert(
            "box64".to_string(),
            Dependency {
                path: "native/box64".to_string(),
                version: "1.0.0".to_string(),
            },
        );
        deps.insert(
            "gom".to_string(),
            Dependency {
                path: "native/gom".to_string(),
                version: "1.0.0".to_string(),
            },
        );
        let app = AppConfig {
            app: AppConfig::default_for("x").app,
            dependencies: deps,
        };
        let modules = vec![("entry".to_string(), ModuleConfig::default_entry())];
        let native_modules = vec![native("box64"), native("gom")];

        mirror_source_files(&root, &build, &app, &modules, &native_modules);

        let cpp_root = build.join("entry/src/main/cpp");
        // Both modules survive: the file that used to be overwritten exists in both dirs
        // with each module's own content.
        assert_eq!(
            fs::read_to_string(cpp_root.join("box64/napi_init.cpp")).unwrap(),
            "// box64 impl"
        );
        assert_eq!(
            fs::read_to_string(cpp_root.join("gom/napi_init.cpp")).unwrap(),
            "// gom impl"
        );
        // Headers travel with their module; non-sources are not mirrored.
        assert!(cpp_root.join("box64/bridge.h").exists());
        assert!(cpp_root.join("gom/bridge.h").exists());
        assert!(!cpp_root.join("box64/README.md").exists());
        // Top-level CMakeLists wires both subdirectories.
        let top = fs::read_to_string(cpp_root.join("CMakeLists.txt")).unwrap();
        assert!(top.contains("add_subdirectory(box64)"), "{}", top);
        assert!(top.contains("add_subdirectory(gom)"), "{}", top);

        let _ = fs::remove_dir_all(&base);
    }

    /// An existing top-level CMakeLists (module-owned native tree) is left alone.
    #[test]
    fn existing_top_cmake_lists_is_not_overwritten() {
        let base = std::env::temp_dir().join(format!("hvolt-native-owned-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("root");
        let build = base.join("build");
        let src = root.join("native").join("box64");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("napi_init.cpp"), "// x").unwrap();

        let mut deps = BTreeMap::new();
        deps.insert(
            "box64".to_string(),
            Dependency {
                path: "native/box64".to_string(),
                version: "1.0.0".to_string(),
            },
        );
        let app = AppConfig {
            app: AppConfig::default_for("x").app,
            dependencies: deps,
        };
        let modules = vec![("entry".to_string(), ModuleConfig::default_entry())];

        // A module-owned cpp/ tree already declares its own aggregator.
        let cpp_root = build.join("entry/src/main/cpp");
        fs::create_dir_all(&cpp_root).unwrap();
        fs::write(cpp_root.join("CMakeLists.txt"), "project(owned)\n").unwrap();

        mirror_source_files(&root, &build, &app, &modules, &[native("box64")]);

        assert_eq!(
            fs::read_to_string(cpp_root.join("CMakeLists.txt")).unwrap(),
            "project(owned)\n"
        );

        let _ = fs::remove_dir_all(&base);
    }
}
