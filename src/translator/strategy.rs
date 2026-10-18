// Version strategy: generate version-specific HarmonyOS build artifacts.
//
// Generator holds a Box<dyn VersionStrategy> to handle API-level differences
// in app.json5, module.json5, build-profile.json5, and hvigor boilerplate.

use crate::config::{AppConfig, ModuleConfig, NativeModuleConfig};

/// API version → HarmonyOS version string mapping.
/// API 12–24 use "X.Y.Z(N)" format; API 26+ use plain version number.
pub fn api_to_version(api: u32) -> String {
    match api {
        12 => "5.0.0(12)".into(),
        13 => "5.0.1(13)".into(),
        14 => "5.0.2(14)".into(),
        15 => "5.0.3(15)".into(),
        16 => "5.0.4(16)".into(),
        17 => "5.0.5(17)".into(),
        18 => "5.1.0(18)".into(),
        19 => "5.1.1(19)".into(),
        20 => "6.0.0(20)".into(),
        21 => "6.0.1(21)".into(),
        22 => "6.0.2(22)".into(),
        23 => "6.1.0(23)".into(),
        24 => "6.1.1(24)".into(),
        26 => "26.0.0".into(),
        _ => format!("{}.0.0", api),
    }
}

/// Whether this API version is deprecated (< 26).
pub fn is_deprecated(api: u32) -> bool {
    api < 26
}

/// Strategy trait for version-specific artifact generation.
pub trait VersionStrategy {
    fn version_string(&self) -> String;
    fn sdk_version_string(&self) -> String; // for build-profile.json5
    fn compile_sdk_version_string(&self) -> String;
    fn hvigor_config(&self) -> &str;
    fn oh_package_root(&self, app: &AppConfig, native_module_names: &[String]) -> String;

    fn generate_app_json5(&self, config: &AppConfig) -> String;
    fn generate_root_build_profile(
        &self,
        modules: &[(String, ModuleConfig)],
        app: &AppConfig,
        native_module_names: &[String],
    ) -> String;
    fn generate_module_oh_package(
        &self,
        module: &crate::config::ModuleInfo,
        app_config: &AppConfig,
        native_modules: &[(String, NativeModuleConfig)],
    ) -> String;
    fn generate_module_json5(&self, module: &ModuleConfig, app_config: &AppConfig) -> String;
    fn generate_module_build_profile(
        &self,
        module: &crate::config::ModuleInfo,
        native_modules: &[(String, NativeModuleConfig)],
    ) -> String;
}

// ─── V12 Strategy (API 12–24, same format as V26) ───────────────

pub type V12Strategy = V26Strategy;

// ─── V26 Strategy (API 26+) ─────────────────────────────────────

pub struct V26Strategy {
    version: String,
}

impl V26Strategy {
    pub fn new(api: u32) -> Self {
        Self {
            version: api_to_version(api),
        }
    }
}

fn signing_json(app: &AppConfig) -> String {
    if app.app.signing.is_empty() {
        return String::new();
    }
    let items: Vec<String> = app
        .app
        .signing
        .iter()
        .map(|(name, s)| {
            let profile_line = if s.profile.is_empty() {
                String::new()
            } else {
                format!("        \"profile\": \"../{}\",\n", s.profile)
            };
            format!(
                r#"    {{
      "name": "{}",
      "type": "HarmonyOS",
      "material": {{
        "certpath": "../{}",
        "keyAlias": "{}",
        "keyPassword": "{}",
{}        "signAlg": "{}",
        "storeFile": "../{}",
        "storePassword": "{}"
      }}
    }}"#,
                name,
                s.certpath,
                s.key_alias,
                s.key_password,
                profile_line,
                s.sign_alg,
                s.store_file,
                s.store_password
            )
        })
        .collect();
    format!("  \"signingConfigs\": [\n{}\n  ],\n", items.join(",\n"))
}

impl VersionStrategy for V26Strategy {
    fn version_string(&self) -> String {
        self.version.clone()
    }

