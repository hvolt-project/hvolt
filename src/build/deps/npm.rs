//! Third-party packages the app declares in `hvolt.toml` `[dependencies]`.
//!
//! A HAP is not only its own modules: `import { parse } from 'yaml'` names a package the
//! device expects to find as `yaml.browser.dist.index` inside the merged abc. The project
//! lists those packages in `[dependencies]`, and that table is the whole declaration: kind,
//! version and entry all come from it, or from the package's own directory shape when the
//! declaration leaves a field out. No hvigor manifest is read at build time.
//!
//! A bare import the table does not cover is an error naming the package. It is never
//! resolved by guessing from a leftover `oh-package.json5`.

use std::path::{Path, PathBuf};

use crate::support::config::{AppConfig, Dependency};

/// How a package has to be compiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// ArkTS component library: `src/main/ets` + an entry file, components to expand.
    Arkts,
    /// Plain JavaScript/TypeScript tree: compiled as it lies.
    Js,
    /// Native module: its `.so` is built by the native chain, not compiled into the abc.
    Native,
    /// Byte-code HAR: ships `ets/modules.abc` and only `.d.ets` declarations, so it cannot
    /// be compiled from source — its bytecode is merged into the app's abc instead.
    ByteCode,
}

impl Kind {
    /// Parse the `type` field, or `None` when it names something else.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "arkts" => Some(Self::Arkts),
            "bytecode" | "byte-code" => Some(Self::ByteCode),
            "js" | "javascript" | "npm" => Some(Self::Js),
            "native" => Some(Self::Native),
            _ => None,
        }
    }
}

/// One package from `[dependencies]`, reduced to what the compile needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Key of the `[dependencies]` entry.
    pub name: String,
    /// Import name when it differs from the key (`alias`).
    pub alias: String,
    /// Directory the package occupies.
    pub dir: PathBuf,
    /// Package version, which becomes the record's `&<version>` suffix.
    pub version: String,
    /// Root the sources are collected from: the package dir, or its `src/main/ets`.
    pub source_root: PathBuf,
    /// Entry module inside `source_root`, without extension, e.g. `browser/dist/index`.
    pub entry: String,
    /// The `[dependencies].entry` value as declared, e.g. `./dist/index.js`. Records carry the
    /// declaration verbatim (extension dropped), which is what the device resolves the package
    /// by; `entry` alone cannot reconstruct it after the bundle substitution.
    pub declared: Option<String>,
    pub kind: Kind,
}

impl Package {
    /// Record prefix in the form the device requests: `<alias>/<source root>`.
    pub fn record_prefix_slash(&self) -> String {
        let root = self.source_root_relative();
        if root.is_empty() {
            self.alias.clone()
        } else {
            format!("{}/{}", self.alias, root)
        }
    }

    /// Directory this package occupies inside the staging tree: the ohpm layout the reference
    /// compiles from, with the package's source root appended.
    ///
    /// The reference keys a package by its resolved version and keeps the scope inside the
    /// directory name (`oh_modules/.ohpm/@mcui+mccharts@2.8.9/oh_modules/@mcui/mccharts/…`);
    /// a flat `<name>` staging directory put every package record at a path the device never
    /// asks for (measured against clashbox, 2026-11-06).
    pub fn staging_dir(&self) -> String {
        let root = self.source_root_relative();
        if root.is_empty() {
            self.ohpm_dir()
        } else {
            format!("{}/{}", self.ohpm_dir(), root)
        }
    }

    /// The package's own directory inside the staging tree, without its source root:
    /// `oh_modules/.ohpm/<name>@<version>/oh_modules/<name>`, with `/` in the name flattened
    /// to `+` (`@mcui+mccharts@2.8.9`).
    pub fn ohpm_dir(&self) -> String {
        format!(
            "oh_modules/.ohpm/{}@{}/oh_modules/{}",
            self.name.replace('/', "+"),
            self.version,
            self.name
        )
    }

