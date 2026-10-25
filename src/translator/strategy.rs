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
        module: &ModuleConfig,
        app_config: &AppConfig,
        native_modules: &[(String, NativeModuleConfig)],
    ) -> String;
    fn generate_module_json5(&self, module: &ModuleConfig, app_config: &AppConfig) -> String;
    fn generate_module_build_profile(
        &self,
        module: &crate::config::ModuleInfo,
        native_modules: &[(String, NativeModuleConfig)],
        inline_cpp: bool,
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
    "buildVersion": "{}",
    "icon": "{}",
    "label": "$string:app_name"{}
  }}
}}"#,
            config.app.bundle,
            config.app.vendor,
            config.app.version_code(),
            config.app.version,
            // buildVersion is a separate counter in DevEco; "1" is only the default for new projects.
            if config.app.build_version.is_empty() {
                "1"
            } else {
                config.app.build_version.as_str()
            },
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
            .map(|(dir, module)| {
                let apply_products: Vec<String> = app
                    .app
                    .products
                    .iter()
                    .map(|p| format!("\"{}\"", p))
                    .collect();
                // hvigor requires this name to match module.json5's moduleName, which comes
                // from [module] name — the directory only decides where sources live.
                let module_name = if module.module.name.is_empty() {
                    dir.clone()
                } else {
                    module.module.name.clone()
                };
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
                    module_name,
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

        let sv = if app.app.target_sdk_version.is_empty() {
            self.sdk_version_string()
        } else {
            app.app.target_sdk_version.clone()
        };
        let csv = self.compile_sdk_version_string();
        let compat_sdk = if app.app.compatible_sdk_version.is_empty() {
            &app.app.sdk_version
        } else {
            &app.app.compatible_sdk_version
        };
        let runtime_os = crate::config::effective_runtime_os(&app.app.runtime_os);
        // signingConfigs entries carry a fixed "type": "HarmonyOS" (signing profile kind),
        // unrelated to the project runtimeOS resolved above from config.rs.
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

        // Explicitly configurable: an atomic service may still be delivered with the install,
        // and a plain app may be installation-free. Fall back to the bundle_type default.
        let default_with_install = !is_atomic;
        let default_install_free = is_atomic;
        let with_install = m.delivery_with_install.unwrap_or(default_with_install);
        let install_free = m.installation_free.unwrap_or(default_install_free);
        // A HAR is a static package: its module schema rejects deliveryWithInstall /
        // installationFree (00303038), so those keys are only emitted for installable modules.
        let is_har = ht == "har";
        let install_section = if is_har {
            String::new()
        } else {
            format!(
                "    \"deliveryWithInstall\": {},\n    \"installationFree\": {}",
                with_install, install_free
            )
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
        let ability_icon = if m.icon.is_empty() { icon } else { &m.icon };
        let start_window_icon = if m.start_window_icon.is_empty() {
            ability_icon
        } else {
            &m.start_window_icon
        };
        // Optional upstream: an absent orientation must stay absent, not become auto_rotation.
        let orientation: &str = m.orientation.as_str();

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
            // Labels and orientation are optional in DevEco's module.json5; reproducing the
            // source means emitting them only when the project asked for them.
            let entry_desc = if m.entry_description.is_empty() {
                format!("\"$string:{}_desc\"", entry_name)
            } else {
                format!("\"{}\"", m.entry_description)
            };
            let entry_label = if m.entry_label.is_empty() {
                format!("\"$string:{}_label\"", entry_name)
            } else {
                format!("\"{}\"", m.entry_label)
            };
            let entry_orientation = if orientation.is_empty() {
                String::new()
            } else {
                format!("\n        \"orientation\": \"{}\",", orientation)
            };
            ui_abilities.push(format!(
                r#"      {{
        "name": "{entry_name}",
        "srcEntry": "{entry_src}",
        "description": {entry_desc},
        "icon": "{ability_icon}",
        "label": {entry_label},
        "startWindowIcon": "{start_window_icon}",
        "startWindowBackground": "$color:start_window_background",{entry_orientation}
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
                let extra_desc = if a.description.is_empty() {
                    format!("\"$string:{}_desc\"", name)
                } else {
                    format!("\"{}\"", a.description)
                };
                let extra_label = if a.label.is_empty() {
                    format!("\"$string:{}_label\"", name)
                } else {
                    format!("\"{}\"", a.label)
                };
                ui_abilities.push(format!(
                    r#"      {{
        "name": "{name}",
        "srcEntry": "{src}",
        "description": {extra_desc},
        "icon": "{icon}",
        "label": {extra_label},
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
                        // Keep the role directory the ability came from; DevEco uses
                        // entryformability / entrybackupability / … per extension type.
                        let role = if ea.dir.is_empty() {
                            "entryformability"
                        } else {
                            &ea.dir
                        };
                        let src_path = format!("./ets/{}/{}", role, ea.entry);
                        let exported_line = match ea.exported {
                            Some(v) => format!(",\n        \"exported\": {}", v),
                            None => String::new(),
                        };
                        let metadata_json = extension_metadata(ea);
                        // Same rule as UIAbility: reproduce what the project declared. A backup
                        // extension with no label must not gain a generated one.
                        // Only what the project declared: a backup extension that carried no
                        // label must not gain a generated one.
                        let label_line = if ea.label.is_empty() {
                            String::new()
                        } else {
                            format!("\n        \"label\": \"{}\",", ea.label)
                        };
                        let desc_line = if ea.description.is_empty() {
                            String::new()
                        } else {
                            format!("\n        \"description\": \"{}\",", ea.description)
                        };
                        format!(
                            r#"      {{
        "name": "{}",
        "srcEntry": "{}",{label_line}{desc_line}
        "type": "{}"{}{}
      }}"#,
                            ea.name, src_path, ea.ability_type, exported_line, metadata_json
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
                    // A permission that declared nothing stays bare: injecting
                    // `usedScene:{abilities:[],when:"always"}` states a runtime scene the project
                    // never asked for.
                    let used_scene = if p.abilities.is_empty() {
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

        // Entry modules declare each ability's srcEntry inside `abilities`; a second module-level
        // srcEntry is what DevEco projects never have, so it is only emitted for HSP/HAR. Those
        // keep their entry file at the module root (assemble copies it there), so the path must
        // not point into entryability/ — hvigor rejects the generated manifest otherwise.
        let src_entry_line = if is_entry || is_har {
            String::new()
        } else if ht == "shared" || ht == "hsp" {
            format!(
                ",\n    \"srcEntry\": \"./{}\"",
                super::with_ets_suffix(&m.entry)
            )
        } else {
            format!(
                ",\n    \"srcEntry\": \"./ets/entryability/{}\"",
                super::with_ets_suffix(&m.entry)
            )
        };

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
        inline_cpp: bool,
    ) -> String {
        let native_options_str = if native_modules.is_empty() && !inline_cpp {
            String::new()
        } else {
            // ABIs come from the module config so 32-bit or emulator targets can opt in.
            let abi_json = module
                .abi_filters
                .iter()
                .map(|a| format!("\"{}\"", a))
                .collect::<Vec<_>>()
                .join(", ");
            let count = if inline_cpp { 1 } else { native_modules.len() };
            let items: Vec<String> = (0..count)
                .map(|_| {
                    format!(
                        r#"    "externalNativeOptions": {{
      "path": "./src/main/cpp/CMakeLists.txt",
      "arguments": "{arguments}",
      "cppFlags": "{cpp_flags}",
      "abiFilters": [{}]
    }}"#,
                        abi_json,
                        arguments = module.native_arguments,
                        cpp_flags = module.cpp_flags,
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

        let obfuscation = if module.obfuscation {
            r#"      "name": "release",
      "arkOptions": {
        "obfuscation": {
          "ruleOptions": {
            "enable": true,
            "files": ["./obfuscation-rules.txt"]
          }
        }
      }"#
        } else {
            r#"      "name": "release",
      "arkOptions": {
        "obfuscation": {
          "ruleOptions": {
            "enable": false
          }
        }
      }"#
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
        config: &ModuleConfig,
        app_config: &AppConfig,
        native_modules: &[(String, NativeModuleConfig)],
    ) -> String {
        let module = &config.module;
        // A shared/HAR module's `main` must name its own entry file; the old hardcoded
        // "index.ets" pointed at a file the project never had (case included).
        let main_field = if module.module_type == "hsp"
            || module.module_type == "shared"
            || module.module_type == "har"
        {
            format!("\"main\": \"{}\",", super::with_ets_suffix(&module.entry))
        } else {
            r#""main": "","#.to_string()
        };

        // Module-scoped declarations win; an entry module additionally inherits the app-level
        // ones, because its own `[dependencies]` block used to be silently ignored.
        let mut declared: std::collections::BTreeMap<&str, String> = Default::default();
        if module.module_type.is_empty() || module.module_type == "entry" {
            for (name, dep) in &app_config.dependencies {
                if native_module_names(native_modules).contains(name) {
                    continue;
                }
                declared.insert(name.as_str(), format!("file:./../{}", dep.path));
            }
        }
        for (name, dep) in &config.dependencies {
            declared.insert(name.as_str(), format!("file:./../{}", dep.path));
        }
        let mut dep_entries: Vec<String> = declared
            .iter()
            .map(|(name, path)| format!("    \"{}\": \"{}\"", name, path))
            .collect();
        for (name, _cfg) in native_modules {
            dep_entries.push(format!(
                "    \"{}.so\": \"file:./src/main/cpp/types/{}\"",
                name, name
            ));
        }
        let deps_block = if dep_entries.is_empty() {
            r#"  "dependencies": {}"#.to_string()
        } else {
            format!("  \"dependencies\": {{\n{}\n  }}", dep_entries.join(",\n"))
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

#[cfg(test)]
mod module_output_tests {
    use super::*;

    fn module_named(module_name: &str) -> ModuleConfig {
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = module_name.to_string();
        cfg.module.device_types = vec!["phone".to_string()];
        cfg
    }

    /// hvigor rejects the project when build-profile's module name differs from module.json5's.
    /// The name comes from `[module] name`; the directory only says where the sources are.
    #[test]
    fn build_profile_uses_the_declared_module_name() {
        let app = AppConfig::default_for("aigis");
        let modules = vec![("src".to_string(), module_named("entry"))];
        let out = V26Strategy::new(26).generate_root_build_profile(&modules, &app, &[]);
        assert!(out.contains("\"name\": \"entry\""), "{}", out);
        assert!(out.contains("\"srcPath\": \"./src\""), "{}", out);
    }

    /// An atomic service can still be delivered with the installation, and a plain app can be
    /// installation-free — both are user decisions, not bundle_type consequences.
    #[test]
    fn install_flags_can_be_overridden() {
        let atomic_app = AppConfig::default_for("aigis");
        let mut atomic_app = atomic_app;
        atomic_app.app.bundle_type = "atomicService".to_string();

        let mut cfg = module_named("entry");
        cfg.module.delivery_with_install = Some(true);
        cfg.module.installation_free = Some(true);
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &atomic_app);
        assert!(out.contains("\"deliveryWithInstall\": true"), "{}", out);
        assert!(out.contains("\"installationFree\": true"), "{}", out);
    }

    /// The entry ability's presentation is configurable; DevEco projects routinely differ
    /// from the app icon and from `auto_rotation`.
    #[test]
    fn entry_ability_presentation_is_configurable() {
        let app = AppConfig::default_for("aigis");
        let mut cfg = module_named("entry");
        cfg.module.icon = "$media:startIcon".to_string();
        cfg.module.start_window_icon = "$media:startIcon".to_string();
        cfg.module.orientation = "auto_rotation_unspecified".to_string();
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app);
        assert!(out.contains("\"icon\": \"$media:startIcon\""), "{}", out);
        assert!(
            out.contains("\"startWindowIcon\": \"$media:startIcon\""),
            "{}",
            out
        );
        assert!(
            out.contains("\"orientation\": \"auto_rotation_unspecified\""),
            "{}",
            out
        );
    }

    /// Entry modules carry srcEntry per ability; a module-level duplicate is not what
    /// DevEco writes and only adds noise to diffs.
    #[test]
    fn entry_module_has_no_module_level_src_entry() {
        let app = AppConfig::default_for("aigis");
        let cfg = module_named("entry");
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app);
        assert_eq!(out.matches("\"srcEntry\"").count(), 1, "{}", out);
    }
}

#[cfg(test)]
mod inline_native_tests {
    use super::*;

    /// A module that ships its own cmake tree must declare externalNativeOptions, otherwise
    /// hvigor never builds it and the package silently carries no .so.
    #[test]
    fn module_owned_cpp_emits_external_native_options() {
        let module = ModuleConfig::default_entry().module;
        let out = V26Strategy::new(26).generate_module_build_profile(&module, &[], true);
        assert!(out.contains("\"externalNativeOptions\""), "{}", out);
        assert!(out.contains("./src/main/cpp/CMakeLists.txt"), "{}", out);
        assert!(out.contains("arm64-v8a"), "{}", out);
    }

    /// No native code, no native block: the empty buildOption must stay empty.
    #[test]
    fn plain_module_has_no_native_options() {
        let module = ModuleConfig::default_entry().module;
        let out = V26Strategy::new(26).generate_module_build_profile(&module, &[], false);
        assert!(!out.contains("externalNativeOptions"), "{}", out);
    }
}

#[cfg(test)]
mod extension_dir_tests {
    use super::*;
    use crate::config::AbilityConfig;

    /// A backup extension belongs under entrybackupability; writing it into entryformability
    /// makes module.json5 lie about the ability's home (and collides with a form ability that
    /// shares the file name).
    #[test]
    fn extension_src_entry_keeps_its_role_directory() {
        let app = AppConfig::default_for("paintboard");
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = "entry".to_string();
        cfg.module.device_types = vec!["phone".to_string()];
        cfg.module.abilities = vec![AbilityConfig {
            name: "EntryBackupAbility".to_string(),
            entry: "EntryBackupAbility.ets".to_string(),
            dir: "entrybackupability".to_string(),
            ability_type: "backup".to_string(),
            ..Default::default()
        }];
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app);
        assert!(
            out.contains("./ets/entrybackupability/EntryBackupAbility.ets"),
            "{}",
            out
        );
    }

    /// Without a recorded directory the type default still applies.
    #[test]
    fn extension_src_entry_falls_back_to_entryformability() {
        let app = AppConfig::default_for("app");
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = "entry".to_string();
        cfg.module.device_types = vec!["phone".to_string()];
        cfg.module.abilities = vec![AbilityConfig {
            name: "EntryFormAbility".to_string(),
            entry: "EntryFormAbility.ets".to_string(),
            ability_type: "form".to_string(),
            ..Default::default()
        }];
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app);
        assert!(
            out.contains("./ets/entryformability/EntryFormAbility.ets"),
            "{}",
            out
        );
    }
}

#[cfg(test)]
mod declared_labels_tests {
    use super::*;
    use crate::config::AbilityConfig;

    fn app() -> AppConfig {
        AppConfig::default_for("x")
    }

    fn module() -> crate::config::ModuleInfo {
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = "entry".into();
        cfg.module.device_types = vec!["phone".into()];
        cfg.module
    }

    /// A backup extension that declared no label gains none (and no extra string resources).
    #[test]
    fn extension_without_label_gets_no_generated_label() {
        let mut m = module();
        m.abilities = vec![AbilityConfig {
            name: "EntryBackupAbility".into(),
            entry: "EntryBackupAbility.ets".into(),
            dir: "entrybackupability".into(),
            ability_type: "backup".into(),
            ..Default::default()
        }];
        let cfg = ModuleConfig {
            module: m,
            dependencies: Default::default(),
        };
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app());
        assert!(!out.contains("EntryBackupAbility_label"), "{}", out);
    }

    /// A declared label is used verbatim instead of a generated `$string:<name>_label`.
    #[test]
    fn declared_label_is_used() {
        let mut m = module();
        m.entry_label = "$string:app_title".into();
        let cfg = ModuleConfig {
            module: m,
            dependencies: Default::default(),
        };
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app());
        assert!(out.contains("\"label\": \"$string:app_title\""), "{}", out);
    }

    /// Orientation is optional upstream: no value, no key.
    #[test]
    fn absent_orientation_emits_no_key() {
        let cfg = ModuleConfig {
            module: module(),
            dependencies: Default::default(),
        };
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app());
        assert!(!out.contains("\"orientation\""), "{}", out);
    }
}

#[cfg(test)]
mod declared_sdk_tests {
    use super::*;

    fn app_with(target: &str) -> AppConfig {
        let mut app = AppConfig::default_for("x");
        app.app.products = vec!["default".into()];
        app.app.target_sdk_version = target.to_string();
        app
    }

    /// targetSdkVersion is independent from the compile SDK: a project may compile against 26
    /// and target 24, and that changes runtime behaviour.
    #[test]
    fn target_sdk_is_emitted_when_declared() {
        let cfg = ModuleConfig::default_entry();
        let out = V26Strategy::new(26).generate_root_build_profile(
            &[("src".into(), cfg)],
            &app_with("6.1.1(24)"),
            &[],
        );
        assert!(
            out.contains("\"targetSdkVersion\": \"6.1.1(24)\""),
            "{}",
            out
        );
    }

    /// Without a declared target it still falls back to the compile SDK.
    #[test]
    fn target_sdk_falls_back_to_compile() {
        let cfg = ModuleConfig::default_entry();
        let out = V26Strategy::new(26).generate_root_build_profile(
            &[("src".into(), cfg)],
            &app_with(""),
            &[],
        );
        assert!(out.contains("\"targetSdkVersion\": \"26.0.0\""), "{}", out);
    }
}

#[cfg(test)]
mod permission_scene_tests {
    use super::*;
    use crate::config::ModulePermission;

    /// A bare `{ "name": ... }` permission must stay bare — injecting
    /// `usedScene:{abilities:[],when:"always"}` claims a runtime scene the project never set.
    #[test]
    fn permission_without_abilities_has_no_used_scene() {
        let app = AppConfig::default_for("x");
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = "entry".into();
        cfg.module.device_types = vec!["phone".into()];
        cfg.module.permissions = vec![ModulePermission {
            name: "ohos.permission.INTERNET".into(),
            reason: String::new(),
            abilities: vec![],
            when: String::new(),
        }];
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app);
        assert!(!out.contains("usedScene"), "{}", out);
    }

    /// When the project does declare a scene, it is kept.
    #[test]
    fn permission_with_abilities_keeps_used_scene() {
        let app = AppConfig::default_for("x");
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = "entry".into();
        cfg.module.device_types = vec!["phone".into()];
        cfg.module.permissions = vec![ModulePermission {
            name: "ohos.permission.PRIVACY_WINDOW".into(),
            reason: String::new(),
            abilities: vec!["EntryAbility".into()],
            when: "always".into(),
        }];
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &app);
        assert!(out.contains("\"usedScene\""), "{}", out);
        assert!(out.contains("EntryAbility"), "{}", out);
    }
}

#[cfg(test)]
mod declared_orientation_tests {
    use super::*;

    fn module_with(orientation: &str) -> crate::config::ModuleInfo {
        let mut cfg = ModuleConfig::default_entry();
        cfg.module.name = "entry".into();
        cfg.module.device_types = vec!["phone".into()];
        cfg.module.orientation = orientation.into();
        cfg.module
    }

    /// `auto_rotation` is a declarED choice, not "nothing": it must reach module.json5.
    #[test]
    fn explicit_auto_rotation_is_emitted() {
        let cfg = ModuleConfig {
            module: module_with("auto_rotation"),
            dependencies: Default::default(),
        };
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &AppConfig::default_for("x"));
        assert!(
            out.contains("\"orientation\": \"auto_rotation\""),
            "{}",
            out
        );
    }
}

