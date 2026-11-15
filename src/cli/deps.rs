//! The package cache: what is in it, whether it is intact, and how to reclaim it.
//!
//! The cache is shared by every project on the machine and survives `hvolt clean`, which is why
//! it gets its own report and its own tidy-up: an entry is only removed when a person asks for
//! it, and `prune` prints what it would delete unless it is told to act.

use std::fs;
use std::path::{Path, PathBuf};

use crate::build::deps::npm_cache;

fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}{}", bytes, UNITS[unit])
    } else {
        format!("{:.1}{}", value, UNITS[unit])
    }
}

/// Every version directory under `<root>/<registry>/<name...>/<version>`.
///
/// A package name may carry a scope (`@ohos/hypium`), so the depth is not fixed: a version
/// directory is a leaf, and everything between the registry and it is the name.
fn entries() -> Result<Vec<(String, String, String, PathBuf)>, String> {
    let root = npm_cache::root();
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    let registries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return Ok(out),
    };
    for registry in registries.flatten() {
        if !registry.path().is_dir() {
            continue;
        }
        let registry_name = registry.file_name().to_string_lossy().to_string();
        collect(registry.path(), &mut vec![registry_name], &mut out);
    }
    out.sort();
    Ok(out)
}

/// A version directory is named like a release and sits below the registry and the name.
///
/// Depth alone cannot decide it: a scoped name (`@ohos/hypium`) is two segments, and a package's
/// own tree has leaf directories of its own. The naming shape is what separates the two, and a
/// match stops the walk so a package's insides are never mistaken for further packages.
fn looks_like_version(segment: &str) -> bool {
    let mut parts = 0;
    for piece in segment.split('.') {
        if piece.is_empty() || !piece.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        parts += 1;
    }
    parts >= 2 && !segment.is_empty()
}

fn collect(dir: PathBuf, rel: &mut Vec<String>, out: &mut Vec<(String, String, String, PathBuf)>) {
    if rel.len() >= 3 {
        let last = rel[rel.len() - 1].clone();
        if looks_like_version(&last) {
            let registry = rel[0].clone();
            let name = rel[1..rel.len() - 1].join("/");
            out.push((registry, name, last, dir));
            return;
        }
    }
    let mut children: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                children.push(path);
            }
        }
    }
    if children.is_empty() {
        return;
    }
    for child in children {
        let segment = child
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        rel.push(segment);
        collect(child, rel, out);
        rel.pop();
    }
}

fn size_of(path: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => stack.push(path),
                Ok(meta) => total += meta.len(),
                Err(_) => {}
            }
        }
    }
    total
}

pub(crate) fn list() {
    let found = match entries() {
        Ok(found) => found,
        Err(e) => {
            eprintln!("✗ {e}");
            crate::support::cli_exit(1);
        }
    };
    if found.is_empty() {
        println!(
            "dependencies: cache is empty at {}",
            npm_cache::root().display()
        );
        return;
    }
    let mut total = 0;
    for (registry, name, version, path) in &found {
        let size = size_of(path);
        total += size;
        println!("{registry}/{name}@{version}  {}", human(size));
    }
    println!("{} package(s), {}", found.len(), human(total));
}

pub(crate) fn verify() {
    let found = match entries() {
        Ok(found) => found,
        Err(e) => {
            eprintln!("✗ {e}");
            crate::support::cli_exit(1);
        }
    };
    let mut bad = 0;
    for (registry, name, version, path) in &found {
        let manifest = path.join("oh-package.json5");
        if !manifest.is_file() {
            println!("✗ {registry}/{name}@{version}: no oh-package.json5");
            bad += 1;
            continue;
        }
        let text = fs::read_to_string(&manifest).unwrap_or_default();
        if text.trim().is_empty() {
            println!("✗ {registry}/{name}@{version}: oh-package.json5 is empty");
            bad += 1;
        }
    }
    if bad == 0 {
        println!("✓ {} package(s) intact", found.len());
    } else {
        println!(
            "{bad} of {} package(s) incomplete; rerun the build to fetch them again",
            found.len()
        );
        crate::support::cli_exit(1);
    }
}

pub(crate) fn prune(act: bool) {
    match crate::build::deps::registry::sweep_leftovers() {
        Ok(0) => {}
        Ok(swept) => println!("swept {swept} leftover lock/unpack directory(ies)"),
        Err(e) => eprintln!("✗ {e}"),
    }
    let found = match entries() {
        Ok(found) => found,
        Err(e) => {
            eprintln!("✗ {e}");
            crate::support::cli_exit(1);
        }
    };
    let mut total = 0;
    for (registry, name, version, path) in &found {
        total += size_of(path);
        if act {
            match fs::remove_dir_all(path) {
                Ok(()) => println!("removed {registry}/{name}@{version}"),
                Err(e) => println!("✗ {registry}/{name}@{version}: {e}"),
            }
        } else {
            println!("would remove {registry}/{name}@{version}");
        }
    }
    if found.is_empty() {
        println!("nothing to prune");
    } else if act {
        println!("removed {} package(s), {}", found.len(), human(total));
    } else {
        println!(
            "{} package(s), {} — rerun with --yes to remove",
            found.len(),
            human(total)
        );
    }
}
