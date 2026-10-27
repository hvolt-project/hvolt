// Translator: user structure → HarmonyOS standard layout
//
// Public API:
//   discover_modules()   — scan for modules
//   full_translate()     — generate entire build/ directory from scratch
//   normalize_ets()      — strip .ets suffix
//   with_ets_suffix()    — ensure .ets suffix

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::config::AppConfig;
pub use crate::config::{ModuleConfig, NativeModuleConfig};
use crate::translator_resources;

mod strategy;
use strategy::{is_deprecated, V12Strategy, V26Strategy, VersionStrategy};

/// Parse sdk_version to extract API level for deprecation check.
/// "5.0.0(12)" → 12, "26.0.0" → 26, "26.1.0" → 26 (major)
fn api_from_version(s: &str) -> u32 {
    if let Some(api_str) = s
        .split('(')
        .nth(1)
        .and_then(|x| x.trim_end_matches(')').parse::<u32>().ok())
    {
        return api_str;
    }
    s.split('.')
        .next()
        .and_then(|x| x.parse::<u32>().ok())
        .unwrap_or(26)
}

/// Generator holds version-specific strategy for building artifacts.
pub struct Generator {
    strategy: Box<dyn VersionStrategy>,
    pub version_string: String,
}

impl Generator {
    pub fn new(sdk_version: &str) -> Self {
        let api = api_from_version(sdk_version);
        let strategy: Box<dyn VersionStrategy> = match api {
            12..=24 => Box::new(V12Strategy::new(api)),
            26.. => Box::new(V26Strategy::new(api)),
            _ => Box::new(V26Strategy::new(api)),
        };
        // For 26+, use the version string as-is (it's already the correct format)
        let vs = if api >= 26 {
            sdk_version.to_string()
        } else {
            strategy.version_string()
        };
        Self {
            strategy,
            version_string: vs,
        }
    }

    pub fn is_deprecated(&self) -> bool {
        let api = api_from_version(&self.version_string);
        is_deprecated(api)
    }
}

/// `type = "native"` detection, parsed rather than string-matched:
/// the old literal `contains("type = \"native\"")` missed `type="native"`,
/// single quotes, and any other spacing.
fn is_native_module(raw: &str) -> bool {
    raw.parse::<toml::Value>()
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(|t| t.as_str())
                .map(|s| s == "native")
        })
        .unwrap_or(false)
}

/// One module file's parsed form.
// The ETS variant holds the whole module config (now including native flags); boxing it would
// only move the allocation, and discovery handles a handful of modules at a time.
#[allow(clippy::large_enum_variant)]
enum ModuleKind {
    Ets(ModuleConfig),
    Native(NativeModuleConfig),
}

/// Parse a module `hvolt.toml` on ANY discovery path.
///
/// The design rule (`type = "native"` → native module, never in `modules[]`)
/// used to be implemented only in the `[dependencies]` path; a native MEMBER
/// was therefore parsed as an ETS module and silently skipped (and under the
/// strict-parse change it failed with a confusing "missing field `module`").
fn parse_module_file(path: &Path) -> Result<ModuleKind, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    if is_native_module(&raw) {
        toml::from_str::<NativeModuleConfig>(&raw)
            .map(ModuleKind::Native)
            .map_err(|e| format!("native module {} is invalid: {}", path.display(), e))
    } else {
        ModuleConfig::from_file(path)
            .map(ModuleKind::Ets)
            .map_err(|e| format!("module {} is invalid: {}", path.display(), e))
    }
}

/// Discover the project's modules.
///
/// Returns Err when any declared module/dependency cannot be read or parsed,
/// or when the loaded set is empty. The old version printed a warning and
/// `continue`d, so `assemble` reported "✓ 0 modules" with exit 0 on a
/// mistyped config.
/// Modules and native modules as declared by the project config.
type ModuleSets = (
    Vec<(String, ModuleConfig)>,
    Vec<(String, NativeModuleConfig)>,
);

