//! Third-party packages the app declares in `hvolt.toml` `[dependencies]`.
//!
//! A HAP is not only its own modules: `import { parse } from 'yaml'` names a package the
//! device expects to find as `yaml.browser.dist.index` inside the merged abc.  The flat
//! project lists those packages in `[dependencies]`; `hvolt.toml` is the configuration
//! source, so a package's hvigor manifests (`oh-package.json5`, `package.json`) are read as
//! *content* to locate its sources.
//!
//! One exception to that division: a package the sources import while `hvolt.toml` does not
//! declare it (`hvolt migrate` can lose a declaration) is taken from the project's own
//! `oh-package.json5`. There the manifest is used as the declaration the flat config is
//! missing — a name the sources merely mention (a typo, a system module) does not qualify.

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
    packages.extend(discover_imported(app, project_dir, &packages)?);
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages.dedup_by(|a, b| a.name == b.name);
    Ok(packages)
}

/// Packages the project's sources import while `[dependencies]` does not declare them.
///
/// `hvolt migrate` can drop a declaration (measured on clashbox, 2026-11-06: `EntryAbility.ets`
/// imports `@cxy/sandboxfinder` dynamically and the project's `oh-package.json5` lists it,
/// but the generated `hvolt.toml` never mentions it). The import alone is not evidence — a
/// typo, a system module or a member of the project itself reads the same way — so a package
/// is admitted only when the project's own `oh-package.json5` lists it under `dependencies`.
fn discover_imported(
    app: &AppConfig,
    project_dir: &Path,
    declared: &[Package],
) -> Result<Vec<Package>, String> {
    let manifest = manifest_dependencies(&project_dir.join("oh-package.json5"));
    if manifest.is_empty() {
        return Ok(Vec::new());
    }
    let mut imported = std::collections::BTreeSet::new();
    collect_imported_names(project_dir, &mut imported);
    let mut out = Vec::new();
    for (name, spec) in &manifest {
        if declared.iter().any(|package| &package.name == name)
            || app.dependencies.contains_key(name)
            || app.app.members.iter().any(|member| member == name)
            || !imported.contains(name)
            || spec.starts_with("file:")
            || spec.starts_with("link:")
        {
            continue;
        }
        let Some(dir) =
            installed_package_dir(project_dir, name).or_else(|| cached_package_dir(name, spec))
        else {
            continue;
        };
        let kind = detect_kind(&dir);
        if kind == Kind::Native {
            continue;
        }
        // The installed package's manifest carries the version it was resolved to; the
        // declaration only carries the range (`^1.0.5`).
        let version = read_manifest_field(&dir, "version")
            .or_else(|| exact_version(spec))
            .unwrap_or_else(|| "1.0.0".to_string());
        let dep = Dependency {
            path: None,
            version: Some(spec.clone()),
            registry: None,
            kind: None,
            entry: None,
            alias: None,
            optional: false,
        };
        let package = layout(name, &dep, &dir, kind, version)
            .map_err(|e| format!("dependency '{name}' is imported but cannot be laid out: {e}"))?;
        out.push(package);
    }
    Ok(out)
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

/// `dependencies` of an `oh-package.json5`, as `name → declared range`.
///
/// The file is JSON5 (comments, trailing commas, single quotes), so the object is read
/// structurally instead of through `serde_json`.
fn manifest_dependencies(path: &Path) -> std::collections::BTreeMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Default::default();
    };
    let Some(start) = text
        .find("\"dependencies\"")
        .or_else(|| text.find("dependencies"))
    else {
        return Default::default();
    };
    let Some(open) = text[start..].find('{').map(|at| start + at) else {
        return Default::default();
    };
    let mut out = std::collections::BTreeMap::new();
    let mut index = open + 1;
    while let Some((key, after_key)) = json5_string(&text, index) {
        let Some(colon) = text[after_key..].find(':').map(|at| after_key + at) else {
            break;
        };
        let Some((value, after_value)) = json5_string(&text, colon + 1) else {
            break;
        };
        out.insert(key, value);
        index = after_value;
    }
    out
}

/// The quoted string at or after `from`, with the index just past its closing quote.
fn json5_string(text: &str, from: usize) -> Option<(String, usize)> {
    let index = skip_json5(text, from);
    let quote = text[index..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let start = index + quote.len_utf8();
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            let end = start + offset;
            return Some((text[start..end].to_string(), end + ch.len_utf8()));
        }
    }
    None
}

/// The index of the next JSON5 value: whitespace, commas and comments are skipped.
fn skip_json5(text: &str, from: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = from;
    loop {
        match bytes.get(index) {
            Some(b' ' | b'\t' | b'\r' | b'\n' | b',') => index += 1,
            Some(b'/') if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            Some(b'/') if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            _ => return index,
        }
    }
}

/// `<project>/oh_modules/<name>`, or the `.ohpm` store entry for it.
fn installed_package_dir(project_dir: &Path, name: &str) -> Option<PathBuf> {
    let flat = project_dir.join("oh_modules").join(name);
    if flat.is_dir() {
        return Some(flat);
    }
    let escaped = name.replace('/', "+");
    let store = project_dir.join("oh_modules").join(".ohpm");
    for entry in std::fs::read_dir(store).ok()?.flatten() {
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if dir_name.starts_with(&format!("{escaped}@")) || dir_name == escaped {
            let nested = entry.path().join("oh_modules").join(name);
            if nested.is_dir() {
                return Some(nested);
            }
        }
    }
    None
}

