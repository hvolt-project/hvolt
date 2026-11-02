// hap_pack: assemble a HAP from hvigor's intermediate outputs.
//
// Replaces `java -jar app_packing_tool.jar --mode hap`. The container is a zip whose entries are
// all STORED (measured: app_packing_tool never deflates HAP content), with a fixed entry order and
// a single timestamp shared by every entry. The layout below mirrors what the tool emits, which is
// also what the device's installer expects.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Where each part of the package comes from, mirroring `app_packing_tool.jar`'s flags.
pub struct PackInputs {
    /// `--json-path`: the processed module.json.
    pub module_json: PathBuf,
    /// `--index-path`: compiled resource index.
    pub resources_index: PathBuf,
    /// `--resources-path`: compiled resource tree.
    pub resources_dir: PathBuf,
    /// `--lib-path`: stripped native libraries (`<abi>/*.so`, no `libs/` level).
    pub lib_dir: PathBuf,
    /// `--ets-path`: compiled ArkTS output directory (contains `modules.abc`).
    pub ets_dir: PathBuf,
    /// `--pack-info-path`.
    pub pack_info: PathBuf,
    /// `--pkg-context-path`.
    pub pkg_context: PathBuf,
}

/// Derive the part locations from a module's `build/default` directory.
pub fn inputs_from_module_build(module_build: &Path) -> PackInputs {
    let intermediates = module_build.join("intermediates");
    PackInputs {
        module_json: intermediates.join("package/default/module.json"),
        resources_index: intermediates.join("res/default/resources.index"),
        resources_dir: intermediates.join("res/default/resources"),
        lib_dir: intermediates.join("stripped_native_libs/default"),
        ets_dir: intermediates.join("loader_out/default/ets"),
        pack_info: module_build.join("outputs/default/pack.info"),
        pkg_context: intermediates.join("loader/default/pkgContextInfo.json"),
    }
}

fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    // The packing tool walks directories in name order; keeping the same order is what makes the
    // resulting archive byte-identical and keeps the central directory stable.
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let entry = format!("{prefix}{name}");
        if path.is_dir() {
            collect_files(&path, &format!("{entry}/"), out)?;
        } else {
            out.push((entry, path));
        }
    }
    Ok(())
}

/// Strip whitespace from JSON text, leaving string contents untouched.
///
/// `app_packing_tool` minifies the JSON files it packs (measured: a 36-byte indented
/// `main_pages.json` lands as 23 bytes, `{"src":["pages/Index"]}`); `module.json` arrives already
/// minified so it passes through unchanged either way.
fn minify_json(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len());
    let mut in_string = false;
    let mut escaped = false;
    for &byte in body {
        if in_string {
            out.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                out.push(byte);
            }
            b' ' | b'\t' | b'\r' | b'\n' => {}
            _ => out.push(byte),
        }
    }
    out
}

/// Build the archive at `out`.
pub fn pack(inputs: &PackInputs, out: &Path) -> Result<(), String> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    entries.push(("module.json".into(), inputs.module_json.clone()));
    entries.push(("resources.index".into(), inputs.resources_index.clone()));
    collect_files(&inputs.lib_dir, "libs/", &mut entries)?;
    collect_files(&inputs.resources_dir, "resources/", &mut entries)?;
    collect_files(&inputs.ets_dir, "ets/", &mut entries)?;
    entries.push(("pack.info".into(), inputs.pack_info.clone()));
    entries.push(("pkgContextInfo.json".into(), inputs.pkg_context.clone()));

    for (name, path) in &entries {
        if !path.is_file() {
            return Err(format!(
                "required package part missing: {name} ({})",
                path.display()
            ));
        }
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let file = std::fs::File::create(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);
    for (name, path) in &entries {
        let mut body = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        if name.starts_with("resources/") && name.ends_with(".json") {
            body = minify_json(&body);
        }
        writer
            .start_file(name.as_str(), options)
            .map_err(|e| format!("zip entry {name}: {e}"))?;
        writer
            .write_all(&body)
            .map_err(|e| format!("write {name}: {e}"))?;
    }
    writer.finish().map_err(|e| format!("finish: {e}"))?;
    Ok(())
}
