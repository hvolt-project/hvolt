// module_json: render the runtime module.json from hvolt.toml, standalone.
//
// The runtime manifest goes through three stages upstream (process_profile →
// merge_profile → package) before packing. The final file is what the installer reads, so
// this module renders that final shape directly from the same config hvolt.toml already
// carries: AppConfig (root) + ModuleConfig (module) + the restool id table for *Id fields.
//
// The reference for every field below is the packaged module.json that the
// produces for the same project; the tests pin the diff against it.

use serde_json::{json, Value};

use crate::build::compile::ets_resources::ResourceTable;
use crate::support::config::{AbilityConfig, AppConfig, ModuleConfig, ModuleInfo, Skill};

/// One module's build: the module config plus the root app config that scopes it.
pub struct RenderInput<'a> {
    pub app: &'a AppConfig,
    pub module: &'a ModuleConfig,
    /// restool's id table, used to turn `$string:...` refs into *Id fields.
    pub ids: &'a ResourceTable,
    /// "debug" or "release" — becomes `app.buildMode` and `app.debug`.
    pub build_mode: &'a str,
    /// Full four-part SDK version, e.g. "26.0.0.23".
    pub compile_sdk_version: &'a str,
    /// SDK release type, e.g. "Beta1".
    pub api_release_type: &'a str,
    /// The ark bytecode version for this project's compatible API level, as the SDK's own
    /// `ts2abc.js --target-api-version <n>` reports it (e.g. "ark12.0.6.0").
    pub virtual_machine: &'a str,
    /// Whether the module declares any `@InsightIntent*` decorator: the installer reads
    /// `hasInsightIntent` to decide whether to look for the generated manifest.
    pub has_insight_intent: bool,
    /// Whether the module ships a `resources/base/profile/route_map.json`.
    ///
    /// A `NavPathStack` route resolves through this table. Without the `routerMap` field the
    /// installer never loads it and `pushPathByName('ImportConfigFromURL')` reports
    /// `can't find name in config file` while still creating an empty node — measured on
    /// clashbox 2026-11-08, where the packaged `route_map.json` was present but unreferenced.
    pub has_route_map: bool,
}

/// Resolve `$string:name` / `$media:name` / `$color:name` / `$profile:name` to its
/// numeric id via the table; `None` when the resource is missing (caller decides).
fn res_id(ids: &ResourceTable, reference: &str) -> Option<u32> {
    let rest = reference.strip_prefix('$')?;
    let (kind, name) = rest.split_once(':')?;
    ids.get(&(kind.to_string(), name.to_string())).copied()
}

/// The `app.*` block.
fn app_block(
    app: &AppConfig,
    ids: &ResourceTable,
    build_mode: &str,
    compile_sdk_version: &str,
    api_release_type: &str,
) -> Value {
    let appf = &app.app;
    let debug = build_mode == "debug";
    let mut app_json = json!({
        "bundleName": appf.bundle,
        "vendor": appf.vendor,
        "versionCode": appf.version_code(),
        "versionName": appf.version,
        "icon": appf.icon,
        "label": "$string:app_name",
        "apiReleaseType": api_release_type,
        "compileSdkVersion": compile_sdk_version,
        "targetAPIVersion": appf.target_api_version(),
        "minAPIVersion": appf.min_api_version(),
        "compileSdkType": appf.compile_sdk_type(),
        "targetMinorAPIVersion": 0,
        "targetPatchAPIVersion": 0,
        "minMinorAPIVersion": 0,
        "minPatchAPIVersion": 0,
        "appEnvironments": [],
        "bundleType": appf.bundle_type,
        "buildMode": build_mode,
        "debug": debug,
        "iconId": res_id(ids, &appf.icon),
        "labelId": res_id(ids, "$string:app_name"),
    });
    // hvigor records a build version only when the project declares one; the key is absent
    // otherwise, and an explicit `null` still counts as a different document.
    if !appf.build_version.is_empty() {
        app_json["buildVersion"] = json!(appf.build_version);
    }
    app_json
}

/// What one ability contributes to the manifest, already resolved from config.
struct AbilitySpec<'a> {
    name: &'a str,
    src_entry: String,
    icon: &'a str,
    label: &'a str,
    description: Option<&'a str>,
    start_window_icon: &'a str,
    start_window_background: &'a str,
    skills: &'a [Skill],
}