#[cfg(test)]
mod build_version_tests {
    use super::*;

    /// buildVersion is DevEco's own counter, not a constant: carry it when declared.
    #[test]
    fn declared_build_version_is_emitted() {
        let mut app = AppConfig::default_for("x");
        app.app.build_version = "7".into();
        let out = V26Strategy::new(26).generate_app_json5(&app);
        assert!(out.contains("\"buildVersion\": \"7\""), "{}", out);
    }

    /// With nothing declared, the historical default stays.
    #[test]
    fn build_version_defaults_to_one() {
        let app = AppConfig::default_for("x");
        let out = V26Strategy::new(26).generate_app_json5(&app);
        assert!(out.contains("\"buildVersion\": \"1\""), "{}", out);
    }
}

#[cfg(test)]
mod native_options_placement_tests {
    use super::*;

    /// externalNativeOptions belongs to buildOption only. Emitting it again inside
    /// buildOptionSet[].release produced a second copy at a schema-invalid position.
    #[test]
    fn external_native_options_appear_once() {
        let module = ModuleConfig::default_entry().module;
        let out = V26Strategy::new(26).generate_module_build_profile(&module, &[], true);
        assert_eq!(out.matches("externalNativeOptions").count(), 1, "{}", out);
    }

    #[test]
    fn obfuscated_module_still_has_one_native_block() {
        let mut module = ModuleConfig::default_entry().module;
        module.obfuscation = true;
        let out = V26Strategy::new(26).generate_module_build_profile(&module, &[], true);
        assert_eq!(out.matches("externalNativeOptions").count(), 1, "{}", out);
        assert!(out.contains("obfuscation"), "{}", out);
    }
}

