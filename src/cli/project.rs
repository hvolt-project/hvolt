// Project: init, import, doctor, clean, check, reports, parse checks, language server.
use std::path::PathBuf;

pub(crate) fn init(name: String) {
    // validate the GENERATED bundle segment against the official
    // schema semantics, not the raw name charset — and keep nested
    // paths working (scaffold.plan takes the basename;
    // the previous charset check both missed _abc/abc_ and locked
    // `init examples/test-app`)
    if let Err(e) = crate::project::scaffold::validate_project_name(&name) {
        eprintln!("✗ {}", e);
        crate::support::cli_exit(2);
    }
    crate::project::scaffold::init(&name);
    println!("✓ Created hvolt project: {}/", name);
    println!("  Next: cd {} && hvolt build", name);
}

pub(crate) fn migrate(from: Option<PathBuf>, out: Option<PathBuf>, dry_run: bool) {
    if let Err(e) = crate::project::import::run(from.as_deref(), out.as_deref(), dry_run) {
        eprintln!("✗ {}", e);
        crate::support::cli_exit(1);
    }
}

pub(crate) fn dep(action: crate::support::cli::DepAction) {
    match action {
        crate::support::cli::DepAction::Sync { project } => sync_deps(project),
    }
}

/// `hvolt dep sync`: registry packages on disk → `deps/<scope>/<name>/`.
fn sync_deps(project: Option<PathBuf>) {
    let dir = project.unwrap_or_else(|| PathBuf::from("."));
    match crate::project::dep_sync::sync(&dir) {
        Ok(moved) if moved.is_empty() => println!("no registry dependency to move"),
        Ok(moved) => {
            for item in &moved {
                println!(
                    "✓ {} → {} (from {})",
                    item.package,
                    item.dest,
                    item.from.display()
                );
            }
            println!("{} package(s) now live under deps/", moved.len());
        }
        Err(e) => {
            eprintln!("✗ {}", e);
            crate::support::cli_exit(1);
        }
    }
}

pub(crate) fn doctor(product: Option<String>) {
    let rep = crate::project::doctor::run(std::path::Path::new("."), product.as_deref());
    crate::support::cli_exit(rep.finish());
}

pub(crate) fn clean() {
    let build_dir = std::path::Path::new(".").join("build");
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir).expect("clean build dir");
        println!("✓ Cleaned build/");
    } else {
        println!("✓ Already clean (no build/)");
    }
}

/// Print a `resources.index` as JSON: the structure the oracle compares.
///
/// Two builds of one project must agree on what the index *says* — every config, every
/// `(type, name)` entry, its value bytes, and which entries share an id — while the id numbers
/// themselves are the producing tool's business (the reference numbers resources in its own
/// order, see `compare-artifacts.py`).
pub(crate) fn index_dump(file: PathBuf) {
    let data = match std::fs::read(&file) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("{}: {error}", file.display());
            crate::support::cli_exit(1);
        }
    };
    let index = match crate::build::artifacts::res_index::parse(&data) {
        Ok(index) => index,
        Err(error) => {
            eprintln!("{}: {error}", file.display());
            crate::support::cli_exit(1);
        }
    };
    let mut out = String::new();
    out.push_str("{\"version\":");
    out.push_str(&json_string(&index.version));
    out.push_str(",\"configs\":[");
    for (at, config) in index.configs.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        out.push_str("{\"qualifiers\":[");
        for (pos, qualifier) in config.qualifiers.iter().enumerate() {
            if pos > 0 {
                out.push(',');
            }
            out.push_str(&format!("[{},{}]", qualifier.kind, qualifier.value));
        }
        out.push_str("],\"entries\":[");
        for (pos, entry) in config.entries.iter().enumerate() {
            if pos > 0 {
                out.push(',');
            }
            // The value is compared by digest, not by length: two values of the same length
            // are not the same value.
            let digest = {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&entry.value);
                format!("{:x}", hasher.finalize())
            };
            out.push_str(&format!(
                "{{\"id\":{},\"type\":\"{:?}\",\"name\":{},\"value\":{},\"value_len\":{}}}",
                entry.id,
                entry.kind,
                json_string(&entry.name),
                json_string(&digest[..16]),
                entry.value.len()
            ));
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    println!("{out}");
}