/// One ability entry: name, source path, resource references and their ids.
fn ability_block(spec: &AbilitySpec<'_>, ids: &ResourceTable, runtime_os: &str) -> Value {
    let mut out = json!({
        "name": spec.name,
        "srcEntry": spec.src_entry,
        "icon": spec.icon,
        "label": spec.label,
        "startWindowIcon": spec.start_window_icon,
        "startWindowBackground": spec.start_window_background,
        "exported": true,
    });
    if let Some(desc) = spec.description {
        out["description"] = json!(desc);
        out["descriptionId"] = json!(res_id(ids, desc));
    }
    if spec.skills.is_empty() {
        // A HarmonyOS build declares the home skill under `action.system.home`; the
        // OpenHarmony variant is the one that carries the `ohos.want.` prefix.
        let home_action = if runtime_os.eq_ignore_ascii_case("openharmony") {
            "ohos.want.action.home"
        } else {
            "action.system.home"
        };
        out["skills"] = json!(vec![skill_block(&Skill {
            actions: vec![home_action.into()],
            entities: vec!["entity.system.home".into()],
            uris: vec![],
            permissions: vec![],
            domain_verify: None,
        })]);
    } else {
        out["skills"] = json!(spec.skills.iter().map(skill_block).collect::<Vec<_>>());
    }
    out["iconId"] = json!(res_id(ids, spec.icon));
    out["labelId"] = json!(res_id(ids, spec.label));
    out["startWindowIconId"] = json!(res_id(ids, spec.start_window_icon));
    out["startWindowBackgroundId"] = json!(res_id(ids, spec.start_window_background));
    out
}

fn skill_block(s: &Skill) -> Value {
    let mut out = json!({});
    if !s.actions.is_empty() {
        out["actions"] = json!(s.actions);
    }
    if !s.entities.is_empty() {
        out["entities"] = json!(s.entities);
    }
    if !s.uris.is_empty() {
        // Only the fields the uri actually sets: an explicit `null` is rejected by the
        // installer's manifest parser (ERR_INSTALL_PARSE_PROFILE_PROP_TYPE_ERROR).
        out["uris"] = json!(s
            .uris
            .iter()
            .map(|u| {
                let mut o = serde_json::Map::new();
                if !u.scheme.is_empty() {
                    o.insert("scheme".into(), json!(u.scheme));
                }
                if let Some(v) = u.host.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("host".into(), json!(v));
                }
                if let Some(v) = u.port.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("port".into(), json!(v));
                }
                if let Some(v) = u.path.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("path".into(), json!(v));
                }
                if let Some(v) = u.path_start_with.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("pathStartWith".into(), json!(v));
                }
                if let Some(v) = u.path_regex.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("pathRegex".into(), json!(v));
                }
                if let Some(v) = u.mime_type.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("type".into(), json!(v));
                }
                if let Some(v) = u.utd.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("utd".into(), json!(v));
                }
                if let Some(v) = u.max_file_supported {
                    o.insert("maxFileSupported".into(), json!(v));
                }
                if let Some(v) = u.link_feature.as_ref().filter(|v| !v.is_empty()) {
                    o.insert("linkFeature".into(), json!(v));
                }
                Value::Object(o)
            })
            .collect::<Vec<_>>());
    }
    out
}

/// `srcEntry` for an ability: `<source root>/<dir>/<file>`, spelled the way the device
/// resolves it. An entry that already names its root keeps it verbatim.
fn ability_src_entry(dir: &str, entry: &str) -> String {
    let entry = entry.trim_start_matches("./");
    if entry.starts_with("ets/") || entry.starts_with("src/") {
        return format!("./{entry}");
    }
    let dir = dir.trim_matches('/');
    if dir.is_empty() {
        format!("./ets/{entry}")
    } else {
        format!("./ets/{dir}/{entry}")
    }
}

/// The module entry ability: name from the `entry` file stem, `srcEntry` as the flat
/// layout the build actually compiles (`./EntryAbility.ets` beside the toml).
fn entry_ability(
    info: &ModuleInfo,
    app_icon: &str,
    ids: &ResourceTable,
    runtime_os: &str,
) -> Value {
    let stem = std::path::Path::new(&info.entry)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&info.entry)
        .to_string();
    // The app icon is the fallback when the module declares none.
    let icon = if info.icon.is_empty() {
        app_icon
    } else {
        &info.icon
    };
    let label = if info.entry_label.is_empty() {
        "$string:EntryAbility_label"
    } else {
        &info.entry_label
    };
    let description = Some(if info.entry_description.is_empty() {
        "$string:EntryAbility_desc"
    } else {
        &info.entry_description
    });
    // The start window uses the module's own `startIcon`; falling back to the app icon would
    // point the splash screen at a resource the module scope does not own.
    let start_icon = if info.start_window_icon.is_empty() {
        "$media:startIcon"
    } else {
        &info.start_window_icon
    };
    ability_block(
        &AbilitySpec {
            name: &stem,
            src_entry: ability_src_entry("", &info.entry),
            icon,
            label,
            description,
            start_window_icon: start_icon,
            start_window_background: "$color:start_window_background",
            skills: &info.skills,
        },
        ids,
        runtime_os,
    )
}