    fn sdk_version_string(&self) -> String {
        self.version.clone()
    }

    fn compile_sdk_version_string(&self) -> String {
        self.version.clone()
    }

    fn hvigor_config(&self) -> &str {
        r#"{
  "modelVersion": "26.0.0",
  "dependencies": {},
  "execution": {},
  "logging": {},
  "debugging": {},
  "nodeOptions": {}
}"#
    }

    fn oh_package_root(&self, app: &AppConfig, native_module_names: &[String]) -> String {
        let deps_block = if app.dependencies.is_empty() {
            r#""dependencies": {}"#.to_string()
        } else {
            let deps: Vec<String> = app
                .dependencies
                .iter()
                .filter(|(name, _dep)| !native_module_names.contains(name))
                .map(|(name, dep)| {
                    if dep.path.ends_with(".har") {
                        // HAR relative from build/ root: vendor/scrypt.har
                        format!(r#"    "{}": "file:{}""#, name, dep.path)
                    } else {
                        format!(r#"    "{}": "file:./{}""#, name, name)
                    }
                })
                .collect();
            if deps.is_empty() {
                r#""dependencies": {}"#.to_string()
            } else {
                format!(r#""dependencies": {{ {} }}"#, deps.join(",\n"))
            }
        };
        format!(
            r#"{{
  "modelVersion": "26.0.0",
  "description": "Please describe the basic information.",
  {},
  "devDependencies": {{
    "@ohos/hypium": "1.0.25",
    "@ohos/hamock": "1.0.0"
  }}
}}"#,
            deps_block
        )
    }

    fn generate_app_json5(&self, config: &AppConfig) -> String {
        let bundle_type_line = if config.app.bundle_type == "atomicService" {
            ",\n    \"bundleType\": \"atomicService\""
        } else {
            ""
        };
        format!(
            r#"{{
  "app": {{
    "bundleName": "{}",
    "vendor": "{}",
    "versionCode": {},
    "versionName": "{}",
    "buildVersion": "1",
    "icon": "{}",
    "label": "$string:app_name"{}
  }}
}}"#,
            config.app.bundle,
            config.app.vendor,
            config.app.version_code(),
            config.app.version,
            config.app.icon,
            bundle_type_line,
        )
    }
    fn generate_root_build_profile(
        &self,
        modules: &[(String, ModuleConfig)],
        app: &AppConfig,
        native_module_names: &[String],
    ) -> String {
        let entries: Vec<String> = modules
            .iter()
            .map(|(dir, _)| {
                let apply_products: Vec<String> = app
                    .app
                    .products
                    .iter()
                    .map(|p| format!("\"{}\"", p))
                    .collect();
                format!(
                    r#"    {{
      "name": "{}",
      "srcPath": "./{}",
      "targets": [
        {{
          "name": "default",
          "applyToProducts": [
            {}
          ]
        }}
      ]
    }}"#,
                    dir,
                    dir,
                    apply_products.join(",\n            ")
                )
            })
            .collect();

        // Add dependencies (HSPs) that are not hvolt-managed modules
        let declared: Vec<String> = modules.iter().map(|(d, _)| d.clone()).collect();
        let dep_entries: Vec<String> = app
            .dependencies
            .iter()
            .filter(|(name, dep)| {
                !dep.path.ends_with(".har")
                    && !declared.contains(name)
                    && !native_module_names.contains(name)
            })
            .map(|(name, dep)| {
                format!(
                    r#"    {{
      "name": "{}",
      "srcPath": "./{}"
    }}"#,
                    name, dep.path
                )
            })
            .collect();

        let all_entries: Vec<String> = entries.into_iter().chain(dep_entries).collect();

        let sv = self.sdk_version_string();
        let csv = self.compile_sdk_version_string();
        let compat_sdk = if app.app.compatible_sdk_version.is_empty() {
            &app.app.sdk_version
        } else {
            &app.app.compatible_sdk_version
        };
        let runtime_os = if app.app.runtime_os.is_empty() {
            "HarmonyOS"
        } else {
            app.app.runtime_os.as_str()
        };
        let signing = signing_json(app);

        let products: Vec<String> = app
            .app
            .products
            .iter()
            .map(|p| {
                let sc = if app.app.signing.contains_key(p.as_str()) {
                    format!("\n        \"signingConfig\": \"{}\",", p)
                } else {
                    String::new()
                };
                format!(
                    r#"      {{
        "name": "{}",{}
        "compileSdkVersion": "{csv}",
        "targetSdkVersion": "{sv}",
        "compatibleSdkVersion": "{compat_sdk}",
        "runtimeOS": "{runtime_os}",
        "buildOption": {{
          "nativeCompiler": "BiSheng",
          "strictMode": {{
            "caseSensitiveCheck": true,
            "useNormalizedOHMUrl": true
          }}
        }}
      }}"#,
                    p, sc
                )
            })
            .collect();

        format!(
            r#"{{
  "app": {{
{}
    "products": [
{}
    ],
    "buildModeSet": [
      {{
        "name": "debug"
      }},
      {{
        "name": "release"
      }}
    ]
  }},
  "modules": [
{}
  ]
}}"#,
            signing,
            products.join(",\n"),
            all_entries.join(",\n"),
        )
    }

    fn generate_module_json5(&self, config: &ModuleConfig, app_config: &AppConfig) -> String {
        let m = &config.module;
        let ht = to_harmonyos_type(&m.module_type);
        // Entry module: module_type is empty or "entry" / "feature"
        let is_entry = ht.is_empty() || ht == "entry" || ht == "feature";
        let is_atomic = app_config.app.bundle_type == "atomicService";

        let install_section = if is_atomic {
            "    \"deliveryWithInstall\": false,\n    \"installationFree\": true"
        } else {
            "    \"deliveryWithInstall\": true,\n    \"installationFree\": false"
        };

        let main_element_name = super::normalize_ets(&m.entry);
        let main_element = if is_entry {
            format!(",\n    \"mainElement\": \"{}\"", main_element_name)
        } else {
            String::new()
        };

        let pages_section = if (is_entry || ht == "shared" || ht == "hsp") && !m.routes.is_empty() {
            ",\n    \"pages\": \"$profile:main_pages\"".to_string()
        } else {
            String::new()
        };

        let icon = &app_config.app.icon;

        let abilities_json = if is_entry {
            let mut ui_abilities: Vec<String> = Vec::new();

            // Entry ability (always first UIAbility)
            let entry_name = main_element_name.clone();
            let entry_src = format!("./ets/entryability/{}", super::with_ets_suffix(&entry_name));
            let entry_skills = if m.skills.is_empty() {
                r#"            "entities": ["entity.system.home"],
            "actions": ["action.system.home"]"#
                    .to_string()
            } else {
                format_skills(&m.skills)
            };
            ui_abilities.push(format!(
                r#"      {{
        "name": "{entry_name}",
        "srcEntry": "{entry_src}",
        "description": "$string:{entry_name}_desc",
        "icon": "{icon}",
        "label": "$string:{entry_name}_label",
        "startWindowIcon": "{icon}",
        "startWindowBackground": "$color:start_window_background",
        "orientation": "auto_rotation",
        "exported": true,
        "skills": [
          {{
{entry_skills}
          }}
        ]
      }}"#
            ));

            // Additional UIAbility (ability_type empty)
            for a in m.abilities.iter().filter(|a| a.ability_type.is_empty()) {
                let name = super::normalize_ets(&a.name);
                let src = format!("./ets/entryability/{}", super::with_ets_suffix(&name));
                let skills_block = if a.skills.is_empty() {
                    r#"            "entities": ["entity.system.home"],
            "actions": ["action.system.home"]"#
                        .to_string()
                } else {
                    format_skills(&a.skills)
                };
                let orientation_line = if a.orientation.is_empty() {
                    String::new()
                } else {
                    format!(",\n        \"orientation\": \"{}\"", a.orientation)
                };
                let extra = format_ability_extra(a);
                ui_abilities.push(format!(
                    r#"      {{
        "name": "{name}",
        "srcEntry": "{src}",
        "description": "$string:{name}_desc",
        "icon": "{icon}",
        "label": "$string:{name}_label",
        "startWindowIcon": "{icon}",
        "startWindowBackground": "$color:start_window_background",
        "exported": true{orientation_line}{extra},
        "skills": [
          {{
{skills_block}
          }}
        ]
      }}"#
                ));
            }
            format!(
                ",\n    \"abilities\": [\n{}\n    ]",
                ui_abilities.join(",\n")
            )
        } else {
            String::new()
        };

        let extension_json = {
            let exts: Vec<&crate::config::AbilityConfig> = m
                .abilities
                .iter()
                .filter(|a| !a.ability_type.is_empty())
                .collect();
            if exts.is_empty() {
                String::new()
            } else {
                let items: Vec<String> = exts
                    .iter()
                    .map(|ea| {
                        let src_path = format!("./ets/entryformability/{}", ea.entry);
                        let exported_line = match ea.exported {
                            Some(v) => format!(",\n        \"exported\": {}", v),
                            None => String::new(),
                        };
                        let metadata_json = extension_metadata(ea);
                        format!(
                            r#"      {{
        "name": "{}",
        "srcEntry": "{}",
        "label": "$string:{}_label",
        "description": "$string:{}_desc",
        "type": "{}"{}{}
      }}"#,
                            ea.name,
                            src_path,
                            ea.name,
                            ea.name,
                            ea.ability_type,
                            exported_line,
                            metadata_json
                        )
                    })
                    .collect();
                format!(
                    ",\n    \"extensionAbilities\": [\n{}\n    ]",
                    items.join(",\n")
                )
            }
        };

        let permissions_json = if !m.permissions.is_empty() {
            let items: Vec<String> = m
                .permissions
                .iter()
                .map(|p| {
                    let abilities: Vec<String> =
                        p.abilities.iter().map(|a| format!("\"{}\"", a)).collect();
                    let reason_line = if p.reason.is_empty() {
                        String::new()
                    } else {
                        format!(",\n        \"reason\": \"{}\"", p.reason)
                    };
                    let used_scene = if p.abilities.is_empty() && p.when.is_empty() {
                        String::new()
                    } else {
                        format!(
                            r#",
        "usedScene": {{
          "abilities": [{}],
          "when": "{}"
        }}"#,
                            abilities.join(", "),
                            p.when
                        )
                    };
                    format!(
                        r#"      {{
        "name": "{}"{}{}
      }}"#,
                        p.name, reason_line, used_scene
                    )
                })
                .collect();
            format!(
                ",\n    \"requestPermissions\": [\n{}\n    ]",
                items.join(",\n")
            )
        } else {
            String::new()
        };

        let device_types_json = m
            .device_types
            .iter()
            .map(|d| format!("      \"{}\"", d))
            .collect::<Vec<_>>()
            .join(",\n");

        let src_entry_line = format!(
            ",\n    \"srcEntry\": \"./ets/entryability/{}\"",
            super::with_ets_suffix(&m.entry)
        );

        format!(
            r#"{{
  "module": {{
    "name": "{}",
    "type": "{}",
    "description": "$string:module_desc"{}{}{},
    "deviceTypes": [
{}
    ],
{}{}{}{}
  }}
}}"#,
            m.name,
            ht,
            main_element,
            pages_section,
            src_entry_line,
            device_types_json,
            install_section,
            abilities_json,
            extension_json,
            permissions_json
        )
    }

    fn generate_module_build_profile(
        &self,
        module: &crate::config::ModuleInfo,
        native_modules: &[(String, NativeModuleConfig)],
    ) -> String {
        let native_options_str = if native_modules.is_empty() {
            String::new()
        } else {
            // ABIs come from the module config so 32-bit or emulator targets can opt in.
            let abi_json = module
                .abi_filters
                .iter()
                .map(|a| format!("\"{}\"", a))
                .collect::<Vec<_>>()
                .join(", ");
            let items: Vec<String> = (0..native_modules.len())
                .map(|_| {
                    format!(
                        r#"    "externalNativeOptions": {{
      "path": "./src/main/cpp/CMakeLists.txt",
      "arguments": "",
      "cppFlags": "",
      "abiFilters": [{}]
    }}"#,
                        abi_json
                    )
                })
                .collect();
            items.join(",\n")
        };

        let native_block = if native_options_str.is_empty() {
            r#"  "buildOption": {}"#.to_string()
        } else {
            format!("  \"buildOption\": {{\n{}\n  }}", native_options_str)
        };

        let native_set_options = if native_options_str.is_empty() {
            String::new()
        } else {
            format!(",\n{}", native_options_str)
        };

        let obfuscation = if module.obfuscation {
            format!(
                r#"      "name": "release",
      "arkOptions": {{
        "obfuscation": {{
          "ruleOptions": {{
            "enable": true,
            "files": ["./obfuscation-rules.txt"]
          }}
        }}
      }}{}"#,
                native_set_options
            )
        } else {
            format!(
                r#"      "name": "release",
      "arkOptions": {{
        "obfuscation": {{
          "ruleOptions": {{
            "enable": false
          }}
        }}
      }}{}"#,
                native_set_options
            )
        };

        format!(
            r#"{{
  "apiType": "stageMode",
{},
  "buildOptionSet": [
    {{
{}
    }}
  ],
  "targets": [
    {{
      "name": "default"
    }}
  ]
}}"#,
            native_block, obfuscation
        )
    }

    fn generate_module_oh_package(
        &self,
        module: &crate::config::ModuleInfo,
        app_config: &AppConfig,
        native_modules: &[(String, NativeModuleConfig)],
    ) -> String {
        let main_field = if module.module_type == "hsp"
            || module.module_type == "shared"
            || module.module_type == "har"
        {
            r#""main": "index.ets","#.to_string()
        } else {
            r#""main": "","#.to_string()
        };

        let deps_block = if module.module_type.is_empty() || module.module_type == "entry" {
            let mut dep_entries: Vec<String> = app_config
                .dependencies
                .iter()
                .filter(|(name, _dep)| !native_module_names(native_modules).contains(name))
                .map(|(name, dep)| format!("    \"{}\": \"file:./../{}\"", name, dep.path))
                .collect();
            // Add native deps with local paths
            for (name, _cfg) in native_modules {
                dep_entries.push(format!(
                    "    \"{}.so\": \"file:./src/main/cpp/types/{}\"",
                    name, name
                ));
            }

            if dep_entries.is_empty() {
                r#"  "dependencies": {}"#.to_string()
            } else {
                format!("  \"dependencies\": {{\n{}\n  }}", dep_entries.join(",\n"))
            }
        } else {
            r#"  "dependencies": {}"#.to_string()
        };

        format!(
            r#"{{
  "name": "{}",
  "version": "1.0.0",
  "description": "Please describe the basic information.",
  {}
  "author": "",
  "license": "",
  {}
}}"#,
            module.name, main_field, deps_block
        )
    }
}

