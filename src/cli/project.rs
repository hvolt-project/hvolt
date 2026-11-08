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
        std::process::exit(2);
    }
    crate::project::scaffold::init(&name);
    println!("✓ Created hvolt project: {}/", name);
    println!("  Next: cd {} && hvolt build", name);
}

pub(crate) fn migrate(from: Option<PathBuf>, out: Option<PathBuf>, dry_run: bool) {
    if let Err(e) = crate::project::import::run(from.as_deref(), out.as_deref(), dry_run) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

pub(crate) fn doctor() {
    let rep = crate::project::doctor::run(std::path::Path::new("."));
    std::process::exit(rep.finish());
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

pub(crate) fn check(package: Option<PathBuf>, size_limit: Option<u64>) {
    if let Err(e) = crate::project::check::check_package(
        std::path::Path::new("."),
        package.as_deref(),
        size_limit,
    ) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
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
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ {e}");
            std::process::exit(1);
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
        std::process::exit(2);
    }
    if let Err(e) = crate::support::lsp::serve(sdk.as_deref()) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
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