    /// Path of a staged file relative to the package directory: its source root followed by
    /// `inside_source_root` (`browser/dist/index.js`, `src/main/ets/…`). The reference's
    /// filesInfo carries this form in its fourth field.
    pub fn package_rel(&self, inside_source_root: &str) -> String {
        let root = self.source_root_relative();
        match (root.is_empty(), inside_source_root.is_empty()) {
            (true, _) => inside_source_root.to_string(),
            (false, true) => root,
            (false, false) => format!("{}/{}", root, inside_source_root),
        }
    }

    /// Normalized url the compiler must emit for an import of `spec` inside this package.
    ///
    /// `spec` is a package-relative module path without extension.  The url keeps the slash
    /// form the device normalizes onto the dotted record.
    pub fn url(&self, spec: &str) -> String {
        // A path inside the package is relative to its source root, while the record name is
        // relative to the package directory: the root segment has to be put back.
        let root = self.source_root_relative();
        let mut path = if spec.is_empty() || spec == "." {
            self.entry.clone()
        } else {
            spec.replace(['\\', '/'], "/")
                .trim_start_matches('/')
                .to_string()
        };
        if !root.is_empty() && path != root && !path.starts_with(&format!("{root}/")) {
            path = format!("{root}/{path}");
        }
        format!("@normalized:N&&&{}/{}&{}", self.alias, path, self.version)
    }

    /// The source root as a package-relative path (`browser` for a browser bundle, empty when
    /// the package root is the source root).
    fn source_root_relative(&self) -> String {
        match self.source_root.strip_prefix(&self.dir) {
            Ok(rest) if !rest.as_os_str().is_empty() => {
                rest.to_string_lossy().replace(['\\', '/'], "/")
            }
            _ => String::new(),
        }
    }
}

/// Every package `[dependencies]` declares that has to be compiled into the abc.
///
/// Native modules and path dependencies pointing at project members are skipped: the former
/// are built by the native chain, the latter are members of their own.
pub fn discover(app: &AppConfig, project_dir: &Path) -> Result<Vec<Package>, String> {
    let lock = Lock::read(project_dir);
    let mut packages: Vec<Package> = Vec::new();
    for (name, dep) in &app.dependencies {
        let Some(dir) = resolve_dir(dep, name, project_dir, &lock)? else {
            continue;
        };
        let kind = dep
            .kind
            .as_deref()
            .and_then(Kind::parse)
            .or_else(|| manifest_kind(&dir))
            .unwrap_or_else(|| detect_kind(&dir));
        if kind == Kind::Native {
            continue;
        }
        if !dir.is_dir() {
            if dep.optional {
                continue;
            }
            // Two different mistakes land here: a `path` that points nowhere, and a
            // registry dependency that has no local copy — hvolt does not fetch packages, so
            // "check your path" is not advice the second case can act on.
            return Err(match &dep.path {
                Some(path) => format!(
                    "dependency '{name}' declares `path = \"{path}\"` but {} does not exist",
                    dir.display()
                ),
                None => format!(
                    "dependency '{name}' is a registry dependency with no local copy at {}. \
                     hvolt does not fetch packages: vendor it and declare `path = \"...\"` \
                     in [dependencies]",
                    dir.display()
                ),
            });
        }
        // A path dependency carries the version it declares; a registry dependency carries
        // the version the lock resolved, so records and imports agree on one string.
        let version = match &dep.path {
            Some(_) => dep.version.clone().unwrap_or_else(|| "1.0.0".to_string()),
            None => lock
                .version_of(name)
                .or_else(|| dep.version.clone())
                .unwrap_or_else(|| "1.0.0".to_string()),
        };
        let package = layout(name, dep, &dir, kind, version)?;
        packages.push(package);
    }
    verify_imported(app, project_dir, &packages)?;
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages.dedup_by(|a, b| a.name == b.name);
    Ok(packages)
}

