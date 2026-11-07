// Native sign: sign and verify a package in-process, without java or hap-sign-tool.jar.
//
// The SDK ships signing as a Java tool. `hapsigner` (MIT) performs the same work in
// Rust: localSign CMS signing of the package, `sign-profile` CMS signing of the
// provisioning profile, and the `verify-app` / `verify-profile` checks that accept the
// result. Inputs are the ones the CLI takes (p12 store, key alias, certificate chain,
// signed profile p7b), so the identity a package carries is the same either way.

use std::fs;
use std::path::Path;

/// Everything `sign-app -mode localSign` needs, as the paths the caller named.
pub(crate) struct LocalSign<'a> {
    pub store_file: &'a Path,
    pub store_password: &'a str,
    pub key_alias: &'a str,
    pub key_password: &'a str,
    pub cert_chain: &'a Path,
    pub profile: &'a Path,
    pub sign_alg: &'a str,
}

/// Everything `sign-profile -mode localSign` needs.
pub(crate) struct ProfileSign<'a> {
    pub store_file: &'a Path,
    pub store_password: &'a str,
    pub key_alias: &'a str,
    pub key_password: &'a str,
    /// Certificate that signs the profile (`-profileCertFile`), not the app certificate.
    pub profile_cert: &'a Path,
    pub sign_alg: &'a str,
}

/// What the acceptance check reads back out of a finished package.
#[derive(Debug)]
pub(crate) struct VerifiedPackage {
    pub bundle: String,
    pub profile: serde_json::Value,
}

/// Map the SDK's `-signAlg` spelling onto the algorithm the signer implements.
///
/// Only the ECDSA spellings are mapped: they are the ones the SDK test chain and
/// every hvolt signing path use (`DEFAULT_SIGN_ALG`). An unmapped name is a caller
/// error, not a silent fallback to a different identity.
pub(crate) fn algorithm_from_flag(flag: &str) -> Result<hapsigner::SigningAlgorithm, String> {
    use hapsigner::SigningAlgorithm as A;
    match flag {
        "SHA256withECDSA" => Ok(A::EcdsaSha256),
        "SHA384withECDSA" => Ok(A::EcdsaSha384),
        "SHA512withECDSA" => Ok(A::EcdsaSha512),
        other => Err(format!(
            "unsupported --sign-alg {other} (SHA256withECDSA|SHA384withECDSA|SHA512withECDSA)"
        )),
    }
}

/// Pick the stored alias that names `wanted`, ignoring case.
///
/// A p12 keeps the alias as it was typed at generation time (the official test
/// store holds them lower-case), while callers name them the way the SDK CLI
/// documents them (`OpenHarmony Application Release`). The Java tool matched
/// case-insensitively; this restores that, and only that.
fn resolve_alias<'a>(available: &'a [String], wanted: &str) -> Option<&'a str> {
    available
        .iter()
        .find(|a| a.eq_ignore_ascii_case(wanted))
        .map(String::as_str)
}

/// Decrypt the private key named by `key_alias` out of the p12 store.
///
/// The one place alias lookup happens, so package signing and profile signing cannot
/// disagree about which key an alias names.
fn load_key(
    store: &[u8],
    store_password: &str,
    key_alias: &str,
    key_password: &str,
) -> Result<hapsigner::SigningKey, String> {
    match hapsigner::SigningKey::from_pkcs12(store, store_password, key_alias, key_password) {
        Ok(key) => Ok(key),
        // The only alias failure worth retrying: the store names it differently.
        Err(hapsigner::SignError::KeyAliasNotFound { available, .. }) => {
            let stored = resolve_alias(&available, key_alias).ok_or_else(|| {
                format!("private key alias '{key_alias}' not found; available: {available:?}")
            })?;
            hapsigner::SigningKey::from_pkcs12(store, store_password, stored, key_password)
                .map_err(|e| format!("load key as '{stored}': {e}"))
        }
        Err(e) => Err(format!("load key: {e}")),
    }
}

