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

use crate::sdk;
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
/// (keytool -list measured; an invented "Application Debug" alias fails with
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
    let resp = crate::http::agent()
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

/// Full localSign flow for the project at `project_dir`.
pub fn sign(project_dir: &Path, apl: &str, kind: ProfileKind) -> Result<(), String> {
    use crate::config::AppConfig;
    use std::process::Command;

    let app_cfg = AppConfig::from_file(&project_dir.join("hvolt.toml"))?;
    let bundle = app_cfg.app.bundle.clone();
    let env = sdk::detect("").map_err(|e| format!("SDK env: {}", e))?;

    println!(
        "OpenHarmony localSign ({}, apl={}): {}",
        match kind {
            ProfileKind::Debug => "debug",
            ProfileKind::Release => "release",
        },
        apl,
        bundle
    );
    let dir = ensure_materials()?;
    let p12 = dir.join("OpenHarmony.p12");
    let app_pem = dir.join("OpenHarmonyApplication.pem");
    let (profile_pem, flavor_name) = match kind {
        ProfileKind::Debug => (dir.join("OpenHarmonyProfileDebug.pem"), "debug"),
        ProfileKind::Release => (dir.join("OpenHarmonyProfileRelease.pem"), "release"),
    };
    let cert_pem = fs::read_to_string(&app_pem).map_err(|e| format!("read app cert: {}", e))?;

    // unsgned profile for THIS project (fresh validity, pinned bundle)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1_760_000_000);
    // anchor validity at the existing profile when it still has >90d life
    let anchor = existing_valid_not_before(&dir, &bundle, flavor_name, apl, kind, now);
    let pj = profile_json(&bundle, apl, kind, anchor, &cert_pem)?;
    let unsgned = dir.join(format!("{}-{}.profile.json", bundle, flavor_name));
    let rendered = serde_json::to_string_pretty(&pj).map_err(|e| e.to_string())?;
    // write only on content change: unconditional writes refresh mtime and
    // defeat the mtime-based p7b cache below
    let same = fs::read_to_string(&unsgned)
        .map(|old| old == rendered)
        .unwrap_or(false);
    if !same {
        fs::write(&unsgned, &rendered).map_err(|e| format!("write profile json: {}", e))?;
    }

    // p7b (cached; inputs = profile json + p12 + profile cert)
    let p7b = dir.join(format!("{}-{}.p7b", bundle, flavor_name));
    // rendered already anchors validity (existing_valid_not_before): equality
    // against the cached file means identity AND window are still good.
    let reuse = fs::read_to_string(&unsgned)
        .map(|old| old == rendered)
        .unwrap_or(false);
    if !reuse {
        fs::write(&unsgned, &rendered).map_err(|e| format!("write profile json: {}", e))?;
    }
    if !profile_cache_valid(&p7b, &[&unsgned, &p12, &profile_pem]) {
        let (app_alias, profile_alias) = aliases(kind);
        let _ = app_alias;
        let tool = crate::sign::find_hap_sign_tool(&env)?;
        let java = crate::sign::find_java(&env);
        let out = Command::new(&java)
            .args([
                "-jar",
                &tool.to_string_lossy(),
                "sign-profile",
                "-mode",
                "localSign",
                "-keyAlias",
                profile_alias,
                "-keyPwd",
                TEST_P12_PWD,
                "-keystoreFile",
                &p12.to_string_lossy(),
                "-keystorePwd",
                TEST_P12_PWD,
                "-signAlg",
                "SHA256withECDSA",
                "-profileCertFile",
                &profile_pem.to_string_lossy(),
                "-inFile",
                &unsgned.to_string_lossy(),
                "-outFile",
                &p7b.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("sign-profile: {}", e))?;
        let so = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() || so.contains("[Fail]") {
            let _ = fs::remove_file(&p7b);
            return Err(format!(
                "sign-profile failed:\n{}{}",
                so.trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        println!("  ✓ profile p7b generated");
    } else {
        println!("  ✓ profile p7b cache valid");
    }

    // sign the unsigned hap with the test chain
    let (app_alias, _) = aliases(kind);
    crate::sign::sign_app(
        &env,
        project_dir,
        &p12.to_string_lossy(),
        TEST_P12_PWD,
        app_alias,
        TEST_P12_PWD,
        &app_pem.to_string_lossy(),
        &p7b.to_string_lossy(),
        "SHA256withECDSA",
    )?;

    // acceptance: verify the FINAL package, not the logs (skill discipline)
    verify_signed(&env, project_dir)?;
    Ok(())
}

fn verify_signed(env: &crate::sdk::SdkEnv, project_dir: &Path) -> Result<(), String> {
    use std::process::Command;
    let signed = project_dir.join("output");
    let hap = fs::read_dir(&signed)
        .map_err(|e| format!("output/: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with("-signed.hap"))
        .ok_or("no signed hap in output/".to_string())?;
    let tool = crate::sign::find_hap_sign_tool(env)?;
    let java = crate::sign::find_java(env);
    let tmp = std::env::temp_dir().join(format!("hvolt-verify-{}", std::process::id()));
    fs::create_dir_all(&tmp).ok();
    let out = Command::new(&java)
        .args([
            "-jar",
            &tool.to_string_lossy(),
            "verify-app",
            "-inFile",
            &hap.to_string_lossy(),
            "-outCertChain",
            &tmp.join("chain.cer").to_string_lossy(),
            "-outProfile",
            &tmp.join("profile.p7b").to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("verify-app: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "verify-app failed: {}",
            String::from_utf8_lossy(&out.stdout).trim()
        ));
    }
    let out = Command::new(&java)
        .args([
            "-jar",
            &tool.to_string_lossy(),
            "verify-profile",
            "-inFile",
            &tmp.join("profile.p7b").to_string_lossy(),
            "-outFile",
            &tmp.join("result.json").to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("verify-profile: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "verify-profile failed: {}",
            String::from_utf8_lossy(&out.stdout).trim()
        ));
    }
    let r: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.join("result.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("parse verify result: {}", e))?;
    if r["verifiedPassed"] != serde_json::Value::Bool(true) {
        return Err(format!("signed HAP failed its own verification: {}", r));
    }
    let sha = sdk::sha256_file(&hap)?;
    let bundle_in = r["content"]["bundle-info"]["bundle-name"]
        .as_str()
        .unwrap_or("?");
    println!(
        "✓ verified final package: {} (bundle {})",
        hap.file_name().unwrap().to_string_lossy(),
        bundle_in
    );
    println!("  SHA-256 {}", sha);
    let _ = fs::remove_dir_all(&tmp);
    Ok(())
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
}