/// Dependency keys every member module declares in its own `hvolt.toml`.
fn module_dependency_names(
    project_dir: &Path,
    members: &[String],
) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    for member in members {
        let path = project_dir.join(member).join("hvolt.toml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = text.parse::<toml::Value>() else {
            continue;
        };
        if let Some(table) = value.get("dependencies").and_then(|deps| deps.as_table()) {
            names.extend(table.keys().cloned());
        }
    }
    names
}

/// Fail when a source imports a third-party package `[dependencies]` does not declare.
///
/// Build time reads no hvigor manifest: the dependency table in `hvolt.toml` is the only
/// declaration. A bare import that is neither a project member nor declared there is an error
/// naming the package — never something reconstructed from a leftover `oh-package.json5`.
fn verify_imported(
    app: &AppConfig,
    project_dir: &Path,
    declared: &[Package],
) -> Result<(), String> {
    let mut imported = std::collections::BTreeSet::new();
    collect_imported_names(project_dir, &mut imported);
    // A member module declares its own dependencies in its own `hvolt.toml`; those names are
    // legal imports of the project even though the root table does not carry them.
    let module_deps = module_dependency_names(project_dir, &app.app.members);
    let mut missing: Vec<&String> = imported
        .iter()
        .filter(|name| !declared.iter().any(|package| &package.name == *name))
        .filter(|name| !app.dependencies.contains_key(*name))
        .filter(|name| !app.app.members.iter().any(|member| member == *name))
        .filter(|name| !module_deps.contains(*name))
        // Native bindings are declared by the module that builds the `.so`, and
        // `BuildProfile` is a constant module the toolchain emits.
        .filter(|name| !name.ends_with(".so") && *name != "BuildProfile")
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    missing.sort();
    let names: Vec<String> = missing.iter().map(|name| format!("'{name}'")).collect();
    Err(format!(
        "sources import {} but hvolt.toml [dependencies] does not declare it: hvolt reads no \
         oh-package.json5, so every imported package is declared in [dependencies]",
        names.join(", ")
    ))
}

/// Bare package names the project's sources reference, static or dynamic.
fn collect_imported_names(dir: &Path, out: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            // Build outputs, installed packages and credentials hold no source of the project.
            if name.starts_with('.')
                || matches!(
                    name.as_str(),
                    "build" | "oh_modules" | "node_modules" | "signing" | "output"
                )
            {
                continue;
            }
            collect_imported_names(&path, out);
            continue;
        }
        if !is_source_name(&name) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let requests = crate::build::ets_transform::module_requests(&text)
            .into_iter()
            .chain(
                dynamic_requests(&text)
                    .into_iter()
                    .map(|request| request.request),
            );
        for request in requests {
            if let Some(package) = package_name_of(&request) {
                out.insert(package);
            }
        }
    }
}

fn is_source_name(name: &str) -> bool {
    (name.ends_with(".ets") || name.ends_with(".ts"))
        && !name.ends_with(".d.ets")
        && !name.ends_with(".d.ts")
}

/// System namespaces whose first segment is not a package name.
const SYSTEM_SCOPES: &[&str] = &["@ohos", "@hms", "@system", "@arkts", "@kit"];

/// The package a bare specifier names, or `None` when it names a relative path, a system
/// module, a kit or an already-normalized url.
fn package_name_of(spec: &str) -> Option<String> {
    if spec.starts_with('.')
        || spec.starts_with('/')
        || spec.starts_with('#')
        || spec.starts_with("@normalized:")
    {
        return None;
    }
    let mut parts = spec.split('/');
    let first = parts.next()?;
    if first.is_empty() || SYSTEM_SCOPES.contains(&first) {
        return None;
    }
    if first.starts_with('@') {
        return Some(format!("{first}/{}", parts.next()?));
    }
    Some(first.to_string())
}

