//! es2abc_inproc: run the ArkTS compiler inside this process.
//!
//! The device-side build has no process spawn to lean on: the IDE loads
//! `libes2abc.so` next to `libhvolt.so`, and one C symbol (`es2abc_entry`)
//! takes the same argv the CLI would have received. Everything the compiler
//! prints is captured so the caller reports it the way a subprocess would.
//!
//! Only compiled with the `ohos` feature; the desktop build keeps spawning.
#![cfg(feature = "ohos")]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

type Entry = unsafe extern "C" fn(c_int, *const *const c_char) -> c_int;

/// Resolve `libes2abc.so`: `HVOLT_ES2ABC_LIB` first, else beside this library.
fn library_path() -> Result<std::path::PathBuf, String> {
    if let Ok(p) = std::env::var("HVOLT_ES2ABC_LIB") {
        return Ok(p.into());
    }
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    let here = self_symbol() as *const libc::c_void;
    // SAFETY: dladdr on our own symbol only fills `info`.
    #[allow(unsafe_code)]
    let ok = unsafe { libc::dladdr(here, &mut info) } != 0;
    if ok && !info.dli_fname.is_null() {
        // SAFETY: dladdr guarantees a NUL-terminated path on success.
        #[allow(unsafe_code)]
        let path = unsafe { CStr::from_ptr(info.dli_fname) }
            .to_string_lossy()
            .to_string();
        let dir = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let beside = dir.join("libes2abc.so");
        if beside.is_file() {
            return Ok(beside);
        }
        if std::path::Path::new(&path).is_file() {
            return Err(format!(
                "libes2abc.so not found beside {} (set HVOLT_ES2ABC_LIB)",
                dir.display()
            ));
        }
    }
    Err("cannot locate libes2abc.so (set HVOLT_ES2ABC_LIB)".to_string())
}

fn self_symbol() -> *const u8 {
    library_path as *const () as *const u8
}

/// The option names this build of libes2abc.so advertises, parsed from `--help`.
///
/// The merge argv mirrors the SDK's ets-loader, which drives the commercial es2abc;
/// the open-source library behind libes2abc.so carries a different option set, so the
/// caller drops what this returns absent instead of guessing from a version string.
pub fn supported_flags() -> Result<std::collections::HashSet<String>, String> {
    let (code, out) = run(&["--help".to_string()])?;
    let mut set = std::collections::HashSet::new();
    for line in out.lines() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("--") else { continue };
        let name = rest
            .split(|c: char| c == ':' || c == ' ' || c == '=' || c == '\t')
            .next()
            .unwrap_or("");
        if !name.is_empty() {
            set.insert(format!("--{name}"));
        }
    }
    if set.is_empty() {
        return Err(format!(
            "es2abc --help produced no options (exit {code}): {}",
            out.trim_end()
        ));
    }
    Ok(set)
}

/// Run one es2abc command line in-process.
///
/// Returns `(exit_code, captured_output)`. `exit_code != 0` is a compiler
/// rejection, not a loader failure; loader failures come back as `Err`.
pub fn run(args: &[String]) -> Result<(i32, String), String> {
    crate::ffi::trace("inproc:enter");
    let path = library_path()?;
    crate::ffi::trace(&format!("inproc:path={}", path.display()));
    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "library path contains a NUL byte".to_string())?;

    // SAFETY: dlopen/dlsym on a path we own; both results are checked.
    #[allow(unsafe_code)]
    let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        // SAFETY: dlerror returns a NUL-terminated message or NULL.
        #[allow(unsafe_code)]
        let msg = unsafe {
            let e = libc::dlerror();
            if e.is_null() {
                "unknown dlopen failure".to_string()
            } else {
                CStr::from_ptr(e).to_string_lossy().to_string()
            }
        };
        return Err(format!("dlopen {}: {msg}", path.display()));
    }

    // SAFETY: the symbol name is a static C string.
    #[allow(unsafe_code)]
    let sym = unsafe { libc::dlsym(handle, c"es2abc_entry".as_ptr()) };
    if sym.is_null() {
        return Err(format!("{} has no es2abc_entry symbol", path.display()));
    }
    // SAFETY: the library exports this exact signature (napi probe verified it).
    #[allow(unsafe_code)]
    let entry: Entry = unsafe { std::mem::transmute(sym) };

    let mut owned: Vec<CString> = Vec::with_capacity(args.len() + 1);
    owned.push(CString::new("es2abc").expect("static has no NUL"));
    for a in args {
        owned.push(
            CString::new(a.as_bytes()).map_err(|_| "argument contains a NUL byte".to_string())?,
        );
    }
    let ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();

    crate::ffi::trace("inproc:calling-entry");
    let capture = crate::ffi::redirect::Capture::start();
    // SAFETY: argv is a NUL-terminated pointer array alive for the call.
    #[allow(unsafe_code)]
    let code = unsafe { entry(ptrs.len() as c_int, ptrs.as_ptr()) };
    crate::ffi::trace(&format!("inproc:entry-returned={code}"));
    let output = capture.map(|c| c.finish()).unwrap_or_default();

    Ok((code, output))
}
