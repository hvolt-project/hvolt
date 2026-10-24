// Import: standard HarmonyOS (DevEco Studio) layout → flat hvolt layout.
//
// The two directions are asymmetric: `assemble` translates a flat project into the layout
// hvigor expects, while `import` has to reverse-engineer the decisions DevEco Studio made —
// module names, ability presentation, resources split between AppScope and modules, local
// `.har` dependencies. Anything that has no flat equivalent is reported instead of dropped
// silently, because a flattened project that builds differently is worse than a clear "no".

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::{
    AbilityConfig, AppConfig, AppFields, Dependency, MetadataItem, ModuleConfig, ModuleInfo,
    ModulePermission, SigningConfig, Skill, UriScheme,
};

/// Fields found in the source project that the flat layout cannot express.
pub type Gaps = Vec<String>;

pub fn run(from: Option<&Path>, out: Option<&Path>, dry_run: bool) -> Result<(), String> {
    let from = from.unwrap_or(Path::new(".")).to_path_buf();
    if !from.join("AppScope/app.json5").is_file() {
        return Err(format!(
            "{} does not look like a HarmonyOS project (no AppScope/app.json5)",
            from.display()
        ));
    }
    let out = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_out(&from));

    let mut gaps: Gaps = Vec::new();
    let app_cfg = read_app_config(&from, &mut gaps)?;
    let modules = read_modules(&from, &mut gaps)?;

    println!("import {} → {}", from.display(), out.display());
    println!(
        "  app: {} {} ({})",
        app_cfg.app.bundle, app_cfg.app.version, app_cfg.app.bundle_type
    );
    for (dir, cfg) in &modules {
        println!(
            "  module: {} ({}) → {}/  routes {}",
            cfg.module.name,
            if cfg.module.module_type.is_empty() {
                "entry"
            } else {
                cfg.module.module_type.as_str()
            },
            dir,
            cfg.module.routes.len()
        );
    }
    if dry_run {
        print_gaps(&gaps);
        println!("  (dry run: nothing written)");
        return Ok(());
    }

    fs::create_dir_all(&out).map_err(|e| format!("create {}: {}", out.display(), e))?;
    // Dependencies are resolved before the toml is written: the file gets exactly one
    // [dependencies] table, holding the local `.har` (and any directory dependency).
    let mut app_cfg = app_cfg;
    app_cfg.dependencies = collect_dependencies(&from, &out, &mut gaps)?;
    write_app_toml(&out, &app_cfg)?;
    for (dir, cfg) in &modules {
        let module_root = out.join(dir);
        let src_main = from.join(dir).join("src/main");
        let mut ability_files: Vec<String> = vec![cfg.module.entry.clone()];
        ability_files.extend(cfg.module.abilities.iter().map(|a| a.entry.clone()));
        copy_ets(
            &src_main.join("ets"),
            &module_root,
            &ability_files,
            &mut gaps,
        )?;
        copy_resources(&src_main.join("resources"), &module_root.join("resources"))?;
        // In-module native code is copied here and wired by `assemble`, which declares
        // externalNativeOptions for `<member>/cpp` and packs `<member>/libs` — no gap to report.
        let cpp = src_main.join("cpp");
        if cpp.is_dir() {
            copy_dir(&cpp, &module_root.join("cpp"))?;
        }
        let libs = from.join(dir).join("libs");
        if libs.is_dir() {
            copy_dir(&libs, &module_root.join("libs"))?;
        }
        copy_obfuscation(&from.join(dir), &module_root, cfg, &mut gaps)?;
        write_module_toml(&module_root, cfg)?;
    }
    import_app_scope(&from, &out, &mut gaps)?;

    print_gaps(&gaps);
    println!("  written. next: cd {} && hvolt assemble", out.display());
    Ok(())
}

fn print_gaps(gaps: &Gaps) {
    if gaps.is_empty() {
        return;
    }
    println!("  not expressible in the flat layout ({}):", gaps.len());
    for g in gaps {
        println!("    - {}", g);
    }
}

fn default_out(from: &Path) -> PathBuf {
    let name = from
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".into());
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(format!("{}-flat", name))
}

/// JSON5 (comments, trailing commas) → serde_json value.
fn read_json5(path: &Path) -> Result<Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let cleaned = strip_json5(&raw);
    serde_json::from_str(&cleaned).map_err(|e| format!("parse {}: {}", path.display(), e))
}