/// One `import('<spec>')` request: the expression form the static import scanner does not
/// cover, with the byte range of its quoted literal.
pub(crate) struct DynamicRequest {
    pub(crate) request: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Every `import('<spec>')` in `text`.
///
/// The reference normalizes these the same way a static import is normalized
/// (`import("@normalized:N&&&@cxy/sandboxfinder/Index&1.0.5")`, measured on clashbox,
/// 2026-11-06), so both the dependency scan and the staging rewrite need them. The token
/// stream decides: a string in a comment or the `import` of a member expression is not a
/// request.
pub(crate) fn dynamic_requests(text: &str) -> Vec<DynamicRequest> {
    let tokens = crate::build::lex::tokenize(text);
    let mut out = Vec::new();
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        let is_member_call = index > 0 && tokens[index - 1].is_punct('.');
        if !is_member_call
            && tokens[index].kind == crate::build::lex::Kind::Ident
            && tokens[index].text == "import"
            && tokens[index + 1].is_punct('(')
        {
            let mut next = index + 2;
            while next < tokens.len() && tokens[next].kind == crate::build::lex::Kind::Comment {
                next += 1;
            }
            if let Some(token) = tokens
                .get(next)
                .filter(|t| t.kind == crate::build::lex::Kind::Str)
            {
                out.push(DynamicRequest {
                    request: token.text.trim_matches(['"', '\'', '`']).to_string(),
                    start: token.start,
                    end: token.end,
                });
            }
        }
        index += 1;
    }
    out
}

/// Where a dependency lives: its `path`, or the package cache for a registry dependency.
///
/// A registry dependency's directory is keyed by the *resolved* version, never by the range
/// it was declared with: `^2.8.0` is not a directory name, `2.8.0` is.
fn resolve_dir(
    dep: &Dependency,
    name: &str,
    project_dir: &Path,
    lock: &Lock,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = &dep.path {
        return Ok(Some(project_dir.join(path)));
    }
    let resolved = match lock.version_of(name) {
        Some(version) => version,
        // An exact version needs no lock: it names its own directory.
        None => match dep.version.as_deref() {
            Some(version) if is_exact(version) => version.to_string(),
            Some(range) => {
                return Err(format!(
                    "dependency '{name}' = \"{range}\" needs a pinned version: hvolt does not \
                     fetch from a registry, so write the exact version (\"1.2.3\") or add \
                     `path` pointing at a vendored copy"
                ))
            }
            None => return Ok(None),
        },
    };
    let registry = dep.registry.as_deref().unwrap_or("default");
    Ok(Some(crate::build::deps::npm_cache::package_dir(
        registry, name, &resolved,
    )))
}

/// Whether a version names one release rather than a range.
pub fn is_exact(version: &str) -> bool {
    !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        && version.chars().any(|c| c.is_ascii_digit())
}

/// Detect a package's kind from its contents, per the layout table in the spec.
/// `[module]` of a package's own `hvolt.toml`, the declaration a flattened dependency ships.
fn package_manifest(dir: &Path) -> Option<toml::Value> {
    let text = std::fs::read_to_string(dir.join("hvolt.toml")).ok()?;
    let value: toml::Value = text.parse().ok()?;
    value.get("module").cloned()
}

/// The kind a package declares for itself.
fn manifest_kind(dir: &Path) -> Option<Kind> {
    let module = package_manifest(dir)?;
    match module.get("type").and_then(|value| value.as_str())? {
        "bytecode" | "byte-code" => Some(Kind::ByteCode),
        "arkts" => Some(Kind::Arkts),
        "native" => Some(Kind::Native),
        "js" | "javascript" => Some(Kind::Js),
        _ => None,
    }
}

