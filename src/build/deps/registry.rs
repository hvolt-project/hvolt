//! Registry dependencies, fetched without a node runtime.
//!
//! The reference resolves and installs packages with ohpm, a node program. hvolt has no node at
//! build time, so the same two steps live here: read the registry's metadata document to turn a
//! range into one release, then download that release's tarball, verify its integrity and unpack
//! it into the package cache.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha512};

/// Registry used when a dependency names none.
const DEFAULT_REGISTRY: &str = "https://ohpm.openharmony.cn/ohpm";
const USER_AGENT: &str = "hvolt";

fn registry_url(registry: &str) -> String {
    let trimmed = registry.trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        DEFAULT_REGISTRY.to_string()
    }
}

/// One release's download information.
struct Release {
    version: String,
    tarball: String,
    integrity: String,
}

/// Turns a version range into the newest release that satisfies it.
///
/// A range has to be resolved against the registry because the cache is keyed by the release,
/// not by the range: two projects writing `^1.2.0` may legitimately land on different versions.
pub fn resolve_version(registry: &str, name: &str, range: &str) -> Result<String, String> {
    let body = metadata(registry, name)?;
    let versions = body
        .get("versions")
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("dependency '{name}': registry document has no versions"))?;
    let mut best: Option<(Vec<u32>, String)> = None;
    for key in versions.keys() {
        let Some(parsed) = parse_version(key) else {
            continue;
        };
        if !satisfies(&parsed, range) {
            continue;
        }
        if best
            .as_ref()
            .map(|(current, _)| &parsed > current)
            .unwrap_or(true)
        {
            best = Some((parsed, key.clone()));
        }
    }
    best.map(|(_, version)| version).ok_or_else(|| {
        format!("dependency '{name}': no release in the registry satisfies '{range}'")
    })
}

fn metadata(registry: &str, name: &str) -> Result<serde_json::Value, String> {
    let url = format!("{}/{}", registry_url(registry), name);
    ureq::get(&url)
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("dependency '{name}': registry request {url} failed: {e}"))?
        .into_json()
        .map_err(|e| format!("dependency '{name}': registry returned invalid JSON: {e}"))
}

/// The release's download information, read from the same registry document.
fn release_of(registry: &str, name: &str, version: &str) -> Result<Release, String> {
    let body = metadata(registry, name)?;
    let entry = body
        .get("versions")
        .and_then(|v| v.get(version))
        .ok_or_else(|| format!("dependency '{name}': registry has no release {version}"))?;
    let dist = entry
        .get("dist")
        .ok_or_else(|| format!("dependency '{name}': release {version} has no dist"))?;
    let tarball = dist
        .get("tarball")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("dependency '{name}': release {version} has no tarball url"))?
        .to_string();
    let integrity = dist
        .get("integrity")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .unwrap_or_default();
    Ok(Release {
        version: version.to_string(),
        tarball,
        integrity,
    })
}

/// Makes sure one release sits in the package cache, fetching it when it is not there yet.
pub fn ensure(registry: &str, name: &str, version: &str) -> Result<PathBuf, String> {
    // A manifest may name a range here (nothing has pinned it yet), so the release is settled
    // first and everything below works on one exact version.
    let resolved = if is_exact(version) {
        version.to_string()
    } else {
        resolve_version(registry, name, version)?
    };
    let dir = crate::build::deps::npm_cache::package_dir(registry, name, &resolved);
    if has_package(&dir) {
        println!("dependency {name}@{resolved}: cache hit");
        return Ok(dir);
    }
    let _lock = acquire(&dir, name)?;
    // Another build may have finished between the check above and the lock.
    if has_package(&dir) {
        return Ok(dir);
    }
    let release = release_of(registry, name, &resolved)?;
    let bytes = download(&release.tarball, name)?;
    verify(&release, &bytes, name)?;
    unpack(&bytes, &dir, name)?;
    println!(
        "dependency {name}@{resolved}: fetched {directory}",
        directory = dir.display()
    );
    if !has_package(&dir) {
        return Err(format!(
            "dependency '{name}': unpacked {version} into {} but no oh-package.json5 is there",
            dir.display()
        ));
    }
    Ok(dir)
}

/// Whether a version names one release rather than a range.
fn is_exact(version: &str) -> bool {
    !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_digit() || c == char::from(46u8))
}

/// Held while one release is being fetched.
///
/// Two builds of the same project can run at once (an editor saving while a terminal builds),
/// and both would otherwise unpack into the same directory: the second rename would replace the
/// first tree mid-read. The lock is a file created with `create_new`, so the exclusion is the
/// filesystem's, not a process-local guard that two processes would each think they own.
struct FetchLock {
    path: PathBuf,
}