pub fn discover_modules(root: &Path, app_config: &AppConfig) -> Result<ModuleSets, String> {
    let mut errors: Vec<String> = Vec::new();
    let mut modules = Vec::new();
    let mut declared: HashSet<String> = HashSet::new();
    let mut native_modules = Vec::new();

    // Dependencies from [dependencies]: HSP, HAR, or native modules
    for (dep_name, dep) in &app_config.dependencies {
        let dep_dir = root.join(&dep.path);
        // .har files are pre-built — skip module discovery
        if dep.path.ends_with(".har") {
            continue;
        }
        let toml = dep_dir.join("hvolt.toml");
        if !toml.exists() {
            errors.push(format!(
                "dependency '{}' declared but no hvolt.toml at {}",
                dep_name,
                dep_dir.display()
            ));
            continue;
        }
        // Read raw toml to check type first
        let raw = match std::fs::read_to_string(&toml) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("cannot read {}: {}", toml.display(), e));
                continue;
            }
        };
        if is_native_module(&raw) {
            match toml::from_str::<NativeModuleConfig>(&raw) {
                Ok(cfg) => {
                    native_modules.push((dep_name.clone(), cfg));
                }
                Err(e) => {
                    errors.push(format!(
                        "native module {} is invalid: {}",
                        toml.display(),
                        e
                    ));
                }
            }
        } else {
            // Legacy ETS/HSP module
            match toml::from_str::<ModuleConfig>(&raw) {
                Ok(cfg) => {
                    let dir_name = dep_name.clone();
                    declared.insert(dir_name.clone());
                    modules.push((dir_name, cfg));
                }
                Err(e) => errors.push(format!("module {} is invalid: {}", toml.display(), e)),
            }
        }
    }

    // Members: explicit module list, or scan for subdirs with hvolt.toml not in [dependencies]
    if !app_config.app.members.is_empty() {
        for member in &app_config.app.members {
            if declared.contains(member) {
                continue;
            }
            let member_dir = root.join(member);
            let toml = member_dir.join("hvolt.toml");
            if !toml.exists() {
                errors.push(format!(
                    "member '{}' declared but no hvolt.toml at {}",
                    member,
                    member_dir.display()
                ));
                continue;
            }
            match parse_module_file(&toml) {
                Ok(ModuleKind::Ets(cfg)) => modules.push((member.clone(), cfg)),
                Ok(ModuleKind::Native(cfg)) => native_modules.push((member.clone(), cfg)),
                Err(e) => errors.push(e),
            }
        }
    } else {
        // Fallback: scan for subdirs with hvolt.toml
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
                if declared.contains(&dir_name) {
                    continue;
                }
                let toml = path.join("hvolt.toml");
                if toml.exists() {
                    match parse_module_file(&toml) {
                        Ok(ModuleKind::Ets(cfg)) => modules.push((dir_name, cfg)),
                        Ok(ModuleKind::Native(cfg)) => native_modules.push((dir_name, cfg)),
                        Err(e) => errors.push(e),
                    }
                }
            }
        }
    }

    // Combination rules (runtimeOS default, bundle_type, device_types enums) live in
    // config.rs as pure functions; the translator only applies them.
    errors.extend(crate::config::validate_combination(
        &app_config.app,
        &modules,
    ));

    if !errors.is_empty() {
        return Err(errors.join("\n  "));
    }
    if modules.is_empty() && native_modules.is_empty() {
        return Err(format!(
            "no module could be loaded (members: {:?}, dependencies: {:?}) — \
             every module needs a readable hvolt.toml",
            app_config.app.members,
            app_config.dependencies.keys().collect::<Vec<_>>()
        ));
    }
    Ok((modules, native_modules))
}