/// The entry a package declares for itself.
fn manifest_main(dir: &Path) -> Option<String> {
    let module = package_manifest(dir)?;
    module
        .get("main")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn detect_kind(dir: &Path) -> Kind {
    // A package that ships its own bytecode and no implementation source has to be merged,
    // not compiled: compiling it would only see the `.d.ets` declarations.
    if (dir.join("modules.abc").is_file() || dir.join("ets/modules.abc").is_file())
        && !has_implementation_sources(dir)
    {
        return Kind::ByteCode;
    }
    if dir.join("src/main/ets").is_dir() || dir.join("Index.ets").is_file() {
        return Kind::Arkts;
    }
    if dir.join("CMakeLists.txt").is_file() || dir.join("cpp").is_dir() {
        return Kind::Native;
    }
    Kind::Js
}

/// Read a package's own layout to place its sources and its entry.
fn layout(
    name: &str,
    dep: &Dependency,
    dir: &Path,
    kind: Kind,
    version: String,
) -> Result<Package, String> {
    let alias = dep.import_name(name).to_string();
    let (source_root, entry) = match kind {
        Kind::Arkts => {
            let ets = dir.join("src").join("main").join("ets");
            // The declared entry decides the root: a package may keep `index.ets` at its own
            // root while every component lives under `src/main/ets`. Rooting at
            // `src/main/ets` in that case loses the entry file and names the records after a
            // path that never exists. The declaration is `[dependencies].entry`.
            let declared = dep
                .entry
                .as_deref()
                .map(|entry| entry.trim_start_matches("./").to_string())
                .filter(|entry| dir.join(entry).is_file());
            if let Some(main) = declared {
                return Ok(Package {
                    name: name.to_string(),
                    alias,
                    dir: dir.to_path_buf(),
                    version,
                    source_root: dir.to_path_buf(),
                    entry: strip_source(&main),
                    declared: dep.entry.clone(),
                    kind,
                });
            }
            let root = if ets.is_dir() { ets } else { dir.to_path_buf() };
            let entry = dep
                .entry
                .clone()
                .or_else(|| manifest_main(dir))
                .map(|e| strip_source(&e))
                .unwrap_or_else(default_arkts_entry);
            (root, entry)
        }
        Kind::Js => {
            // A package that ships a browser bundle is compiled from that bundle alone: its
            // Node build beside it resolves `require` and node builtins, which no device has.
            let browser = dir.join("browser");
            let (root, entry) = if browser.join("dist/index.js").is_file() {
                // A declaration names the entry relative to the package root; the compiler
                // roots a browser bundle at `browser/`, so that prefix is dropped here.
                let entry = dep
                    .entry
                    .clone()
                    .or_else(|| manifest_main(dir))
                    .map(|e| relative_to_root(&e, &browser, dir))
                    .unwrap_or_else(|| "dist/index".to_string());
                (browser, entry)
            } else {
                let entry = dep
                    .entry
                    .clone()
                    .or_else(|| manifest_main(dir))
                    .or_else(|| js_entry(dir))
                    .unwrap_or_else(|| "index".to_string());
                (dir.to_path_buf(), strip_js_ext(&entry))
            };
            (root, entry)
        }
        Kind::Native | Kind::ByteCode => (dir.to_path_buf(), "Index".to_string()),
    };
    Ok(Package {
        name: name.to_string(),
        alias,
        dir: dir.to_path_buf(),
        version,
        source_root,
        entry,
        declared: dep.entry.clone(),
        kind,
    })
}

/// Whether a package carries `.ets`/`.ts` files that are not declarations.
fn has_implementation_sources(dir: &Path) -> bool {
    fn walk(dir: &Path, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if walk(&path, depth + 1) {
                    return true;
                }
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_source = matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ets" | "ts")
            );
            if is_source && !name.ends_with(".d.ets") && !name.ends_with(".d.ts") {
                return true;
            }
        }
        false
    }
    walk(dir, 0)
}

/// The names a package declares as types (`interface` / `type`) across its declaration files.
///
/// A byte-code HAR's abc carries no record for them — the compiler elided the declarations
/// when it built the HAR — so an importer that keeps them as value imports fails at link time.
pub fn type_declarations(package: &Package) -> String {
    let mut out = String::new();
    collect_declarations(&package.dir, &mut out, 0);
    out
}

/// The names a directory declares as types, across its `.d.ets`/`.d.ts` files.
pub fn type_declarations_in(dir: &std::path::Path) -> String {
    let mut out = String::new();
    collect_declarations(dir, &mut out, 0);
    out
}