#[cfg(test)]
mod shared_module_tests {
    use super::*;
    use crate::config::ModuleInfo;

    fn shared_module(entry: &str) -> ModuleConfig {
        ModuleConfig {
            dependencies: Default::default(),
            module: ModuleInfo {
                name: "shared".into(),
                routes: vec![],
                entry: entry.into(),
                skills: vec![],
                module_type: "hsp".into(),
                abilities: vec![],
                device_types: vec!["phone".into()],
                permissions: vec![],
                obfuscation: false,
                obfuscation_rules: String::new(),
                icon: String::new(),
                start_window_icon: String::new(),
                orientation: String::new(),
                entry_label: String::new(),
                entry_description: String::new(),
                delivery_with_install: None,
                installation_free: None,
                abi_filters: vec!["arm64-v8a".into()],
                cpp_flags: String::new(),
                native_arguments: String::new(),
            },
        }
    }

    /// A shared module's srcEntry is resolved from src/main/, where assemble puts the entry file,
    /// not from entryability/.
    #[test]
    fn shared_module_src_entry_is_module_relative() {
        let cfg = shared_module("Index.ets");
        let out = V26Strategy::new(26).generate_module_json5(&cfg, &AppConfig::default_for("x"));
        assert!(out.contains("\"srcEntry\": \"./Index.ets\""), "{}", out);
        assert!(!out.contains("entryability"), "{}", out);
    }