/// Remove `//` and `/* */` comments and trailing commas without touching string content.
fn strip_json5(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str: Option<char> = None;
    while let Some(c) = chars.next() {
        match in_str {
            Some(quote) => {
                out.push(c);
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                } else if c == quote {
                    in_str = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    in_str = Some(c);
                    out.push(c);
                } else if c == '/' && chars.peek() == Some(&'/') {
                    for n in chars.by_ref() {
                        if n == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                } else if c == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    let mut prev = '\0';
                    for n in chars.by_ref() {
                        if prev == '*' && n == '/' {
                            break;
                        }
                        prev = n;
                    }
                } else {
                    out.push(c);
                }
            }
        }
    }
    // trailing commas before } or ]
    let bytes: Vec<char> = out.chars().collect();
    let mut fixed = String::with_capacity(out.len());
    for (i, c) in bytes.iter().enumerate() {
        if *c == ',' {
            let next = bytes[i + 1..].iter().find(|x| !x.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        fixed.push(*c);
    }
    fixed
}

fn str_at(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn read_app_config(from: &Path, gaps: &mut Gaps) -> Result<AppConfig, String> {
    let app = read_json5(&from.join("AppScope/app.json5"))?;
    let a = app.get("app").cloned().unwrap_or(Value::Null);

    let bundle = str_at(&a, &["bundleName"]).ok_or("AppScope/app.json5 has no app.bundleName")?;
    let version = str_at(&a, &["versionName"]).unwrap_or_else(|| "1.0.0".into());
    let label = str_at(&a, &["label"]).unwrap_or_default();
    let name = resolve_app_name(from, &label)
        .unwrap_or_else(|| bundle.rsplit('.').next().unwrap_or("app").to_string());

    // products / signing configs live in the root build-profile
    let bp_path = from.join("build-profile.json5");
    let mut products = vec!["default".to_string()];
    let mut signing = BTreeMap::new();
    let sdk_version = "26.0.0".to_string();
    let mut compatible_sdk_version = String::new();
    let mut target_sdk = String::new();
    if bp_path.is_file() {
        let bp = read_json5(&bp_path)?;
        if let Some(app_bp) = bp.get("app") {
            if let Some(list) = app_bp.get("products").and_then(|p| p.as_array()) {
                let names: Vec<String> = list
                    .iter()
                    .filter_map(|p| p.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect();
                if !names.is_empty() {
                    products = names;
                }
                // Each key is read on its own: taking the first hit let `targetSdkVersion`
                // shadow `compatibleSdkVersion`, silently raising the compatibility floor
                // (a project pinned to API 23 came out pinned to 26).
                let product = &list[0];
                let compile = str_at(product, &["compileSdkVersion"]);
                let target = str_at(product, &["targetSdkVersion"]);
                let compatible = str_at(product, &["compatibleSdkVersion"]);
                // targetSdkVersion is its own statement (a project may compile against 26 and
                // target 24); it is carried verbatim, so it never needs a warning. Only an
                // explicit compileSdkVersion can genuinely diverge from what hvolt builds with.
                target_sdk = target.clone().unwrap_or_default();
                if let Some(explicit_compile) = compile.clone() {
                    let normalised = normalise_sdk(&explicit_compile);
                    if normalised != sdk_version {
                        gaps.push(format!(
                            "source compiles against {explicit_compile}; hvolt builds with the installed {sdk_version} — install that SDK and set compile_sdk_version to match"
                        ));
                    }
                }
                // compatibleSdkVersion is a device-reachability decision, not a build knob:
                // it is carried over verbatim (including the "(NN)" suffix hvigor expects
                // below API 26) instead of being rebased onto the compile SDK.
                compatible_sdk_version = compatible.clone().unwrap_or_default();
            }
            if let Some(cfgs) = app_bp.get("signingConfigs").and_then(|c| c.as_array()) {
                for c in cfgs {
                    let Some(conf_name) = c.get("name").and_then(|n| n.as_str()) else {
                        continue;
                    };
                    let m = c.get("material").cloned().unwrap_or(Value::Null);
                    // Materials that are not on this machine (a project cloned from another
                    // OS, for example) cannot sign anything — dropping them keeps `hvolt
                    // build` honest instead of failing inside the signing tool.
                    let cfg = SigningConfig {
                        store_file: str_at(&m, &["storeFile"]).unwrap_or_default(),
                        store_password: str_at(&m, &["storePassword"]).unwrap_or_default(),
                        key_alias: str_at(&m, &["keyAlias"]).unwrap_or_default(),
                        key_password: str_at(&m, &["keyPassword"]).unwrap_or_default(),
                        certpath: str_at(&m, &["certpath"]).unwrap_or_default(),
                        profile: str_at(&m, &["profile"]).unwrap_or_default(),
                        sign_alg: str_at(&m, &["signAlg"])
                            .unwrap_or_else(|| "SHA256withECDSA".into()),
                    };
                    let missing = [&cfg.store_file, &cfg.certpath, &cfg.profile]
                        .into_iter()
                        .filter(|p| !p.is_empty())
                        .filter(|p| {
                            let path = Path::new(p);
                            !(path.is_file() || from.join(p).is_file())
                        })
                        .count();
                    for (label, pw) in [
                        ("storePassword", &cfg.store_password),
                        ("keyPassword", &cfg.key_password),
                    ] {
                        if crate::config::SigningConfig::password_looks_encoded(pw) {
                            gaps.push(format!(
                                "signing config {conf_name}: {label} looks DevEco-encoded (hex), not plaintext — \
                                 hap-sign-tool will fail with 'keystore password was incorrect'; set the real password by hand"
                            ));
                        }
                    }
                    if missing > 0 {
                        gaps.push(format!(
                            "signing config {conf_name} dropped: {missing} of its materials are not on this machine"
                        ));
                    } else {
                        signing.insert(conf_name.to_string(), cfg);
                    }
                }
            }
            if let Some(mode_set) = app_bp.get("buildModeSet") {
                if !mode_set.is_null() {
                    gaps.push(
                        "buildModeSet ignored (hvolt always builds the default product)"
                            .to_string(),
                    );
                }
            }
        }
    }
    if !signing.is_empty() {
        gaps.push(format!(
            "signingConfigs imported as [app.signing.{}]; absolute paths to certificates were kept as-is",
            signing.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if label.starts_with("$string:") {
        let key = label.trim_start_matches("$string:");
        gaps.push(format!(
            "app label $string:{key} depends on AppScope strings — kept in app-resources/ when the file exists"
        ));
    }

    // modules (directory names) come from the root build-profile
    let mut members: Vec<String> = Vec::new();
    if let Ok(bp) = read_json5(&from.join("build-profile.json5")) {
        if let Some(mods) = bp.get("modules").and_then(|m| m.as_array()) {
            for m in mods {
                if let Some(p) = m.get("srcPath").and_then(|s| s.as_str()) {
                    members.push(p.trim_start_matches("./").to_string());
                }
            }
        }
    }

    Ok(AppConfig {
        app: AppFields {
            bundle,
            name,
            version,
            version_code: a.get("versionCode").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            build_version: a
                .get("buildVersion")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            vendor: str_at(&a, &["vendor"]).unwrap_or_else(|| "example".into()),
            icon: str_at(&a, &["icon"]).unwrap_or_else(|| "$media:app_icon".into()),
            sdk_path: None,
            sdk_version: sdk_version.clone(),
            compile_sdk_version: sdk_version.clone(),
            target_sdk_version: target_sdk.clone(),
            compatible_sdk_version,
            bundle_type: str_at(&a, &["bundleType"]).unwrap_or_else(|| "app".into()),
            products,
            runtime_os: "HarmonyOS".into(),
            members,
            signing,
        },
        dependencies: BTreeMap::new(),
    })
}

/// "6.1.1(24)" → "26.0.0"-style plain version is what hvolt writes; keep the numeric part.
/// Display name: follow the AppScope string the label points at, so an imported project keeps
/// the name users see instead of the bundle's last path segment.
fn resolve_app_name(from: &Path, label: &str) -> Option<String> {
    let key = label.strip_prefix("$string:")?;
    let path = from.join("AppScope/resources/base/element/string.json");
    let v = read_json5(&path).ok()?;
    let value = v
        .get("string")?
        .as_array()?
        .iter()
        .find(|e| e.get("name").and_then(|n| n.as_str()) == Some(key))?
        .get("value")?
        .as_str()?;
    let cleaned: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    Some(cleaned.trim_matches('-').to_lowercase())
}

fn normalise_sdk(raw: &str) -> String {
    let numeric: String = raw
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if numeric.is_empty() {
        "26.0.0".to_string()
    } else {
        numeric
    }
}

fn read_modules(from: &Path, gaps: &mut Gaps) -> Result<Vec<(String, ModuleConfig)>, String> {
    let bp = read_json5(&from.join("build-profile.json5"))?;
    let mut out = Vec::new();
    let Some(list) = bp.get("modules").and_then(|m| m.as_array()) else {
        return Err("build-profile.json5 has no modules[]".into());
    };
    for entry in list {
        let Some(src_path) = entry.get("srcPath").and_then(|s| s.as_str()) else {
            continue;
        };
        let dir = src_path.trim_start_matches("./").to_string();
        let module_root = from.join(&dir);
        let module_json = module_root.join("src/main/module.json5");
        if !module_json.is_file() {
            gaps.push(format!("{dir}: no src/main/module.json5 — module skipped"));
            continue;
        }
        let mj = read_json5(&module_json)?;
        let m = mj.get("module").cloned().unwrap_or(Value::Null);
        let name = str_at(&m, &["name"]).unwrap_or_else(|| dir.clone());
        let module_type = str_at(&m, &["type"]).unwrap_or_else(|| "entry".into());
        let device_types = m
            .get("deviceTypes")
            .and_then(|d| d.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec!["phone".to_string()]);

        // routes come from the profile main_pages.json
        let pages_profile = str_at(&m, &["pages"]).unwrap_or_default();
        let routes = if let Some(profile) = pages_profile.strip_prefix("$profile:") {
            let p = module_root
                .join("src/main/resources/base/profile")
                .join(format!("{profile}.json"));
            if p.is_file() {
                read_json5(&p)?
                    .get("src")
                    .and_then(|s| s.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                gaps.push(format!("{dir}: pages profile {profile}.json missing"));
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // abilities: the first UIAbility is the module entry
        let mut entry = "EntryAbility.ets".to_string();
        let mut skills: Vec<Skill> = Vec::new();
        let mut abilities: Vec<AbilityConfig> = Vec::new();
        let mut entry_icon = String::new();
        let mut entry_label = String::new();
        let mut entry_description = String::new();
        let mut entry_start_icon = String::new();
        let mut entry_orientation = String::new();
        let mut first = true;
        for ab in m
            .get("abilities")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let file = ability_file(&ab, "entryability");
            // labels/descriptions are carried verbatim now; nothing to report

            let ab_skills = parse_skills(&ab);
            if first {
                entry = file;
                skills = ab_skills;
                entry_icon = str_at(&ab, &["icon"]).unwrap_or_default();
                entry_label = str_at(&ab, &["label"]).unwrap_or_default();
                entry_description = str_at(&ab, &["description"]).unwrap_or_default();
                entry_start_icon = str_at(&ab, &["startWindowIcon"]).unwrap_or_default();
                entry_orientation = str_at(&ab, &["orientation"]).unwrap_or_default();
                first = false;
            } else {
                abilities.push(AbilityConfig {
                    name: str_at(&ab, &["name"]).unwrap_or_default(),
                    entry: file,
                    dir: ability_dir(&ab),
                    label: str_at(&ab, &["label"]).unwrap_or_default(),
                    description: str_at(&ab, &["description"]).unwrap_or_default(),
                    ability_type: String::new(),
                    skills: ab_skills,
                    exported: ab.get("exported").and_then(|e| e.as_bool()),
                    orientation: str_at(&ab, &["orientation"]).unwrap_or_default(),
                    ..Default::default()
                });
            }
        }
        for ext in m
            .get("extensionAbilities")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let entry_file = ability_file(&ext, "entryformability");
            let role_dir = ability_dir(&ext);
            let metadata = ext
                .get("metadata")
                .and_then(|m| m.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            Some(MetadataItem {
                                name: x.get("name")?.as_str()?.to_string(),
                                resource: x.get("resource")?.as_str()?.to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut ability = AbilityConfig {
                name: str_at(&ext, &["name"]).unwrap_or_default(),
                entry: entry_file,
                dir: role_dir,
                label: str_at(&ext, &["label"]).unwrap_or_default(),
                description: str_at(&ext, &["description"]).unwrap_or_default(),
                ability_type: str_at(&ext, &["type"]).unwrap_or_default(),
                exported: ext.get("exported").and_then(|e| e.as_bool()),
                metadata,
                ..Default::default()
            };
            if let Some(cfg) = ext.get("configProfile") {
                ability.config_profile = Some(cfg.clone());
            }
            if ability.label.is_empty() && ability.description.is_empty() {
                gaps.push(format!(
                    "{dir}: extension ability {} declares no label/description — the generated module.json5 omits them too",
                    ability.name
                ));
            }
            abilities.push(ability);
        }

        let permissions: Vec<ModulePermission> = m
            .get("requestPermissions")
            .and_then(|p| p.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        let name = p.get("name")?.as_str()?.to_string();
                        let scene = p.get("usedScene").cloned().unwrap_or(Value::Null);
                        Some(ModulePermission {
                            name,
                            reason: str_at(p, &["reason"]).unwrap_or_default(),
                            abilities: scene
                                .get("abilities")
                                .and_then(|x| x.as_array())
                                .map(|x| {
                                    x.iter()
                                        .filter_map(|y| y.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            when: str_at(&scene, &["when"]).unwrap_or_else(|| "always".into()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let (cpp_flags, native_arguments, abi_filters) = read_native_flags(&module_root);
        let cfg = ModuleConfig {
            module: ModuleInfo {
                name,
                routes,
                entry,
                skills,
                // a single HAP module is the entry module; hsp/shared would need multi-module support
                module_type: if module_type == "entry" {
                    String::new()
                } else {
                    module_type.clone()
                },
                abilities,
                device_types,
                permissions,
                obfuscation: read_obfuscation(&module_root, gaps, &dir),
                cpp_flags,
                native_arguments,
                obfuscation_rules: String::new(),
                icon: if entry_icon.is_empty() {
                    String::new()
                } else {
                    entry_icon
                },
                start_window_icon: if entry_start_icon.is_empty() {
                    String::new()
                } else {
                    entry_start_icon
                },
                entry_label,
                entry_description,
                // Only an absent key stays absent: a project that declares auto_rotation
                // explicitly means it, and dropping it changes the product.
                orientation: entry_orientation,
                delivery_with_install: m.get("deliveryWithInstall").and_then(|v| v.as_bool()),
                installation_free: m.get("installationFree").and_then(|v| v.as_bool()),
                abi_filters,
            },
        };
        out.push((dir, cfg));
    }
    Ok(out)
}

/// Role directory from `srcEntry` ("./ets/entrybackupability/X.ets" → "entrybackupability").
fn ability_dir(ability: &Value) -> String {
    ability
        .get("srcEntry")
        .and_then(|s| s.as_str())
        .and_then(|s| s.rsplit_once('/'))
        .and_then(|(dir, _)| dir.rsplit('/').next())
        .filter(|d| *d != "ets")
        .map(|d| d.to_string())
        .unwrap_or_default()
}

/// Ability source file name from its `srcEntry` ("./ets/entryability/EntryAbility.ets").
fn ability_file(ability: &Value, expect_dir: &str) -> String {
    ability
        .get("srcEntry")
        .and_then(|s| s.as_str())
        .and_then(|s| s.rsplit('/').next())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{expect_dir}/EntryAbility.ets"))
}

fn parse_skills(ability: &Value) -> Vec<Skill> {
    ability
        .get("skills")
        .and_then(|s| s.as_array())
        .map(|list| {
            list.iter()
                .map(|s| Skill {
                    actions: string_list(s, "actions"),
                    entities: string_list(s, "entities"),
                    uris: s
                        .get("uris")
                        .and_then(|u| u.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|u| {
                                    Some(UriScheme {
                                        scheme: u.get("scheme")?.as_str()?.to_string(),
                                        host: u
                                            .get("host")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        port: u
                                            .get("port")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        path: u
                                            .get("path")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        path_start_with: u
                                            .get("pathStartWith")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        path_regex: u
                                            .get("pathRegex")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        mime_type: u
                                            .get("type")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        utd: u
                                            .get("utd")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                        max_file_supported: u
                                            .get("maxFileSupported")
                                            .and_then(|h| h.as_u64())
                                            .map(|n| n as u32),
                                        link_feature: u
                                            .get("linkFeature")
                                            .and_then(|h| h.as_str())
                                            .map(String::from),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    permissions: string_list(s, "permissions"),
                    domain_verify: s.get("domainVerify").and_then(|d| d.as_bool()),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn string_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Module-level `buildOption.externalNativeOptions` flags. Dropping them silently rebuilds
/// the native code with default optimisation (the source set `-O3 -fvisibility=hidden`).
fn read_native_flags(module_root: &Path) -> (String, String, Vec<String>) {
    let default_abis = vec!["arm64-v8a".to_string()];
    let bp = module_root.join("build-profile.json5");
    if !bp.is_file() {
        return (String::new(), String::new(), default_abis);
    }
    let Ok(v) = read_json5(&bp) else {
        return (String::new(), String::new(), default_abis);
    };
    let Some(opts) = v.pointer("/buildOption/externalNativeOptions") else {
        return (String::new(), String::new(), default_abis);
    };
    let cpp_flags = opts
        .get("cppFlags")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let arguments = opts
        .get("arguments")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    // ABI list decides which devices can run the native code: a narrowed default (arm64-v8a)
    // silently drops x86_64 and 32-bit targets, so the source list is carried over.
    let abi_filters: Vec<String> = opts
        .get("abiFilters")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let abi_filters = if abi_filters.is_empty() {
        default_abis
    } else {
        abi_filters
    };
    (cpp_flags, arguments, abi_filters)
}

fn read_obfuscation(module_root: &Path, gaps: &mut Gaps, dir: &str) -> bool {
    let bp = module_root.join("build-profile.json5");
    if !bp.is_file() {
        return false;
    }
    let Ok(v) = read_json5(&bp) else {
        return false;
    };
    let enabled = v
        .get("buildOptionSet")
        .and_then(|s| s.as_array())
        .map(|sets| {
            sets.iter().any(|s| {
                s.pointer("/arkOptions/obfuscation/ruleOptions/enable")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if enabled && !module_root.join("obfuscation-rules.txt").is_file() {
        gaps.push(format!(
            "{dir}: obfuscation enabled but the rules file is not named obfuscation-rules.txt"
        ));
    }
    enabled
}

/// Locate an ability source by file name anywhere below `ets/`.
fn find_ets(ets_src: &Path, file: &str) -> Result<Option<PathBuf>, String> {
    for entry in fs::read_dir(ets_src)
        .map_err(|e| format!("read {}: {}", ets_src.display(), e))?
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            if let Some(hit) = find_ets(&path, file)? {
                return Ok(Some(hit));
            }
        } else if entry.file_name().to_string_lossy() == file {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn copy_ets(
    ets_src: &Path,
    module_out: &Path,
    abilities: &[String],
    gaps: &mut Gaps,
) -> Result<(), String> {
    if !ets_src.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(module_out)
        .map_err(|e| format!("create {}: {}", module_out.display(), e))?;
    // Every ability source is hoisted to the module root, which is where `assemble` looks for
    // it. DevEco puts abilities in role directories of its own choosing — entryability,
    // entryformability, but also entrybackupability and friends — so the set of names comes
    // from module.json5, not from a hardcoded list of two directory names.
    let mut hoisted: Vec<String> = Vec::new();
    for ability in abilities {
        match find_ets(ets_src, ability)? {
            Some(path) => {
                fs::copy(&path, module_out.join(ability))
                    .map_err(|e| format!("copy {}: {}", ability, e))?;
                hoisted.push(ability.clone());
            }
            None => gaps.push(format!(
                "ability source {ability} is declared in module.json5 but not found under ets/ — hvigor will reject the generated srcEntry"
            )),
        }
    }

    for sub in fs::read_dir(ets_src)
        .map_err(|e| format!("read {}: {}", ets_src.display(), e))?
        .flatten()
    {
        let name = sub.file_name().to_string_lossy().to_string();
        if !sub.path().is_dir() {
            // Files sitting directly in ets/ are part of the module (a shared Engine.ets, a
            // .d.ts bridge). Dropping them makes imports like '../EdgeTAMEngine' fail later,
            // so they come along unless an ability already claimed the name.
            let is_source = name.ends_with(".ets")
                || name.ends_with(".ts")
                || name.ends_with(".d.ts")
                || name.ends_with(".json");
            if is_source && !hoisted.contains(&name) {
                fs::copy(sub.path(), module_out.join(&name))
                    .map_err(|e| format!("copy {}: {}", name, e))?;
            }
            continue;
        }
        let mut keep_dir = false;
        for f in fs::read_dir(sub.path())
            .map_err(|e| format!("read {}: {}", sub.path().display(), e))?
            .flatten()
        {
            let file = f.file_name().to_string_lossy().to_string();
            if !file.ends_with(".ets") || !hoisted.contains(&file) {
                keep_dir = true;
            }
        }
        if keep_dir {
            copy_dir(&sub.path(), &module_out.join(&name))?;
        }
    }
    let found: Vec<String> = source_test_dirs(ets_src)
        .into_iter()
        .filter(|d| d.is_dir())
        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    if !found.is_empty() {
        gaps.push(format!(
            "{}: test sources ({}) are not part of the flat layout and no ohosTest target is generated — they stay in the source project",
            found.join("/"),
            found.join(", ")
        ));
    }
    Ok(())
}

/// Source trees beside `src/main/` that the flat layout does not carry.
///
/// `<module>/src/main/ets` → the module's `src/` is two levels up; the test trees live at
/// `src/test`, `src/ohosTest` and `src/mock`.
fn source_test_dirs(ets_src: &Path) -> Vec<PathBuf> {
    let module_src = ets_src
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    let Some(module_src) = module_src else {
        return Vec::new();
    };
    ["test", "ohosTest", "mock"]
        .into_iter()
        .map(|name| module_src.join(name))
        .collect()
}

fn copy_resources(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        copy_dir(src, dst)?;
    }
    Ok(())
}

fn copy_obfuscation(
    module_root: &Path,
    module_out: &Path,
    cfg: &ModuleConfig,
    gaps: &mut Gaps,
) -> Result<(), String> {
    if !cfg.module.obfuscation {
        return Ok(());
    }
    let rules = module_root.join("obfuscation-rules.txt");
    if rules.is_file() {
        fs::copy(&rules, module_out.join("obfuscation-rules.txt"))
            .map_err(|e| format!("copy obfuscation rules: {}", e))?;
    } else {
        gaps.push("obfuscation enabled without obfuscation-rules.txt".into());
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create {}: {}", dst.display(), e))?;
    for entry in fs::read_dir(src)
        .map_err(|e| format!("read {}: {}", src.display(), e))?
        .flatten()
    {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| format!("copy {}: {}", path.display(), e))?;
        }
    }
    Ok(())
}

fn write_app_toml(out: &Path, cfg: &AppConfig) -> Result<(), String> {
    let body = toml::to_string(cfg).map_err(|e| format!("serialize hvolt.toml: {}", e))?;
    let header =
        "# Imported by `hvolt import` — review members, products and signing before building.\n\n";
    fs::write(out.join("hvolt.toml"), format!("{header}{body}"))
        .map_err(|e| format!("write hvolt.toml: {}", e))
}

fn write_module_toml(dir: &Path, cfg: &ModuleConfig) -> Result<(), String> {
    let body = toml::to_string(cfg).map_err(|e| format!("serialize module hvolt.toml: {}", e))?;
    let header = "# Imported by `hvolt import`.\n\n";
    fs::write(dir.join("hvolt.toml"), format!("{header}{body}"))
        .map_err(|e| format!("write {}/hvolt.toml: {}", dir.display(), e))
}

/// AppScope resources → `app-resources/` (the flat home for application-scope resources).
fn import_app_scope(from: &Path, out: &Path, gaps: &mut Gaps) -> Result<(), String> {
    let src = from.join("AppScope/resources");
    if !src.is_dir() {
        return Ok(());
    }
    // AppScope/resources/base/** → app-resources/base/**
    copy_dir(&src, &out.join("app-resources"))?;
    gaps.push(
        "AppScope resources copied to app-resources/ — the app icon must also be reachable from resources/ if the entry module references it".into(),
    );
    Ok(())
}

/// Local `file:` dependencies (a bundled .har) → [dependencies], with the payload copied in.
fn collect_dependencies(
    from: &Path,
    out: &Path,
    gaps: &mut Gaps,
) -> Result<BTreeMap<String, Dependency>, String> {
    let pkg = from.join("oh-package.json5");
    let mut deps: BTreeMap<String, Dependency> = BTreeMap::new();
    if !pkg.is_file() {
        return Ok(deps);
    }
    let v = read_json5(&pkg)?;
    let Some(map) = v.get("dependencies").and_then(|d| d.as_object()) else {
        return Ok(deps);
    };
    for (name, spec) in map {
        let Some(s) = spec.as_str() else { continue };
        let Some(path) = s.strip_prefix("file:") else {
            gaps.push(format!(
                "registry dependency {name} = {s} has no flat equivalent yet"
            ));
            continue;
        };
        let rel = path.trim_start_matches("./");
        let src = from.join(rel);
        let dest = out.join(rel);
        if src.is_dir() {
            copy_dir(&src, &dest)?;
        } else if src.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::copy(&src, &dest).map_err(|e| format!("copy dependency {}: {}", name, e))?;
        } else {
            gaps.push(format!("dependency {name} points at a missing path {path}"));
            continue;
        }
        deps.insert(
            name.clone(),
            Dependency {
                path: rel.to_string(),
                version: "1.0.0".to_string(),
            },
        );
    }
    Ok(deps)
}

#[cfg(test)]
mod tests {
    use super::{ability_file, normalise_sdk, strip_json5};

    /// DevEco writes JSON5: comments and trailing commas must not break the import.
    #[test]
    fn json5_comments_and_trailing_commas_are_stripped() {
        let src = r#"{
          // module
          "name": "entry",
          "deviceTypes": ["phone", "tablet",],
          /* block comment */
          "skills": [{ "actions": ["a"], },],
        }"#;
        let cleaned = strip_json5(src);
        let v: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(v["name"], "entry");
        assert_eq!(v["deviceTypes"].as_array().unwrap().len(), 2);
        assert!(v["skills"].is_array());
    }

    /// Comment markers inside strings are content, not comments.
    #[test]
    fn urls_inside_strings_survive() {
        let src = r#"{ "url": "https://example.com/a//b", "note": "/* not a comment */" }"#;
        let v: serde_json::Value = serde_json::from_str(&strip_json5(src)).unwrap();
        assert_eq!(v["url"], "https://example.com/a//b");
        assert_eq!(v["note"], "/* not a comment */");
    }

    /// "5.0.0(12)" → "5.0.0": the parenthesised API level is not part of the version.
    #[test]
    fn sdk_version_drops_the_api_suffix() {
        assert_eq!(normalise_sdk("5.0.0(12)"), "5.0.0");
        assert_eq!(normalise_sdk("26.0.0"), "26.0.0");
        assert_eq!(normalise_sdk(""), "26.0.0");
    }

    /// Ability sources live in role directories; the flat layout keeps just the file name.
    #[test]
    fn ability_file_is_the_last_path_segment() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"srcEntry": "./ets/entryformability/EntryFormAbility.ets"}"#)
                .unwrap();
        assert_eq!(ability_file(&v, "entryformability"), "EntryFormAbility.ets");
    }
}

#[cfg(test)]
mod sdk_and_password_tests {
    use super::normalise_sdk;
    use crate::config::SigningConfig;

    /// DevEco writes encoded passwords; a 32+ char hex string is never a real password.
    #[test]
    fn encoded_passwords_are_recognised() {
        assert!(SigningConfig::password_looks_encoded(&"a".repeat(64)));
        assert!(SigningConfig::password_looks_encoded(
            "0000001B03B0928F0A1B2C3D4E5F60718293A4B5C6D7E8F9A0B1C2D3E4F5A6B7"
        ));
        assert!(!SigningConfig::password_looks_encoded("0000001B03B0928F")); // 16 chars: too short to be the encoded form
        assert!(!SigningConfig::password_looks_encoded("hunter2"));
        assert!(!SigningConfig::password_looks_encoded(""));
    }

    /// The compatible SDK level keeps its "(NN)" suffix: hvigor below API 26 requires it.
    #[test]
    fn compatible_sdk_version_is_carried_verbatim() {
        assert_eq!(normalise_sdk("6.1.0(23)"), "6.1.0");
    }
}

#[cfg(test)]
mod test_dir_resolution_tests {
    use super::source_test_dirs;
    use std::path::Path;

    /// `<module>/src/main/ets` → `<module>/src/{test,ohosTest,mock}`: one level short was the
    /// bug that made the gap unreachable.
    #[test]
    fn test_trees_resolve_beside_src_main() {
        let dirs = source_test_dirs(Path::new("/p/entry/src/main/ets"));
        let rendered: Vec<String> = dirs
            .iter()
            .map(|d| d.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            rendered,
            vec![
                "/p/entry/src/test".to_string(),
                "/p/entry/src/ohosTest".to_string(),
                "/p/entry/src/mock".to_string()
            ]
        );
    }

    /// A path with no module level above it yields nothing instead of panicking.
    #[test]
    fn shallow_paths_yield_no_candidates() {
        assert!(source_test_dirs(Path::new("ets")).is_empty());
    }
}
