// pipeline: build + sign + push + launch workflow.

use std::path::Path;

use crate::assemble;
use crate::build_hvigor;
use crate::config::AppConfig;
use crate::sdk::SdkEnv;

/// Shared: assemble → detect SDK → strip signing → hvigorw.
///
/// Returns the failure instead of exiting: a library-shaped pipeline stays composable and its
/// error paths stay assertable. Callers decide the exit code.
fn assemble_and_build(project_dir: &Path) -> Result<(AppConfig, SdkEnv), String> {
    assemble::assemble(project_dir);

    let root_toml = project_dir.join("hvolt.toml");
    let app_config = AppConfig::from_file(&root_toml)?;

    let sdk_env = crate::sdk::detect(app_config.app.sdk_path.as_deref().unwrap_or(""))
        .map_err(|e| format!("SDK not available: {}", e))?;

    println!("  SDK detected: {}", sdk_env.ohos_sdk.display());
    build_hvigor::strip_signing_config(project_dir);
    let build_dir = project_dir.join("build");
    build_hvigor::run_ohpm_install(&sdk_env, &build_dir)
        .map_err(|e| format!("ohpm install failed: {}", e))?;
    build_hvigor::run_assemble(&sdk_env, &build_dir).map_err(|e| format!("Build failed: {}", e))?;
    println!("✓ Build succeeded");

    Ok((app_config, sdk_env))
}

/// Build + sign only (no push). Used by `hvolt build`.
/// Err is returned (caller must propagate exit code) — signing failure is a failure.
pub fn build_and_sign(project_dir: &Path) -> Result<(), String> {
    let (app_config, sdk_env) = assemble_and_build(project_dir)?;

    let Some(signing) = app_config.app.signing_for("default") else {
        println!("  (no signing config, skipping sign)");
        return Ok(());
    };

    // DevEco encodes the signing passwords in build-profile.json5. Running SignHap with them
    // fails at the last step with a message about the keystore, which reads like a broken
    // certificate; the real answer is "these are not plaintext", so say it before signing and
    // leave the unsigned package in place.
    if signing.passwords_look_encoded() {
        println!(
            "  (signing skipped: store_password/key_password in hvolt.toml look DevEco-encoded, \
             not plaintext — replace them with the real keystore passwords)"
        );
        println!("  unsigned HAP left in build/");
        return Ok(());
    }

    crate::sign::sign_app(
        &sdk_env,
        project_dir,
        &signing.store_file,
        &signing.store_password,
        &signing.key_alias,
        &signing.key_password,
        &signing.certpath,
        &signing.profile,
        &signing.sign_alg,
    )
    .map_err(|e| format!("Sign failed: {}", e))
}

/// Full pipeline: build + sign + push + launch. Used by `hvolt run`.
/// Push/install failures get the attribution order, not just the raw line:
/// signing trust → bundle update identity → profile validity → device constraint.
pub(crate) fn report_push_failure(e: String) -> ! {
    eprintln!("✗ {}", e);
    eprintln!("  → 归因顺序：签名信任(证书链是否被该设备/ROM 预置) → 包名更新身份(与已装版本签名是否一致) → Profile 时效(validity,UTC 秒) → 设备约束(空间/禁用)");
    eprintln!("    运行时权限失败不在此列——那对应 API/模块声明/ACL，别用重签掩盖功能问题");
    std::process::exit(1);
}

pub fn run(project_dir: &Path, device: Option<&str>) {
    let (app_config, sdk_env) = assemble_and_build(project_dir).unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    });

    let signing = app_config.app.signing_for("default").unwrap_or_else(|| {
        eprintln!("✗ No signing config. Run `hvolt sign` first.");
        std::process::exit(1);
    });
    if signing.passwords_look_encoded() {
        eprintln!(
            "✗ Signing password looks DevEco-encoded (hex), not plaintext — `hvolt run` needs a \
             signed package. Replace store_password/key_password in hvolt.toml."
        );
        std::process::exit(1);
    }
    if let Err(e) = crate::sign::sign_app(
        &sdk_env,
        project_dir,
        &signing.store_file,
        &signing.store_password,
        &signing.key_alias,
        &signing.key_password,
        &signing.certpath,
        &signing.profile,
        &signing.sign_alg,
    ) {
        eprintln!("✗ Sign failed: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = push_app(project_dir, device) {
        report_push_failure(e);
    }

    launch(device, &app_config.app.bundle, "EntryAbility");
}

