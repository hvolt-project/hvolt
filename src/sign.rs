// Sign: local signing only (hap-sign-tool.jar from the SDK toolchains).
//
// Remote signing through AppGallery Connect lives in the optional backend
// (crate::backend); this module has no network or AGC dependency.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One `hap-sign-tool sign-app -mode localSign` invocation.
///
/// `sign_app` and `sign_package` pass the same flags (measured against the SDK 26 tool);
/// describing them once keeps the two call sites from drifting apart, and the contract
/// becomes testable without running java.
pub(crate) struct SignRequest<'a> {
    pub jar: &'a Path,
    pub in_path: &'a Path,
    pub out_path: &'a Path,
    pub key_alias: &'a str,
    pub key_password: &'a str,
    pub store_file: &'a str,
    pub store_password: &'a str,
    pub certpath: &'a str,
    pub profile: &'a str,
    pub sign_alg: &'a str,
    /// `sign_app` passes `-profileSigned 1`; `sign_package` historically does not.
    pub profile_signed: bool,
}

impl SignRequest<'_> {
    pub(crate) fn to_args(&self) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "-jar".into(),
            self.jar.to_string_lossy().into_owned(),
            "sign-app".into(),
            "-mode".into(),
            "localSign".into(),
            "-keyAlias".into(),
            self.key_alias.into(),
            "-keyPwd".into(),
            self.key_password.into(),
            "-keystoreFile".into(),
            self.store_file.into(),
            "-keystorePwd".into(),
            self.store_password.into(),
            "-appCertFile".into(),
            self.certpath.into(),
            "-profileFile".into(),
            self.profile.into(),
            "-signAlg".into(),
            self.sign_alg.into(),
            "-inFile".into(),
            self.in_path.to_string_lossy().into_owned(),
            "-outFile".into(),
            self.out_path.to_string_lossy().into_owned(),
        ];
        if self.profile_signed {
            args.push("-profileSigned".into());
            args.push("1".into());
        }
        args
    }
}

/// Sign the unsigned HAP using hap-sign-tool.jar (local signing).
#[allow(clippy::too_many_arguments)] // mirrors the signing CLI surface 1:1
pub fn sign_app(
    sdk: &crate::sdk::SdkEnv,
    project_dir: &Path,
    store_file: &str,
    store_password: &str,
    key_alias: &str,
    key_password: &str,
    certpath: &str,
    profile: &str,
    sign_alg: &str,
) -> Result<(), String> {
    let java = find_java(sdk);
    let hap_sign_tool = find_hap_sign_tool(sdk)?;
    let unsigned = find_unsigned_hap(&project_dir.join("build"))?;
    let output_dir = project_dir.join("output");
    fs::create_dir_all(&output_dir).map_err(|e| format!("create output/: {}", e))?;
    let output_name = unsigned
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .replace("-unsigned", "-signed");
    let hap_path = output_dir.join(&*output_name);

    if !unsigned.exists() {
        return Err(format!("unsigned HAP not found: {}", unsigned.display()));
    }

    // Check certificate expiration before invoking hap-sign-tool
    let abs_certpath = if Path::new(certpath).is_absolute() {
        let p: &Path = certpath.as_ref();
        p.to_path_buf()
    } else {
        project_dir.join(certpath)
    };
    if abs_certpath.exists() {
        match check_cert_expired(&abs_certpath)? {
            Some(false) => {
                return Err(format!(
                    "Certificate is expired: {}\n  Run `hvolt sign` to renew.",
                    abs_certpath.display()
                ));
            }
            Some(true) => { /* valid */ }
            None => { /* openssl unavailable, skip check */ }
        }
    }

    println!("  Signing HAP...");
    let request = SignRequest {
        jar: &hap_sign_tool,
        in_path: &unsigned,
        out_path: &hap_path,
        key_alias,
        key_password,
        store_file,
        store_password,
        certpath,
        profile,
        sign_alg,
        profile_signed: true,
    };
    let output = Command::new(&java)
        .args(request.to_args())
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("hap-sign-tool: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            "hap-sign-app failed".to_string()
        };
        return Err(format!("Sign failed: {}", detail));
    }

    println!(
        "  ✓ {} ({:.1} KB)",
        output_name,
        hap_path
            .metadata()
            .map(|m| m.len() as f64 / 1024.0)
            .unwrap_or(0.0)
    );
    Ok(())
}