/// The package cache entry for a declared range, when the package is not installed beside
/// the project: `<cache>/<registry>/<name>/<version>`.
fn cached_package_dir(name: &str, spec: &str) -> Option<PathBuf> {
    if let Some(version) = exact_version(spec) {
        let dir = crate::build::deps::npm_cache::package_dir("default", name, &version);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    let mut versions: Vec<PathBuf> = std::fs::read_dir(crate::build::deps::npm_cache::package_dir(
        "default", name, "",
    ))
    .ok()?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| path.is_dir())
    .collect();
    versions.sort();
    versions.pop()
}

/// The exact version a declaration pins (`^1.0.5` → `1.0.5`), or `None` for a range no
/// directory is named after.
fn exact_version(spec: &str) -> Option<String> {
    let trimmed = spec
        .trim()
        .trim_start_matches(['^', '~', '=', '>', '<', ' ']);
    is_exact(trimmed).then(|| trimmed.to_string())
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
fn detect_kind(dir: &Path) -> Kind {
    // A package that ships its own bytecode and no implementation source has to be merged,
    // not compiled: compiling it would only see the `.d.ets` declarations.
    if dir.join("ets/modules.abc").is_file() && !has_implementation_sources(dir) {
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
            // The declared entry decides the root: a package may keep `index.ets` beside its
            // manifest while every component lives under `src/main/ets`.  Rooting at
            // `src/main/ets` in that case loses the entry file and names the records after a
            // path that never exists.
            let declared = read_manifest_field(dir, "main")
                .map(|m| m.trim_start_matches("./").to_string())
                .filter(|m| dir.join(m).is_file());
            if dep.entry.is_none() {
                if let Some(main) = declared {
                    return Ok(Package {
                        name: name.to_string(),
                        alias,
                        dir: dir.to_path_buf(),
                        version,
                        source_root: dir.to_path_buf(),
                        entry: strip_source(&main),
                        kind,
                    });
                }
            }
            let root = if ets.is_dir() { ets } else { dir.to_path_buf() };
            let entry = dep
                .entry
                .clone()
                .map(|e| strip_source(&e))
                .unwrap_or_else(|| default_arkts_entry(dir, &root));
            (root, entry)
        }
        Kind::Js => {
            // A package that ships a browser bundle is compiled from that bundle alone: its
            // Node build beside it resolves `require` and node builtins, which no device has.
            let browser = dir.join("browser");
            let (root, entry) = if browser.join("dist/index.js").is_file() {
                let entry = dep
                    .entry
                    .clone()
                    .map(|e| strip_js_ext(&e))
                    .unwrap_or_else(|| "dist/index".to_string());
                (browser, entry)
            } else {
                let entry = dep
                    .entry
                    .clone()
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

/// The file a package declares as its entry (`main` in its manifest), if any.
///
/// Packages ship several files that map to the same staged name (`index.ets`, `index.ts`,
/// `index.js`); the declared one is the implementation and has to win that collision.
pub fn entry_file_name(package: &Package) -> Option<String> {
    let declared = read_manifest_field(&package.dir, "main")?;
    let name = declared
        .trim_start_matches("./")
        .rsplit('/')
        .next()
        .unwrap_or(&declared)
        .to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// The prebuilt abc a byte-code HAR ships.
pub fn abc_path(package: &Package) -> PathBuf {
    package.dir.join("ets").join("modules.abc")
}

/// An ArkTS package's entry: its manifest `main`, else `Index`.
fn default_arkts_entry(dir: &Path, source_root: &Path) -> String {
    let declared = read_manifest_field(dir, "main");
    if let Some(main) = declared {
        let stem = strip_source(&main);
        let candidate = source_root.join(format!("{stem}.ets"));
        if candidate.is_file() || source_root.join(format!("{stem}.ts")).is_file() {
            return stem;
        }
    }
    "Index".to_string()
}

/// A JavaScript package's entry, from its manifest `module` / `main`, else its browser build.
fn js_entry(dir: &Path) -> Option<String> {
    for key in ["module", "main"] {
        if let Some(value) = read_manifest_field(dir, key) {
            return Some(strip_js_ext(&value));
        }
    }
    // A package that ships a browser bundle is addressed through that bundle.
    if dir.join("browser/dist/index.js").is_file() {
        return Some("browser/dist/index".to_string());
    }
    None
}

/// The first string value of a top-level `"key"` in a package manifest.
///
/// The manifest is content here, not configuration: it says where the package put its files.
fn read_manifest_field(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("oh-package.json5"))
        .or_else(|_| std::fs::read_to_string(dir.join("package.json")))
        .ok()?;
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let open = rest.find('"')?;
    let rest = &rest[open + 1..];
    let close = rest.find('"')?;
    let value = rest[..close].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
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

    #[test]
    fn manifest_dependencies_reads_json5_declarations() {
        let path = std::env::temp_dir().join(format!("hvolt-manifest-{}", std::process::id()));
        std::fs::write(
            &path,
            "{\n  // the store\n  \"dependencies\": {\n    \"yaml\": \"^2.8.0\",\n    '@cxy/x': '^1.0.5',\n  },\n  \"devDependencies\": { \"@ohos/hypium\": \"1.0.19\" }\n}\n",
        )
        .expect("write manifest");
        let deps = super::manifest_dependencies(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(deps.get("yaml").map(String::as_str), Some("^2.8.0"));
        assert_eq!(deps.get("@cxy/x").map(String::as_str), Some("^1.0.5"));
        assert!(!deps.contains_key("@ohos/hypium"), "{deps:?}");
        assert_eq!(deps.len(), 2, "{deps:?}");
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
