// Import: standard HarmonyOS (DevEco Studio) layout → flat hvolt layout.
//
// The two directions are asymmetric: `assemble` translates a flat project into the layout
// DevEco Studio expects, while `import` has to reverse-engineer the decisions it made —
// module names, ability presentation, resources split between AppScope and modules, local
// `.har` dependencies. Anything that has no flat equivalent is reported instead of dropped
// silently, because a flattened project that builds differently is worse than a clear "no".

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::support::config::{
    AbilityConfig, AppConfig, AppFields, BackupConfig, Dependency, FormDef, FormWindow,
    MetadataItem, ModuleConfig, ModuleInfo, ModulePermission, ProductApp, ProductConfig,
    ProductResources, ProductSigning, RouterMapDef, SigningConfig, Skill, UriScheme,
};

/// Fields found in the source project that the hvolt layout cannot express.
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

    println!("migrate {} → {}", from.display(), out.display());
    println!(
        "  app: {} {} ({})",
        app_cfg.app.bundle, app_cfg.app.version, app_cfg.app.bundle_type
    );
    for (dir, cfg) in &modules {
        println!(
            "  module: {} → {}/  [{}]  {} route(s)",
            cfg.module.name,
            dir,
            if cfg.module.module_type.is_empty() {
                "entry"
            } else {
                cfg.module.module_type.as_str()
            },
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
    println!("  written. next: cd {} && hvolt build", out.display());
    Ok(())
}

fn print_gaps(gaps: &Gaps) {
    if gaps.is_empty() {
        return;
    }
    println!("  not expressible in the hvolt layout ({}):", gaps.len());
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

    // Products / signing configs live in the root build-profile.
    let bp_path = from.join("build-profile.json5");
    let mut products = vec!["default".to_string()];
    let mut product_configs: BTreeMap<String, ProductConfig> = BTreeMap::new();
    let mut default_product: Option<String> = None;
    let mut signing = BTreeMap::new();
    let sdk_version = crate::support::config::SDK_PLATFORM_VERSION.to_string();
    let mut compatible_sdk_version = String::new();
    let mut target_sdk = String::new();
    let root_version_code = a.get("versionCode").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if bp_path.is_file() {
        let bp = read_json5(&bp_path)?;
        if let Some(app_bp) = bp.get("app") {
            let product_list: Vec<Value> = app_bp
                .get("products")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();
            if !product_list.is_empty() {
                products.clear();
            }
            if let Some(product) = product_list.first() {
                // Each key is read on its own: taking the first hit let `targetSdkVersion`
                // shadow `compatibleSdkVersion`, silently raising the compatibility floor
                // (a project pinned to API 23 came out pinned to 26).
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
                // it is carried over verbatim (including the "(NN)" suffix the SDK expects
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
                        if crate::support::config::SigningConfig::password_looks_encoded(pw) {
                            gaps.push(format!(
                                "signing config {conf_name}: {label} looks DevEco-encoded (hex), not plaintext — \
                                 the signing key cannot be decrypted with it; set the real password by hand"
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
            // Products are imported after `signingConfigs`: a product's `signingConfig`
            // names one of them, and the name alone is not enough to sign with.
            let identity = (bundle.as_str(), version.as_str(), root_version_code);
            for entry in &product_list {
                let Some(product_name) = entry.get("name").and_then(|name| name.as_str()) else {
                    gaps.push(
                        "build-profile product without a name skipped — hvolt addresses a product                          by [products.<name>]"
                            .to_string(),
                    );
                    continue;
                };
                if !products.iter().any(|known| known == product_name) {
                    products.push(product_name.to_string());
                }
                product_configs.insert(
                    product_name.to_string(),
                    import_product(entry, product_name, identity, &signing, gaps),
                );
            }
            if let Some(first) = product_list
                .first()
                .and_then(|entry| entry.get("name"))
                .and_then(|name| name.as_str())
            {
                // hvigor builds the first product by default; hvolt's implicit default is
                // `default`, so a project whose first product is another name has to say so.
                if first != "default" {
                    default_product = Some(first.to_string());
                }
            }
            if let Some(mode_set) = app_bp.get("buildModeSet") {
                if !mode_set.is_null() {
                    gaps.push(
                        "buildModeSet ignored (hvolt builds one product per `hvolt build                          --product`; there is no release/debug mode set)"
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
        registries: BTreeMap::new(),
        products: product_configs,
        default_product,
        resolved: None,
        app: AppFields {
            bundle,
            name,
            version,
            version_code: root_version_code,
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
            runtime_os: crate::support::config::DEFAULT_RUNTIME_OS.into(),
            members,
            signing,
        },
        dependencies: BTreeMap::new(),
    })
}

/// One `app.products[]` entry → `[products.<name>]`.
///
/// Only the artifact-level fields travel (`bundleName`, `versionName`, `versionCode`,
/// `resource.directories`, `signingConfig`). Everything that decides the toolchain, the module
/// set or the package layout is project-wide in hvolt, so it is reported as a gap instead of
/// being written somewhere it would not take effect.
fn import_product(
    entry: &Value,
    name: &str,
    identity: (&str, &str, u32),
    signing: &BTreeMap<String, SigningConfig>,
    gaps: &mut Gaps,
) -> ProductConfig {
    let (root_bundle, root_version, root_version_code) = identity;
    let mut overrides = ProductApp::default();
    if let Some(bundle) = str_at(entry, &["bundleName"]) {
        if bundle != root_bundle {
            overrides.bundle = Some(bundle);
        }
    }
    if let Some(version) = str_at(entry, &["versionName"]) {
        if version != root_version {
            overrides.version = Some(version);
        }
    }
    if let Some(code) = entry.get("versionCode").and_then(|value| value.as_u64()) {
        let code = code as u32;
        if code != root_version_code {
            overrides.version_code = Some(code);
        }
    }

    let mut resources = ProductResources::default();
    if let Some(resource) = entry.get("resource") {
        resources.directories = resource
            .get("directories")
            .and_then(|dirs| dirs.as_array())
            .map(|dirs| {
                dirs.iter()
                    .filter_map(|dir| dir.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let extra: Vec<&str> = resource
            .as_object()
            .map(|object| {
                object
                    .keys()
                    .map(String::as_str)
                    .filter(|key| *key != "directories")
                    .collect()
            })
            .unwrap_or_default();
        if !extra.is_empty() {
            gaps.push(format!(
                "product `{name}`: resource.{} is not expressible — only resource.directories is carried",
                extra.join(", resource.")
            ));
        }
    }

    if entry
        .get("targets")
        .and_then(|targets| targets.as_array())
        .is_some_and(|targets| !targets.is_empty())
    {
        gaps.push(format!(
            "product `{name}`: modules[].targets is not expressible — hvolt builds the same module              set for every product"
        ));
    }
    for key in [
        "compileSdkVersion",
        "targetSdkVersion",
        "compatibleSdkVersion",
        "runtimeOS",
        "deviceTypes",
        "buildOption",
        "output",
    ] {
        if entry.get(key).is_some() {
            gaps.push(format!(
                "product `{name}`: `{key}` is project-wide in hvolt (it decides the toolchain, the                  module set or the package layout) — the root value is kept"
            ));
        }
    }

    let mut signing_overrides = None;
    if let Some(config_name) = entry.get("signingConfig").and_then(|name| name.as_str()) {
        match signing.get(config_name) {
            Some(material) => {
                signing_overrides = product_signing_overrides(material, signing.get("default"));
            }
            None => gaps.push(format!(
                "product `{name}`: signingConfig `{config_name}` was not imported (its materials                  are missing or DevEco-encoded) — write [products.{name}.signing] by hand"
            )),
        }
    }

    ProductConfig {
        app: overrides,
        signing: signing_overrides,
        resources,
    }
}

/// A product's materials, reduced to the fields that differ from the inherited default.
///
/// An equal field is omitted so the written table says exactly what the product changes, and a
/// project whose default config already carries the same materials gets no duplicated table.
fn product_signing_overrides(
    material: &SigningConfig,
    base: Option<&SigningConfig>,
) -> Option<ProductSigning> {
    let pick = |value: &str, inherited: Option<&str>| match inherited {
        Some(inherited) if inherited == value => None,
        _ => Some(value.to_string()),
    };
    let overrides = ProductSigning {
        store_file: pick(&material.store_file, base.map(|b| b.store_file.as_str())),
        store_password: pick(
            &material.store_password,
            base.map(|b| b.store_password.as_str()),
        ),
        key_alias: pick(&material.key_alias, base.map(|b| b.key_alias.as_str())),
        key_password: pick(
            &material.key_password,
            base.map(|b| b.key_password.as_str()),
        ),
        certpath: pick(&material.certpath, base.map(|b| b.certpath.as_str())),
        profile: pick(&material.profile, base.map(|b| b.profile.as_str())),
        sign_alg: pick(&material.sign_alg, base.map(|b| b.sign_alg.as_str())),
    };
    (overrides != ProductSigning::default()).then_some(overrides)
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
        crate::support::config::SDK_PLATFORM_VERSION.to_string()
    } else {
        numeric
    }
}

/// The widget definitions a module ships, from its `form_config.json`.
///
/// The manifest itself is not carried over any more; its content becomes `[[module.forms]]`.
fn form_defs(profile_dir: &Path) -> Vec<FormDef> {
    let Ok(json) = read_json5(&profile_dir.join("form_config.json")) else {
        return Vec::new();
    };
    json.get("forms")
        .and_then(|value| value.as_array())
        .map(|forms| {
            forms
                .iter()
                .filter_map(|form| {
                    Some(FormDef {
                        name: form.get("name")?.as_str()?.to_string(),
                        display_name: str_of(form, "displayName"),
                        description: str_of(form, "description"),
                        src: form
                            .get("src")?
                            .as_str()?
                            .trim_start_matches("./")
                            .to_string(),
                        ui_syntax: str_of(form, "uiSyntax"),
                        window: form.get("window").map(|window| FormWindow {
                            design_width: window
                                .get("designWidth")
                                .and_then(|value| value.as_u64())
                                .unwrap_or(0) as u32,
                            auto_design_width: window
                                .get("autoDesignWidth")
                                .and_then(|value| value.as_bool())
                                .unwrap_or(false),
                        }),
                        color_mode: str_of(form, "colorMode"),
                        is_dynamic: bool_of(form, "isDynamic"),
                        is_default: bool_of(form, "isDefault"),
                        update_enabled: bool_of(form, "updateEnabled"),
                        scheduled_update_time: form
                            .get("scheduledUpdateTime")
                            .and_then(|value| value.as_str())
                            .map(String::from),
                        update_duration: form
                            .get("updateDuration")
                            .and_then(|value| value.as_u64())
                            .map(|value| value as u32),
                        default_dimension: str_of(form, "defaultDimension"),
                        support_dimensions: form
                            .get("supportDimensions")
                            .and_then(|value| value.as_array())
                            .map(|dimensions| {
                                dimensions
                                    .iter()
                                    .filter_map(|dimension| dimension.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The `backup_config.json` a module ships, in toml form.
fn backup_config(profile_dir: &Path) -> Option<BackupConfig> {
    let json = read_json5(&profile_dir.join("backup_config.json")).ok()?;
    Some(BackupConfig {
        allow_to_backup_restore: bool_of(&json, "allowToBackupRestore"),
    })
}

/// A string field, defaulting the same way the manifest's own defaults do.
fn str_of(json: &Value, field: &str) -> String {
    json.get(field)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn bool_of(json: &Value, field: &str) -> bool {
    json.get(field)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// The routes a module's `route_map.json` declares.
///
/// `page` and `build_function` are kept only when they cannot be derived from the name: the
/// build matches a route name against each file's `@Builder export function` (case-insensitive,
/// underscore-sensitive) and its stem. That resolves both fields for thirteen of clashbox's
/// nineteen routes; `LogDetails` keeps `page` and `EditArrayConfig` keeps `build_function`
/// because one of the two disagrees with the name, and `route-address`, `compatible`,
/// `access_control` and `online_app_search` keep both because neither matches.
fn route_defs(profile_dir: &Path) -> Vec<RouterMapDef> {
    let Ok(json) = read_json5(&profile_dir.join("route_map.json")) else {
        return Vec::new();
    };
    json.get("routerMap")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let name = entry.get("name").and_then(|value| value.as_str())?;
                    let build_function = entry
                        .get("buildFunction")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    let page = entry
                        .get("pageSourceFile")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    // The stem of the builder, without the `Builder` suffix.
                    let derived_builder = build_function
                        .strip_suffix("Builder")
                        .unwrap_or(build_function);
                    let derive_builder = derived_builder.eq_ignore_ascii_case(name);
                    let derive_page = page
                        .rsplit('/')
                        .next()
                        .and_then(|file| file.strip_suffix(".ets"))
                        .is_some_and(|stem| stem.eq_ignore_ascii_case(name));
                    Some(RouterMapDef {
                        name: name.to_string(),
                        page: (!derive_page && !page.is_empty())
                            .then(|| page.trim_start_matches("src/main/ets/").to_string()),
                        build_function: (!derive_builder && !build_function.is_empty())
                            .then(|| build_function.to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The `main` file a module declares in its own `oh-package.json5`.
fn module_main(module_root: &Path) -> Option<String> {
    let json = read_json5(&module_root.join("oh-package.json5")).ok()?;
    json.get("main")
        .and_then(|value| value.as_str())
        .map(|main| main.trim_start_matches("./").to_string())
        // Entry modules declare `main: ""`; an empty string is not a file name.
        .filter(|main| !main.is_empty())
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
        // The route names this module registers, and the file the device addresses it by.
        // Both live in hvigor manifests today (`route_map.json`, `oh-package.json5`) and are
        // folded into `hvolt.toml` so the migrated project carries no json config at all.
        let profile_dir = module_root.join("src/main/resources/base/profile");
        let router_map = route_defs(&profile_dir);
        let main = module_main(&module_root);
        let forms = form_defs(&profile_dir);
        let backup = backup_config(&profile_dir);

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
                                resource: x
                                    .get("resource")
                                    .and_then(|r| r.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                value: x
                                    .get("value")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
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
        for line in out_of_module_cmake_refs(&module_root) {
            gaps.push(format!(
                "{dir}: cpp/CMakeLists.txt may reference paths outside the module ({line}) — the hvolt layout does not preserve them; hvolt does not rewrite CMake files"
            ));
        }
        let cfg = ModuleConfig {
            // Module-scoped deps come from the source module's own oh-package.json5.
            dependencies: module_dependencies(&dir, &from.join(&dir)),
            module: ModuleInfo {
                name,
                routes,
                // Route names only: the page and its `@Builder` are derived by the build.
                router_map,
                main,
                forms,
                backup,
                // An imported module keeps its own visual root, if it declares one.
                ace_super_visual_path: mj
                    .get("aceSuperVisualPath")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim_start_matches("./").to_string())
                    .filter(|s| !s.is_empty()),
                // The module's own entry file, kept as declared: the device loads it as the
                // application entry point, and deriving it from the main ability is not the
                // same thing when a project declares a separate stage file.
                src_entry: mj
                    .get("srcEntry")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim_start_matches("./").to_string())
                    .filter(|s| !s.is_empty()),
                entry,
                skills,
                metadata: Vec::new(),
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
/// CMakeLists lines that point outside the module with a relative path.
///
/// The hvolt layout guarantees nothing about paths above `<member>/`, and hvolt does not rewrite
/// CMake files, so an escaping reference will only surface as a configure error later. Careful
/// wording: a relative path may legitimately resolve inside the source tree, so this is a
/// "may", not a verdict.
fn out_of_module_cmake_refs(module_root: &Path) -> Vec<String> {
    let cmake = module_root.join("cpp/CMakeLists.txt");
    let Ok(text) = fs::read_to_string(&cmake) else {
        return Vec::new();
    };
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with('#'))
        .filter(|l| l.contains("../..") || l.contains("${CMAKE_CURRENT_SOURCE_DIR}/../.."))
        .map(|l| l.chars().take(120).collect::<String>())
        .take(5)
        .collect()
}

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

/// Local `file:` dependencies declared by a module's own oh-package.json5.
///
/// The recorded path must be relative to the assembled build tree, not to the module: assemble
/// writes each module dependency as `file:./../<path>` from inside `build/<member>/`, so a bare
/// `src/main/cpp/types/<name>` would land at `build/src/main/...` instead of
/// `build/<member>/src/main/...`. Paths that already escape the module (`../x`) stay as they are.
fn module_dependencies(member: &str, module_dir: &Path) -> BTreeMap<String, Dependency> {
    let pkg = module_dir.join("oh-package.json5");
    let mut deps = BTreeMap::new();
    if !pkg.is_file() {
        return deps;
    }
    let Ok(v) = read_json5(&pkg) else {
        return deps;
    };
    let Some(map) = v.get("dependencies").and_then(|d| d.as_object()) else {
        return deps;
    };
    for (name, spec) in map {
        if let Some(path) = spec.as_str().and_then(|s| s.strip_prefix("file:")) {
            let cleaned = path.trim_start_matches("./");
            let recorded = if cleaned.starts_with("..") {
                cleaned.to_string()
            } else {
                format!("{}/{}", member, cleaned)
            };
            deps.insert(
                name.clone(),
                Dependency {
                    path: Some(recorded),
                    version: Some("1.0.0".to_string()),
                    ..Dependency::default()
                },
            );
        }
    }
    deps
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
                "ability source {ability} is declared in module.json5 but not found under ets/ — the generated srcEntry cannot resolve"
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
            "{}: test sources ({}) are not part of the hvolt layout and no ohosTest target is generated — they stay in the source project",
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

/// The `profile/` manifests the build derives, so a migrated project does not carry them.
///
/// Each of these is generated from `hvolt.toml` now: `main_pages.json` and `route_map.json`
/// from the module's `routes` and `router_map`, and the form/backup manifests from their own
/// tables. Copying the source files would leave two sources of truth, and the toml would
/// silently lose to whichever the build read second.
const DERIVED_PROFILES: &[&str] = &[
    "route_map.json",
    "main_pages.json",
    "form_config.json",
    "backup_config.json",
];

/// Copy a module's resource tree, leaving the derived `profile/` manifests behind.
fn copy_resources(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }
    copy_resources_inner(src, dst)
}

fn copy_resources_inner(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create {}: {}", dst.display(), e))?;
    for entry in fs::read_dir(src)
        .map_err(|e| format!("read {}: {}", src.display(), e))?
        .flatten()
    {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            copy_resources_inner(&path, &dst.join(&name))?;
            continue;
        }
        // Any `profile/` level, any theme: `dark/profile/main_pages.json` is a derivation of
        // the same route list, not an independent file.
        let in_profile = path
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|parent| parent == "profile");
        if in_profile && DERIVED_PROFILES.iter().any(|derived| *derived == name_str) {
            continue;
        }
        fs::copy(&path, dst.join(&name)).map_err(|e| format!("copy {}: {}", path.display(), e))?;
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
        "# Written by `hvolt migrate` — review members, products and signing before building.\n\n";
    fs::write(out.join("hvolt.toml"), format!("{header}{body}"))
        .map_err(|e| format!("write hvolt.toml: {}", e))
}

fn write_module_toml(dir: &Path, cfg: &ModuleConfig) -> Result<(), String> {
    let body = toml::to_string(cfg).map_err(|e| format!("serialize module hvolt.toml: {}", e))?;
    let header = "# Written by `hvolt migrate`.\n\n";
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

/// `dependencies` of the source project → `[dependencies]`, with local payloads copied in.
///
/// Registry entries keep their declared range; `file:` entries carry the copied payload and a
/// `path`. Nothing is left for the build to rediscover from a manifest.
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
            // A registry dependency is a declaration, not a payload: the version range stays as
            // written and the package payload is resolved from `oh_modules` at build time. It
            // goes into the table so the build reads no hvigor manifest.
            deps.insert(
                name.clone(),
                Dependency {
                    path: None,
                    version: Some(s.to_string()),
                    ..Dependency::default()
                },
            );
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
                path: Some(rel.to_string()),
                version: Some("1.0.0".to_string()),
                ..Dependency::default()
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
    use crate::support::config::SigningConfig;

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

    /// The compatible SDK level keeps its "(NN)" suffix: older SDKs require it.
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

#[cfg(test)]
mod cmake_ref_tests {
    use super::out_of_module_cmake_refs;
    use std::fs;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hvolt-cmake-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("cpp")).unwrap();
        p
    }

    /// An escaping relative path is reported (hedged), because the flat layout cannot preserve it.
    #[test]
    fn escaping_relative_paths_are_reported() {
        let root = scratch("escape");
        fs::write(
            root.join("cpp/CMakeLists.txt"),
            "include_directories(${CMAKE_CURRENT_SOURCE_DIR}/../../.lycium-repo/usr/include)\n",
        )
        .unwrap();
        let refs = out_of_module_cmake_refs(&root);
        assert_eq!(refs.len(), 1, "{:?}", refs);
        let _ = fs::remove_dir_all(&root);
    }

    /// A self-contained CMakeLists produces no noise.
    #[test]
    fn in_module_paths_are_silent() {
        let root = scratch("inside");
        fs::write(
            root.join("cpp/CMakeLists.txt"),
            "include_directories(${CMAKE_CURRENT_SOURCE_DIR}/include)\n",
        )
        .unwrap();
        assert!(out_of_module_cmake_refs(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod module_dependency_path_tests {
    use super::{module_dependencies, read_app_config};
    use std::fs;

    fn module_with_dep(tag: &str, spec: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("hvolt-moddep-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("oh-package.json5"),
            format!(
                "{{\n  \"dependencies\": {{ \"libentry.so\": \"file:{}\" }}\n}}\n",
                spec
            ),
        )
        .unwrap();
        dir
    }

    /// A module-local cpp dependency is recorded against the build tree, so assemble's
    /// `file:./../<path>` lands inside `build/<member>/` rather than beside it.
    #[test]
    fn module_local_path_gets_the_member_prefix() {
        let dir = module_with_dep("local", "./src/main/cpp/types/libentry");
        let deps = module_dependencies("entry", &dir);
        assert_eq!(
            deps["libentry.so"].path.as_deref(),
            Some("entry/src/main/cpp/types/libentry")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Paths that already leave the module keep their meaning.
    #[test]
    fn escaping_path_is_left_alone() {
        let dir = module_with_dep("escape", "../shared");
        let deps = module_dependencies("entry", &dir);
        assert_eq!(deps["libentry.so"].path.as_deref(), Some("../shared"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A DevEco project whose build-profile declares products and signing configs.
    fn dev_project(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "hvolt-import-products-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("AppScope")).unwrap();
        fs::create_dir_all(root.join("signing/debug")).unwrap();
        // A config whose materials are absent on this machine is dropped by the import, so the
        // fixtures have to exist for the product's `signingConfig` to resolve.
        for name in [
            "default.p12",
            "default.cer",
            "default.p7b",
            "release.p12",
            "release.cer",
            "release.p7b",
        ] {
            fs::write(root.join("signing/debug").join(name), "").unwrap();
        }
        fs::write(
            root.join("AppScope/app.json5"),
            r#"{"app":{"bundleName":"org.example.app","versionCode":1000000,"versionName":"1.0.0","vendor":"example"}}"#,
        )
        .unwrap();
        root
    }

    /// `app.products[]` becomes `[products.<name>]`: identity overrides, resource directories,
    /// the linked signing config, and a gap for what hvolt cannot express per product.
    #[test]
    fn build_profile_products_become_product_tables() {
        let root = dev_project("full");
        fs::write(
            root.join("build-profile.json5"),
            r#"{
              "app": {
                "signingConfigs": [
                  {
                    "name": "default",
                    "material": {
                      "storeFile": "signing/debug/default.p12",
                      "storePassword": "plain",
                      "keyAlias": "debugKey",
                      "keyPassword": "plain",
                      "certpath": "signing/debug/default.cer",
                      "profile": "signing/debug/default.p7b",
                      "signAlg": "SHA256withECDSA"
                    }
                  },
                  {
                    "name": "releaseSign",
                    "material": {
                      "storeFile": "signing/debug/release.p12",
                      "storePassword": "plain",
                      "keyAlias": "debugKey",
                      "keyPassword": "plain",
                      "certpath": "signing/debug/release.cer",
                      "profile": "signing/debug/release.p7b",
                      "signAlg": "SHA256withECDSA"
                    }
                  }
                ],
                "products": [
                  { "name": "default", "signingConfig": "default" },
                  {
                    "name": "release",
                    "bundleName": "org.example.app.release",
                    "versionName": "2.0.0",
                    "versionCode": 2000000,
                    "signingConfig": "releaseSign",
                    "resource": { "directories": ["app-resources", "res/release"] },
                    "targets": [{ "name": "default" }],
                    "compileSdkVersion": "26.0.0"
                  }
                ]
              }
            }"#,
        )
        .unwrap();

        let mut gaps = Vec::new();
        let cfg = read_app_config(&root, &mut gaps).expect("import build-profile");
        assert_eq!(cfg.app.bundle, "org.example.app");
        assert_eq!(cfg.app.products, ["default", "release"]);
        assert!(cfg.default_product.is_none(), "first product is default");

        let release = cfg.products.get("release").expect("release product table");
        assert_eq!(
            release.app.bundle.as_deref(),
            Some("org.example.app.release")
        );
        assert_eq!(release.app.version.as_deref(), Some("2.0.0"));
        assert_eq!(release.app.version_code, Some(2000000));
        assert_eq!(
            release.resources.directories,
            ["app-resources", "res/release"]
        );
        // The linked config is carried as the fields that differ from the inherited default.
        let signing = release.signing.as_ref().expect("release signing override");
        assert_eq!(
            signing.store_file.as_deref(),
            Some("signing/debug/release.p12")
        );
        assert_eq!(signing.key_alias, None, "equal field inherits");
        assert!(
            gaps.iter()
                .any(|gap| gap.contains("modules[].targets") && gap.contains("release")),
            "{gaps:?}"
        );
        assert!(
            gaps.iter()
                .any(|gap| gap.contains("compileSdkVersion") && gap.contains("release")),
            "{gaps:?}"
        );

        // The written config resolves to the source's identity and materials.
        let effective = cfg.with_product("release").expect("release resolves");
        assert_eq!(effective.app.bundle, "org.example.app.release");
        assert_eq!(effective.app.version, "2.0.0");
        assert_eq!(effective.app.version_code(), 2_000_000);
        assert_eq!(
            effective.app.signing_for("release").unwrap().store_file,
            "signing/debug/release.p12"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// The first product decides the default, and a product that changes nothing gets no
    /// override table (the inheritance carries it).
    #[test]
    fn the_first_product_becomes_default_product() {
        let root = dev_project("first");
        fs::write(
            root.join("build-profile.json5"),
            r#"{
              "app": {
                "products": [
                  { "name": "free", "bundleName": "org.example.app" },
                  { "name": "paid", "bundleName": "org.example.app.paid" }
                ]
              }
            }"#,
        )
        .unwrap();

        let mut gaps = Vec::new();
        let cfg = read_app_config(&root, &mut gaps).expect("import build-profile");
        assert_eq!(cfg.default_product.as_deref(), Some("free"));
        assert_eq!(cfg.product_names(), ["default", "free", "paid"]);
        let free = cfg.products.get("free").expect("free product");
        assert!(free.app.bundle.is_none(), "identical to the root value");
        assert!(free.app.version.is_none());
        assert!(free.app.version_code.is_none());
        assert!(free.signing.is_none());
        let paid = cfg.products.get("paid").expect("paid product");
        assert_eq!(paid.app.bundle.as_deref(), Some("org.example.app.paid"));
        let _ = fs::remove_dir_all(&root);
    }
}