/// Pure: generate all files for the build/ directory
pub fn full_translate(
    project_root: &Path,
    build_dir: &Path,
    app_config: &AppConfig,
    modules: &[(String, ModuleConfig)],
    native_modules: &[(String, NativeModuleConfig)],
) {
    let gen = Generator::new(&app_config.app.sdk_version);

    if gen.is_deprecated() {
        eprintln!(
            "Warning: SDK version {} is deprecated. Consider upgrading to 26.0.0+.",
            gen.version_string
        );
    }

    // AppScope/app.json5
    let app_json5 = gen.strategy.generate_app_json5(app_config);
    write_file(&build_dir.join("AppScope/app.json5"), &app_json5);

    // AppScope resources. Two inputs:
    //   app-resources/  — application-scope resources (element/*.json, media, …)
    //   resources/base/media — the app icon that older flat projects keep here
    let appscope_res = build_dir.join("AppScope/resources/base");
    let app_res = project_root.join("app-resources");
    if app_res.exists() {
        translator_resources::translate_resources(&app_res, &appscope_res);
    }
    let project_media = project_root.join("resources/media");
    let effective_media = if project_media.exists() {
        &project_media
    } else {
        &project_root.join("resources/base/media")
    };
    if effective_media.exists() {
        fs::create_dir_all(appscope_res.join("media")).expect("create appscope media");
        translator_resources::copy_dir_contents(effective_media, &appscope_res.join("media"));
    }
    // AppScope element/string.json: keep whatever the project declares and only supply
    // `app_name` when it is missing — application strings are user data, not boilerplate.
    let appscope_element = appscope_res.join("element");
    fs::create_dir_all(&appscope_element).expect("create appscope element");
    let appscope_string_json = appscope_element.join("string.json");
    merge_app_scope_strings(&appscope_string_json, &app_config.app.name);

    // Hvigor boilerplate
    write_file(
        &build_dir.join("hvigor/hvigor-config.json5"),
        gen.strategy.hvigor_config(),
    );
    write_file(&build_dir.join("hvigorfile.ts"), HVIGORFILE_ROOT);

    // Collect native module names for root build-profile and oh-package filtering
    let native_names: Vec<String> = native_modules.iter().map(|(n, _)| n.clone()).collect();

    write_file(
        &build_dir.join("oh-package.json5"),
        &gen.strategy.oh_package_root(app_config, &native_names),
    );
    let root_bp = gen
        .strategy
        .generate_root_build_profile(modules, app_config, &native_names);
    write_file(&build_dir.join("build-profile.json5"), &root_bp);
    // Copy HSP/HAR dependencies into build/ so module oh-package.json5 references resolve
    let native_dep_names: Vec<String> = native_modules.iter().map(|(n, _)| n.clone()).collect();
    for (dep_name, dep) in &app_config.dependencies {
        let src = project_root.join(&dep.path);
        if !src.exists() {
            continue;
        }
        // Skip native deps — handled separately below
        if native_dep_names.contains(dep_name) {
            continue;
        }
        let dst = build_dir.join(&dep.path);
        if dep.path.ends_with(".har") {
            // .har files are pre-built — hard-link
            if let Some(parent) = dst.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::remove_file(&dst);
            if let Err(e) = fs::hard_link(&src, &dst) {
                let _ = fs::copy(&src, &dst);
                eprintln!(
                    "  Warning: hard_link {} → {}: {} (fallback to copy)",
                    src.display(),
                    dst.display(),
                    e
                );
            }
        } else {
            // HSP directory — copy recursively
            translator_resources::copy_dir_recursive(&src, &dst);
        }
    }

    // Handle native deps: copy C++ sources to src/main/cpp/<native>/ + generate types bridge
    //
    // Each native module gets its OWN subdirectory. Flat-copying every module into one
    // src/main/cpp/ made same-named files (napi_init.cpp, CMakeLists.txt) overwrite each
    // other, so only the last module survived and the rest silently never compiled.
    for (dir_name, native_cfg) in native_modules {
        let dep_path = dep_path_by_name(app_config, dir_name);
        let native_src_dir = project_root.join(&dep_path);

        for (dir_name_m, _module) in modules {
            let cpp_root = build_dir.join(dir_name_m).join("src/main/cpp");
            let cpp_dir = cpp_root.join(dir_name);
            if !cpp_dir.exists() {
                fs::create_dir_all(&cpp_dir).expect("create cpp dir");
            }
            // Prune sources that vanished from the module (same contract as the mirror path).
            crate::translator_resources::purge_stale_native_files(&native_src_dir, &cpp_dir);
            // Copy sources + headers. Headers were previously skipped entirely, so any
            // module split across .h/.cpp failed to compile.
            if let Ok(entries) = fs::read_dir(&native_src_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path.file_name().unwrap().to_string_lossy().to_string();
                        if crate::translator_resources::is_native_source_file(&name) {
                            let dst = cpp_dir.join(&name);
                            // Rewrite unconditionally: a stale copy must not survive
                            // (the old `if !dst.exists()` skipped updates entirely).
                            crate::translator_resources::hardlink_or_copy(&path, &dst);
                        }
                    }
                }
            }
        }
        // Generate types/ bridge at src/main/cpp/types/<dir_name>/
        for (dir_name_m, _module) in modules {
            let types_dir = build_dir
                .join(dir_name_m)
                .join("src/main/cpp/types")
                .join(dir_name);
            crate::translator_resources::write_native_types_bridge(
                &types_dir,
                &format!("{}.so", native_cfg.name),
                &native_cfg.version,
                &native_cfg.description,
                &native_src_dir.join(&native_cfg.types),
            );
        }
    }

    // Top-level CMakeLists per module: wires the subdirectories together, so each native
    // module keeps its own add_library and never collides.
    for (dir_name_m, _module) in modules {
        let cpp_root = build_dir.join(dir_name_m).join("src/main/cpp");
        let native_names: Vec<&str> = native_modules.iter().map(|(n, _)| n.as_str()).collect();
        if native_names.is_empty() {
            continue;
        }
        // A module-owned cpp/ tree (inline_cpp) writes its own CMakeLists.txt to this exact
        // path — the aggregator must never clobber it.
        if project_root
            .join(dir_name_m)
            .join("cpp/CMakeLists.txt")
            .is_file()
        {
            continue;
        }
        fs::create_dir_all(&cpp_root).expect("create cpp root");
        let body = crate::translator_resources::native_cmake_lists(&native_names);
        let top = cpp_root.join("CMakeLists.txt");
        // Content-aware: rewrite when the existing file is stale (e.g. a native module was
        // added since the last translate); never clobber a module-owned cpp/ tree (inline_cpp).
        match fs::read_to_string(&top) {
            Ok(existing) if existing == body => {}
            _ => {
                if let Err(e) = fs::write(&top, body) {
                    eprintln!("Warning: write top-level CMakeLists.txt: {}", e);
                }
            }
        }
    }

    // Collect native module directory names to skip during ETS file copy
    let native_skip_dirs: Vec<String> = native_modules
        .iter()
        .map(|(name, _)| {
            let path = dep_path_by_name(app_config, name);
            std::path::Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone())
        })
        .collect();

    for (dir_name, module) in modules {
        let module_build = build_dir.join(dir_name);
        let module_src = module_build.join("src/main");
        let module_ets = module_src.join("ets");
        let module_res = module_src.join("resources/base");
        let module_root = project_root.join(dir_name);
        let is_hsp = module.module.module_type == "hsp" || module.module.module_type == "shared";
        // HAR keeps its entry at src/main/ too: oh-package.json5's `main` is resolved from there.
        let entry_at_src_main = is_hsp || module.module.module_type == "har";

        // Copy pages/*.ets → ets/pages/
        translator_resources::copy_dir_contents(
            &module_root.join("pages"),
            &module_ets.join("pages"),
        );

        // Entry file: entry → ets/entryability/ (entry), or → module root (HSP/HAR)
        let entry_file = &module.module.entry;
        let entry_src = module_root.join(with_ets_suffix(entry_file));
        if entry_at_src_main {
            // Shared packages: the entry file sits beside module.json5 (src/main/), because both
            // module.json5's srcEntry and oh-package.json5's `main` resolve from there.
            if entry_src.exists() {
                if let Err(e) = fs::create_dir_all(&module_src) {
                    eprintln!("Warning: create {}: {}", module_src.display(), e);
                }
                let dest = module_src.join(with_ets_suffix(entry_file));
                fs::copy(&entry_src, &dest).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: copy entry {} → {}: {}",
                        entry_src.display(),
                        dest.display(),
                        e
                    );
                    0
                });
            }
        } else {
            // Entry module: EntryAbility.ets → ets/entryability/EntryAbility.ets
            let entry_dest = module_ets
                .join("entryability")
                .join(with_ets_suffix(entry_file));
            if entry_src.exists() {
                fs::create_dir_all(entry_dest.parent().unwrap()).expect("create entryability dir");
                fs::copy(&entry_src, &entry_dest).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: copy entry {} → {}: {}",
                        entry_src.display(),
                        entry_dest.display(),
                        e
                    );
                    0
                });
            }
        }

        // Additional abilities (UIAbility → entryability/, ExtensionAbility → entryformability/)
        for ability in &module.module.abilities {
            let is_ext = !ability.ability_type.is_empty();
            let entry_file_name = &ability.entry;
            let src = module_root.join(entry_file_name);
            let subdir = if is_ext {
                // The role directory is part of the ability's identity in module.json5:
                // a backup extension belongs under entrybackupability, not entryformability.
                if ability.dir.is_empty() {
                    "entryformability"
                } else {
                    ability.dir.as_str()
                }
            } else {
                "entryability"
            };
            let dest = module_ets.join(subdir).join(entry_file_name);
            if src.exists() {
                fs::create_dir_all(dest.parent().unwrap()).expect("create ability dir");
                fs::copy(&src, &dest).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: copy ability {} → {}: {}",
                        src.display(),
                        dest.display(),
                        e
                    );
                    0
                });
            }
        }

        // Generate extension config profile files from config_profile
        for ability in &module.module.abilities {
            if let Some(ref profile) = ability.config_profile {
                let is_ext = !ability.ability_type.is_empty();
                if is_ext {
                    let config_filename = format!("{}_config.json", ability.ability_type);
                    let profile_dir = module_res.join("profile");
                    fs::create_dir_all(&profile_dir).expect("create profile dir");
                    let config_path = profile_dir.join(&config_filename);
                    if !config_path.exists() {
                        let content = serde_json::to_string_pretty(profile)
                            .unwrap_or_else(|_| "{}".to_string());
                        write_file(&config_path, &content);
                    }
                }
            }
        }

        // Copy any remaining .ets files from module root → ets/.
        // Abilities are placed by path (entryability/ or entryformability/), so their source
        // files must not also land at the ets root.
        let placed: Vec<String> = std::iter::once(module.module.entry.clone())
            .chain(module.module.abilities.iter().map(|a| a.entry.clone()))
            .collect();
        translator_resources::copy_ets_files_skip(
            &module_root,
            &module_ets,
            &placed,
            &native_skip_dirs,
        );

        // Module-owned native code. DevEco projects put cmake under `src/main/cpp` inside the
        // module; the flat layout mirrors that as `<member>/cpp` (+ `libs/` for prebuilt
        // binaries). copy_ets_files_skip already refuses to fold these into ets/ — here they
        // are placed where hvigor reads them, and `inline_cpp` makes the module's build-profile
        // declare externalNativeOptions.
        let inline_cpp = module_root.join("cpp/CMakeLists.txt").is_file();
        if inline_cpp {
            // recursive: cmake trees keep headers and sources in subdirectories
            translator_resources::copy_dir_recursive(
                &module_root.join("cpp"),
                &module_src.join("cpp"),
            );
        }
        let module_libs = module_root.join("libs");
        if module_libs.is_dir() {
            // Module-root libs/: hvigor packs from here (src/main/libs/ is ignored).
            crate::translator_resources::mirror_libs_with_warning(
                &module_libs,
                &module_build.join("libs"),
                dir_name,
            );
        }

        // Copy resources/. Both a project-level `resources/` (app icon, shared elements) and a
        // module-level `<member>/resources/` (locales, dark, rawfile) exist in real projects,
        // so they are merged instead of one replacing the other — the module wins on conflicts.
        let project_res = project_root.join("resources");
        let module_res_src = module_root.join("resources");
        for source in resource_source_order(is_hsp, project_res.exists(), module_res_src.exists()) {
            let path = match source {
                ResourceSource::Project => &project_res,
                ResourceSource::Module => &module_res_src,
            };
            translator_resources::translate_resources(path, &module_res);
        }
        if is_hsp {
            // Ensure HSP has at least a minimal string.json for ResourceTable
            let element_dir = module_res.join("element");
            fs::create_dir_all(&element_dir).expect("create element dir");
            let string_json = element_dir.join("string.json");
            if !string_json.exists() {
                write_file(
                    &string_json,
                    r#"{"string":[{"name":"module_desc","value":"shared module"}]}"#,
                );
            }
        }

        // Re-inject ability/form strings after resource mirroring
        // Always: the entry ability's generated label/desc strings are referenced by
        // module.json5 even when the project declares no extra abilities.
        if true {
            let element_dir = module_res.join("element");
            fs::create_dir_all(&element_dir).expect("create element dir");
            let string_json_path = element_dir.join("string.json");
            if let Err(e) = inject_form_strings(&string_json_path, module) {
                eprintln!("Warning: form strings for {}: {}", module.module.name, e);
            }
        }

        // Copy native/
        let native_src = module_root.join("native");
        if native_src.exists() {
            translator_resources::copy_dir_recursive(&native_src, &module_src.join("libs"));
        }

        // module.json5
        let module_json5 = gen.strategy.generate_module_json5(module, app_config);
        write_file(&module_src.join("module.json5"), &module_json5);

        // main_pages.json (only if module has routes)
        if !module.module.routes.is_empty() {
            let main_pages = generate_main_pages(module);
            write_file(&module_res.join("profile/main_pages.json"), &main_pages);
        }

        // Module build-profile.json5
        let module_bp =
            gen.strategy
                .generate_module_build_profile(&module.module, native_modules, inline_cpp);
        write_file(&module_build.join("build-profile.json5"), &module_bp);

        // obfuscation-rules.txt (if enabled)
        // A project-level rules file wins over the template: obfuscation settings are a
        // deliberate release decision (size, log stripping), so they must survive translation.
        if module.module.obfuscation {
            let obfuscation_path = module_build.join("obfuscation-rules.txt");
            let configured = if module.module.obfuscation_rules.is_empty() {
                module_root.join("obfuscation-rules.txt")
            } else {
                module_root.join(&module.module.obfuscation_rules)
            };
            if configured.is_file() {
                if let Err(e) = fs::copy(&configured, &obfuscation_path) {
                    eprintln!(
                        "Warning: copy {} → {}: {}",
                        configured.display(),
                        obfuscation_path.display(),
                        e
                    );
                }
            } else if !obfuscation_path.exists() {
                write_file(&obfuscation_path, OBFUSCATION_RULES_TEMPLATE);
                eprintln!(
                    "Note: {} enables obfuscation but ships no rules file — using the default template",
                    module.module.name
                );
            }
        }

        // Module hvigorfile.ts
        let hvigorfile = match module.module.module_type.as_str() {
            "hsp" | "shared" => HVIGORFILE_HSP,
            // A HAR is a static package: hspTasks cannot build it (00303278).
            "har" => HVIGORFILE_HAR,
            _ => HVIGORFILE_ENTRY,
        };
        write_file(&module_build.join("hvigorfile.ts"), hvigorfile);

        // Module oh-package.json5
        let oh_pkg = gen
            .strategy
            .generate_module_oh_package(module, app_config, native_modules);
        write_file(&module_build.join("oh-package.json5"), &oh_pkg);
    }
}

