//! OpenHarmony localSign: sign HAPs with the OFFICIAL public test certificate
//! chain (the identity pre-trusted by OpenHarmony dev boards / emulators /
//! source-built ROMs). Complementary to `hvolt sign` (AGC cloud certs, which
//! retail Huawei HarmonyOS requires) — the two identity domains are not
//! interchangeable. Verification targets the final package, not the logs:
//! `verify-app` then `verify-profile`, plus a cross-check of bundle / apl /
//! feature / validity inside the HAP.
//!
//! Materials are fetched ONCE from the official gitee repo
//! (openharmony/developtools_hapsigner dist/) and pinned by SHA-256; after
//! that the flow is fully offline.

use crate::sdk::manage as sdk;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const MATERIAL_BASE: &str = "https://gitee.com/openharmony/developtools_hapsigner/raw/master/dist";

/// (file, sha256) — measured 2026-10-10 from the official repo.
const MATERIALS: &[(&str, &str)] = &[
    (
        "OpenHarmony.p12",
        "7a5efc3ea9245596d157f6ccbbdc5b8ae7cd714e7086c3c1645986c9d495d071",
    ),
    (
        "OpenHarmonyApplication.pem",
        "b5c1d6254faa91eab645a32fc3ce136128c94d88a739a0e956e6a49b331387e5",
    ),
    (
        "OpenHarmonyProfileDebug.pem",
        "a7232bdad96a24bf8c5ac3460cd8163b4af605e38ce8a14d9381572ce1ea0344",
    ),
    (
        "OpenHarmonyProfileRelease.pem",
        "515f0bd8173fabb8bf8f146cd61207baa0e2d6ab8693ae7623df7ee6f16af70e",
    ),
    (
        "UnsgnedDebugProfileTemplate.json",
        "e7793d8501b56137b625e66ae8ff832adbbaf95b6e7c8440282c9423d174d16f",
    ),
    (
        "UnsgnedReleasedProfileTemplate.json",
        "5ee5a8ee4cde5586b9ec20dc0405d1ee71a7c7d95cbfdb18926e30418619e343",
    ),
];

/// Keystore passwords of the public test chain (OpenHarmony convention).
pub(crate) const TEST_P12_PWD: &str = "123456";

/// Which profile flavor to stamp.
#[derive(Clone, Copy, PartialEq)]
pub enum ProfileKind {
    Debug,
    Release,
}

/// (app key alias, profile key alias) per flavor — the fixed aliases baked
/// into the official test p12.
/// (app key alias, profile key alias). The official test p12 ships ONE app
/// signing key (Release) — debug/release differ only in the profile chain
/// (measured against the previous keystore tool; an invented "Application Debug" alias fails with
/// "keyAlias is exist" from hap-sign-tool).
pub(crate) fn aliases(kind: ProfileKind) -> (&'static str, &'static str) {
    match kind {
        ProfileKind::Release => (
            "OpenHarmony Application Release",
            "OpenHarmony Application Profile Release",
        ),
        ProfileKind::Debug => (
            "OpenHarmony Application Release",
            "OpenHarmony Application Profile Debug",
        ),
    }
}

/// apl level → (apl, app-feature) pairing. system_basic/system_core mark the
/// app as a system app (hos_system_app); normal for regular apps. NEVER
/// auto-upgrade to the highest level — choose per capability need.
pub(crate) fn apl_pair(apl: &str) -> Result<(String, String), String> {
    let p = match apl {
        "normal" => ("normal", "hos_normal_app"),
        "system_basic" => ("system_basic", "hos_system_app"),
        "system_core" => ("system_core", "hos_system_app"),
        other => {
            return Err(format!(
                "unknown --apl {other} (normal|system_basic|system_core)"
            ))
        }
    };
    Ok((p.0.into(), p.1.into()))
}

