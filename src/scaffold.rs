// Scaffold generator

use std::fs;
use std::path::Path;

use crate::config;

const INDEX_ETS: &str = include_str!("../templates/Index.ets");
const ENTRY_ABILITY_ETS: &str = include_str!("../templates/EntryAbility.ets");
const STRINGS_JSON: &str = include_str!("../templates/strings.json");
const COLORS_JSON: &str = include_str!("../templates/colors.json");
const FLOATS_JSON: &str = include_str!("../templates/floats.json");
const GITIGNORE: &str = include_str!("../templates/gitignore");
const LAYERED_IMAGE_JSON: &str = include_str!("../templates/media/layered_image.json");
const FOREGROUND_PNG: &[u8] = include_bytes!("../templates/media/foreground.png");
const BACKGROUND_PNG: &[u8] = include_bytes!("../templates/media/background.png");
const START_ICON_PNG: &[u8] = include_bytes!("../templates/media/startIcon.png");

/// Comment block prepended to root hvolt.toml with examples of advanced features.
const ROOT_TOML_COMMENT: &str = r#"# hvolt project configuration
# 
# Advanced options (uncomment as needed):
#
#   bundle_type = "atomicService"    # for atomic service (default: "app")
#
#   [dependencies]
#   scrypt = { path = "vendor/scrypt.har", version = "1.0" }
#
"#;

/// Comment block prepended to module hvolt.toml with examples of advanced features.
const MODULE_TOML_COMMENT: &str = r#"# Module configuration
#
# Advanced options (uncomment as needed):
#
#   skills = [
#     { actions = ["action.system.home"], entities = ["entity.system.home"] },
#     { actions = ["ohos.want.action.viewData"], entities = ["entity.system.default"], uris = [{scheme = "https"}] },
#   ]
#
#   [[module.abilities]]
#   name = "ShareAbility"
#   skills = [
#     { actions = ["ohos.want.action.sendData"] },
#   ]
#
#   [[module.abilities]]
#   name = "EntryFormAbility"
#   type = "form"
#
#   [[module.permissions]]
#   name = "ohos.permission.INTERNET"
#   reason = "$string:internet_reason"
#   abilities = ["EntryAbility"]
#   when = "always"
#
"#;

pub struct ScaffoldPlan {
    pub dirs: Vec<String>,
    pub files: Vec<(String, Vec<u8>)>,
}

/// Validate a project name by the segment it becomes in the bundle id.
/// Rule mirrors the official modulecheck schema for non-leading segments:
/// start with an ASCII letter/digit, end with one, only \w in between
/// (hyphens are converted to underscores by the bundle derivation, so they
/// are legal in the name). Catches "my app" (space), "_abc"/"abc_"
/// (underscore edges → invalid bundleName that would only fail at
/// install time), and empty — while nested paths like "examples/my-app"
/// stay valid (basename is what matters).
pub fn validate_project_name(name: &str) -> Result<(), String> {
    let base = std::path::Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if base.len() > 255 {
        // filesystem NAME_MAX: an unbounded name reaches create_dir and
        // panics there (a 500-char name exits 101)
        return Err(format!(
            "project name is too long ({} bytes; limit 255)",
            base.len()
        ));
    }
    let seg = base.replace('-', "_");
    if seg.is_empty() {
        return Err("project name is empty".into());
    }
    let first = seg.chars().next().unwrap();
    let last = seg.chars().last().unwrap();
    if !(first.is_ascii_alphanumeric()) || !(last.is_ascii_alphanumeric()) {
        return Err(format!(
            "project name {:?} would produce bundle segment {:?}, which the HarmonyOS \
             bundleName schema rejects (must start AND end with a letter/digit)",
            base, seg
        ));
    }
    if !seg.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) {
        return Err(format!(
            "project name {:?} contains characters not allowed in bundleName",
            base
        ));
    }
    Ok(())
}

