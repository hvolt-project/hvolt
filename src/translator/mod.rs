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

    // device_types must be legal for the chosen runtimeOS:
    // an OpenHarmony build must not declare "phone".
    let runtime_os = if app_config.app.runtime_os.is_empty() {
        "HarmonyOS"
    } else {
        app_config.app.runtime_os.as_str()
    };
    for (name, cfg) in &modules {
        let illegal = crate::config::illegal_device_types(&cfg.module.device_types, runtime_os);
        if !illegal.is_empty() {
            errors.push(format!(
                "module '{}' declares device_types {:?} that runtime_os={} does not allow \
                 (official enum: [{}]) — edit {}/hvolt.toml",
                name,
                illegal,
                runtime_os,
                crate::config::device_types_for_runtime(runtime_os).join(", "),
                name
            ));
        }
    }

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

    // AppScope resources: copy media files (layered_image, etc.)
    // Try resources/media/ (flat) first, then resources/base/media/ (standard)
    let appscope_res = build_dir.join("AppScope/resources/base");
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
    // Generate AppScope element/string.json with app_name
    let appscope_element = appscope_res.join("element");
    fs::create_dir_all(&appscope_element).expect("create appscope element");
    let appscope_string_json = appscope_element.join("string.json");
    let appscope_strings = format!(
        r#"{{
  "string": [
    {{
      "name": "app_name",
      "value": "{}"
    }}
  ]
}}"#,
        app_config.app.name
    );
    write_file(&appscope_string_json, &appscope_strings);

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

    // Handle native deps: copy C++ sources to src/main/cpp/ + generate types bridge
    for (dir_name, native_cfg) in native_modules {
        let dep_path = dep_path_by_name(app_config, dir_name);
        let native_src_dir = project_root.join(&dep_path);

        // Copy C++ source files into each module's src/main/cpp/
        for (dir_name_m, _module) in modules {
            let cpp_dir = build_dir.join(dir_name_m).join("src/main/cpp");
            if !cpp_dir.exists() {
                fs::create_dir_all(&cpp_dir).expect("create cpp dir");
            }
            // Copy CMakeLists.txt
            let cmake_src = native_src_dir.join("CMakeLists.txt");
            let cmake_dst = cpp_dir.join("CMakeLists.txt");
            if cmake_src.exists() && !cmake_dst.exists() {
                if let Err(e) = fs::copy(&cmake_src, &cmake_dst) {
                    eprintln!(
                        "Warning: copy CMakeLists.txt {} → {}: {}",
                        cmake_src.display(),
                        cmake_dst.display(),
                        e
                    );
                }
            }
            // Copy all .cpp, .c, .cc files
            if let Ok(entries) = fs::read_dir(&native_src_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path.file_name().unwrap().to_string_lossy().to_string();
                        if name.ends_with(".cpp") || name.ends_with(".c") || name.ends_with(".cc") {
                            let dst = cpp_dir.join(&name);
                            if !dst.exists() {
                                let _ = fs::copy(&path, &dst);
                            }
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
            fs::create_dir_all(&types_dir).expect("create native types dir");

            let pkg_name = format!("{}.so", native_cfg.name);
            let oh_pkg = format!(
                r#"{{
  "name": "{}",
  "types": "./Index.d.ts",
  "version": "{}",
  "description": "{}"
}}"#,
                pkg_name, native_cfg.version, native_cfg.description
            );
            write_file(&types_dir.join("oh-package.json5"), &oh_pkg);

            // Copy Index.d.ts from native module source root
            let dts_src = native_src_dir.join(&native_cfg.types);
            let dts_dst = types_dir.join("Index.d.ts");
            if dts_src.exists() {
                if let Err(e) = fs::copy(&dts_src, &dts_dst) {
                    eprintln!(
                        "Warning: copy {} → {}: {}",
                        dts_src.display(),
                        dts_dst.display(),
                        e
                    );
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

        // Copy pages/*.ets → ets/pages/
        translator_resources::copy_dir_contents(
            &module_root.join("pages"),
            &module_ets.join("pages"),
        );

        // Entry file: entry → ets/entryability/ (entry), or → module root (HSP/HAR)
        let entry_file = &module.module.entry;
        let entry_src = module_root.join(with_ets_suffix(entry_file));
        if is_hsp {
            // HSP/HAR: index.ets → build/<name>/index.ets
            if entry_src.exists() {
                let dest = module_build.join(with_ets_suffix(entry_file));
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
                "entryformability"
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

        // Copy any remaining .ets files from module root → ets/
        translator_resources::copy_ets_files_skip(
            &module_root,
            &module_ets,
            &module.module.entry,
            &native_skip_dirs,
        );

        // Copy resources/: project-level resources go to entry module,
        // module-level resources (if any) go to that specific module.
        let project_res = project_root.join("resources");
        let module_res_src = module_root.join("resources");
        if is_hsp {
            // HSP uses module-level resources if present, else project-level
            if module_res_src.exists() {
                translator_resources::translate_resources(&module_res_src, &module_res);
            } else if project_res.exists() {
                translator_resources::translate_resources(&project_res, &module_res);
            }
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
        } else if project_res.exists() {
            // Entry module: use project-level resources
            translator_resources::translate_resources(&project_res, &module_res);
        } else if module_res_src.exists() {
            translator_resources::translate_resources(&module_res_src, &module_res);
        }

        // Re-inject ability/form strings after resource mirroring
        if !module.module.abilities.is_empty() {
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
        let module_bp = gen
            .strategy
            .generate_module_build_profile(&module.module, native_modules);
        write_file(&module_build.join("build-profile.json5"), &module_bp);

        // obfuscation-rules.txt (if enabled)
        if module.module.obfuscation {
            let obfuscation_path = module_build.join("obfuscation-rules.txt");
            if !obfuscation_path.exists() {
                write_file(&obfuscation_path, OBFUSCATION_RULES_TEMPLATE);
            }
        }

        // Module hvigorfile.ts
        let hvigorfile = match module.module.module_type.as_str() {
            "hsp" | "shared" => HVIGORFILE_HSP,
            "har" => HVIGORFILE_HSP,
            _ => HVIGORFILE_ENTRY,
        };
        write_file(&module_build.join("hvigorfile.ts"), hvigorfile);

        // Module oh-package.json5
        let oh_pkg =
            gen.strategy
                .generate_module_oh_package(&module.module, app_config, native_modules);
        write_file(&module_build.join("oh-package.json5"), &oh_pkg);
    }
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
    for base_name in std::iter::once(entry_base.clone()).chain(
        all_abilities
            .iter()
            .filter(|a| a.ability_type.is_empty())
            .map(|a| normalize_ets(&a.name)),
    ) {
        let desc_key = format!("{}_desc", base_name);
        let label_key = format!("{}_label", base_name);
        if !content.contains(&format!("\"name\": \"{}\"", desc_key)) {
            new_entries.push(format!(
                r#"    {{
      "name": "{}",
      "value": "{}"
    }}"#,
                desc_key, base_name
            ));
        }
        if !content.contains(&format!("\"name\": \"{}\"", label_key)) {
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