/// Extract native module names for filtering.
fn native_module_names(native_modules: &[(String, NativeModuleConfig)]) -> Vec<String> {
    native_modules.iter().map(|(n, _)| n.clone()).collect()
}

fn to_harmonyos_type(t: &str) -> &str {
    match t {
        "" => "entry",
        "hsp" => "shared",
        other => other,
    }
}

/// Format a list of Skills into JSON5 skills content (actions, entities, uris).
fn format_skills(skills: &[crate::config::Skill]) -> String {
    let blocks: Vec<String> = skills
        .iter()
        .map(|s| {
            let mut lines: Vec<String> = Vec::new();
            if !s.actions.is_empty() {
                let actions: Vec<String> = s.actions.iter().map(|a| format!("\"{}\"", a)).collect();
                lines.push(format!("            \"actions\": [{}]", actions.join(", ")));
            }
            if !s.entities.is_empty() {
                let entities: Vec<String> =
                    s.entities.iter().map(|e| format!("\"{}\"", e)).collect();
                lines.push(format!(
                    "            \"entities\": [{}]",
                    entities.join(", ")
                ));
            }
            if !s.uris.is_empty() {
                let uris: Vec<String> = s.uris.iter().map(format_uri_object).collect();
                lines.push(format!("            \"uris\": [{}]", uris.join(", ")));
            }
            if !s.permissions.is_empty() {
                let perms: Vec<String> =
                    s.permissions.iter().map(|p| format!("\"{}\"", p)).collect();
                lines.push(format!(
                    "            \"permissions\": [{}]",
                    perms.join(", ")
                ));
            }
            if let Some(dv) = s.domain_verify {
                lines.push(format!("            \"domainVerify\": {}", dv));
            }
            lines.join(",\n")
        })
        .collect();
    blocks.join("\n          },\n          {\n")
}

