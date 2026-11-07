// Sign: local signing only, performed in process by `native_sign` (no java).
//
// Remote signing through AppGallery Connect lives in the optional backend
// (crate::sign::backend); this module has no network or AGC dependency.

use crate::sign::native_sign::{self, LocalSign};
use std::fs;
use std::path::{Path, PathBuf};

/// Sign the unsigned HAP in process (`native_sign`), localSign identity.
#[allow(clippy::too_many_arguments)] // mirrors the signing CLI surface 1:1
pub fn sign_app(
    _sdk: &crate::sdk::manage::SdkEnv,
    project_dir: &Path,
    store_file: &str,
    store_password: &str,
    key_alias: &str,
    key_password: &str,
    certpath: &str,
    profile: &str,
    sign_alg: &str,
) -> Result<(), String> {
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

    // Check certificate expiration before signing
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
    native_sign::sign_package(
        &LocalSign {
            store_file: Path::new(store_file),
            store_password,
            key_alias,
            key_password,
            cert_chain: Path::new(certpath),
            profile: Path::new(profile),
            sign_alg,
        },
        &unsigned,
        &hap_path,
    )?;

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
    _sdk: &crate::sdk::manage::SdkEnv,
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
    native_sign::sign_package(
        &LocalSign {
            store_file: Path::new(store_file),
            store_password,
            key_alias,
            key_password,
            cert_chain: Path::new(certpath),
            profile: Path::new(profile),
            sign_alg,
        },
        in_path,
        out_path,
    )
}

/// The signature algorithm the SDK tool is invoked with for every package hvolt signs.
pub const DEFAULT_SIGN_ALG: &str = "SHA256withECDSA";

/// Signing materials, one field per `hvolt.toml` `[app.signing]` entry.
///
/// Passed in rather than read from a project file: a bare package handed over on the command
/// line is signed with whatever the caller named, and no project needs to exist.
pub struct Materials<'a> {
    pub store_file: &'a Path,
    pub store_password: &'a str,
    pub key_alias: &'a str,
    pub key_password: &'a str,
    pub certpath: &'a Path,
    pub profile: &'a Path,
    pub sign_alg: &'a str,
}

/// Sign a package the caller points at with the materials the caller named.
///
/// The result stays in the package's own directory as `<name>-signed.hap`. A package that is
/// already signed is refused; one that merely carries the signed *name* is not, which is why
/// the check reads the archive instead of the file name.
pub fn sign_file(
    sdk: &crate::sdk::manage::SdkEnv,
    in_path: &Path,
    m: &Materials,
) -> Result<PathBuf, String> {
    if !in_path.exists() {
        return Err(format!("{} not found", in_path.display()));
    }
    if crate::project::hap::is_signed(in_path)? {
        return Err(format!(
            "{} is already signed — refusing to sign it twice",
            in_path.display()
        ));
    }
    if m.certpath.exists() {
        if let Some(false) = check_cert_expired(m.certpath)? {
            return Err(format!(
                "Certificate is expired: {}\n  Run `hvolt sign` to renew.",
                m.certpath.display()
            ));
        }
    }
    let out_path = signed_sibling(in_path);
    // Input and output are the same file when the package already carries the signed name.
    // Signing straight into it would truncate the input before the tool could read it, so the
    // result goes to a sibling temp file and replaces the input only after it is complete.
    let same_path = in_path == out_path;
    let write_to = if same_path {
        in_path.with_file_name(format!(
            "{}.hvolt-tmp.hap",
            in_path.file_stem().unwrap_or_default().to_string_lossy()
        ))
    } else {
        out_path.clone()
    };
    if write_to.exists() {
        std::fs::remove_file(&write_to)
            .map_err(|e| format!("remove stale {}: {}", write_to.display(), e))?;
    }
    println!("  Signing {}...", in_path.display());
    let result = sign_package(
        sdk,
        in_path,
        &write_to,
        &m.store_file.to_string_lossy(),
        m.store_password,
        m.key_alias,
        m.key_password,
        &m.certpath.to_string_lossy(),
        &m.profile.to_string_lossy(),
        m.sign_alg,
    );
    if let Err(e) = result {
        let _ = std::fs::remove_file(&write_to);
        return Err(e);
    }
    if same_path {
        std::fs::rename(&write_to, &out_path)
            .map_err(|e| format!("replace {}: {}", out_path.display(), e))?;
    }
    println!("✓ Signed: {}", out_path.display());
    Ok(out_path)
}