/// Read a certificate file into the DER chain the signer expects.
///
/// Accepts both a raw DER chain and PEM text (one or more
/// `-----BEGIN CERTIFICATE-----` blocks); AGC/DevEco ship `.cer` as PEM.
fn read_cert_chain(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let text = String::from_utf8_lossy(&bytes);
    if text.contains("BEGIN CERTIFICATE") {
        let mut chain = Vec::new();
        for block in text.split("-----BEGIN CERTIFICATE-----").skip(1) {
            let der = block
                .split("-----END CERTIFICATE-----")
                .next()
                .unwrap_or_default();
            let der: String = der.chars().filter(|c| !c.is_whitespace()).collect();
            use base64::Engine;
            let der = base64::engine::general_purpose::STANDARD
                .decode(der.as_bytes())
                .map_err(|e| format!("decode PEM block: {e}"))?;
            chain.push(der);
        }
        if chain.is_empty() {
            return Err(format!(
                "certificate chain {}: no PEM blocks",
                path.display()
            ));
        }
        // AGC/DevEco export `.cer` in root-first order; the signer expects the
        // leaf certificate first (it verifies with the first chain entry).
        chain.reverse();
        return Ok(chain);
    }
    hapsigner::SigningKey::cert_chain_from_bytes(&bytes)
        .map_err(|e| format!("load certificate chain {}: {e}", path.display()))
}

/// Load the materials into the signer's immutable form.
///
/// Everything that can fail on the caller's input (missing file, wrong password,
/// alias mismatch) fails here, before any package byte is touched.
fn load_material(m: &LocalSign<'_>) -> Result<hapsigner::SigningMaterial, String> {
    let store =
        fs::read(m.store_file).map_err(|e| format!("read {}: {}", m.store_file.display(), e))?;
    let algorithm = algorithm_from_flag(m.sign_alg)?;
    let mut key = load_key(&store, m.store_password, m.key_alias, m.key_password)?;
    key.cert_chain = read_cert_chain(m.cert_chain)?;
    let profile =
        fs::read(m.profile).map_err(|e| format!("read {}: {}", m.profile.display(), e))?;
    // `SigningKey` zeroizes on drop, so its fields are copied into the material rather
    // than moved: the key is dropped at the end of this function either way.
    hapsigner::SigningMaterial::from_der(
        key.private_key_der.clone(),
        key.cert_chain.clone(),
        profile,
        algorithm,
    )
    .map_err(|e| format!("load signing material: {e}"))
}

/// Sign `in_path` into `out_path` with the materials the caller named.
pub(crate) fn sign_package(
    m: &LocalSign<'_>,
    in_path: &Path,
    out_path: &Path,
) -> Result<(), String> {
    if !in_path.exists() {
        return Err(format!("{} not found", in_path.display()));
    }
    let material = load_material(m)?;
    let signer = hapsigner::HapSigner::new(material, hapsigner::SignOptions::default());
    signer
        .sign_file(in_path, out_path)
        .map_err(|e| format!("sign {}: {e}", in_path.display()))
}

/// Sign the unsigned provisioning-profile JSON into the p7b the package embeds.
pub(crate) fn sign_profile(
    m: &ProfileSign<'_>,
    unsigned_profile: &Path,
    out_path: &Path,
) -> Result<(), String> {
    let store =
        fs::read(m.store_file).map_err(|e| format!("read {}: {}", m.store_file.display(), e))?;
    let algorithm = algorithm_from_flag(m.sign_alg)?;
    let mut key = load_key(&store, m.store_password, m.key_alias, m.key_password)?;
    key.cert_chain = read_cert_chain(m.profile_cert)?;
    let unsigned = fs::read(unsigned_profile)
        .map_err(|e| format!("read {}: {}", unsigned_profile.display(), e))?;
    let signed = hapsigner::ProfileSigner::new(key, algorithm)
        .sign(&unsigned)
        .map_err(|e| format!("sign profile: {e}"))?;
    fs::write(out_path, &signed).map_err(|e| format!("write {}: {}", out_path.display(), e))
}