fn collect_declarations(dir: &Path, out: &mut String, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_declarations(&path, out, depth + 1);
            continue;
        }
        // Every source file declares its types inline; a package that ships only
        // declarations keeps them in `.d.ets`. Both count.
        let is_source = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ets" | "ts")
        );
        if !is_source {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for keyword in ["interface", "type"] {
            let mut from = 0;
            while let Some(pos) = text[from..].find(keyword) {
                let at = from + pos;
                from = at + keyword.len();
                let rest = text[from..].trim_start();
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if !name.is_empty() {
                    out.push_str(&format!("{keyword} {name}\n"));
                }
            }
        }
    }
}

/// The file a package's resolved entry maps to, if the package ships one.
///
/// Packages ship several files that map to the same staged name (`index.ets`, `index.ts`,
/// `index.js`); the declared entry is the implementation and has to win that collision. The
/// declaration is `[dependencies].entry` in `hvolt.toml`, never a hvigor manifest.
pub fn entry_file_name(package: &Package) -> Option<String> {
    let stem = package.entry.rsplit('/').next()?;
    for ext in ["ets", "ts", "js"] {
        let candidate = format!("{stem}.{ext}");
        if package.source_root.join(&candidate).is_file() || package.dir.join(&candidate).is_file()
        {
            return Some(candidate);
        }
    }
    None
}

/// The prebuilt abc a byte-code HAR ships: `modules.abc` at the package root, or the
/// `ets/modules.abc` an ohpm store copy still carries.
pub fn abc_path(package: &Package) -> PathBuf {
    let flat = package.dir.join("modules.abc");
    if flat.is_file() {
        return flat;
    }
    package.dir.join("ets").join("modules.abc")
}

/// Drop a source-root prefix from a declared entry, and its extension.
fn relative_to_root(entry: &str, root: &Path, dir: &Path) -> String {
    let stem = strip_js_ext(entry);
    let Ok(rest) = root.strip_prefix(dir) else {
        return stem;
    };
    let prefix = rest.to_string_lossy().replace('\\', "/");
    if prefix.is_empty() {
        return stem;
    }
    match stem.strip_prefix(&format!("{prefix}/")) {
        Some(tail) => tail.to_string(),
        None => stem,
    }
}

/// An ArkTS package's entry: `Index`, the name every component library ships.
fn default_arkts_entry() -> String {
    "Index".to_string()
}

/// A JavaScript package's entry, from the tree it ships: its browser build, else `index`.
fn js_entry(dir: &Path) -> Option<String> {
    if dir.join("browser/dist/index.js").is_file() {
        return Some("browser/dist/index".to_string());
    }
    dir.join("index.js").is_file().then(|| "index".to_string())
}

/// Drop the source-root prefix and the extension from a declared entry.
fn strip_source(entry: &str) -> String {
    let mut rest = entry.trim_start_matches("./");
    for prefix in ["src/main/ets/", "src/main/"] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped;
            break;
        }
    }
    strip_js_ext(rest)
}

