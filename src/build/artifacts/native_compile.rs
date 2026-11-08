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
    crate::sdk::manage::component_root(sdk_root, "native")
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

/// The NDK's `libc++_shared.so` for arm64.
///
/// A module built with `CMAKE_CXX_STANDARD 20` (`xb_components`) cannot be `dlopen`-ed
/// without it, and the runtime then reports its exports as `undefined` — the device said
/// `export objects of native so is undefined, so name is @normalized:Y&&&libxb_components.so&`
/// (clashbox, measured 2026-11-08). The reference packer ships the NDK's copy unchanged.
pub(crate) fn cxx_runtime(sdk_root: &Path) -> Option<PathBuf> {
    let path = native_root(sdk_root).join("llvm/lib/aarch64-linux-ohos/libc++_shared.so");
    path.is_file().then_some(path)
}

/// The host's own copy of a build tool, falling back to the SDK's.
///
/// `cmake` and `ninja` are general-purpose tools: the SDK ships pinned copies only so a
/// project builds without installing anything. Preferring the host keeps the SDK a source
/// of OHOS-specific pieces (toolchain file, NDK) instead of a generic tool locker. The
/// SDK copy stays the fallback, so a machine with neither still fails at the same place
/// with the same message.
fn host_tool_or_sdk(name: &str, sdk_bin: &Path) -> PathBuf {
    crate::support::platform::on_path(name).unwrap_or_else(|| sdk_bin.join(toolchain_bin(name)))
}

/// Build one native module (a directory with CMakeLists.txt) for one ABI.
/// Returns the directory holding the built (unstripped) .so files.
pub(crate) fn build(
    sdk_root: &Path,
    module_dir: &Path,
    build_dir: &Path,
    build_type: &str,
) -> Result<PathBuf, String> {
    let native = native_root(sdk_root);
    let sdk_bin = native.join("build-tools/cmake/bin");
    let cmake = host_tool_or_sdk("cmake", &sdk_bin);
    let ninja = host_tool_or_sdk("ninja", &sdk_bin);
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
        // `build_type` comes from the caller, which passes `Debug` — the mode `hvolt build`
        // compiles a project in. Compiling it as Release instead inlines away every template
        // instantiation, shrinking `libxb_components.so` from 206 608 to 92 552 bytes and
        // dropping 178 exported `NapiUtils::toNapiValue<…>` symbols (clashbox, 2026-11-08).
        .arg(format!("-DCMAKE_BUILD_TYPE={build_type}"))
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
pub(crate) fn strip(sdk_root: &Path, dir: &Path) -> Result<(), String> {
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
pub(crate) fn build_all(
    sdk_root: &Path,
    native_names: &[(String, std::path::PathBuf)],
    out_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let libs = out_dir.join("arm64-v8a");
    std::fs::create_dir_all(&libs).map_err(|e| format!("mkdir {}: {e}", libs.display()))?;
    let mut produced = Vec::new();
    for (name, declared_dir) in native_names {
        // The directory a native module lives in is whatever `[dependencies]` says, not a
        // fixed `libs/<name>`: `hvolt import` writes `path = "mylib"`. Only fall back when
        // the dependency declares none.
        let module_dir = std::path::absolute(declared_dir)
            .map_err(|e| format!("absolute {}: {e}", declared_dir.display()))?;
        if !module_dir.join("CMakeLists.txt").exists() {
            // A native dependency with no CMakeLists is a configuration error, not a reason
            // to build a HAP that is silently missing its `.so`.
            return Err(format!(
                "native module '{name}' has no CMakeLists.txt at {}; fix its `path` in \
                 [dependencies]",
                module_dir.display()
            ));
        }
        // Build dir beside libs/, never inside it — the packer walks libs/.
        let build_root = out_dir
            .parent()
            .unwrap_or(out_dir)
            .join(format!(".native-build-{name}"));
        let build_dir = build_root;
        let built = build(sdk_root, &module_dir, &build_dir, "Debug")?;
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

    /// The C++ runtime is read from the SDK's own arm64 toolchain directory.
    ///
    /// A module built with `CMAKE_CXX_STANDARD 20` cannot be loaded without it, so the path
    /// this resolves has to be the one the NDK installs.
    #[test]
    fn cxx_runtime_resolves_the_ndk_copy() {
        let root = std::env::temp_dir().join(format!("hvolt-cxx-{}", std::process::id()));
        let abi_dir =
            crate::sdk::manage::component_root(&root, "native").join("llvm/lib/aarch64-linux-ohos");
        std::fs::create_dir_all(&abi_dir).expect("mkdir");
        let expected = abi_dir.join("libc++_shared.so");
        std::fs::write(&expected, b"x").expect("write");
        assert_eq!(cxx_runtime(&root), Some(expected));
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    /// A tree without the SDK's toolchain yields nothing rather than a wrong path.
    #[test]
    fn cxx_runtime_is_absent_without_the_toolchain() {
        let root = std::env::temp_dir().join(format!("hvolt-nocxx-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("mkdir");
        assert_eq!(cxx_runtime(&root), None);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