/// The leaf certificate of a PEM chain.
///
/// The official test chain is ordered root → CA → leaf (measured 2026-11-03:
/// `OpenHarmony Application Root CA`, `OpenHarmony Application CA`,
/// `OpenHarmony Application Release`), and `bundle-info.distribution-certificate`
/// names the certificate that signed the app — the leaf. A signer reads that field
/// as exactly one certificate, so the whole chain must not be pasted in.
pub(crate) fn leaf_certificate(chain_pem: &str) -> Result<String, String> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let mut blocks = Vec::new();
    let mut rest = chain_pem;
    while let Some(start) = rest.find(BEGIN) {
        let tail = &rest[start..];
        let end = tail
            .find(END)
            .ok_or_else(|| "unterminated PEM certificate block".to_string())?;
        blocks.push(tail[..end + END.len()].to_string());
        rest = &tail[end + END.len()..];
    }
    blocks
        .pop()
        .ok_or_else(|| "no certificate found in the application chain".to_string())
}

/// Build the unsgned profile JSON for this project. The official template's
/// STATIC validity (expired 2023-07) is deliberately recomputed — a copied
/// template would sign an expired identity, which hap-sign-tool happily does.
pub(crate) fn profile_json(
    bundle: &str,
    apl: &str,
    kind: ProfileKind,
    now_epoch: u64,
    cert_pem: &str,
) -> Result<serde_json::Value, String> {
    let (apl_v, feature) = apl_pair(apl)?;
    let days_years3: u64 = 3 * 365 * 86400;
    Ok(json!({
        "version-name": "1.0",
        "version-code": 1,
        "app-distribution-type": "os_integration",
        "uuid": uuid_like(now_epoch),
        "validity": { "not-before": now_epoch, "not-after": now_epoch + days_years3 },
        "type": match kind { ProfileKind::Debug => "debug", ProfileKind::Release => "release" },
        "bundle-info": {
            // Fixed profile field, not the project runtimeOS (config.rs owns that).
            "developer-id": "OpenHarmony",
            (match kind { ProfileKind::Debug => "development-certificate",
                           ProfileKind::Release => "distribution-certificate" }): cert_pem,
            "bundle-name": bundle,
            "apl": apl_v,
            "app-feature": feature,
        },
        "acls": { "allowed-acls": [] },
        "permissions": { "restricted-permissions": [] },
        "issuer": "pki_internal"
    }))
}

/// Deterministic-looking uuid from the clock (profile uuid is an identifier,
/// not a security value; format kept 8-4-4-4-12 hex).
fn uuid_like(seed: u64) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    "hvolt".hash(&mut h);
    let a = h.finish();
    let mut h2 = DefaultHasher::new();
    a.hash(&mut h2);
    let b = h2.finish();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        ((a >> 16) & 0xffff) as u16,
        (a & 0xffff) as u16,
        ((b >> 48) & 0xffff) as u16,
        b & 0xffffffffffff
    )
}

/// Material dir: ~/.hvolt/sign/ohos-local
pub(crate) fn material_dir() -> PathBuf {
    sdk::home_dir()
        .join(".hvolt")
        .join("sign")
        .join("ohos-local")
}

/// True if the file's sha256 equals the pinned one.
pub(crate) fn sha256_matches(path: &Path, want: &str) -> bool {
    sdk::sha256_file(path)
        .map(|got| got == want)
        .unwrap_or(false)
}

/// Fetch any missing/mismatched material, verifying pinned hashes.
/// On mismatch the file is DELETED (never trusted) and re-fetched once.
pub(crate) fn ensure_materials() -> Result<PathBuf, String> {
    let dir = material_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;
    for (name, pin) in MATERIALS {
        let p = dir.join(name);
        if p.exists() && sha256_matches(&p, pin) {
            continue;
        }
        if p.exists() {
            let _ = fs::remove_file(&p); // wrong bytes: refuse, refetch below
        }
        eprintln!("  fetching {} ...", name);
        let url = format!("{}/{}", MATERIAL_BASE, name);
        let body = ureq_get_bytes(&url)?;
        // verify before writing anything
        let got = {
            use std::io::Write;
            let tmp = dir.join(format!("{}.tmp", name));
            let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(&body).map_err(|e| e.to_string())?;
            f.flush().ok();
            sdk::sha256_file(&tmp)?
        };
        if got != *pin {
            let _ = fs::remove_file(dir.join(format!("{}.tmp", name)));
            return Err(format!("{}: SHA-256 mismatch (got {} want {}) — official material changed? refuse to install", name, got, pin));
        }
        fs::rename(dir.join(format!("{}.tmp", name)), &p).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn ureq_get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = crate::support::http::agent()
        .get(url)
        .call()
        .map_err(|e| format!("GET {}: {}", url, e))?;
    let mut buf = Vec::new();
    use std::io::Read;
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("read {}: {}", url, e))?;
    Ok(buf)
}