/// Verify a finished package the way `verify-app` + `verify-profile` do, and read
/// back the profile it carries.
pub(crate) fn verify_application(hap: &Path) -> Result<VerifiedPackage, String> {
    let bytes = fs::read(hap).map_err(|e| format!("read {}: {}", hap.display(), e))?;
    let app = hapsigner::ApplicationVerifier::new(&bytes)
        .verify(hapsigner::InputFormat::Zip)
        .map_err(|e| format!("verify app: {e}"))?;
    let profile_bytes = app
        .profile
        .ok_or_else(|| "signed package carries no provisioning profile".to_string())?;
    let verified = hapsigner::ProfileVerifier::verify(&profile_bytes)
        .map_err(|e| format!("verify profile: {e}"))?;
    let profile: serde_json::Value = serde_json::from_slice(&verified.content)
        .map_err(|e| format!("parse profile content: {e}"))?;
    let bundle = profile["bundle-info"]["bundle-name"]
        .as_str()
        .ok_or_else(|| "profile carries no bundle-name".to_string())?
        .to_string();
    Ok(VerifiedPackage { bundle, profile })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_flag_maps_ecdsa_family() {
        use hapsigner::SigningAlgorithm as A;
        assert_eq!(
            algorithm_from_flag("SHA256withECDSA").unwrap(),
            A::EcdsaSha256
        );
        assert_eq!(
            algorithm_from_flag("SHA384withECDSA").unwrap(),
            A::EcdsaSha384
        );
        assert_eq!(
            algorithm_from_flag("SHA512withECDSA").unwrap(),
            A::EcdsaSha512
        );
    }

    #[test]
    fn unknown_algorithm_is_rejected_with_the_supported_list() {
        let err = algorithm_from_flag("MD5withRSA").unwrap_err();
        assert!(err.contains("MD5withRSA"), "{}", err);
        assert!(err.contains("SHA256withECDSA"), "{}", err);
    }

    #[test]
    fn missing_material_reports_the_path_not_a_bare_io_error() {
        let m = LocalSign {
            store_file: Path::new("/nonexistent/hvolt-test.p12"),
            store_password: "123456",
            key_alias: "alias",
            key_password: "123456",
            cert_chain: Path::new("/nonexistent/cert.pem"),
            profile: Path::new("/nonexistent/profile.p7b"),
            sign_alg: "SHA256withECDSA",
        };
        let err = sign_package(
            &m,
            Path::new("/nonexistent/in.hap"),
            Path::new("/nonexistent/out.hap"),
        )
        .unwrap_err();
        assert!(err.contains("/nonexistent/in.hap"), "{}", err);
    }

    #[test]
    fn absent_input_package_is_refused_before_materials_are_read() {
        let m = LocalSign {
            store_file: Path::new("/nonexistent/hvolt-test.p12"),
            store_password: "123456",
            key_alias: "alias",
            key_password: "123456",
            cert_chain: Path::new("/nonexistent/cert.pem"),
            profile: Path::new("/nonexistent/profile.p7b"),
            sign_alg: "SHA256withECDSA",
        };
        let err = sign_package(
            &m,
            Path::new("/nonexistent/in.hap"),
            Path::new("/nonexistent/out.hap"),
        )
        .unwrap_err();
        assert!(err.contains("not found"), "{}", err);
    }

    /// The p12 stores aliases lower-case; callers name them as the SDK documents them.
    #[test]
    fn alias_resolves_case_insensitively_to_the_stored_spelling() {
        let available = vec![
            "openharmony application release".to_string(),
            "openharmony application profile debug".to_string(),
        ];
        assert_eq!(
            resolve_alias(&available, "OpenHarmony Application Release"),
            Some("openharmony application release")
        );
        assert_eq!(resolve_alias(&available, "Not There"), None);
    }

    /// An exact hit is returned as stored, so the retry never fires needlessly.
    #[test]
    fn alias_matching_is_not_a_prefix_match() {
        let available = vec!["openharmony application release".to_string()];
        assert_eq!(resolve_alias(&available, "openharmony application"), None);
    }

    #[test]
    fn verifying_a_missing_package_reports_the_path() {
        let err = verify_application(Path::new("/nonexistent/out.hap")).unwrap_err();
        assert!(err.contains("/nonexistent/out.hap"), "{}", err);
    }
}
