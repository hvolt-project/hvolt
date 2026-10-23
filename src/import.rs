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
    if !gaps.is_empty() {
        println!("  not expressible in the flat layout ({}):", gaps.len());
        for g in &gaps {
            println!("    - {}", g);
        }
    }

    if dry_run {
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
        copy_ets(&src_main.join("ets"), &module_root, &mut gaps)?;
        copy_resources(&src_main.join("resources"), &module_root.join("resources"))?;
        copy_obfuscation(&from.join(dir), &module_root, cfg, &mut gaps)?;
        write_module_toml(&module_root, cfg)?;
    }
    import_app_scope(&from, &out, &mut gaps)?;

    println!("  written. next: cd {} && hvolt assemble", out.display());
    if !gaps.is_empty() {
        println!(
            "  review the {} listed gaps above before trusting the build.",
            gaps.len()
        );
    }
    Ok(())
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
                // hvolt translates for the SDK it targets; an older API level in the source is
                // reported rather than copied, because build-profile rejects e.g. "5.0.0"
                // against an API-26 toolchain (00306042).
                for key in [
                    "compileSdkVersion",
                    "targetSdkVersion",
                    "compatibleSdkVersion",
                ] {
                    if let Some(v) = str_at(&list[0], &[key]) {
                        let source_api = normalise_sdk(&v);
                        if source_api != sdk_version {
                            gaps.push(format!(
                                "source targets SDK {v}; the imported project is pinned to {sdk_version} — install that SDK and adjust sdk_version to build for the original API level"
                            ));
                        }
                        break;
                    }
                }
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
            vendor: str_at(&a, &["vendor"]).unwrap_or_else(|| "example".into()),
            icon: str_at(&a, &["icon"]).unwrap_or_else(|| "$media:app_icon".into()),
            sdk_path: None,
            sdk_version: sdk_version.clone(),
            compile_sdk_version: sdk_version,
            compatible_sdk_version: String::new(),
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
            entry_description_gap(&ab, gaps, &dir);
            let ab_skills = parse_skills(&ab);
            if first {
                entry = file;
                skills = ab_skills;
                entry_icon = str_at(&ab, &["icon"]).unwrap_or_default();
                entry_start_icon = str_at(&ab, &["startWindowIcon"]).unwrap_or_default();
                entry_orientation = str_at(&ab, &["orientation"]).unwrap_or_default();
                first = false;
            } else {
                abilities.push(AbilityConfig {
                    name: str_at(&ab, &["name"]).unwrap_or_default(),
                    entry: file,
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
                ability_type: str_at(&ext, &["type"]).unwrap_or_default(),
                exported: ext.get("exported").and_then(|e| e.as_bool()),
                metadata,
                ..Default::default()
            };
            if let Some(cfg) = ext.get("configProfile") {
                ability.config_profile = Some(cfg.clone());
            }
            gaps.push(format!(
                "{dir}: extension ability {} imported; its label/description strings are regenerated",
                ability.name
            ));
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
                orientation: if entry_orientation.is_empty() || entry_orientation == "auto_rotation"
                {
                    String::new()
                } else {
                    entry_orientation
                },
                delivery_with_install: m.get("deliveryWithInstall").and_then(|v| v.as_bool()),
                installation_free: m.get("installationFree").and_then(|v| v.as_bool()),
                abi_filters: vec!["arm64-v8a".to_string()],
            },
        };
        out.push((dir, cfg));
    }
    Ok(out)
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

fn entry_description_gap(ability: &Value, gaps: &mut Gaps, dir: &str) {
    if ability.get("description").is_some() || ability.get("label").is_some() {
        gaps.push(format!(
            "{dir}: ability label/description strings are regenerated from the ability name"
        ));
    }
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

fn copy_ets(ets_src: &Path, module_out: &Path, gaps: &mut Gaps) -> Result<(), String> {
    if !ets_src.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(module_out)
        .map_err(|e| format!("create {}: {}", module_out.display(), e))?;
    for role in ["entryability", "entryformability"] {
        let dir = ets_src.join(role);
        if !dir.is_dir() {
            continue;
        }
        for f in fs::read_dir(&dir)
            .map_err(|e| format!("read {}: {}", dir.display(), e))?
            .flatten()
        {
            let name = f.file_name().to_string_lossy().to_string();
            if !name.ends_with(".ets") {
                continue;
            }
            // the entry ability keeps its name at the module root; other ability files too
            fs::copy(f.path(), module_out.join(&name))
                .map_err(|e| format!("copy {}: {}", name, e))?;
        }
    }
    for sub in fs::read_dir(ets_src)
        .map_err(|e| format!("read {}: {}", ets_src.display(), e))?
        .flatten()
    {
        let name = sub.file_name().to_string_lossy().to_string();
        if name == "entryability" || name == "entryformability" {
            continue;
        }
        if sub.path().is_dir() {
            copy_dir(&sub.path(), &module_out.join(&name))?;
        }
    }
    let test_dirs = [ets_src.join("../test"), ets_src.join("../ohosTest")];
    if test_dirs.iter().any(|d| d.is_dir()) {
        gaps.push("test/ohosTest sources are not part of the flat layout — left behind".into());
    }
    Ok(())
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