/// not-before of a still-valid cached profile json (>90 days remaining and
/// same identity), else `now` (fresh window).
pub(crate) fn existing_valid_not_before(
    dir: &Path,
    bundle: &str,
    flavor: &str,
    apl: &str,
    kind: ProfileKind,
    now: u64,
) -> u64 {
    let p = dir.join(format!("{}-{}.profile.json", bundle, flavor));
    let ty = match kind {
        ProfileKind::Debug => "debug",
        ProfileKind::Release => "release",
    };
    fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .filter(|v| {
            v["bundle-info"]["bundle-name"].as_str() == Some(bundle)
                && v["bundle-info"]["apl"].as_str() == Some(apl)
                && v["type"].as_str() == Some(ty)
        })
        .and_then(|v| {
            serde_json::from_value::<(u64, u64)>(json!([
                v["validity"]["not-before"],
                v["validity"]["not-after"]
            ]))
            .ok()
        })
        .map(|(nb, na)| if na > now + 90 * 86400 { nb } else { now })
        .unwrap_or(now)
}

/// p7b staleness: regenerate when any input (generated profile json / p12 / cert)
/// is newer than the existing p7b (the sign.js rule); a failed regeneration
/// already cleared the p7b, so the next run regenerates.
pub(crate) fn profile_cache_valid(p7b: &Path, inputs: &[&Path]) -> bool {
    let last = match p7b.metadata() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let p7b_t = match last.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    inputs.iter().all(|i| {
        i.metadata()
            .and_then(|m| m.modified())
            .map(|t| t <= p7b_t)
            .unwrap_or(false)
    }) && !inputs.iter().any(|i| !i.exists())
}

/// Test-chain materials for one bundle: what `sign_app` and `sign_package` both need.
pub(crate) struct TestChain {
    pub env: crate::sdk::manage::SdkEnv,
    pub p12: PathBuf,
    pub app_pem: PathBuf,
    pub p7b: PathBuf,
    pub alias: &'static str,
}

fn flavor_label(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Debug => "debug",
        ProfileKind::Release => "release",
    }
}

