//! hvolt apply — quickfix (hqf) incremental deployment.
//!
//! Channel knowledge (all measured on a retail device, 2026-10-10):
//! - `hvigorw --mode module -p module=<m>@default assembleDevHqf` produces
//!   an unsigned .hqf; patchVersionCode AUTO-INCREMENTS (3000000→3000001)
//!   — hvolt never manages the version.
//! - the hqf chain enforces SDK management mode: OHOS_BASE_SDK_HOME must
//!   point at the sdk-ohos view (run_hqf injects it).
//! - post-signing an hqf with hap-sign-tool sign-app works with identical
//!   flags (keeps hvolt's post-sign model; SignHqf inside hvigor is skipped
//!   because signingConfigs are stripped).
//! - device side: `bm quickfix -a -f <hqf>` — NO -t (with -t it fails 8520706
//!   whatever path is given; the usage text misleads).
//! - identity: the patch must carry the SAME signing identity as the
//!   installed package (verified: AGC-signed pkg + AGC-signed hqf accepted).
//! - re-applying an equal/lower version fails ("Deploy hqf failed");
//!   `bm quickfix -r -b` removes but the version watermark stays —
//!   auto-increment covers this.

use crate::assemble;
use crate::build_hvigor;
use crate::config::AppConfig;
use crate::sdk;
use crate::sign;
use std::fs;
use std::path::Path;

pub fn apply(project_dir: &Path, device: Option<&str>) -> Result<(), String> {
    let root_toml = project_dir.join("hvolt.toml");
    let cfg = AppConfig::from_file(&root_toml)?;
    let signing = cfg.app.signing.get("default")
        .ok_or("no signing config — run `hvolt sign` first (apply patches must match the installed identity)")?
        .clone();
    let env = sdk::detect(cfg.app.sdk_path.as_deref().unwrap_or(""))?;

    // 1. translate + full incremental build (abc freshness)
    assemble::assemble(project_dir);
    let build_dir = project_dir.join("build");
    build_hvigor::strip_signing_config(project_dir);
    build_hvigor::run_ohpm_install(&env, &build_dir)?;
    build_hvigor::run_assemble(&env, &build_dir)?;
    println!("✓ build fresh");

    // 2. hqf packaging
    let module = build_hvigor::first_module(&build_dir)?;
    build_hvigor::run_hqf(&env, &build_dir, &module)?;
    let unsigned = find_hqf(&build_dir)?;
    println!("✓ hqf packaged: {}", unsigned.display());

    // 3. post-sign with the installed identity
    let signed = project_dir.join("output").join("patch.hqf");
    fs::create_dir_all(project_dir.join("output")).map_err(|e| e.to_string())?;
    let base = project_dir;
    sign::sign_package(
        &env,
        &unsigned,
        &signed,
        &base.join(&signing.store_file).to_string_lossy(),
        &signing.store_password,
        &signing.key_alias,
        &signing.key_password,
        &base.join(&signing.certpath).to_string_lossy(),
        &base.join(&signing.profile).to_string_lossy(),
        &signing.sign_alg,
    )?;
    let sha = sdk::sha256_file(&signed)?;
    println!("✓ hqf signed (sha256 {}…)", &sha[..16]);

    // 4. device side
    let remote = "/data/local/tmp/hvolt_patch.hqf";
    let signed_abs = signed
        .canonicalize()
        .map_err(|e| format!("resolve patch: {}", e))?;
    let cfg_g = sdk::load_global_config();
    let ao = if cfg_g.device_host.is_empty() {
        let (_t, so) = crate::devtools::hdc_exec(
            device,
            &["file", "send", &signed_abs.to_string_lossy(), remote],
        )?;
        if !so.contains("FileTransfer finish") {
            return Err(format!("hqf send failed: {}", so.trim()));
        }
        let (_t, ao) =
            crate::devtools::hdc_exec(device, &["shell", "bm", "quickfix", "-a", "-f", remote])?;
        let _ = crate::devtools::hdc_exec(device, &["shell", "rm", remote]);
        ao
    } else {
        let (serial, local_hdc) = crate::target::resolve(device)?;
        let hdc_str = crate::devtools::remote_hdc_path(&cfg_g.device_hdc, &local_hdc);
        crate::target::remote_push_and_run(
            &cfg_g.device_host,
            &hdc_str,
            &serial,
            &signed_abs,
            remote,
            &["bm", "quickfix", "-a", "-f", remote],
        )?
    };
    patch_applied(&ao)?;
    // 5. verify from the device itself
    let (_t, qo) = crate::devtools::hdc_exec(
        device,
        &["shell", "bm", "quickfix", "-q", "-b", &cfg.app.bundle],
    )?;
    let pvc = patch_version_line(&qo);
    println!("✓ quickfix applied — {}", pvc);
    Ok(())
}

/// `bm quickfix -q -b <bundle>` prints the applied patch version; extract that line.
///
/// Falls back to a placeholder rather than failing: the query is informational only.
pub(crate) fn patch_version_line(output: &str) -> String {
    output
        .lines()
        .find(|l| l.contains("patch version code"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "patch version code: ?".into())
}

/// `bm quickfix -a` must acknowledge the apply; anything else is the device's own error text.
///
/// Kept as a named predicate so the accepted wording is pinned by a test instead of an
/// inline `contains` in the middle of the deploy path.
pub(crate) fn patch_applied(output: &str) -> Result<(), String> {
    if output.contains("apply quickfix succeed") {
        Ok(())
    } else {
        Err(format!("bm quickfix: {}", output.trim()))
    }
}

fn find_hqf(build_dir: &Path) -> Result<std::path::PathBuf, String> {
    // <build>/<module>/build/default/outputs/default/<module>-default-unsigned.hqf
    let mut found: Vec<std::path::PathBuf> = Vec::new();
    let mut stack = vec![build_dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.to_string_lossy().ends_with("-unsigned.hqf") {
                    found.push(p);
                }
            }
        }
    }
    found.sort();
    found.pop().ok_or("no unsigned hqf produced".to_string())
}

#[cfg(test)]
mod patch_output_tests {
    use super::{patch_applied, patch_version_line};

    /// The apply must be acknowledged; otherwise the device's own text is surfaced.
    #[test]
    fn apply_requires_an_acknowledgement() {
        assert!(patch_applied("apply quickfix succeed\n").is_ok());
        let err = patch_applied("Deploy hqf failed").unwrap_err();
        assert!(err.contains("Deploy hqf failed"));
    }

    /// The version line is pulled out of multi-line output and trimmed.
    #[test]
    fn version_line_is_extracted_and_trimmed() {
        let out = "begin\n  patch version code: 3000001  \nend";
        assert_eq!(patch_version_line(out), "patch version code: 3000001");
    }

    /// No version line degrades to a placeholder rather than an empty string.
    #[test]
    fn missing_version_line_degrades_to_placeholder() {
        assert_eq!(patch_version_line(""), "patch version code: ?");
        assert_eq!(patch_version_line("other text"), "patch version code: ?");
    }
}