/// Which resource directories feed a module, in application order (later wins).
///
/// A real project keeps shared/app-level resources under the project `resources/` **and**
/// module-owned ones (locales, dark, rawfile) under `<member>/resources/`. Treating the two as
/// alternatives silently dropped every module resource, so both are applied — the module last.
/// Shared (HSP/HAR) modules own their resources outright; the project copy is only a fallback.
#[derive(Debug, PartialEq)]
enum ResourceSource {
    Project,
    Module,
}

fn resource_source_order(
    is_hsp: bool,
    project_exists: bool,
    module_exists: bool,
) -> Vec<ResourceSource> {
    if is_hsp {
        if module_exists {
            vec![ResourceSource::Module]
        } else if project_exists {
            vec![ResourceSource::Project]
        } else {
            Vec::new()
        }
    } else {
        let mut order = Vec::new();
        if project_exists {
            order.push(ResourceSource::Project);
        }
        if module_exists {
            order.push(ResourceSource::Module);
        }
        order
    }
}

/// Add an `app_name` entry only when the project did not declare one.
fn with_app_name(mut entries: Vec<serde_json::Value>, app_name: &str) -> Vec<serde_json::Value> {
    let has_app_name = entries
        .iter()
        .any(|e| e.get("name").and_then(|n| n.as_str()) == Some("app_name"));
    if !has_app_name {
        entries.push(serde_json::json!({ "name": "app_name", "value": app_name }));
    }
    entries
}