/// Sign ANY package file (hap/hsp/hqf — the tool treats them alike;
/// measured: hqf post-sign works with identical flags).
#[allow(clippy::too_many_arguments)] // mirrors the signing CLI surface 1:1
pub fn sign_package(
    sdk: &crate::sdk::SdkEnv,
    in_path: &Path,
    out_path: &Path,
    store_file: &str,
    store_password: &str,
    key_alias: &str,
    key_password: &str,
    certpath: &str,
    profile: &str,
    sign_alg: &str,
) -> Result<(), String> {
    let java = find_java(sdk);
    let tool = find_hap_sign_tool(sdk)?;
    let request = SignRequest {
        jar: &tool,
        in_path,
        out_path,
        key_alias,
        key_password,
        store_file,
        store_password,
        certpath,
        profile,
        sign_alg,
        profile_signed: false,
    };
    let out = Command::new(&java)
        .args(request.to_args())
        .output()
        .map_err(|e| format!("hap-sign-tool: {}", e))?;
    let so = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || so.contains("[Fail]") {
        return Err(format!(
            "sign failed:\n{}{}",
            so.trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub(crate) fn find_java(sdk: &crate::sdk::SdkEnv) -> PathBuf {
    let candidates = [
        crate::platform::find_in_dirs(
            "java",
            [sdk.java_home.join("bin").clone(), sdk.java_home.clone()],
        )
        .unwrap_or_else(|| sdk.java_home.join("bin").join("java")),
        sdk.java_home.join("bin").join("java.exe"),
    ];
    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }
    PathBuf::from("java")
}

pub(crate) fn find_hap_sign_tool(sdk: &crate::sdk::SdkEnv) -> Result<PathBuf, String> {
    find_tool_jar(sdk, "hap-sign-tool.jar")
}

/// Locate a tool jar under any SDK layout (DevEco / hvolt store / ohos view).
pub(crate) fn find_tool_jar(sdk: &crate::sdk::SdkEnv, jar_name: &str) -> Result<PathBuf, String> {
    let candidates = [
        sdk.ohos_sdk
            .join(format!("openharmony/toolchains/lib/{}", jar_name)),
        sdk.ohos_sdk
            .join(format!("default/openharmony/toolchains/lib/{}", jar_name)),
    ];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    Err(format!("{} not found in SDK", jar_name))
}

pub(crate) fn find_unsigned_hap(build_dir: &Path) -> Result<PathBuf, String> {
    if let Ok(entries) = fs::read_dir(build_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let outputs = path.join("build/default/outputs/default");
                if outputs.exists() {
                    if let Ok(hap_entries) = fs::read_dir(&outputs) {
                        for hap in hap_entries.flatten() {
                            let hap_path = hap.path();
                            let name = hap_path.file_name().unwrap_or_default().to_string_lossy();
                            if name.ends_with("-unsigned.hap") {
                                return Ok(hap_path);
                            }
                        }
                    }
                }
            }
        }
    }
    Err(format!(
        "unsigned HAP not found under {}",
        build_dir.display()
    ))
}

/// Check if a certificate file is expired using `openssl x509 -checkend 0`.
/// Returns `Ok(None)` if openssl is unavailable (pass-through), `Ok(Some(true))` if valid,
/// `Ok(Some(false))` if expired.
pub(crate) fn check_cert_expired(certpath: &Path) -> Result<Option<bool>, String> {
    let openssl = std::process::Command::new("openssl")
        .arg("x509")
        .arg("-in")
        .arg(certpath)
        .arg("-noout")
        .arg("-checkend")
        .arg("0")
        .output();

    match openssl {
        Ok(out) => {
            if out.status.success() {
                Ok(Some(true))
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("unable to load certificate") {
                    Err(format!("Cannot parse certificate: {}", certpath.display()))
                } else {
                    Ok(Some(false))
                }
            }
        }
        Err(_) => {
            // openssl not on PATH — skip check
            Ok(None)
        }
    }
}

#[cfg(test)]
mod request_tests {
    use super::SignRequest;
    use std::path::Path;

    fn req(profile_signed: bool) -> SignRequest<'static> {
        SignRequest {
            jar: Path::new("/sdk/hap-sign-tool.jar"),
            in_path: Path::new("/p/output/entry-unsigned.hap"),
            out_path: Path::new("/p/output/entry-signed.hap"),
            key_alias: "debugKey",
            key_password: "kp",
            store_file: "/p/signing/debug/debug.p12",
            store_password: "sp",
            certpath: "/p/signing/debug/debug.cer",
            profile: "/p/signing/debug/debug.p7b",
            sign_alg: "SHA256withECDSA",
            profile_signed,
        }
    }

    /// The flags measured against SDK 26's hap-sign-tool must not drift.
    #[test]
    fn args_carry_the_measured_sign_app_contract() {
        let args = req(true).to_args();
        let got: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got[..5],
            [
                "-jar",
                "/sdk/hap-sign-tool.jar",
                "sign-app",
                "-mode",
                "localSign"
            ]
        );
        for (flag, value) in [
            ("-keyAlias", "debugKey"),
            ("-keyPwd", "kp"),
            ("-keystoreFile", "/p/signing/debug/debug.p12"),
            ("-keystorePwd", "sp"),
            ("-appCertFile", "/p/signing/debug/debug.cer"),
            ("-profileFile", "/p/signing/debug/debug.p7b"),
            ("-signAlg", "SHA256withECDSA"),
            ("-inFile", "/p/output/entry-unsigned.hap"),
            ("-outFile", "/p/output/entry-signed.hap"),
        ] {
            let i = got
                .iter()
                .position(|a| *a == flag)
                .unwrap_or_else(|| panic!("missing {flag}"));
            assert_eq!(got[i + 1], value, "value of {flag}");
        }
    }

    /// `-profileSigned 1` is what the two call sites differ on, so it must stay opt-in.
    #[test]
    fn profile_signed_is_opt_in() {
        let with = req(true).to_args();
        let without = req(false).to_args();
        assert!(with.iter().any(|a| a == "-profileSigned"));
        assert!(!without.iter().any(|a| a == "-profileSigned"));
        assert_eq!(with.len(), without.len() + 2);
    }
}