    /// `main` names the module's own entry file instead of a hardcoded index.ets.
    #[test]
    fn shared_module_main_names_its_entry() {
        let cfg = shared_module("Index.ets");
        let out = V26Strategy::new(26).generate_module_oh_package(
            &cfg,
            &AppConfig::default_for("x"),
            &[],
        );
        assert!(out.contains("\"main\": \"Index.ets\""), "{}", out);
    }
}

#[cfg(test)]
mod module_dependency_tests {
    use super::*;
    use crate::config::{Dependency, ModuleInfo};
    use std::collections::BTreeMap;

    fn shared_with_deps() -> ModuleConfig {
        let mut deps = BTreeMap::new();
        deps.insert(
            "shared_b".to_string(),
            Dependency {
                path: "shared_b".into(),
                version: "1.0.0".into(),
            },
        );
        ModuleConfig {
            dependencies: deps,
            module: ModuleInfo {
                name: "shared_a".into(),
                routes: vec![],
                entry: "Index.ets".into(),
                skills: vec![],
                module_type: "hsp".into(),
                abilities: vec![],
                device_types: vec!["phone".into()],
                permissions: vec![],
                obfuscation: false,
                obfuscation_rules: String::new(),
                icon: String::new(),
                start_window_icon: String::new(),
                orientation: String::new(),
                entry_label: String::new(),
                entry_description: String::new(),
                delivery_with_install: None,
                installation_free: None,
                abi_filters: vec!["arm64-v8a".into()],
                cpp_flags: String::new(),
                native_arguments: String::new(),
            },
        }
    }

    /// A shared module's own `[dependencies]` must reach its oh-package.json5; they used to be
    /// dropped, leaving a module that imports shared_b declaring no dependency on it.
    #[test]
    fn module_scope_dependencies_are_emitted() {
        let cfg = shared_with_deps();
        let out = V26Strategy::new(26).generate_module_oh_package(
            &cfg,
            &AppConfig::default_for("x"),
            &[],
        );
        assert!(out.contains("shared_b"), "{}", out);
        assert!(out.contains("file:./../shared_b"), "{}", out);
    }
}
