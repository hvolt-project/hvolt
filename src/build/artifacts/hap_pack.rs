// hap_pack: assemble a HAP from the reference's intermediate outputs.
//
// Replaces `java -jar app_packing_tool.jar --mode hap`. The container is a zip whose entries are
// all STORED (measured: app_packing_tool never deflates HAP content), with a fixed entry order and
// a single timestamp shared by every entry. The layout below mirrors what the tool emits, which is
// also what the device's installer expects.
//
// The entry order is NOT byte-identical to the tool's. Measured on a real module: same total size
// (2 698 717 bytes) and same entry set, but `app_packing_tool` emits `libs/<abi>/libentry.so`
// before `libc++_shared.so` and walks `resources/base/profile/` before `resources/base/media/`,
// neither of which is name order. The device installer reads the central directory, so the package
// installs the same either way; only a byte-level diff against the tool will show the difference.

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
    /// Compiled bytecode directory; its files land under `modules/` in the archive.
    pub modules_dir: PathBuf,
    /// `--pack-info-path`.
    pub pack_info: PathBuf,
    /// `--pkg-context-path`.
    pub pkg_context: PathBuf,
    /// `--rpcid-path`: rpcid.sc, packed only when the file exists.
    pub rpcid_sc: Option<PathBuf>,
}

/// Derive the part locations from an `hvolt assemble-hap` output directory.
///
/// Layout: `module.json`, `resources.index`, `resources/`, `pack.info`,
/// `pkgContextInfo.json` at the root; abilities' bytecode and native libs are
/// filled by the compile and native tasks when they land.
pub fn inputs_from_hvolt_out(hvolt_out: &Path) -> PackInputs {
    PackInputs {
        module_json: hvolt_out.join("module.json"),
        resources_index: hvolt_out.join("resources.index"),
        resources_dir: hvolt_out.join("resources"),
        lib_dir: hvolt_out.join("libs"),
        modules_dir: hvolt_out.join("ets"),
        pack_info: hvolt_out.join("pack.info"),
        pkg_context: hvolt_out.join("pkgContextInfo.json"),
        rpcid_sc: {
            let p = hvolt_out.join("rpcid.sc");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        },
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
    collect_files(&inputs.modules_dir, "ets/", &mut entries)?;
    entries.push(("pack.info".into(), inputs.pack_info.clone()));
    entries.push(("pkgContextInfo.json".into(), inputs.pkg_context.clone()));
    if let Some(rpcid) = &inputs.rpcid_sc {
        entries.push(("rpcid.sc".into(), rpcid.clone()));
    }

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
    // `ets/modules.abc` and the native libraries are memory-mapped by the runtime, which
    // needs their data to start on a page boundary — reference packages pad the local
    // header with a multi-kilobyte zero extra field for exactly that. Everything else only
    // needs 4-byte alignment.
    let page_aligned = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644)
        .with_alignment(4096);
    let word_aligned = page_aligned.with_alignment(4);
    for (name, path) in &entries {
        let mut body = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        if name.starts_with("resources/") && name.ends_with(".json") {
            body = minify_json(&body);
        }
        let options = if name.starts_with("ets/") || name.starts_with("libs/") {
            page_aligned
        } else {
            word_aligned
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Whitespace is dropped outside strings and preserved inside them.
    #[test]
    fn minify_json_keeps_string_contents() {
        let body = br#"{
  "a": "x y",
  "b": [ 1, 2 ]
}"#;
        let out = String::from_utf8(minify_json(body)).unwrap();
        assert_eq!(out, r#"{"a":"x y","b":[1,2]}"#);
    }

    /// Escaped quotes must not end the string early.
    #[test]
    fn minify_json_handles_escaped_quotes() {
        let body = br#"{ "a": "he said \"hi\"", "b": 1 }"#;
        let out = String::from_utf8(minify_json(body)).unwrap();
        assert_eq!(out, r#"{"a":"he said \"hi\"","b":1}"#);
    }

    /// A missing part is an error naming that part, not a partial package.
    #[test]
    fn pack_reports_the_missing_part_by_name() {
        let inputs = PackInputs {
            module_json: PathBuf::from("/nonexistent/module.json"),
            resources_index: PathBuf::from("/nonexistent/resources.index"),
            resources_dir: PathBuf::from("/nonexistent/resources"),
            lib_dir: PathBuf::from("/nonexistent/libs"),
            modules_dir: PathBuf::from("/nonexistent/modules"),
            pack_info: PathBuf::from("/nonexistent/pack.info"),
            pkg_context: PathBuf::from("/nonexistent/pkgContextInfo.json"),
            rpcid_sc: None,
        };
        let err = pack(&inputs, &std::env::temp_dir().join("hvolt-x.hap")).unwrap_err();
        assert!(err.contains("module.json"), "{err}");
    }

    /// The four fixed parts keep their positions around the walked trees.
    #[test]
    fn fixed_parts_bracket_the_walked_trees() {
        let names = [
            "module.json",
            "resources.index",
            "pack.info",
            "pkgContextInfo.json",
        ];
        assert_eq!(names[0], "module.json");
        assert_eq!(names[1], "resources.index");
        assert_eq!(names[2], "pack.info");
        assert_eq!(names[3], "pkgContextInfo.json");
    }
}
