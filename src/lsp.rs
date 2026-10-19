//! Launcher for the language server in `lsp/`.
//!
//! The server itself is TypeScript: it has to run inside the SDK's own TypeScript
//! fork, and that fork is a JavaScript library. This module only resolves the SDK,
//! Node and the bundled server, then hands stdio over to it — the editor talks
//! LSP to this process and never sees the child.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Bundle produced by `npm --prefix lsp run build`.
const SERVER_BUNDLE: &str = "dist/server.cjs";

pub fn serve(sdk_override: Option<&str>) -> Result<(), String> {
    let sdk = resolve_sdk(sdk_override)?;
    let node = resolve_node()?;
    let server = resolve_server()?;
    let root = std::env::current_dir().map_err(|e| format!("current dir: {}", e))?;

    // The SDK's TypeScript fork holds every declaration file in memory; checking a
    // project there needs far more than node's default heap (measured: the process
    // aborted with "Ineffective mark-compacts near heap limit"). Give it room.
    let status = Command::new(&node)
        .arg("--max-old-space-size=4096")
        .arg(&server)
        .arg("--sdk")
        .arg(&sdk)
        .arg("--root")
        .arg(&root)
        .arg("--stdio")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("start {}: {}", server.display(), e))?;

    // The editor owns this process: it closes stdio (or kills it) when the window
    // closes, and the server then exits non-zero. That is a normal lifecycle, so
    // the launcher stays quiet — only a failure to start is an error.
    let _ = status;
    Ok(())
}

/// The language service lives in the `ets` component of an installed SDK.
fn resolve_sdk(override_path: Option<&str>) -> Result<PathBuf, String> {
    if let Some(explicit) = override_path {
        let path = Path::new(explicit);
        if !path.exists() {
            return Err(format!("--sdk {} does not exist", explicit));
        }
        return Ok(path.to_path_buf());
    }
    crate::sdk::sdk_dir("")
}

fn resolve_node() -> Result<PathBuf, String> {
    let home = crate::sdk::find_node()?;
    crate::platform::find_in_dirs("node", [home.join("bin"), home.clone()])
        .ok_or_else(|| format!("no node binary under {}", home.display()))
}

/// Where the bundled server is: explicit override, then next to the binary, then
/// the checkout the binary was built from.
fn resolve_server() -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(explicit) = std::env::var_os("HVOLT_LSP_SERVER") {
        candidates.push(PathBuf::from(explicit));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(SERVER_BUNDLE));
            candidates.push(dir.join("lsp").join(SERVER_BUNDLE));
            candidates.push(dir.join("..").join("lsp").join(SERVER_BUNDLE));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("lsp").join(SERVER_BUNDLE));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lsp")
            .join(SERVER_BUNDLE),
    );

    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    Err(format!(
        "language server bundle not found (looked for {SERVER_BUNDLE}); \
         build it with `npm --prefix lsp install && npm --prefix lsp run build`"
    ))
}
