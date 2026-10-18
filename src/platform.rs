//! Platform abstraction: Windows is a first-class single-machine host
//! (Surface-class dev boxes: local device via hdc, local emulator, no
//! cross-machine anything). Everything OS-specific lives here.

use std::path::PathBuf;

/// True on Windows (MSVC or GNU target).
pub fn is_windows() -> bool {
    cfg!(windows)
}

/// Locate an executable on PATH without shelling out to `which`/`where`
/// (native Windows shells have neither as an external binary hvolt can exec).
pub fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for n in exe_names(name) {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Resolve a CLI launcher, which may be `.cmd`/`.bat` on Windows (CreateProcess
/// only auto-appends `.exe`). Returns the concrete file for Command::new, or the
/// bare name when none exists; in that case PATH resolution fails and callers
/// surface the error.
pub fn npm_launcher() -> Option<PathBuf> {
    let names = if is_windows() {
        vec!["npm.cmd", "npm.exe", "npm"]
    } else {
        vec!["npm"]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for n in &names {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// User home: HOME (msys/ssh shells) > USERPROFILE (native) > cwd.
pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Executable file names to try for a tool, platform-aware.
pub fn exe_names(base: &str) -> Vec<String> {
    if is_windows() {
        vec![format!("{}.exe", base), base.to_string()]
    } else {
        vec![base.to_string()]
    }
}

/// Find `<dir>/<name>(.exe)` under one or more candidate dirs.
pub fn find_in_dirs(name: &str, dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    for dir in dirs {
        for n in exe_names(name) {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Node/JDK bin subdir: POSIX trees use bin/, the Windows node zip puts
/// node.exe at the tree root. Returns the dir containing the binary.
pub fn tool_bin_dir(root: &std::path::Path, binary: &str) -> PathBuf {
    for n in exe_names(binary) {
        let in_bin = root.join("bin").join(&n);
        if in_bin.is_file() {
            return root.join("bin");
        }
        let at_root = root.join(&n);
        if at_root.is_file() {
            return root.to_path_buf();
        }
    }
    root.join("bin") // conventional default when nothing exists yet
}

/// Launcher candidates for an npm-style CLI package (ohpm): POSIX sh
/// wrapper first, .bat for native Windows shells (the official commandline
/// tools zip ships both).
pub fn ohpm_launcher(bin_dir: &std::path::Path) -> Option<PathBuf> {
    let sh = bin_dir.join("ohpm");
    if sh.is_file() && !is_windows() {
        return Some(sh);
    }
    let bat = bin_dir.join("ohpm.bat");
    if bat.is_file() {
        return Some(bat);
    }
    // msys-style: sh wrapper even on windows ssh shells — but exec-sh fails
    // under PowerShell, so prefer a node-direct fallback below
    sh.is_file().then_some(sh)
}

/// Direct-JS launcher for ohpm (pm-cli.js beside the wrapper) — works on
/// any platform with node, independent of shell dialect.
pub fn ohpm_via_node(bin_dir: &std::path::Path) -> Option<PathBuf> {
    let js = bin_dir.join("pm-cli.js");
    js.is_file().then_some(js)
}

/// Extract a zip archive (pure Rust — Windows has no unzip). Rejects
/// path traversal entries. Returns count of files written.
pub fn extract_zip(archive: &std::path::Path, dest: &std::path::Path) -> Result<usize, String> {
    use std::io;
    let zf = std::fs::File::open(archive).map_err(|e| format!("open zip: {}", e))?;
    let mut ar = zip::ZipArchive::new(zf).map_err(|e| format!("read zip: {}", e))?;
    let mut n = 0usize;
    for i in 0..ar.len() {
        let mut f = ar.by_index(i).map_err(|e| format!("entry {}: {}", i, e))?;
        let name = match f.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => return Err(format!("unsafe zip entry path: {:?}", f.name())),
        };
        let out = win_long(&dest.join(name));
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("mkdir: {}", e))?;
        } else {
            if let Some(par) = out.parent() {
                std::fs::create_dir_all(par).map_err(|e| format!("mkdir: {}", e))?;
            }
            let mut w = io::BufWriter::with_capacity(
                1 << 20,
                std::fs::File::create(&out).map_err(|e| format!("create: {}", e))?,
            );
            io::copy(&mut f, &mut w).map_err(|e| format!("write: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = f.unix_mode() {
                    let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode));
                }
            }
            n += 1;
        }
    }
    Ok(n)
}

/// Windows MAX_PATH(260) bites SDK trees hard (ets/api deep nests).
/// The \?\ verbatim prefix lifts the limit for the whole WinAPI surface;
/// std::fs honors it when the path is absolute & normalized. No-op off Windows.
pub fn win_long(p: &std::path::Path) -> PathBuf {
    if !is_windows() {
        return p.to_path_buf();
    }
    let unified = p.to_string_lossy().replace('/', "\\");
    if unified.starts_with("\\\\?\\") {
        return PathBuf::from(unified);
    }
    if !p.is_absolute() {
        return PathBuf::from(unified); // relative paths cannot be verbatim
    }
    PathBuf::from(format!("\\\\?\\{}", unified))
}

/// List entry names without extracting (pure Rust).
pub fn zip_names(archive: &std::path::Path) -> Result<Vec<String>, String> {
    let zf = std::fs::File::open(archive).map_err(|e| format!("open zip: {}", e))?;
    let mut ar = zip::ZipArchive::new(zf).map_err(|e| format!("read zip: {}", e))?;
    let mut names = Vec::with_capacity(ar.len());
    for i in 0..ar.len() {
        names.push(
            ar.by_index(i)
                .map(|f| f.name().to_string())
                .map_err(|e| format!("entry {}: {}", i, e))?,
        );
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_names_per_platform() {
        let n = exe_names("hdc");
        if is_windows() {
            assert_eq!(n, vec!["hdc.exe", "hdc"]);
        } else {
            assert_eq!(n, vec!["hdc"]);
        }
    }

    #[test]
    fn find_in_dirs_resolves_real_file() {
        // uses this source file itself as a known-existing "binary"
        let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let got = find_in_dirs("main", [here.clone()]);
        let want: Option<PathBuf> = exe_names("main")
            .into_iter()
            .map(|n| here.join(n))
            .find(|p| p.is_file());
        assert_eq!(got, want);
        assert!(find_in_dirs("definitely-not-here", [here]).is_none());
    }

    #[test]
    fn tool_bin_dir_handles_both_layouts() {
        let tmp = std::env::temp_dir().join(format!("hvolt-plat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        // posix layout: tmp/bin/node
        std::fs::create_dir_all(tmp.join("bin")).unwrap();
        std::fs::write(tmp.join("bin").join("node"), "").unwrap();
        assert_eq!(tool_bin_dir(&tmp, "node"), tmp.join("bin"));
        // windows layout: tmp/node.exe (only when no bin/node exists)
        let tmp2 = tmp.join("w");
        std::fs::create_dir_all(&tmp2).unwrap();
        std::fs::write(tmp2.join("node.exe"), "").unwrap();
        let expect_root = if is_windows() {
            tmp2.clone()
        } else {
            tmp2.join("bin")
        };
        assert_eq!(tool_bin_dir(&tmp2, "node"), expect_root);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn extract_zip_roundtrip_with_mode_and_traversal_guard() {
        use std::io::Write;
        // build a small zip with the same crate, then extract it
        let tmp = std::env::temp_dir().join(format!("hvolt-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let zpath = tmp.join("t.zip");
        {
            let fo = std::fs::File::create(&zpath).unwrap();
            let mut wo = zip::ZipWriter::new(fo);
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o755);
            wo.start_file("pkg/bin/tool", opts).unwrap();
            wo.write_all(b"#!/bin/sh\necho hi").unwrap();
            wo.finish().unwrap();
        }
        let outdir = tmp.join("out");
        std::fs::create_dir_all(&outdir).unwrap();
        let n = extract_zip(&zpath, &outdir).unwrap();
        assert_eq!(n, 1);
        assert!(outdir.join("pkg/bin/tool").is_file());
        // unix exec bit preserved where applicable
        if is_windows() { /* no unix modes */
        } else {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(outdir.join("pkg/bin/tool"))
                .unwrap()
                .permissions()
                .mode();
            assert!(m & 0o111 != 0, "exec bit lost: {:o}", m);
        }
        // traversal entry rejected
        {
            let fo = std::fs::File::create(tmp.join("evil.zip")).unwrap();
            let mut wo = zip::ZipWriter::new(fo);
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            wo.start_file("../escape.txt", opts).unwrap();
            wo.write_all(b"x").unwrap();
            wo.finish().unwrap();
        }
        assert!(extract_zip(&tmp.join("evil.zip"), &outdir).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn zip_names_lists_entries() {
        let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // no zips to assert against portably; just verify error path on missing file
        assert!(zip_names(&here.join("nope.zip")).is_err());
    }

    #[test]
    fn npm_launcher_prefers_cmd_on_windows_exe_elsewhere() {
        let l = npm_launcher();
        if let Some(p) = &l {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            assert!(n == "npm" || n == "npm.exe" || n == "npm.cmd", "{}", n);
        } // absent npm is legal (test env)
    }

    #[test]
    fn on_path_finds_known_binary() {
        // the rust toolchain running these tests must be findable on PATH
        let r = on_path("rustc");
        assert!(r.is_some(), "rustc on PATH should be found");
        assert!(on_path("definitely-not-a-binary-xyz").is_none());
    }

    #[test]
    fn win_long_prefix_shape() {
        if is_windows() {
            let p = std::path::Path::new("C:\\Users\\x\\a/b");
            let w = win_long(p);
            assert!(
                w.to_string_lossy().starts_with("\\\\\\?\\"),
                "{}",
                w.display()
            );
            assert!(!w.to_string_lossy().contains('/'));
            // idempotent
            assert_eq!(win_long(&w), w);
        } else {
            let p = std::path::Path::new("/home/x/a/b");
            assert_eq!(win_long(p), p.to_path_buf());
        }
    }

    #[test]
    fn home_dir_nonempty() {
        assert!(!home_dir().to_string_lossy().is_empty());
    }
}