/// `<dir>/<name>-unsigned.hap` → `<dir>/<name>-signed.hap`; any other name gains `-signed`.
///
/// One rule for every input: drop an existing `-unsigned`/`-signed` marker, then emit the
/// signed name. An input that already carries the signed name therefore maps onto itself; the
/// caller writes through a temp file in that case, so the input is never truncated.
pub fn signed_sibling(in_path: &Path) -> PathBuf {
    let name = in_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let stem = name.strip_suffix(".hap").unwrap_or(&name);
    let stem = stem
        .strip_suffix("-unsigned")
        .or_else(|| stem.strip_suffix("-signed"))
        .unwrap_or(stem);
    in_path.with_file_name(format!("{}-signed.hap", stem))
}

pub(crate) fn find_unsigned_hap(build_dir: &Path) -> Result<PathBuf, String> {
    // Native (standalone) chain only: packed `<module>-unsigned.hap` sits in
    // the build root. No foreign layout is recognized.
    if let Ok(entries) = fs::read_dir(build_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.ends_with("-unsigned.hap") {
                    return Ok(path);
                }
            }
        }
    }
    Err(format!(
        "unsigned HAP not found under {}",
        build_dir.display()
    ))
}

/// Whether a certificate file is still valid, read without an external process.
///
/// This used to shell out to `openssl x509 -checkend 0`. `x509-cert` (already in the tree
/// through `hapsigner`) parses the same DER and exposes `notAfter`, so there is no reason
/// to depend on a binary that a bare host may not have.
///
/// `Ok(None)` used to mean "openssl missing, skip the check". Nothing can be missing now,
/// so it is never returned; the `Option` stays only so the caller's match keeps compiling.
pub(crate) fn check_cert_expired(certpath: &Path) -> Result<Option<bool>, String> {
    use der::Decode;
    use x509_cert::Certificate;

    let raw = std::fs::read(certpath).map_err(|e| format!("read {}: {}", certpath.display(), e))?;
    // AGC/DevEco ship `.cer` as PEM text; peel the first certificate off it.
    let text = String::from_utf8_lossy(&raw);
    let der = if text.contains("BEGIN CERTIFICATE") {
        let block = text
            .split("-----BEGIN CERTIFICATE-----")
            .nth(1)
            .and_then(|b| b.split("-----END CERTIFICATE-----").next())
            .unwrap_or_default();
        let der: String = block.chars().filter(|c| !c.is_whitespace()).collect();
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(der.as_bytes())
            .map_err(|e| format!("Cannot parse certificate: {} ({e})", certpath.display()))?
    } else {
        raw
    };
    let cert = Certificate::from_der(&der)
        .map_err(|e| format!("Cannot parse certificate: {} ({e})", certpath.display()))?;
    let not_after = cert.tbs_certificate.validity.not_after.to_system_time();
    Ok(Some(std::time::SystemTime::now() <= not_after))
}

#[cfg(test)]
mod request_tests {
    use super::signed_sibling;
    use std::path::Path;

    /// A bare package is signed beside itself: the directory is kept and `-unsigned` becomes
    /// `-signed`, which is the name every downstream reader looks for.
    #[test]
    fn signed_sibling_replaces_the_unsigned_suffix() {
        let out = signed_sibling(Path::new("/tmp/app-unsigned.hap"));
        assert_eq!(out, Path::new("/tmp/app-signed.hap"));
    }

    /// An input already carrying the signed name maps onto itself, so the writer has to route
    /// through a temp file; the name rule itself never invents a second suffix.
    #[test]
    fn signed_sibling_of_a_signed_package_maps_onto_itself() {
        let out = signed_sibling(Path::new("/tmp/app-signed.hap"));
        assert_eq!(out, Path::new("/tmp/app-signed.hap"));
    }

    /// A package carrying neither suffix gains one, in the same directory as the input.
    #[test]
    fn signed_sibling_marks_a_plain_package() {
        let out = signed_sibling(Path::new("/tmp/app.hap"));
        assert_eq!(out, Path::new("/tmp/app-signed.hap"));
    }
}
