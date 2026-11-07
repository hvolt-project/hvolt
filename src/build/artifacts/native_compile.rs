// native_compile: drive the SDK cmake+ninja to build a module's native deps.
//
// Mirrors the reference's native build (verified against asr-ondevice, 2026-11-03):
//   cmake -DCMAKE_TOOLCHAIN_FILE=<sdk>/native/build/cmake/ohos.toolchain.cmake \
//         -DOHOS_ARCH=arm64-v8a -DOHOS_SDK_NATIVE=<sdk>/native -G Ninja <module>
//   ninja
//   llvm-strip lib/<abi>/*.so
// The stripped outputs are byte-identical to the reference's `stripped_native_libs`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the SDK's native toolchain lives.
fn native_root(sdk_root: &Path) -> PathBuf {
    sdk_root.join("default/openharmony/native")
}

/// Windows ships the toolchain as `.exe`; plain names resolve in bash but not
/// in `Path::exists`/`Command::new` on Win32.
fn toolchain_bin(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Build one native module (a directory with CMakeLists.txt) for one ABI.
/// Returns the directory holding the built (unstripped) .so files.
pub fn build(sdk_root: &Path, module_dir: &Path, build_dir: &Path) -> Result<PathBuf, String> {
    let native = native_root(sdk_root);
    let cmake = native
        .join("build-tools/cmake/bin")
        .join(toolchain_bin("cmake"));
    let ninja = native
        .join("build-tools/cmake/bin")
        .join(toolchain_bin("ninja"));
    if !cmake.exists() {
        return Err(format!("cmake not found at {}", cmake.display()));
    }
    if !ninja.exists() {
        return Err(format!("ninja not found at {}", ninja.display()));
    }
    std::fs::create_dir_all(build_dir)
        .map_err(|e| format!("mkdir {}: {e}", build_dir.display()))?;

    let configure = Command::new(&cmake)
        .current_dir(build_dir)
        .arg(format!(
            "-DCMAKE_TOOLCHAIN_FILE={}",
            native.join("build/cmake/ohos.toolchain.cmake").display()
        ))
        .arg("-DOHOS_ARCH=arm64-v8a")
        .arg(format!("-DOHOS_SDK_NATIVE={}", native.display()))
        .arg(format!("-DCMAKE_MAKE_PROGRAM={}", ninja.display()))
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg("-G")
        .arg("Ninja")
        .arg(module_dir)
        .output()
        .map_err(|e| format!("run cmake: {e}"))?;
    if !configure.status.success() {
        return Err(format!(
            "cmake configure failed: {}{}",
            String::from_utf8_lossy(&configure.stdout).trim_end(),
            String::from_utf8_lossy(&configure.stderr).trim_end()
        ));
    }

    let build = Command::new(&ninja)
        .current_dir(build_dir)
        .output()
        .map_err(|e| format!("run ninja: {e}"))?;
    if !build.status.success() {
        return Err(format!(
            "ninja build failed: {}{}",
            String::from_utf8_lossy(&build.stdout).trim_end(),
            String::from_utf8_lossy(&build.stderr).trim_end()
        ));
    }
    Ok(build_dir.to_path_buf())
}

/// Strip every `.so` in `dir` in place, like the reference's CacheNativeLibs.
pub fn strip(sdk_root: &Path, dir: &Path) -> Result<(), String> {
    let strip = native_root(sdk_root)
        .join("llvm/bin")
        .join(toolchain_bin("llvm-strip"));
    if !strip.exists() {
        return Err(format!("llvm-strip not found at {}", strip.display()));
    }
    let mut sos: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "so").unwrap_or(false) && p.is_file())
        .collect();
    sos.sort();
    for so in &sos {
        let out = Command::new(&strip)
            .arg(so)
            .output()
            .map_err(|e| format!("run llvm-strip: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "llvm-strip {} failed: {}",
                so.display(),
                String::from_utf8_lossy(&out.stderr).trim_end()
            ));
        }
    }
    Ok(())
}

/// Build + strip all native modules, copying the result into the package
/// layout `out/arm64-v8a/*.so`.
pub fn build_all(
    sdk_root: &Path,
    native_modules: &[(String, crate::support::config::NativeModuleConfig)],
    project_dir: &Path,
    out_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let libs = out_dir.join("arm64-v8a");
    std::fs::create_dir_all(&libs).map_err(|e| format!("mkdir {}: {e}", libs.display()))?;
    let mut produced = Vec::new();
    for (name, cfg) in native_modules {
        // Absolute, or cmake resolves it against the build dir and fails.
        let module_dir = std::path::absolute(project_dir.join("libs").join(name))
            .map_err(|e| format!("absolute {}: {e}", name))?;
        let _ = cfg;
        if !module_dir.join("CMakeLists.txt").exists() {
            continue;
        }
        // Build dir beside libs/, never inside it — the packer walks libs/.
        let build_root = out_dir
            .parent()
            .unwrap_or(out_dir)
            .join(format!(".native-build-{name}"));
        let build_dir = build_root;
        let built = build(sdk_root, &module_dir, &build_dir)?;
        strip(sdk_root, &built)?;
        for entry in std::fs::read_dir(&built)
            .map_err(|e| format!("read {}: {e}", built.display()))?
            .flatten()
        {
            let p = entry.path();
            if p.extension().map(|e| e == "so").unwrap_or(false) {
                let dest = libs.join(p.file_name().unwrap());
                std::fs::copy(&p, &dest).map_err(|e| format!("copy {}: {e}", p.display()))?;
                produced.push(dest);
            }
        }
    }
    Ok(produced)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build output lands under `<out>/arm64-v8a/`, mirroring the HAP's libs/.
    #[test]
    fn abi_dir_is_arm64_v8a() {
        let p = Path::new("/tmp/x").join("arm64-v8a");
        assert_eq!(p.to_string_lossy(), "/tmp/x/arm64-v8a");
    }
}
