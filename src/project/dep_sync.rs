// `hvolt dep sync`: registry packages already on disk → `deps/<scope>/<name>/`.
//
// The flat project keeps its dependencies as ordinary directories that describe themselves
// with the same `hvolt.toml` format as a member module. That removes the ohpm store shape
// (`oh_modules/.ohpm/<name>@<version>/oh_modules/<name>/`) from the project: no `src/main/ets`
// layer, no `ets/modules.abc` inside a byte-code package, no hvigor manifest left behind.
//
// Only packages already on disk are converted. hvolt does not fetch from a registry, so a
// package that is not present is reported instead of downloaded.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::support::config::{AppConfig, Dependency};

/// One package that moved into `deps/`.
pub struct Moved {
    /// Package name as the root `[dependencies]` spells it.
    pub package: String,
    /// Directory it came from.
    pub from: PathBuf,
    /// Directory it now occupies, relative to the project root.
    pub dest: String,
}

/// What a package is, decided from the tree it ships.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// ArkTS sources compiled into the app's abc.
    Arkts,
    /// Byte-code HAR: the abc is merged, the `.d.ets` declarations describe it.
    ByteCode,
    /// Plain JavaScript tree compiled as it lies.
    Js,
}

impl Shape {
    fn as_str(self) -> &'static str {
        match self {
            Self::Arkts => "arkts",
            Self::ByteCode => "bytecode",
            Self::Js => "js",
        }
    }
}

/// Move every registry dependency of `project_dir` into `deps/` and rewrite the declarations.
pub fn sync(project_dir: &Path) -> Result<Vec<Moved>, String> {
    let root = project_dir.join("hvolt.toml");
    let text =
        std::fs::read_to_string(&root).map_err(|e| format!("read {}: {e}", root.display()))?;
    let mut app: AppConfig =
        toml::from_str(&text).map_err(|e| format!("parse {}: {e}", root.display()))?;

    let mut moved: Vec<Moved> = Vec::new();
    let mut declared: BTreeMap<String, Dependency> = BTreeMap::new();
    for (name, dep) in app.dependencies.clone() {
        if dep.path.is_some() {
            declared.insert(name, dep);
            continue;
        }
        let Some(from) = package_dir(project_dir, &name, dep.version.as_deref()) else {
            return Err(format!(
                "dependency '{name}' is declared but no copy exists under oh_modules/. \
                 hvolt does not fetch packages: place it there once and re-run `hvolt dep sync`"
            ));
        };
        let dest_rel = dest_relative(&name);
        let dest = project_dir.join(&dest_rel);
        if dest.exists() {
            return Err(format!(
                "{} already exists: remove it first, or declare the dependency with `path`",
                dest.display()
            ));
        }
        let shape = flatten(&from, &dest)?;
        let entry = package_entry(&from, shape);
        write_manifest(&dest, &name, shape, entry.as_deref())?;
        let mut dep = dep;
        dep.path = Some(dest_rel.replace('\\', "/"));
        declared.insert(name.clone(), dep);
        moved.push(Moved {
            package: name,
            from,
            dest: dest_rel,
        });
    }
    app.dependencies = declared;
    let body = toml::to_string(&app).map_err(|e| format!("serialize hvolt.toml: {e}"))?;
    std::fs::write(&root, body).map_err(|e| format!("write {}: {e}", root.display()))?;
    Ok(moved)
}

/// `@cxy/sandboxfinder` → `deps/cxy/sandboxfinder`; `yaml` → `deps/yaml`.
fn dest_relative(name: &str) -> String {
    let path = match name.strip_prefix('@') {
        Some(rest) => rest.to_string(),
        None => name.to_string(),
    };
    format!("deps/{path}")
}

/// Where a package currently lives: the flat `oh_modules/<name>`, else the `.ohpm` store.
fn package_dir(project_dir: &Path, name: &str, version: Option<&str>) -> Option<PathBuf> {
    let flat = project_dir.join("oh_modules").join(name);
    if flat.is_dir() {
        return Some(flat);
    }
    let store = project_dir.join("oh_modules").join(".ohpm");
    let escaped = name.replace('/', "+");
    let wanted = version.map(|v| format!("{escaped}@{v}"));
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&store)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            dir_name.starts_with(&format!("{escaped}@"))
        })
        .collect();
    candidates.sort();
    if let Some(wanted) = wanted {
        if let Some(hit) = candidates.iter().find(|path| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .as_deref()
                == Some(wanted.as_str())
        }) {
            let nested = hit.join("oh_modules").join(name);
            if nested.is_dir() {
                return Some(nested);
            }
        }
    }
    for candidate in candidates.iter().rev() {
        let nested = candidate.join("oh_modules").join(name);
        if nested.is_dir() {
            return Some(nested);
        }
    }
    None
}