/// A JSON string literal for `text` (the output never carries control bytes: names and version
/// strings, both read from the file's own pool).
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub(crate) fn check(package: Option<PathBuf>, size_limit: Option<u64>) {
    // Source-level names first: the package gate only sees files, and a package can be perfectly
    // well formed while a file inside it calls a name that was never imported.
    if crate::build::stage::semantic_check::enabled() {
        let (diagnostics, read, skipped) =
            crate::build::stage::semantic_check::check_sources(std::path::Path::new("."));
        for diagnostic in &diagnostics {
            println!(
                "{}:{}:{}: error: {}",
                diagnostic.file, diagnostic.line, diagnostic.column, diagnostic.message
            );
        }
        if skipped > 0 {
            println!(
                "semantic: {read} source(s), {skipped} not readable without lowering \
                 (checked during build)"
            );
        }
        if !diagnostics.is_empty() {
            eprintln!("✗ {} undefined identifier(s)", diagnostics.len());
            crate::support::cli_exit(1);
        }
    }
    if let Err(e) = crate::project::check::check_package(
        std::path::Path::new("."),
        package.as_deref(),
        size_limit,
    ) {
        eprintln!("✗ {}", e);
        crate::support::cli_exit(1);
    }
}

pub(crate) fn export_report(ts_root: PathBuf) {
    match crate::build::artifacts::export_check::scan(&ts_root) {
        Ok(missing) if missing.is_empty() => {
            println!("✓ no export mismatches in {}", ts_root.display());
        }
        Ok(missing) => {
            println!(
                "{} import(s) name something the target does not export:",
                missing.len()
            );
            for m in &missing {
                println!("  {}: {} from {}", m.from, m.name, m.spec);
            }
            crate::support::cli_exit(1);
        }
        Err(e) => {
            eprintln!("✗ {e}");
            crate::support::cli_exit(1);
        }
    }
}

pub(crate) fn parse_report(dir: Option<String>) {
    fn collect_ets(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                // `build/` holds staged copies of the same sources.
                if p.file_name().and_then(|n| n.to_str()) == Some("build") {
                    continue;
                }
                collect_ets(&p, out);
            } else if p.extension().is_some_and(|e| e == "ets")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".d.ets"))
                    .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    let root = std::path::PathBuf::from(dir.unwrap_or_else(|| ".".to_string()));
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_ets(&root, &mut files);
    files.sort();
    // Cross-module component names decide whether `Foo({...}) { ... }` is a call
    // block, exactly as in the real build; collect them before parsing.
    let mut project_builders: Vec<String> = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        for line in src.lines() {
            let t = line.trim_start();
            for prefix in ["struct ", "export struct "] {
                if let Some(rest) = t.strip_prefix(prefix) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() && !project_builders.contains(&name) {
                        project_builders.push(name);
                    }
                }
            }
        }
    }
    let mut ok = 0usize;
    let mut failures: Vec<(std::path::PathBuf, String)> = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        match crate::build::lower::ets_component::parse_components(&src, &project_builders) {
            Ok(_) => ok += 1,
            Err(e) => failures.push((f.clone(), e)),
        }
    }
    println!(
        "files: {}  parsed: {}  failed: {}",
        files.len(),
        ok,
        failures.len()
    );
    for (f, e) in &failures {
        println!("✗ {}", f.display());
        for line in e.lines() {
            println!("    {}", line);
        }
    }
}

pub(crate) fn lsp(stdio: bool, sdk: Option<String>) {
    if !stdio {
        eprintln!("✗ hvolt lsp speaks LSP over stdio; pass --stdio");
        crate::support::cli_exit(2);
    }
    if let Err(e) = crate::support::lsp::serve(sdk.as_deref()) {
        eprintln!("✗ {}", e);
        crate::support::cli_exit(1);
    }
}

/// Recursively copy a directory tree.
#[allow(dead_code)] // used by the scaffold copy path only
fn copy_dir_flat(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("read {}: {e}", src.display()))?
        .flatten()
    {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_flat(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}