fn strip_js_ext(entry: &str) -> String {
    let entry = entry.trim_start_matches("./");
    for suffix in [".ets", ".ts", ".js", ".mjs", ".cjs"] {
        if let Some(stem) = entry.strip_suffix(suffix) {
            return stem.to_string();
        }
    }
    entry.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_parses_the_declared_type() {
        assert_eq!(Kind::parse("arkts"), Some(Kind::Arkts));
        assert_eq!(Kind::parse("JS"), Some(Kind::Js));
        assert_eq!(Kind::parse("native"), Some(Kind::Native));
        assert_eq!(Kind::parse("wasm"), None);
    }

    #[test]
    fn entry_strips_the_source_root_and_extension() {
        assert_eq!(strip_source("./src/main/ets/Index.ets"), "Index");
        assert_eq!(strip_source("Index.ets"), "Index");
        assert_eq!(strip_js_ext("./dist/index.js"), "dist/index");
    }

    #[test]
    fn record_prefix_keeps_the_source_root_only_for_arkts_packages() {
        let arkts = Package {
            name: "@cxy/sandboxfinder".into(),
            alias: "@cxy/sandboxfinder".into(),
            dir: PathBuf::from("/p"),
            version: "1.0.5".into(),
            source_root: PathBuf::from("/p/src/main/ets"),
            entry: "Index".into(),
            declared: None,
            kind: Kind::Arkts,
        };
        assert_eq!(
            arkts.record_prefix_slash(),
            "@cxy/sandboxfinder/src/main/ets"
        );
        assert_eq!(
            arkts.staging_dir(),
            "oh_modules/.ohpm/@cxy+sandboxfinder@1.0.5/oh_modules/@cxy/sandboxfinder/src/main/ets"
        );

        let js = Package {
            name: "yaml".into(),
            alias: "yaml".into(),
            dir: PathBuf::from("/p"),
            version: "2.8.0".into(),
            source_root: PathBuf::from("/p/browser"),
            entry: "dist/index".into(),
            declared: None,
            kind: Kind::Js,
        };
        assert_eq!(js.record_prefix_slash(), "yaml/browser");
        assert_eq!(js.url(""), "@normalized:N&&&yaml/browser/dist/index&2.8.0");
        assert_eq!(
            js.url("dist/util"),
            "@normalized:N&&&yaml/browser/dist/util&2.8.0"
        );
        assert_eq!(
            js.url("browser/dist/util"),
            "@normalized:N&&&yaml/browser/dist/util&2.8.0"
        );
        assert_eq!(
            js.staging_dir(),
            "oh_modules/.ohpm/yaml@2.8.0/oh_modules/yaml/browser"
        );
        assert_eq!(js.package_rel("dist/index.js"), "browser/dist/index.js");
    }

    #[test]
    fn dynamic_imports_are_read_from_the_token_stream() {
        let text = "const a = import('@cxy/x');\n\
                    // import('@ignored/y')\n\
                    const b = `import('@ignored/z')`;\n\
                    const c = holder.import('@ignored/w');\n\
                    import(\n  /* keep */ '@scope/y'\n);\n";
        let requests: Vec<String> = super::dynamic_requests(text)
            .into_iter()
            .map(|request| request.request)
            .collect();
        assert_eq!(requests, vec!["@cxy/x", "@scope/y"], "{requests:?}");
    }

    #[test]
    fn only_bare_package_specifiers_name_a_package() {
        assert_eq!(package_name_of("@cxy/x"), Some("@cxy/x".into()));
        assert_eq!(package_name_of("yaml"), Some("yaml".into()));
        assert_eq!(package_name_of("yaml/dist/index"), Some("yaml".into()));
        assert_eq!(package_name_of("./local"), None);
        assert_eq!(package_name_of("@ohos.animator"), None);
        assert_eq!(package_name_of("@kit.ArkUI"), None);
        assert_eq!(
            package_name_of("@normalized:N&&&yaml/dist/index&2.8.0"),
            None
        );
    }
}

/// The resolved dependency versions a build has to agree with.
///
/// `hvolt.lock` pins resolved versions; until it exists every ranged dependency has
/// to be declared with an exact version, which is a fact the declaration itself carries.
#[derive(Debug, Default)]
pub struct Lock {
    resolved: std::collections::BTreeMap<String, String>,
}

impl Lock {
    /// Read `hvolt.lock` beside the project; a missing or unreadable lock resolves nothing.
    pub fn read(project_dir: &Path) -> Self {
        let text = std::fs::read_to_string(project_dir.join("hvolt.lock")).unwrap_or_default();
        let mut resolved = std::collections::BTreeMap::new();
        let mut current: Option<String> = None;
        for line in text.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if key == "name" {
                current = Some(value.to_string());
            } else if key == "version" {
                if let Some(name) = current.take() {
                    resolved.insert(name, value.to_string());
                }
            }
        }
        Self { resolved }
    }

    pub fn version_of(&self, name: &str) -> Option<String> {
        self.resolved.get(name).cloned()
    }
}
