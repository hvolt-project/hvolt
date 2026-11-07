// Sign: sign one package with the given materials, the localSign test chain, or the AGC backend.
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)] // one parameter per CLI flag, as in the clap variant
pub(crate) fn sign(
    hap: Option<PathBuf>,
    local: bool,
    apl: String,
    flavor: String,
    p12: Option<PathBuf>,
    profile: Option<PathBuf>,
    cert: Option<PathBuf>,
    key_alias: String,
    p12_password: String,
    key_password: String,
    bundle: Option<String>,
) {
    let kind = match flavor.as_str() {
        "debug" => crate::sign::sign_local::ProfileKind::Debug,
        _ => crate::sign::sign_local::ProfileKind::Release,
    };
    let project = std::path::Path::new(".");
    // A package named on the command line is signed where it lies; the project flow
    // below resolves its own unsigned HAP under build/ and writes into output/.
    if let Some(package) = hap {
        let result = match (p12, profile, cert) {
            (Some(p12), Some(profile), Some(cert)) => sign_with_materials(
                &package,
                p12,
                profile,
                cert,
                &key_alias,
                &p12_password,
                &key_password,
            ),
            (None, None, None) => {
                crate::sign::sign_local::sign_external(&package, &apl, kind, bundle.as_deref())
            }
            _ => Err(
                "--p12, --profile and --cert must be given together (or none of them, to \
                 use the localSign test chain)"
                    .to_string(),
            ),
        };
        if let Err(e) = result {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
        return;
    }
    if local {
        if let Err(e) = crate::sign::sign_local::sign(project, &apl, kind) {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
        return;
    }
    if let Err(e) = crate::sign::backend::sign_remote(project) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

/// Sign one named package with materials given on the command line.
///
/// Three files are needed because a signature is only meaningful with all of them; a partial
/// set is a mistake at the call site, not something to guess around.
fn sign_with_materials(
    package: &std::path::Path,
    p12: std::path::PathBuf,
    profile: std::path::PathBuf,
    cert: std::path::PathBuf,
    key_alias: &str,
    p12_password: &str,
    key_password: &str,
) -> Result<(), String> {
    let env = crate::sdk::manage::detect("").map_err(|e| format!("SDK env: {}", e))?;
    let materials = crate::sign::package::Materials {
        store_file: &p12,
        store_password: p12_password,
        key_alias,
        key_password,
        certpath: &cert,
        profile: &profile,
        sign_alg: crate::sign::package::DEFAULT_SIGN_ALG,
    };
    crate::sign::package::sign_file(&env, package, &materials)?;
    Ok(())
}