/// Copy a package into `dest` in the shape the flat layout uses, and report what it is.
fn flatten(from: &Path, dest: &Path) -> Result<Shape, String> {
    let shape = detect(from);
    std::fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    match shape {
        Shape::Arkts => {
            // Sources under `src/main/ets` move to the package root; files that already sit at
            // the root (`Index.ets`) stay beside them.
            let ets = from.join("src").join("main").join("ets");
            if ets.is_dir() {
                copy_tree(&ets, dest)?;
            }
            copy_root_sources(from, dest)?;
            // A package root file may address its own sources through the old layout
            // (`./src/main/ets/components/x`). Once those sources live at the package root,
            // those specifiers have to point there instead.
            rewrite_root_relative_imports(dest)?;
        }
        Shape::ByteCode => {
            let abc = from.join("ets").join("modules.abc");
            std::fs::copy(&abc, dest.join("modules.abc"))
                .map_err(|e| format!("copy {}: {e}", abc.display()))?;
            copy_root_sources(from, dest)?;
        }
        Shape::Js => {
            copy_tree(from, dest)?;
            for name in ["oh-package.json5", "package.json", "build-profile.json5"] {
                let stale = dest.join(name);
                if stale.is_file() {
                    let _ = std::fs::remove_file(stale);
                }
            }
        }
    }
    Ok(shape)
}

fn detect(from: &Path) -> Shape {
    // A package that ships a prebuilt `ets/modules.abc` is a byte-code HAR: the abc is what the
    // app merges, and its `src/` holds declarations rather than code to compile.
    if from.join("ets").join("modules.abc").is_file() {
        return Shape::ByteCode;
    }
    if from.join("src").join("main").join("ets").is_dir()
        || [from.join("Index.ets"), from.join("index.ets")]
            .iter()
            .any(|path| path.is_file())
    {
        return Shape::Arkts;
    }
    Shape::Js
}

/// Source files at the package root, which the flattened tree keeps beside the moved ones.
fn copy_root_sources(from: &Path, dest: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(from) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        let is_source = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ets") | Some("ts") | Some("d.ets")
        );
        if is_source || name == "ResourceTable.txt" {
            std::fs::copy(&path, dest.join(&name))
                .map_err(|e| format!("copy {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// Drop the `src/main/ets/` prefix from relative specifiers at the package root.
fn rewrite_root_relative_imports(dest: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dest) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_source = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ets") | Some("ts")
        );
        if !is_source {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rewritten = text
            .replace("'./src/main/ets/", "'./")
            .replace("\"./src/main/ets/", "\"./")
            .replace("\"src/main/ets/", "\"");
        if rewritten != text {
            std::fs::write(&path, rewritten)
                .map_err(|e| format!("rewrite {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let target = dest.join(&name);
        if name.to_string_lossy() == "oh_modules" || name.to_string_lossy() == ".DS_Store" {
            continue;
        }
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else if path.is_file() {
            std::fs::copy(&path, &target).map_err(|e| format!("copy {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// The entry file at a package root, spelled as the directory spells it.
///
/// `is_file()` cannot be used to pick between `Index.ets` and `index.ets`: on a case-insensitive
/// filesystem both probes succeed, and the manifest would name a file that does not exist.
fn root_entry(from: &Path) -> Option<String> {
    let entries = std::fs::read_dir(from).ok()?;
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for wanted in ["Index.ets", "index.ets", "Index.ts", "index.ts"] {
        if names.iter().any(|name| name == wanted) {
            return Some(wanted.to_string());
        }
    }
    None
}

/// The entry a package is addressed by, taken from the tree it ships.
fn package_entry(from: &Path, shape: Shape) -> Option<String> {
    match shape {
        Shape::Arkts => root_entry(from),
        Shape::ByteCode => Some("Index.d.ets".to_string()),
        Shape::Js => {
            if from.join("browser/dist/index.js").is_file() {
                Some("browser/dist/index".to_string())
            } else if from.join("index.js").is_file() {
                Some("index".to_string())
            } else {
                None
            }
        }
    }
}

/// Write the package's own `hvolt.toml`.
fn write_manifest(
    dest: &Path,
    name: &str,
    shape: Shape,
    entry: Option<&str>,
) -> Result<(), String> {
    let mut lines = vec![
        "[module]".to_string(),
        format!("name = {}", toml_string(name)),
        format!("type = {}", toml_string(shape.as_str())),
    ];
    if let Some(entry) = entry {
        lines.push(format!("main = {}", toml_string(entry)));
    }
    let body = format!("{}\n", lines.join("\n"));
    let path = dest.join("hvolt.toml");
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