/// Additional abilities from `[module.abilities]`.
fn extra_abilities(
    list: &[AbilityConfig],
    info: &ModuleInfo,
    ids: &ResourceTable,
    runtime_os: &str,
) -> Vec<Value> {
    let default_icon = if info.icon.is_empty() { "" } else { &info.icon };
    list.iter()
        .filter(|a| a.ability_type.is_empty())
        .map(|a| {
            let label = if a.label.is_empty() { "" } else { &a.label };
            let description = if a.description.is_empty() {
                None
            } else {
                Some(a.description.as_str())
            };
            ability_block(
                &AbilitySpec {
                    name: &a.name,
                    src_entry: ability_src_entry(&a.dir, &a.entry),
                    icon: default_icon,
                    label,
                    description,
                    start_window_icon: default_icon,
                    start_window_background: "$color:start_window_background",
                    skills: &a.skills,
                },
                ids,
                runtime_os,
            )
        })
        .collect()
}

/// `[module.abilities]` entries whose `type` marks them as ExtensionAbilities.
fn extension_abilities(list: &[AbilityConfig], info: &ModuleInfo) -> Vec<Value> {
    let _ = info;
    list.iter()
        .filter(|a| !a.ability_type.is_empty())
        .map(|a| {
            let mut o = serde_json::Map::new();
            o.insert("name".into(), json!(a.name));
            // `srcEntry` is resolved by the device as `<module>/<module source root>/<srcEntry>`,
            // and every other module-internal address carries that same source root. The
            // entry declaration therefore has to spell it out too, or the entry record and
            // the records its imports name diverge.
            o.insert(
                "srcEntry".into(),
                json!(ability_src_entry(&a.dir, &a.entry)),
            );
            o.insert("type".into(), json!(a.ability_type));
            if let Some(exported) = a.exported {
                o.insert("exported".into(), json!(exported));
            }
            if !a.metadata.is_empty() {
                o.insert(
                    "metadata".into(),
                    json!(a
                        .metadata
                        .iter()
                        .map(|m| {
                            let mut mo = serde_json::Map::new();
                            mo.insert("name".into(), json!(m.name));
                            if !m.resource.is_empty() {
                                mo.insert("resource".into(), json!(m.resource));
                            }
                            Value::Object(mo)
                        })
                        .collect::<Vec<_>>()),
                );
            }
            Value::Object(o)
        })
        .collect()
}

fn permissions_block(info: &ModuleInfo, ids: &ResourceTable) -> Vec<Value> {
    info.permissions
        .iter()
        .map(|p| {
            let mut out = json!({
                "name": p.name,
            });
            if !p.reason.is_empty() {
                out["reason"] = json!(p.reason);
                out["reasonId"] = json!(res_id(ids, &p.reason));
            }
            if !p.abilities.is_empty() {
                out["usedScene"] = json!({
                    "abilities": p.abilities,
                    "when": p.when,
                });
            }
            out
        })
        .collect()
}