/// Produce (or reuse) the test chain for `bundle`.
///
/// Bound to the bundle rather than to the working directory on purpose: the profile pins the
/// identity, so a package from elsewhere signs under its own bundle name.
pub(crate) fn test_chain(bundle: &str, apl: &str, kind: ProfileKind) -> Result<TestChain, String> {
    let env = sdk::detect("").map_err(|e| format!("SDK env: {}", e))?;
    let dir = ensure_materials()?;
    let p12 = dir.join("OpenHarmony.p12");
    let app_pem = dir.join("OpenHarmonyApplication.pem");
    let (profile_pem, flavor_name) = match kind {
        ProfileKind::Debug => (dir.join("OpenHarmonyProfileDebug.pem"), "debug"),
        ProfileKind::Release => (dir.join("OpenHarmonyProfileRelease.pem"), "release"),
    };
    let chain_pem = fs::read_to_string(&app_pem).map_err(|e| format!("read app cert: {}", e))?;
    // The profile names the signing certificate, not the chain it hangs off.
    let cert_pem = leaf_certificate(&chain_pem)?;

    // unsgned profile for THIS bundle (fresh validity, pinned identity)
    let now = now_epoch();
    // anchor validity at the existing profile when it still has >90d life
    let anchor = existing_valid_not_before(&dir, bundle, flavor_name, apl, kind, now);
    let pj = profile_json(bundle, apl, kind, anchor, &cert_pem)?;
    let unsgned = dir.join(format!("{}-{}.profile.json", bundle, flavor_name));
    let rendered = serde_json::to_string_pretty(&pj).map_err(|e| e.to_string())?;
    // write only on content change: unconditional writes refresh mtime and
    // defeat the mtime-based p7b cache below. The rendered text already anchors validity
    // (existing_valid_not_before), so staying equal to it means identity AND window are good.
    let unchanged = fs::read_to_string(&unsgned)
        .map(|old| old == rendered)
        .unwrap_or(false);
    if !unchanged {
        fs::write(&unsgned, &rendered).map_err(|e| format!("write profile json: {}", e))?;
    }

    // p7b (cached; inputs = profile json + p12 + profile cert)
    let p7b = dir.join(format!("{}-{}.p7b", bundle, flavor_name));
    if !profile_cache_valid(&p7b, &[&unsgned, &p12, &profile_pem]) {
        let (_, profile_alias) = aliases(kind);
        let signed = crate::sign::native_sign::sign_profile(
            &crate::sign::native_sign::ProfileSign {
                store_file: &p12,
                store_password: TEST_P12_PWD,
                key_alias: profile_alias,
                key_password: TEST_P12_PWD,
                profile_cert: &profile_pem,
                sign_alg: crate::sign::package::DEFAULT_SIGN_ALG,
            },
            &unsgned,
            &p7b,
        );
        if let Err(e) = signed {
            // A half-written p7b must not pass the cache check next run.
            let _ = fs::remove_file(&p7b);
            return Err(format!("sign-profile failed: {e}"));
        }
        println!("  ✓ profile p7b generated");
    } else {
        println!("  ✓ profile p7b cache valid");
    }

    let (app_alias, _) = aliases(kind);
    Ok(TestChain {
        env,
        p12,
        app_pem,
        p7b,
        alias: app_alias,
    })
}

/// Full localSign flow for the project at `project_dir`.
pub fn sign(project_dir: &Path, apl: &str, kind: ProfileKind) -> Result<(), String> {
    use crate::support::config::AppConfig;

    let app_cfg = AppConfig::from_file(&project_dir.join("hvolt.toml"))?;
    let bundle = app_cfg.app.bundle.clone();

    println!(
        "OpenHarmony localSign ({}, apl={}): {}",
        flavor_label(kind),
        apl,
        bundle
    );

    let chain = test_chain(&bundle, apl, kind)?;
    crate::sign::package::sign_app(
        &chain.env,
        project_dir,
        &chain.p12.to_string_lossy(),
        TEST_P12_PWD,
        chain.alias,
        TEST_P12_PWD,
        &chain.app_pem.to_string_lossy(),
        &chain.p7b.to_string_lossy(),
        "SHA256withECDSA",
    )?;

    // acceptance: verify the FINAL package, not the logs (skill discipline)
    let signed = crate::build::deps::pipeline::find_signed_hap(project_dir)?;
    verify_package(&chain.env, &signed)
}

/// localSign a package the caller points at, bound to that package's own bundle.
///
/// The output sits beside the input (`<name>-signed.hap`); `output/` and the project's own
/// unsigned package are not involved, so a bare package can be signed without a build.
pub fn sign_external(
    hap: &Path,
    apl: &str,
    kind: ProfileKind,
    bundle_override: Option<&str>,
) -> Result<(), String> {
    if !hap.exists() {
        return Err(format!("{} not found", hap.display()));
    }
    // The package's own identity wins; the flag is only for a package that does not carry one.
    let bundle = match bundle_override {
        Some(bundle) => bundle.to_string(),
        None => crate::project::hap::bundle_name(hap)?,
    };
    println!(
        "OpenHarmony localSign ({}, apl={}): {} ← {}",
        flavor_label(kind),
        apl,
        bundle,
        hap.display()
    );

    let chain = test_chain(&bundle, apl, kind)?;
    let materials = crate::sign::package::Materials {
        store_file: &chain.p12,
        store_password: TEST_P12_PWD,
        key_alias: chain.alias,
        key_password: TEST_P12_PWD,
        certpath: &chain.app_pem,
        profile: &chain.p7b,
        sign_alg: crate::sign::package::DEFAULT_SIGN_ALG,
    };
    let out = crate::sign::package::sign_file(&chain.env, hap, &materials)?;

    verify_package(&chain.env, &out)
}