fn format_uri_object(u: &crate::config::UriScheme) -> String {
    let mut fields: Vec<String> = Vec::new();
    fields.push(format!("\"scheme\": \"{}\"", u.scheme));
    if let Some(ref v) = u.host {
        fields.push(format!("\"host\": \"{}\"", v));
    }
    if let Some(ref v) = u.port {
        fields.push(format!("\"port\": \"{}\"", v));
    }
    if let Some(ref v) = u.path {
        fields.push(format!("\"path\": \"{}\"", v));
    }
    if let Some(ref v) = u.path_start_with {
        fields.push(format!("\"pathStartWith\": \"{}\"", v));
    }
    if let Some(ref v) = u.path_regex {
        fields.push(format!("\"pathRegex\": \"{}\"", v));
    }
    if let Some(ref v) = u.mime_type {
        fields.push(format!("\"type\": \"{}\"", v));
    }
    if let Some(ref v) = u.utd {
        fields.push(format!("\"utd\": \"{}\"", v));
    }
    if let Some(v) = u.max_file_supported {
        fields.push(format!("\"maxFileSupported\": {}", v));
    }
    if let Some(ref v) = u.link_feature {
        fields.push(format!("\"linkFeature\": \"{}\"", v));
    }
    format!("{{ {} }}", fields.join(", "))
}