/// Map known hdc install error codes to readable hints.
pub(crate) fn explain_install_error(output: &str) -> String {
    // Extract the [Fail] line(s) — they contain the actual error
    let fail_lines: Vec<&str> = output.lines().filter(|l| l.contains("[Fail]")).collect();

    // Extract the error code from any line
    let error_code = output.lines().find_map(|l| {
        let l = l.trim();
        let start = l.find("error:")?;
        let err = &l[start..];
        // Grab the error code like INSTALL_FAILED_xxx or INSTALL_PARSE_FAILED_xxx
        let code = err.split_whitespace().last().unwrap_or(err);
        Some(code.to_string())
    });

    let hint = error_code.as_deref().and_then(|code| match code {
        c if c.contains("INSTALL_FAILED_SIGNATURE_ERROR") => {
            Some("  Hint: signature mismatch or certificate expired. Run `hvolt sign` to renew.")
        }
        c if c.contains("INSTALL_FAILED_INVALID_APK") => {
            Some("  Hint: invalid HAP format — try `hvolt build` to rebuild.")
        }
        c if c.contains("INSTALL_PARSE_FAILED_USESDK_ERROR") => {
            Some("  Hint: SDK version mismatch — check compile_sdk_version in hvolt.toml.")
        }
        c if c.contains("INSTALL_FAILED_NO_SIGNATURE") => {
            Some("  Hint: HAP is not signed. Run `hvolt sign` first.")
        }
        c if c.contains("INSTALL_FAILED_BUNDLE_PARSE_FAILURE") => {
            Some("  Hint: bundle parse failure — check module.json5 configuration.")
        }
        c if c.contains("INSTALL_FAILED_VERSION_DOWNGRADE") => {
            Some("  Hint: version downgrade not allowed — bump versionCode in hvolt.toml.")
        }
        c if c.contains("INSTALL_FAILED_APP_SOURCE_NOT_TRUSTED") => {
            Some("  Hint: app source not trusted — check signing config.")
        }
        _ => None,
    });

    let mut msg = String::new();
    if !fail_lines.is_empty() {
        msg.push_str(&fail_lines.join("\n"));
        if let Some(code) = &error_code {
            if !msg.contains(code) {
                msg.push_str(&format!("\n  Error: {}", code));
            }
        }
    } else if let Some(code) = &error_code {
        msg.push_str(&format!("Error: {}", code));
    } else {
        msg.push_str(output.trim());
    }
    if let Some(h) = hint {
        msg.push_str(&format!("\n{}", h));
    }
    msg
}