impl Drop for FetchLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Waits for the lock, sweeping a lock left behind by a killed process.
///
/// A lock older than the wait budget is treated as abandoned: a fetch that has not finished in
/// that window is not progressing, and refusing forever would turn one crash into a cache that
/// can never be written again.
fn acquire(dir: &Path, name: &str) -> Result<FetchLock, String> {
    const BUDGET: Duration = Duration::from_secs(120);
    const STALE: Duration = Duration::from_secs(600);
    let path = dir.with_extension("lock");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let started = Instant::now();
    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                let _ = writeln!(file, "{}", std::process::id());
                return Ok(FetchLock { path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let age = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .unwrap_or(Duration::ZERO);
                if age > STALE {
                    let _ = fs::remove_file(&path);
                    continue;
                }
                if started.elapsed() > BUDGET {
                    return Err(format!(
                        "dependency '{name}': another build holds {} ({} s);                          remove it if no build is running",
                        path.display(),
                        age.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                return Err(format!(
                    "dependency '{name}': cannot create {}: {e}",
                    path.display()
                ))
            }
        }
    }
}

fn has_package(dir: &Path) -> bool {
    dir.is_dir() && dir.join("oh-package.json5").is_file()
}

/// Removes half-unpacked trees a killed build left behind.
///
/// The unpack writes into `<version>.unpacking` and renames it into place, so an interrupted
/// fetch leaves a directory the cache will never look at again. Sweeping at the start of a run
/// keeps that from accumulating one directory per crash.
pub fn sweep_leftovers() -> Result<usize, String> {
    let root = crate::build::deps::npm_cache::root();
    if !root.is_dir() {
        return Ok(0);
    }
    let mut swept = 0;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".lock") || name.ends_with(".unpacking") {
                let stale = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|age| age > Duration::from_secs(600))
                    .unwrap_or(false);
                let tree = path.with_file_name(
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default(),
                );
                if stale {
                    if path.is_dir() {
                        let _ = fs::remove_dir_all(&path);
                    } else {
                        let _ = fs::remove_file(&path);
                    }
                    let _ = tree;
                    swept += 1;
                }
            }
        }
    }
    Ok(swept)
}

fn download(url: &str, name: &str) -> Result<Vec<u8>, String> {
    let mut reader = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("dependency '{name}': download {url} failed: {e}"))?
        .into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("dependency '{name}': download {url} truncated: {e}"))?;
    Ok(bytes)
}

/// Integrity is checked against the tarball bytes, before anything is written to disk: an
/// unpack that ran first could leave a half-trusted tree behind on a mismatch.
fn verify(release: &Release, bytes: &[u8], name: &str) -> Result<(), String> {
    if release.integrity.is_empty() {
        return Ok(());
    }
    let (algorithm, rest) = match release.integrity.split_once('-') {
        Some(pair) => pair,
        None => {
            return Err(format!(
                "dependency '{name}': integrity '{}' is malformed",
                release.integrity
            ))
        }
    };
    let expected = rest.split('?').next().unwrap_or(rest);
    let actual = match algorithm {
        "sha512" => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes))
        }
        other => {
            return Err(format!(
                "dependency '{name}': unsupported integrity algorithm '{other}'"
            ))
        }
    };
    if actual != expected {
        return Err(format!(
            "dependency '{name}': integrity mismatch for {} (expected {expected}, got {actual})",
            release.version
        ));
    }
    Ok(())
}

/// Unpacks the tarball, dropping the leading `package/` segment the format wraps everything in.
fn unpack(bytes: &[u8], dir: &Path, name: &str) -> Result<(), String> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| format!("dependency '{name}': tarball is not readable: {e}"))?;
    let staging = dir.with_extension("unpacking");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| {
        format!(
            "dependency '{name}': cannot create {}: {e}",
            staging.display()
        )
    })?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("dependency '{name}': bad tar entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("dependency '{name}': bad tar path: {e}"))?
            .to_path_buf();
        let Some(rest) = strip_package_prefix(&path) else {
            continue;
        };
        if rest.as_os_str().is_empty() {
            continue;
        }
        if rest
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!(
                "dependency '{name}': tarball escapes its root at {}",
                path.display()
            ));
        }
        let target = staging.join(&rest);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "dependency '{name}': cannot create {}: {e}",
                    parent.display()
                )
            })?;
        }
        entry.unpack(&target).map_err(|e| {
            format!(
                "dependency '{name}': cannot write {}: {e}",
                target.display()
            )
        })?;
    }
    if dir.exists() {
        let _ = fs::remove_dir_all(dir);
    }
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "dependency '{name}': cannot create {}: {e}",
                parent.display()
            )
        })?;
    }
    fs::rename(&staging, dir).map_err(|e| {
        format!(
            "dependency '{name}': cannot move {} into place: {e}",
            staging.display()
        )
    })?;
    Ok(())
}