/// Ensure `AppScope/resources/base/element/string.json` exists and declares `app_name`.
///
/// Application-scope strings are user data (version label, privacy links, …), so a file that
/// the project already supplies is preserved as-is; only a missing `app_name` is added.
fn merge_app_scope_strings(path: &Path, app_name: &str) {
    let entries: Vec<serde_json::Value> = if path.is_file() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("string").and_then(|s| s.as_array()).cloned())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // An application string table that already names the app is user data: rewriting it would
    // reformat the file (trailing newline, key order) for no reason.
    let had_app_name = path.is_file()
        && fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                v.get("string").and_then(|s| s.as_array()).map(|a| {
                    a.iter()
                        .any(|e| e.get("name").and_then(|n| n.as_str()) == Some("app_name"))
                })
            })
            .unwrap_or(false);
    if had_app_name {
        return;
    }
    let entries = with_app_name(entries, app_name);
    let doc = serde_json::json!({ "string": entries });
    write_file(
        path,
        &serde_json::to_string_pretty(&doc).unwrap_or_default(),
    );
}

fn generate_main_pages(config: &ModuleConfig) -> String {
    let pages: Vec<String> = config
        .module
        .routes
        .iter()
        .map(|p| {
            let path = p.trim_end_matches(".ets");
            format!(r#"    "{}""#, path)
        })
        .collect();
    format!(
        r#"{{
  "src": [
{}
  ]
}}"#,
        pages.join(",\n")
    )
}