/// Pure: compute scaffold layout from project name/path — no I/O.
pub fn plan(name: &str) -> ScaffoldPlan {
    let src = format!("{}/src", name);

    // Use basename for the app name (e.g., "examples/test-app" → "test-app")
    let app_name = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());

    let app_config = config::AppConfig::default_for(&app_name);
    let app_toml_lines = toml::to_string_pretty(&app_config).expect("serialize app config");
    let app_toml = format!("{}{}", ROOT_TOML_COMMENT, app_toml_lines);

    let module_config = config::ModuleConfig::default_entry();
    let module_toml_lines =
        toml::to_string_pretty(&module_config).expect("serialize module config");
    let module_toml = format!("{}{}", MODULE_TOML_COMMENT, module_toml_lines);

    // Propagate the project name into display strings: app_name is the bundle
    // label, EntryAbility_label is what the home screen shows.
    let strings_json = STRINGS_JSON
        .replace("\"HelloWorld\"", &format!("\"{}\"", app_name))
        .replace("\"label\"", &format!("\"{}\"", app_name));

    let res_base = format!("{}/resources/base", name);

    let dirs = vec![
        format!("{}/pages", src),
        format!("{}/element", res_base),
        format!("{}/media", res_base),
    ];

    let files: Vec<(String, Vec<u8>)> = vec![
        (format!("{}/hvolt.toml", name), app_toml.into_bytes()),
        (format!("{}/hvolt.toml", src), module_toml.into_bytes()),
        (
            format!("{}/EntryAbility.ets", src),
            ENTRY_ABILITY_ETS.as_bytes().to_vec(),
        ),
        (
            format!("{}/pages/Index.ets", src),
            INDEX_ETS.as_bytes().to_vec(),
        ),
        (
            format!("{}/element/string.json", res_base),
            strings_json.into_bytes(),
        ),
        (
            format!("{}/element/color.json", res_base),
            COLORS_JSON.as_bytes().to_vec(),
        ),
        (
            format!("{}/element/float.json", res_base),
            FLOATS_JSON.as_bytes().to_vec(),
        ),
        (
            format!("{}/media/layered_image.json", res_base),
            LAYERED_IMAGE_JSON.as_bytes().to_vec(),
        ),
        (
            format!("{}/media/foreground.png", res_base),
            FOREGROUND_PNG.to_vec(),
        ),
        (
            format!("{}/media/background.png", res_base),
            BACKGROUND_PNG.to_vec(),
        ),
        (
            format!("{}/media/startIcon.png", res_base),
            START_ICON_PNG.to_vec(),
        ),
        (
            format!("{}/.gitignore", name),
            GITIGNORE.as_bytes().to_vec(),
        ),
    ];

    ScaffoldPlan { dirs, files }
}

/// Execute the plan on disk.
pub fn init(name: &str) {
    let plan = plan(name);
    let root = Path::new(name);

    if root.exists() {
        eprintln!("Error: directory '{}' already exists", name);
        std::process::exit(1);
    }

    // FS errors are reported, not surfaced as panics
    if let Err(e) = create_scaffold_files(root, &plan) {
        eprintln!("✗ {}", e);
        // remove what was already written — no half-scaffold left behind
        let _ = fs::remove_dir_all(root);
        std::process::exit(1);
    }

    // Print summary
    println!("Scaffold created: {}/", name);
    for dir in &plan.dirs {
        println!("  {}", dir);
    }
    for (path, _) in &plan.files {
        println!("  {}", path);
    }
}
fn create_scaffold_files(root: &Path, plan: &ScaffoldPlan) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| format!("create {}: {}", root.display(), e))?;
    for dir in &plan.dirs {
        fs::create_dir_all(dir).map_err(|e| format!("create dir {}: {}", dir, e))?;
    }
    for (path, content) in &plan.files {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create parent: {}", e))?;
        }
        let mut file =
            fs::File::create(path).map_err(|e| format!("create file {}: {}", path, e))?;
        std::io::Write::write_all(&mut file, content)
            .map_err(|e| format!("write file {}: {}", path, e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_content(plan: &ScaffoldPlan, suffix: &str) -> String {
        plan.files
            .iter()
            .find(|(p, _)| p.ends_with(suffix))
            .map(|(_, c)| String::from_utf8(c.clone()).unwrap())
            .unwrap_or_else(|| panic!("no file ending in {}", suffix))
    }

    #[test]
    fn name_validation_follows_official_bundle_schema() {
        assert!(validate_project_name("my-app").is_ok());
        assert!(
            validate_project_name("examples/test-app").is_ok(),
            "nested paths stay legal (F2)"
        );
        assert!(validate_project_name("MyApp").is_ok());
        assert!(
            validate_project_name("1abc").is_ok(),
            "leading digit is legal for non-leading segments"
        );
        assert!(validate_project_name("abc__x").is_ok());
        // underscore at either edge → invalid bundleName
        assert!(validate_project_name("_abc").is_err());
        assert!(validate_project_name("abc_").is_err());
        assert!(validate_project_name("my app").is_err());
        assert!(validate_project_name("").is_err());
        assert!(
            validate_project_name("-abc").is_err(),
            "leading hyphen becomes leading _"
        );
        // trailing slash: Path::file_name drops it ("a/b/" → "b") — real
        // assertion (an err() || ok() form would be vacuous)
        assert!(validate_project_name("a/b/").is_ok());
        assert!(
            validate_project_name(&"x".repeat(300)).is_err(),
            "over-long name rejected"
        );
    }

    #[test]
    fn plan_propagates_project_name_into_strings() {
        let p = plan("demo");
        let s = file_content(&p, "element/string.json");
        // regression: placeholders "HelloWorld"/"label" used to ship verbatim
        assert!(
            !s.contains("HelloWorld"),
            "app_name placeholder leaked: {}",
            s
        );
        assert!(!s.contains("\"label\""), "label placeholder leaked: {}", s);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let find = |name: &str| {
            v["string"]
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["name"] == name)
                .unwrap()["value"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(find("app_name"), "demo");
        assert_eq!(find("EntryAbility_label"), "demo");
    }

    #[test]
    fn plan_uses_basename_for_nested_name() {
        let p = plan("examples/my-app");
        let s = file_content(&p, "element/string.json");
        assert!(
            s.contains("\"my-app\""),
            "nested name not propagated: {}",
            s
        );
        assert!(file_content(&p, "hvolt.toml").contains("name = \"my-app\""));
    }
}