/// Push signed HAP to device via hdc (local) or scp+remote hdc (cross-machine).
pub fn push_app(project_dir: &Path, device: Option<&str>) -> Result<(), String> {
    let cfg = crate::sdk::load_global_config();
    if !cfg.device_host.is_empty() {
        let hap = find_signed_hap(project_dir)?;
        println!(
            "  remote install: {} via {}",
            cfg.device_target, cfg.device_host
        );
        let out = crate::target::remote_install(
            &cfg.device_host,
            &cfg.device_hdc,
            &cfg.device_target,
            &hap,
        )?;
        for line in out.lines() {
            if !line.trim().is_empty() {
                println!("  device-host: {}", line.trim());
            }
        }
        println!("✓ Installed to remote device");
        return Ok(());
    }
    let (target, mut cmd) = crate::target::hdc_targeted_command(device)?;
    println!("  push target: {}", target);
    let signed = find_signed_hap(project_dir)?;
    cmd.arg("install").arg(&signed);
    let output = cmd.output().map_err(|e| format!("hdc install: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Always show hdc output so user sees install progress
    for line in stdout.lines() {
        println!("  hdc: {}", line);
    }
    for line in stderr.lines() {
        eprintln!("  hdc: {}", line);
    }

    // hdc can exit 0 even when install fails (error goes to stdout)
    if output.status.success() && !stdout.contains("[Fail]") && !stdout.contains("failed") {
        println!("✓ Installed to device");
        Ok(())
    } else {
        let detail = explain_install_error(&stdout);
        Err(format!("hdc install failed: {}", detail))
    }
}

fn launch(device: Option<&str>, bundle: &str, entry: &str) {
    let cfg = crate::sdk::load_global_config();
    if !cfg.device_host.is_empty() {
        match crate::target::remote_launch(
            &cfg.device_host,
            &cfg.device_hdc,
            &cfg.device_target,
            bundle,
            entry,
        ) {
            Ok(out) => println!(
                "✓ App launched on remote device: {}.{} ({})",
                bundle,
                entry,
                out.trim()
            ),
            Err(e) => {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        return;
    }
    let (_, mut cmd) = match crate::target::hdc_targeted_command(device) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
    };

    cmd.arg("shell")
        .arg("aa")
        .arg("start")
        .arg("-a")
        .arg(entry)
        .arg("-b")
        .arg(bundle);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("✗ Launch failed: hdc error — {}", e);
            std::process::exit(1); // `return` here made `hvolt run` exit 0 on launch failure
        }
    };
    if output.status.success() && !String::from_utf8_lossy(&output.stdout).contains("error") {
        println!("✓ App launched: {}.{}", bundle, entry);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("✗ Launch failed: {}{}", stderr.trim(), stdout.trim());
        std::process::exit(1);
    }
}

/// Locate the signed hap in output/.
/// Newest *-signed.hap under output/ — "newest" by mtime, matching the
/// help text (push used to take the FIRST directory entry while
/// claiming newest — multi-hap projects could install an arbitrary package).
pub fn find_signed_hap(project_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let output_dir = project_dir.join("output");
    let mut cands: Vec<std::path::PathBuf> = std::fs::read_dir(&output_dir)
        .map_err(|_| "output/ not found — run `hvolt build` first".to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().ends_with("-signed.hap"))
                .unwrap_or(false)
        })
        .collect();
    cands.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .ok()
            .cmp(&a.metadata().and_then(|m| m.modified()).ok())
    });
    cands
        .into_iter()
        .next()
        .ok_or_else(|| "No *-signed.hap in output/ — run `hvolt build` first".to_string())
}

#[cfg(test)]
mod install_error_tests {
    use super::explain_install_error;

    /// Signature problems must point at the command that fixes them.
    #[test]
    fn signature_errors_point_at_signing() {
        assert!(
            explain_install_error("error: INSTALL_FAILED_SIGNATURE_ERROR").contains("hvolt sign")
        );
        assert!(explain_install_error("error: INSTALL_FAILED_NO_SIGNATURE").contains("not signed"));
    }

    /// A downgrade is fixed by bumping versionCode, so the hint must name it.
    #[test]
    fn version_downgrade_names_version_code() {
        let msg = explain_install_error("error: INSTALL_FAILED_VERSION_DOWNGRADE");
        assert!(msg.contains("versionCode"));
    }

    /// [Fail] lines carry the real reason and are kept; unrelated progress noise is dropped.
    #[test]
    fn fail_lines_are_preserved_and_noise_dropped() {
        let out = "[Fail]Error: failed to install bundle\nprogress: 40%\nerror: INSTALL_FAILED_INVALID_APK";
        let msg = explain_install_error(out);
        assert!(msg.contains("[Fail]Error: failed to install bundle"));
        assert!(msg.contains("hvolt build"));
        assert!(!msg.contains("progress: 40%"));
    }

    /// An unknown code is surfaced, but no hint is invented for it.
    #[test]
    fn unknown_codes_surface_without_a_hint() {
        let msg = explain_install_error("error: INSTALL_SOMETHING_NEW");
        assert!(msg.contains("INSTALL_SOMETHING_NEW"));
        assert!(!msg.contains("Hint:"));
    }

    /// No code, no [Fail] line: pass the trimmed output through and never panic.
    #[test]
    fn unrecognised_output_is_passed_through() {
        assert_eq!(
            explain_install_error("  plain failure text  \n"),
            "plain failure text"
        );
        assert_eq!(explain_install_error(""), "");
    }
}