/// Ensure string.json has {name}_desc and {name}_label for all abilities.
/// Add `start_window_background` to the module's color table when it is missing.
fn ensure_start_window_color(element_dir: &Path) {
    let path = element_dir.join("color.json");
    let mut entries: Vec<serde_json::Value> = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("color").and_then(|c| c.as_array()).cloned())
        .unwrap_or_default();
    let has = entries
        .iter()
        .any(|e| e.get("name").and_then(|n| n.as_str()) == Some("start_window_background"));
    if has {
        return;
    }
    entries.push(serde_json::json!({ "name": "start_window_background", "value": "#FFFFFF" }));
    write_file(
        &path,
        &serde_json::to_string_pretty(&serde_json::json!({ "color": entries })).unwrap_or_default(),
    );
}

pub fn inject_form_strings(string_json_path: &Path, module: &ModuleConfig) -> Result<(), String> {
    let all_abilities = &module.module.abilities;

    use std::io::Read;
    let mut content = String::new();
    if string_json_path.exists() {
        // read failure must NOT fall through to "create new" — that path
        // silently REBUILDS the file and drops every existing string
        // (data-loss surface)
        let mut f = std::fs::File::open(string_json_path)
            .map_err(|e| format!("read {}: {}", string_json_path.display(), e))?;
        f.read_to_string(&mut content)
            .map_err(|e| format!("read {}: {}", string_json_path.display(), e))?;
    }

    let element_dir = string_json_path.parent().unwrap();

    let mut new_entries: Vec<String> = Vec::new();

    // Entry ability label
    let entry_base = normalize_ets(&module.module.entry);
    let entry_has_label = !module.module.entry_label.is_empty();
    let entry_has_desc = !module.module.entry_description.is_empty();
    for base_name in std::iter::once(entry_base.clone()).chain(
        all_abilities
            .iter()
            .filter(|a| a.ability_type.is_empty())
            .map(|a| normalize_ets(&a.name)),
    ) {
        let desc_key = format!("{}_desc", base_name);
        let label_key = format!("{}_label", base_name);
        let supplied = base_name == entry_base && (entry_has_label || entry_has_desc);
        if !supplied && !content.contains(&format!("\"name\": \"{}\"", desc_key)) {
            new_entries.push(format!(
                r#"    {{
      "name": "{}",
      "value": "{}"
    }}"#,
                desc_key, base_name
            ));
        }
        if !supplied && !content.contains(&format!("\"name\": \"{}\"", label_key)) {
            new_entries.push(format!(
                r#"    {{
      "name": "{}",
      "value": "{}"
    }}"#,
                label_key, base_name
            ));
        }
    }

    // Extension ability strings: skip if type's config profile resource json already defines them
    for a in all_abilities.iter().filter(|a| !a.ability_type.is_empty()) {
        let config_resource = format!("{}.json", a.ability_type);
        let already_defined = element_dir.join(&config_resource).exists();
        if already_defined {
            continue;
        }
        if a.label.is_empty() && a.description.is_empty() {
            continue; // nothing to generate: module.json5 omits both lines
        }
        let desc_key = format!("{}_desc", a.name);
        let label_key = format!("{}_label", a.name);
        if !content.contains(&format!("\"name\": \"{}\"", desc_key)) {
            new_entries.push(format!(
                r#"    {{
      "name": "{}",
      "value": "{}"
    }}"#,
                desc_key, a.name
            ));
        }
        if !content.contains(&format!("\"name\": \"{}\"", label_key)) {
            new_entries.push(format!(
                r#"    {{
      "name": "{}",
      "value": "{}"
    }}"#,
                label_key, a.name
            ));
        }
    }

    // The generated entry ability also references $color:start_window_background; supply it when
    // the project has no color table of its own, otherwise hvigor rejects the manifest.
    ensure_start_window_color(element_dir);

    if new_entries.is_empty() {
        return Ok(());
    }

    if content.is_empty() {
        // Create new string.json
        content = format!(
            r#"{{
  "string": [
{}
  ]
}}"#,
            new_entries.join(",\n")
        );
    } else {
        // Insert before the closing ] of "string" array
        let insert_str = new_entries.join(",\n");
        if let Some(pos) = content.rfind(']') {
            // Check if there are existing entries (non-empty array)
            let before_bracket = &content[..pos].trim_end();
            if before_bracket.ends_with('[') {
                // Empty array: replace ] with new entries + ]
                content.replace_range(pos..pos + 1, &format!("\n{}\n  ]", insert_str));
            } else {
                // Non-empty array: prepend comma
                content.replace_range(pos..pos + 1, &format!(",\n{}\n  ]", insert_str));
            }
        }
    }

    write_file(string_json_path, &content);
    Ok(())
}