/// Render the complete module.json for one module.
pub fn render(input: &RenderInput<'_>) -> Value {
    let appf = &input.app.app;
    let info = &input.module.module;
    let runtime_os = &appf.runtime_os;
    let mut abilities = vec![entry_ability(info, &appf.icon, input.ids, runtime_os)];
    abilities.extend(extra_abilities(
        &info.abilities,
        info,
        input.ids,
        runtime_os,
    ));
    // An ability carrying a `type` (vpn, backup, form, …) is an ExtensionAbility: the
    // installer validates it against the profile, and declaring it as a UIAbility fails
    // with ERR_INSTALL_PARSE_PROFILE_PROP_TYPE_ERROR.
    let extensions = extension_abilities(&info.abilities, info);
    let main_element = std::path::Path::new(&info.entry)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&info.entry);
    let mut module = json!({
        "name": info.name,
        "type": if info.module_type.is_empty() { "entry" } else { &info.module_type },
        "mainElement": main_element,
        "pages": "$profile:main_pages",
        "deviceTypes": info.device_types,
        "deliveryWithInstall": info.delivery_with_install.unwrap_or(true),
        "installationFree": info.installation_free.unwrap_or(false),
        "abilities": abilities,
        "packageName": info.name,
        "virtualMachine": input.virtual_machine,
        "compileMode": "esmodule",
        "dependencies": Vec::<Value>::new(),
    });
    if input.has_insight_intent {
        module["hasInsightIntent"] = json!(true);
    }
    if input.has_route_map {
        // The installer loads a route table only through this reference; the packaged
        // `route_map.json` alone is inert (`ImportConfigFromURL`/`ImportConfigFromFile` both
        // resolve through it).
        module["routerMap"] = json!("$profile:route_map");
    }
    if let Some(src) = &info.src_entry {
        // The device loads this file as the application entry point; without it the runtime
        // derives an entry from the main ability and never finds it.
        module["srcEntry"] = json!(ability_src_entry("", src));
    }
    if !extensions.is_empty() {
        module["extensionAbilities"] = json!(extensions);
    }
    if !info.metadata.is_empty() {
        module["metadata"] = json!(info
            .metadata
            .iter()
            .map(|m| {
                let mut out = json!({ "name": m.name });
                if !m.resource.is_empty() {
                    out["resource"] = json!(m.resource);
                }
                if !m.value.is_empty() {
                    out["value"] = json!(m.value);
                }
                out
            })
            .collect::<Vec<_>>());
    }
    let permissions = permissions_block(info, input.ids);
    if !permissions.is_empty() {
        module["requestPermissions"] = json!(permissions);
    }
    module["description"] = json!("$string:module_desc");
    if let Some(desc_id) = res_id(input.ids, "$string:module_desc") {
        module["descriptionId"] = json!(desc_id);
    }
    json!({
        "app": app_block(
            input.app,
            input.ids,
            input.build_mode,
            input.compile_sdk_version,
            input.api_release_type,
        ),
        "module": module,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn ids() -> ResourceTable {
        crate::build::compile::ets_resources::parse_table(
            "\
string app_name 0x01000000
media layered_image 0x01000003
string module_desc 0x01000004
string EntryAbility_desc 0x01000005
string EntryAbility_label 0x01000006
string mic_reason 0x01000007
color start_window_background 0x01000008
",
        )
    }

    fn input() -> (AppConfig, ModuleConfig) {
        let app = AppConfig::default_for("asr_ondevice");
        // ModuleInfo has no Default derive; build the one field set asr uses.
        let module = ModuleConfig {
            module: ModuleInfo {
                src_entry: None,
                name: "src".into(),
                entry: "EntryAbility.ets".into(),
                device_types: vec!["default".into(), "tablet".into(), "2in1".into()],
                skills: vec![],
                metadata: vec![],
                module_type: String::new(),
                abilities: vec![],
                permissions: vec![],
                obfuscation: false,
                cpp_flags: String::new(),
                native_arguments: String::new(),
                obfuscation_rules: String::new(),
                icon: String::new(),
                start_window_icon: String::new(),
                orientation: String::new(),
                entry_label: String::new(),
                entry_description: String::new(),
                delivery_with_install: None,
                installation_free: None,
                abi_filters: vec!["arm64-v8a".into()],
                routes: vec!["pages/Index.ets".into()],
                ace_super_visual_path: None,
            },
            dependencies: BTreeMap::new(),
        };
        (app, module)
    }

    /// The app block carries the encoded API version: HarmonyOS encodes major*1e7,
    /// OpenHarmony keeps the bare major.
    #[test]
    fn api_version_encoding_follows_runtime() {
        let (mut app, _) = input();
        app.app.compile_sdk_version = "26.0.0".into();
        app.app.runtime_os = "OpenHarmony".into();
        assert_eq!(app.app.target_api_version(), 26);
        app.app.runtime_os = "HarmonyOS".into();
        assert_eq!(app.app.target_api_version(), 260000000);
    }

    /// Every `$kind:name` reference in the manifest lands as a numeric *Id field
    /// when the resource table has the entry.
    #[test]
    fn resource_references_become_ids() {
        let (app, module) = input();
        let ids = ids();
        let out = render(&RenderInput {
            has_insight_intent: false,
            has_route_map: false,
            virtual_machine: "",
            app: &app,
            module: &module,
            ids: &ids,
            build_mode: "release",
            compile_sdk_version: "26.0.0.23",
            api_release_type: "Beta1",
        });
        assert_eq!(out["app"]["labelId"], 0x01000000);
        assert_eq!(out["module"]["descriptionId"], 0x01000004);
        let ability = &out["module"]["abilities"][0];
        assert_eq!(ability["descriptionId"], 0x01000005);
        assert_eq!(ability["labelId"], 0x01000006);
    }

    /// The srcEntry spells the module source root the device resolves against, not the
    /// `ets/entryability/` path the reference's translated tree uses.
    #[test]
    fn src_entry_is_flat() {
        let (app, module) = input();
        let ids = ids();
        let out = render(&RenderInput {
            has_insight_intent: false,
            has_route_map: false,
            virtual_machine: "",
            app: &app,
            module: &module,
            ids: &ids,
            build_mode: "release",
            compile_sdk_version: "26.0.0.23",
            api_release_type: "Beta1",
        });
        let ability = &out["module"]["abilities"][0];
        assert_eq!(ability["srcEntry"], "./ets/EntryAbility.ets");
        assert_eq!(out["module"]["mainElement"], "EntryAbility");
    }
}
