// pack_info: render the HAP's pack.info from hvolt.toml.
//
// pack.info is the package-content description the installer reads for upgrades.
// The reference shape is the one the platform expects (verified against asr-ondevice's
// `outputs/default/pack.info`): a `summary` with the app and module digest, and a
// `packages` array with one entry per module×product.

use serde_json::{json, Value};

use crate::build::artifacts::module_json;
use crate::build::artifacts::module_json::RenderInput;
use crate::support::config::{AppConfig, ModuleConfig};

/// What pack.info needs beyond the shared manifest inputs: the product it is
/// packed for (the `name` field is `<module>-<product>`).
pub struct PackInput<'a> {
    /// Ark bytecode version for this project's compatible API level.
    pub virtual_machine: &'a str,
    /// Mirrors `module.json`: the module declares at least one `@InsightIntent*`.
    pub has_insight_intent: bool,
    pub module_name: &'a str,
    pub module: &'a ModuleConfig,
    pub app: &'a AppConfig,
    pub product: &'a str,
    pub build_mode: &'a str,
    pub compile_sdk_version: &'a str,
    pub api_release_type: &'a str,
    pub ids: &'a crate::build::compile::ets_resources::ResourceTable,
}

/// `summary.app.version`, with `build` only when the project declares one.
fn version_block(version: &crate::support::config::AppFields) -> serde_json::Value {
    let mut block = json!({
        "code": version.version_code(),
        "name": version.version,
    });
    if !version.build_version.is_empty() {
        block["build"] = json!(version.build_version);
    }
    block
}

/// Render pack.info for one module and product.
pub fn render(input: &PackInput<'_>) -> Value {
    // The module digest reuses the same config expansion as module.json: entry stem,
    // device types, abilities, and the distro fields all live in the same place.
    let manifest = module_json::render(&RenderInput {
        app: input.app,
        module: input.module,
        ids: input.ids,
        build_mode: input.build_mode,
        compile_sdk_version: input.compile_sdk_version,
        api_release_type: input.api_release_type,
        virtual_machine: input.virtual_machine,
        has_insight_intent: input.has_insight_intent,
        // pack.info's digest reads abilities/mainAbility/label; the route table does not
        // reach it.
        has_route_map: false,
    });
    let manifest_module = &manifest["module"];

    // `mainAbility` is the first ability in the manifest.
    let main_ability = manifest_module["abilities"][0]["name"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // abilities digest: name + label only.
    let abilities: Vec<Value> = manifest_module["abilities"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| json!({ "name": a["name"], "label": a["label"] }))
                .collect()
        })
        .unwrap_or_default();

    let module_type = manifest_module["type"].as_str().unwrap_or("entry");
    let device_types = manifest_module["deviceTypes"].clone();
    let installation_free = manifest_module["installationFree"]
        .as_bool()
        .unwrap_or(false);
    let delivery = manifest_module["deliveryWithInstall"]
        .as_bool()
        .unwrap_or(true);

    let version = &input.app.app;
    json!({
        "summary": {
            "app": {
                "bundleName": version.bundle,
                "bundleType": version.bundle_type,
                "version": version_block(version),
            },
            "modules": [
                {
                    "mainAbility": main_ability,
                    "deviceType": device_types,
                    "abilities": abilities,
                    "distro": {
                        "moduleType": module_type,
                        "installationFree": installation_free,
                        "deliveryWithInstall": delivery,
                        "moduleName": input.module_name,
                    },
                    // pack.info carries the plain API levels; the encoded forms live in
                    // module.json.
                    "apiVersion": {
                        "compatible": version.compatible_api_level(),
                        "releaseType": input.api_release_type,
                        "target": version.target_api_level(),
                    }
                }
            ]
        },
        "packages": [
            {
                "deviceType": manifest["module"]["deviceTypes"],
                "moduleType": module_type,
                "deliveryWithInstall": delivery,
                "name": format!("{}-{}", input.module_name, input.product),
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::config::{ModuleInfo, Skill};

    fn app() -> AppConfig {
        let mut app = AppConfig::default_for("asr_ondevice");
        app.app.runtime_os = "OpenHarmony".into();
        app.app.compile_sdk_version = "26.0.0".into();
        app.app.compatible_sdk_version = "26.0.0".into();
        app
    }

    fn module() -> ModuleConfig {
        ModuleConfig {
            module: ModuleInfo {
                router_map: Vec::new(),
                main: None,
                forms: Vec::new(),
                backup: None,
                src_entry: None,
                name: "src".into(),
                entry: "EntryAbility.ets".into(),
                device_types: vec!["default".into(), "tablet".into(), "2in1".into()],
                skills: vec![Skill {
                    actions: vec!["action.system.home".into()],
                    entities: vec!["entity.system.home".into()],
                    uris: vec![],
                    permissions: vec![],
                    domain_verify: None,
                }],
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
            dependencies: std::collections::BTreeMap::new(),
        }
    }

    fn ids() -> crate::build::compile::ets_resources::ResourceTable {
        crate::build::compile::ets_resources::parse_table(
            "\
string app_name 0x01000000
media layered_image 0x01000003
string module_desc 0x01000008
string EntryAbility_desc 0x01000005
string EntryAbility_label 0x01000006
color start_window_background 0x01000009
",
        )
    }

    /// The summary digest names the main ability, device types, distro and API
    /// version — the fields the installer checks before an upgrade.
    #[test]
    fn the_summary_carries_the_installer_fields() {
        let a = app();
        let m = module();
        let ids = ids();
        let out = render(&PackInput {
            has_insight_intent: false,
            virtual_machine: "",
            module_name: "src",
            module: &m,
            app: &a,
            product: "default",
            build_mode: "release",
            compile_sdk_version: "26.0.0.23",
            api_release_type: "Beta1",
            ids: &ids,
        });
        assert_eq!(
            out["summary"]["app"]["bundleName"],
            "com.example.asr_ondevice"
        );
        assert_eq!(out["summary"]["app"]["version"]["code"], 1000000);
        assert_eq!(out["summary"]["modules"][0]["mainAbility"], "EntryAbility");
        assert_eq!(out["summary"]["modules"][0]["distro"]["moduleName"], "src");
        assert_eq!(out["summary"]["modules"][0]["apiVersion"]["compatible"], 26);
        assert_eq!(out["summary"]["modules"][0]["apiVersion"]["target"], 26);
    }

    /// The package name is `<module>-<product>`.
    #[test]
    fn the_package_name_joins_module_and_product() {
        let a = app();
        let m = module();
        let ids = ids();
        let out = render(&PackInput {
            has_insight_intent: false,
            virtual_machine: "",
            module_name: "src",
            module: &m,
            app: &a,
            product: "default",
            build_mode: "release",
            compile_sdk_version: "26.0.0.23",
            api_release_type: "Beta1",
            ids: &ids,
        });
        assert_eq!(out["packages"][0]["name"], "src-default");
        assert_eq!(out["packages"][0]["moduleType"], "entry");
    }
}