/// Verify the package that was just written, not the logs that announced it.
///
/// Same acceptance as `verify-app` + `verify-profile`: the signing block and the
/// profile's CMS signature must check out, and the profile must still cover now.
fn verify_package(_env: &crate::sdk::manage::SdkEnv, hap: &Path) -> Result<(), String> {
    let verified = crate::sign::native_sign::verify_application(hap)?;
    let now = now_epoch();
    if !profile_is_current(&verified.profile, now) {
        return Err(format!(
            "signed profile does not cover now ({now}): {}",
            verified.profile["validity"]
        ));
    }
    let sha = sdk::sha256_file(hap)?;
    println!(
        "✓ verified final package: {} (bundle {})",
        hap.file_name().unwrap().to_string_lossy(),
        verified.bundle
    );
    println!("  SHA-256 {}", sha);
    Ok(())
}

/// Seconds since the epoch, with a fixed fallback for a clock before 1970.
fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1_760_000_000)
}

/// Whether the profile's validity window covers `now`.
///
/// `verify-profile` reported this as part of `verifiedPassed`; with the check in
/// process it is stated explicitly, so an expired profile cannot pass as signed.
fn profile_is_current(profile: &serde_json::Value, now: u64) -> bool {
    let not_before = profile["validity"]["not-before"].as_u64();
    let not_after = profile["validity"]["not-after"].as_u64();
    match (not_before, not_after) {
        (Some(nb), Some(na)) => nb <= now && now <= na,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apl_pairs_are_explicit_not_max_escalated() {
        assert_eq!(
            apl_pair("normal").unwrap(),
            ("normal".into(), "hos_normal_app".into())
        );
        assert_eq!(
            apl_pair("system_basic").unwrap(),
            ("system_basic".into(), "hos_system_app".into())
        );
        assert_eq!(
            apl_pair("system_core").unwrap(),
            ("system_core".into(), "hos_system_app".into())
        );
        assert!(apl_pair("highest").is_err());
    }

    #[test]
    fn leaf_certificate_takes_the_last_block_of_a_chain() {
        let chain = "-----BEGIN CERTIFICATE-----\nROOT\n-----END CERTIFICATE-----\n\
                     -----BEGIN CERTIFICATE-----\nLEAF\n-----END CERTIFICATE-----";
        let leaf = leaf_certificate(chain).unwrap();
        assert!(leaf.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(leaf.contains("LEAF"));
        assert!(!leaf.contains("ROOT"));
    }

    #[test]
    fn leaf_certificate_rejects_a_chain_without_certificates() {
        assert!(leaf_certificate("not a certificate").is_err());
    }

    #[test]
    fn aliases_follow_flavor() {
        assert_eq!(
            aliases(ProfileKind::Release).0,
            "OpenHarmony Application Release"
        );
        assert_eq!(
            aliases(ProfileKind::Debug).1,
            "OpenHarmony Application Profile Debug"
        );
    }

    #[test]
    fn profile_json_recomputes_validity_and_identity() {
        let now = 1_760_000_000u64;
        let j = profile_json(
            "com.demo.app",
            "system_basic",
            ProfileKind::Release,
            now,
            "PEMDATA",
        )
        .unwrap();
        assert_eq!(j["bundle-info"]["bundle-name"], "com.demo.app");
        assert_eq!(j["bundle-info"]["apl"], "system_basic");
        assert_eq!(j["bundle-info"]["app-feature"], "hos_system_app");
        assert!(j["bundle-info"]["distribution-certificate"]
            .as_str()
            .unwrap()
            .contains("PEMDATA"));
        assert_eq!(j["validity"]["not-before"], now);
        assert_eq!(j["validity"]["not-after"], now + 3 * 365 * 86400); // NOT the template's expired 1689473258
        assert_ne!(j["validity"]["not-after"].as_u64().unwrap(), 1689473258);
        assert_eq!(j["type"], "release");
        let d = profile_json("com.demo.app", "normal", ProfileKind::Debug, now, "X").unwrap();
        assert_eq!(d["type"], "debug");
        assert!(d["bundle-info"]["development-certificate"].is_string());
    }

    #[test]
    fn validity_anchor_reuses_fresh_window_only() {
        let dir = std::env::temp_dir().join(format!("hvolt-anchor-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let now: u64 = 1_760_000_000;
        // absent file → anchor at now
        assert_eq!(
            existing_valid_not_before(
                &dir,
                "com.x",
                "release",
                "normal",
                ProfileKind::Release,
                now
            ),
            now
        );
        // fresh window (>90d left), same identity → keep original nb
        let nb = now - 30 * 86400;
        let j = profile_json("com.x", "normal", ProfileKind::Release, nb, "C").unwrap();
        fs::write(
            dir.join("com.x-release.profile.json"),
            serde_json::to_string(&j).unwrap(),
        )
        .unwrap();
        assert_eq!(
            existing_valid_not_before(
                &dir,
                "com.x",
                "release",
                "normal",
                ProfileKind::Release,
                now
            ),
            nb
        );
        // near-expiry window → re-anchor now
        let soon = now - (3 * 365 - 10) * 86400;
        let j2 = profile_json("com.x", "normal", ProfileKind::Release, soon, "C").unwrap();
        fs::write(
            dir.join("com.x-release.profile.json"),
            serde_json::to_string(&j2).unwrap(),
        )
        .unwrap();
        assert_eq!(
            existing_valid_not_before(
                &dir,
                "com.x",
                "release",
                "normal",
                ProfileKind::Release,
                now
            ),
            now
        );
        // identity mismatch (apl changed) → re-anchor
        assert_eq!(
            existing_valid_not_before(
                &dir,
                "com.x",
                "release",
                "system_basic",
                ProfileKind::Release,
                now
            ),
            now
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn uuid_shape_stable_and_hex() {
        let u1 = uuid_like(123);
        let u2 = uuid_like(123);
        assert_eq!(u1, u2, "deterministic from seed");
        let parts: Vec<&str> = u1.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert!(parts[0].len() == 8 && parts[4].len() == 12);
    }

    #[test]
    fn cache_valid_requires_p7b_newer_than_inputs() {
        let dir = std::env::temp_dir().join(format!("hvolt-pc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a");
        let b = dir.join("b");
        let p = dir.join("p.p7b");
        fs::write(&a, "x").unwrap();
        fs::write(&b, "x").unwrap();
        assert!(!profile_cache_valid(&p, &[&a])); // no p7b → invalid
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(&p, "y").unwrap();
        assert!(profile_cache_valid(&p, &[&a, &b])); // p7b newest → valid
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(&a, "z").unwrap(); // input refreshed after p7b
        assert!(!profile_cache_valid(&p, &[&a, &b])); // stale
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_pin_check_roundtrip() {
        let dir = std::env::temp_dir().join(format!("hvolt-sha-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.bin");
        fs::write(&f, b"abc").unwrap();
        let real = sdk::sha256_file(&f).unwrap();
        assert_eq!(
            real,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(sha256_matches(&f, &real));
        assert!(!sha256_matches(&f, "deadbeef"));
        assert!(!sha256_matches(&dir.join("nope"), &real));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validity_window_is_enforced_at_both_ends() {
        let now = 1_760_000_000u64;
        let window = |nb: u64, na: u64| serde_json::json!({ "validity": { "not-before": nb, "not-after": na } });
        assert!(profile_is_current(&window(now - 10, now + 10), now));
        assert!(profile_is_current(&window(now, now), now));
        assert!(!profile_is_current(&window(now + 1, now + 10), now));
        assert!(!profile_is_current(&window(now - 10, now - 1), now));
    }

    #[test]
    fn a_profile_without_a_validity_window_is_not_current() {
        assert!(!profile_is_current(&serde_json::json!({}), 1_760_000_000));
    }
}