/// Look up a dependency's path from app_config by dependency name.
fn dep_path_by_name(app_config: &AppConfig, name: &str) -> String {
    app_config
        .dependencies
        .get(name)
        .map(|d| d.path.clone())
        .unwrap_or_else(|| name.to_string())
}

// ---- hvigor boilerplate constants ----

const HVIGORFILE_ROOT: &str = r#"import { appTasks } from '@ohos/hvigor-ohos-plugin';

export default {
  system: appTasks,
  plugins: []
}"#;

const HVIGORFILE_ENTRY: &str = r#"import { hapTasks } from '@ohos/hvigor-ohos-plugin';

export default {
  system: hapTasks,
  plugins: []
}"#;

const HVIGORFILE_HAR: &str = r#"import { harTasks } from '@ohos/hvigor-ohos-plugin';

export default {
  system: harTasks,
  plugins: []
}"#;

const HVIGORFILE_HSP: &str = r#"import { hspTasks } from '@ohos/hvigor-ohos-plugin';

export default {
  system: hspTasks,
  plugins: []
}"#;

const OBFUSCATION_RULES_TEMPLATE: &str = r#"# Obfuscation Rules
# Add your obfuscation rules here.
# See: https://developer.huawei.com/consumer/cn/doc/harmonyos-guides-V5/source-obfuscation-V5