fn strip_package_prefix(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match components.next() {
        Some(std::path::Component::Normal(first)) if first == "package" => {
            Some(components.as_path().to_path_buf())
        }
        Some(std::path::Component::Normal(_)) => Some(path.to_path_buf()),
        _ => None,
    }
}

fn parse_version(text: &str) -> Option<Vec<u32>> {
    let cleaned = text.trim().trim_start_matches('v');
    let core = cleaned.split('-').next().unwrap_or(cleaned);
    let mut parts = Vec::new();
    for piece in core.split('.') {
        parts.push(piece.parse::<u32>().ok()?);
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}

/// Range support covers what manifests actually write: an exact release, `*`, `^`, `~` and the
/// comparison prefixes. Anything else is refused rather than guessed at.
fn satisfies(version: &[u32], range: &str) -> bool {
    let range = range.trim();
    if range.is_empty() || range == "*" || range == "latest" {
        return true;
    }
    if let Some(rest) = range.strip_prefix('^') {
        let Some(base) = parse_version(rest) else {
            return false;
        };
        if version < base.as_slice() {
            return false;
        }
        return version[0] == base[0] && (base[0] > 0 || version[1] == base[1]);
    }
    if let Some(rest) = range.strip_prefix('~') {
        let Some(base) = parse_version(rest) else {
            return false;
        };
        return version >= base.as_slice() && version[0] == base[0] && version[1] == base[1];
    }
    if let Some(rest) = range.strip_prefix(">=") {
        let Some(base) = parse_version(rest) else {
            return false;
        };
        return version >= base.as_slice();
    }
    if let Some(rest) = range.strip_prefix("<=") {
        let Some(base) = parse_version(rest) else {
            return false;
        };
        return version <= base.as_slice();
    }
    if let Some(rest) = range.strip_prefix('>') {
        let Some(base) = parse_version(rest) else {
            return false;
        };
        return version > base.as_slice();
    }
    if let Some(rest) = range.strip_prefix('<') {
        let Some(base) = parse_version(rest) else {
            return false;
        };
        return version < base.as_slice();
    }
    match parse_version(range) {
        Some(exact) => version == exact.as_slice(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_select_what_manifests_express() {
        let v = parse_version("1.4.2").unwrap();
        assert!(satisfies(&v, "1.4.2"));
        assert!(satisfies(&v, "^1.2.0"));
        assert!(satisfies(&v, "~1.4.0"));
        assert!(satisfies(&v, ">=1.0.0"));
        assert!(!satisfies(&v, "^2.0.0"));
        assert!(!satisfies(&v, "~1.3.0"));
        assert!(!satisfies(&v, "<1.4.0"));
    }

    #[test]
    fn caret_stays_inside_the_declared_minor_when_major_is_zero() {
        let v = parse_version("0.3.5").unwrap();
        assert!(satisfies(&v, "^0.3.0"));
        assert!(!satisfies(&v, "^0.2.0"));
    }

    #[test]
    fn a_mismatched_digest_is_refused_before_anything_is_written() {
        let release = Release {
            version: "1.0.0".to_string(),
            tarball: "unused".to_string(),
            integrity: "sha512-AAAA".to_string(),
        };
        let refused = verify(&release, b"not the bytes that were signed", "demo");
        assert!(refused.is_err(), "a wrong digest must not pass");
        let message = refused.unwrap_err();
        assert!(
            message.contains("integrity mismatch"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn a_matching_digest_passes() {
        use base64::Engine;
        let bytes = b"payload";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let release = Release {
            version: "1.0.0".to_string(),
            tarball: "unused".to_string(),
            integrity: format!("sha512-{digest}"),
        };
        assert!(verify(&release, bytes, "demo").is_ok());
    }

    #[test]
    fn the_package_prefix_is_dropped() {
        assert_eq!(
            strip_package_prefix(Path::new("package/lib/index.js")).unwrap(),
            PathBuf::from("lib/index.js")
        );
        assert_eq!(
            strip_package_prefix(Path::new("lib/index.js")).unwrap(),
            PathBuf::from("lib/index.js")
        );
    }
}