/// Generate optional ability JSON5 fields (launchType, permissions, metadata, window, etc.)
fn format_ability_extra(a: &crate::config::AbilityConfig) -> String {
    let mut lines: Vec<String> = Vec::new();
    if !a.launch_type.is_empty() {
        lines.push(format!("        \"launchType\": \"{}\"", a.launch_type));
    }
    if !a.ability_permissions.is_empty() {
        let perms: Vec<String> = a
            .ability_permissions
            .iter()
            .map(|p| format!("\"{}\"", p))
            .collect();
        lines.push(format!("        \"permissions\": [{}]", perms.join(", ")));
    }
    if !a.metadata.is_empty() {
        let items: Vec<String> = a.metadata.iter().map(|m| {
            format!("          {{\n            \"name\": \"{}\",\n            \"resource\": \"{}\"\n          }}", m.name, m.resource)
        }).collect();
        lines.push(format!(
            "        \"metadata\": [\n{}\n        ]",
            items.join(",\n")
        ));
    }
    if let Some(v) = a.continuable {
        lines.push(format!("        \"continuable\": {}", v));
    }
    if let Some(ref v) = a.start_window {
        lines.push(format!("        \"startWindow\": \"{}\"", v));
    }
    if let Some(v) = a.remove_mission_after_terminate {
        lines.push(format!("        \"removeMissionAfterTerminate\": {}", v));
    }
    if let Some(v) = a.allow_self_redirect {
        lines.push(format!("        \"allowSelfRedirect\": {}", v));
    }
    if let Some(v) = a.priority {
        lines.push(format!("        \"priority\": {}", v));
    }
    if !a.support_window_mode.is_empty() {
        let modes: Vec<String> = a
            .support_window_mode
            .iter()
            .map(|m| format!("\"{}\"", m))
            .collect();
        lines.push(format!(
            "        \"supportWindowMode\": [{}]",
            modes.join(", ")
        ));
    }
    if let Some(v) = a.max_window_ratio {
        lines.push(format!("        \"maxWindowRatio\": {}", v));
    }
    if let Some(v) = a.min_window_ratio {
        lines.push(format!("        \"minWindowRatio\": {}", v));
    }
    if let Some(v) = a.max_window_width {
        lines.push(format!("        \"maxWindowWidth\": {}", v));
    }
    if let Some(v) = a.min_window_width {
        lines.push(format!("        \"minWindowWidth\": {}", v));
    }
    if let Some(v) = a.max_window_height {
        lines.push(format!("        \"maxWindowHeight\": {}", v));
    }
    if let Some(v) = a.min_window_height {
        lines.push(format!("        \"minWindowHeight\": {}", v));
    }
    if !a.background_modes.is_empty() {
        let modes: Vec<String> = a
            .background_modes
            .iter()
            .map(|m| format!("\"{}\"", m))
            .collect();
        lines.push(format!(
            "        \"backgroundModes\": [{}]",
            modes.join(", ")
        ));
    }
    if let Some(v) = a.exclude_from_missions {
        lines.push(format!("        \"excludeFromMissions\": {}", v));
    }
    if let Some(v) = a.unclearable_mission {
        lines.push(format!("        \"unclearableMission\": {}", v));
    }
    if let Some(v) = a.isolation_process {
        lines.push(format!("        \"isolationProcess\": {}", v));
    }
    if let Some(v) = a.exclude_from_dock {
        lines.push(format!("        \"excludeFromDock\": {}", v));
    }
    if let Some(v) = a.recoverable {
        lines.push(format!("        \"recoverable\": {}", v));
    }
    if let Some(ref v) = a.prefer_multi_window_orientation {
        lines.push(format!(
            "        \"preferMultiWindowOrientation\": \"{}\"",
            v
        ));
    }
    if !a.continue_type.is_empty() {
        let types: Vec<String> = a
            .continue_type
            .iter()
            .map(|t| format!("\"{}\"", t))
            .collect();
        lines.push(format!("        \"continueType\": [{}]", types.join(", ")));
    }
    if !a.continue_bundle_name.is_empty() {
        let names: Vec<String> = a
            .continue_bundle_name
            .iter()
            .map(|n| format!("\"{}\"", n))
            .collect();
        lines.push(format!(
            "        \"continueBundleName\": [{}]",
            names.join(", ")
        ));
    }
    if let Some(ref v) = a.process {
        lines.push(format!("        \"process\": \"{}\"", v));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!(",\n{}", lines.join(",\n"))
    }
}

/// Generate metadata JSON5 for a whitelisted extension ability from its type.
/// Whitelist: form, backup, inputMethod, dataShare. Non-whitelist types get no metadata.
fn extension_metadata(ea: &crate::config::AbilityConfig) -> String {
    let is_whitelisted = matches!(
        ea.ability_type.as_str(),
        "form" | "backup" | "inputMethod" | "dataShare"
    );
    if !is_whitelisted {
        return String::new();
    }
    let config_name = format!("{}_config", ea.ability_type);
    format!(
        r#",
        "metadata": [
          {{
            "name": "ohos.extension.{}",
            "resource": "$profile:{}"
          }}
        ]"#,
        ea.ability_type, config_name
    )
}