-keep-property-name
-keep-global-name
"#;

// ---- ets path helpers ----

pub fn normalize_ets(s: &str) -> String {
    s.trim_end_matches(".ets")
        .trim_end_matches(".ts")
        .to_string()
}

pub fn with_ets_suffix(s: &str) -> String {
    if s.ends_with(".ets") || s.ends_with(".ts") {
        s.to_string()
    } else {
        format!("{}.ets", s)
    }
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    let mut file =
        fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {}", path.display(), e));
    file.write_all(content.as_bytes()).expect("write file");
}

#[cfg(test)]
mod native_detect_tests {
    use super::is_native_module;

    /// The structured check must accept every legal spelling of the marker
    /// (the old literal match only knew the double-quoted, space-padded one).
    #[test]
    fn native_marker_is_recognized_in_all_legal_spellings() {
        assert!(is_native_module("name = \"x\"\ntype = \"native\"\n"));
        assert!(is_native_module("type=\"native\""));
        assert!(is_native_module("type = 'native'"));
        assert!(is_native_module(
            "name=\"x\"\n type = \"native\"\n version=\"1\""
        ));
    }

    #[test]
    fn non_native_and_garbage_are_not_native() {
        assert!(!is_native_module("[module]\nname = \"src\"\n"));
        assert!(!is_native_module("type = \"ets\""));
        assert!(!is_native_module("type = 12"));
        assert!(!is_native_module("this is not toml at all ]["));
    }
}

#[cfg(test)]
mod app_scope_tests {
    use super::with_app_name;

    /// Application strings belong to the project: they must survive translation untouched.
    #[test]
    fn project_app_strings_are_kept_verbatim() {
        let entries = vec![
            serde_json::json!({ "name": "app_name", "value": "DemoApp" }),
            serde_json::json!({ "name": "app_version", "value": "0.2.5 (2005)" }),
        ];
        let merged = with_app_name(entries, "aigis");
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0]["value"], "DemoApp");
        assert_eq!(merged[1]["name"], "app_version");
    }

    /// A project that declares nothing still gets the label app.json5 points at.
    #[test]
    fn missing_app_name_is_supplied() {
        let merged = with_app_name(Vec::new(), "aigis");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0]["name"], "app_name");
        assert_eq!(merged[0]["value"], "aigis");
    }
}

#[cfg(test)]
mod resource_order_tests {
    use super::{resource_source_order, ResourceSource};

    /// Entry modules merge both sources, module last so its locales win on conflicts.
    #[test]
    fn entry_module_merges_project_and_module() {
        assert_eq!(
            resource_source_order(false, true, true),
            vec![ResourceSource::Project, ResourceSource::Module]
        );
        assert_eq!(
            resource_source_order(false, false, true),
            vec![ResourceSource::Module]
        );
        assert_eq!(
            resource_source_order(false, true, false),
            vec![ResourceSource::Project]
        );
    }

    /// A shared module owns its resources; the project directory never overrides it.
    #[test]
    fn shared_module_prefers_its_own_resources() {
        assert_eq!(
            resource_source_order(true, true, true),
            vec![ResourceSource::Module]
        );
        assert_eq!(
            resource_source_order(true, true, false),
            vec![ResourceSource::Project]
        );
        assert!(resource_source_order(true, false, false).is_empty());
    }
}

#[cfg(test)]
mod app_scope_verbatim_tests {
    use super::merge_app_scope_strings;
    use std::fs;

    /// A string table that already names the app must come back byte-identical — reformatting
    /// it drops the trailing newline and reorders nothing the user wrote.
    #[test]
    fn existing_app_strings_are_left_untouched() {
        let dir = std::env::temp_dir().join(format!("hvolt-appscope-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("string.json");
        let original = "{\n  \"string\": [\n    {\n      \"name\": \"app_name\",\n      \"value\": \"DemoApp\"\n    }\n  ]\n}\n";
        fs::write(&path, original).unwrap();

        merge_app_scope_strings(&path, "demo-app");

        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let _ = fs::remove_dir_all(&dir);
    }
}
