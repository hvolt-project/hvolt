//! hvolt check — package-side quality gate, computed from the archive itself.
//!
//! The SDK's app_check_tool reports three statistics (duplicate files, files over a
//! size threshold, per-suffix totals). Those are properties of the package archive,
//! so they are computed here directly: no java, no jar, no stat.json round trip.

use crate::project::hap::PackageStat;
use std::fs;
use std::path::Path;

pub use crate::build::deps::pipeline::find_signed_hap;

/// The package-side gate verdict: only the file-size rule and the duplicate rule can fail it.
pub(crate) fn is_clean(st: &PackageStat) -> bool {
    let size_ok = st.size_limit.is_none() || st.over_limit.is_empty();
    size_ok && st.duplicates.is_empty()
}

/// Entry name in the same shape app_check_tool reported: a leading `/`.
fn entry_name(raw: &str) -> String {
    format!("/{}", raw.trim_start_matches('/'))
}

/// The suffix a path is counted under: its last extension, or "(none)".
fn suffix_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() && !ext.contains('/') => ext.to_string(),
        _ => "(none)".to_string(),
    }
}

/// Files over the threshold, duplicate groups, and per-suffix totals for `hap`.
///
/// Duplicates are decided by content digest, computed only for entries that share a
/// size: two files of different length cannot be the same file, so a package with no
/// repeated sizes costs no hashing at all.
pub(crate) fn package_stat(hap: &Path, size_limit: Option<u64>) -> Result<PackageStat, String> {
    let file = fs::File::open(hap).map_err(|e| format!("open {}: {}", hap.display(), e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("read archive {}: {}", hap.display(), e))?;

    let mut entries: Vec<(String, u64)> = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("read entry {index}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        entries.push((entry_name(entry.name()), entry.size()));
    }

    let mut over_limit: Vec<(String, u64)> = match size_limit {
        Some(limit) => entries
            .iter()
            .filter(|(_, size)| *size > limit)
            .map(|(name, size)| (name.clone(), *size))
            .collect(),
        None => Vec::new(),
    };
    over_limit.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let duplicates = duplicate_groups(&mut archive, &entries)?;

    let mut suffix_totals: std::collections::HashMap<String, (u64, usize)> =
        std::collections::HashMap::new();
    for (name, size) in &entries {
        let slot = suffix_totals
            .entry(suffix_of(name))
            .or_insert((0_u64, 0_usize));
        slot.0 += size;
        slot.1 += 1;
    }
    let mut suffixes: Vec<(String, u64, usize)> = suffix_totals
        .into_iter()
        .map(|(suffix, (total, count))| (suffix, total, count))
        .collect();
    suffixes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    Ok(PackageStat {
        size_limit,
        over_limit,
        duplicates,
        suffixes,
    })
}

/// Group entries whose contents are byte-identical, reporting each group once.
fn duplicate_groups(
    archive: &mut zip::ZipArchive<fs::File>,
    entries: &[(String, u64)],
) -> Result<Vec<(String, Vec<String>, u64)>, String> {
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::io::Read;

    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, (_, size)) in entries.iter().enumerate() {
        by_size.entry(*size).or_default().push(index);
    }

    let mut by_digest: HashMap<(u64, String), Vec<usize>> = HashMap::new();
    for (size, indices) in by_size {
        // A size that occurs once cannot be a duplicate; skip reading it.
        if indices.len() < 2 {
            continue;
        }
        for index in indices {
            let (name, _) = &entries[index];
            let mut reader = archive
                .by_index(index)
                .map_err(|e| format!("read {name}: {e}"))?;
            let mut hasher = Sha256::new();
            let mut buf = [0_u8; 64 * 1024];
            loop {
                let read = reader
                    .read(&mut buf)
                    .map_err(|e| format!("read {name}: {e}"))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buf[..read]);
            }
            let digest = format!("{:x}", hasher.finalize());
            by_digest.entry((size, digest)).or_default().push(index);
        }
    }

    let mut groups: Vec<(String, Vec<String>, u64)> = by_digest
        .into_iter()
        .filter(|(_, indices)| indices.len() > 1)
        .map(|((size, _), mut indices)| {
            indices.sort_unstable();
            let names: Vec<String> = indices.iter().map(|i| entries[*i].0.clone()).collect();
            let mut rest = names[1..].to_vec();
            rest.sort();
            (names[0].clone(), rest, size)
        })
        .collect();
    groups.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    Ok(groups)
}

pub fn check_package(
    project_dir: &Path,
    package: Option<&Path>,
    size_limit: Option<u64>,
) -> Result<(), String> {
    let hap_path = match package {
        Some(path) => path.to_path_buf(),
        None => find_signed_hap(project_dir)?,
    };
    println!(
        "Package: {}",
        hap_path.file_name().unwrap().to_string_lossy()
    );

    let st = package_stat(&hap_path, size_limit)?;
    let clean = is_clean(&st);

    if let Some(lim) = st.size_limit {
        if st.over_limit.is_empty() {
            println!("✓ no file over {} B", lim);
        } else {
            println!("⚠ {} file(s) over {} B:", st.over_limit.len(), lim);
            for (f, sz) in st.over_limit.iter().take(10) {
                println!("    {:>8.1} KB  {}", *sz as f64 / 1024.0, f);
            }
        }
    }
    if st.duplicates.is_empty() {
        println!("✓ no duplicate files");
    } else {
        println!("⚠ {} duplicate group(s):", st.duplicates.len());
        for (first, rest, sz) in st.duplicates.iter().take(10) {
            println!(
                "    {:>8.1} KB × {}  ({})",
                *sz as f64 / 1024.0,
                rest.len() + 1,
                first
            );
        }
    }
    if !st.suffixes.is_empty() {
        println!("Size by suffix (top 8):");
        for (sfx, total, cnt) in st.suffixes.iter().take(8) {
            println!(
                "    {:>9.1} KB  {:>4}  {}",
                *total as f64 / 1024.0,
                cnt,
                sfx
            );
        }
    }
    if !clean {
        // A gate that exits 0 on failure is not a gate: `hvolt check && hvolt push` would
        // always pass in CI.
        return Err("package-side check failed (see the detail above)".to_string());
    }
    println!("\n✓ package checks passed");
    Ok(())
}

#[cfg(test)]
mod gate_tests {
    use super::{is_clean, package_stat, suffix_of};
    use crate::project::hap::PackageStat;
    use std::io::Write;

    fn stat(limit: Option<u64>, over: Vec<(&str, u64)>, dups: Vec<(&str, u64)>) -> PackageStat {
        PackageStat {
            size_limit: limit,
            over_limit: over.into_iter().map(|(p, s)| (p.to_string(), s)).collect(),
            duplicates: dups
                .into_iter()
                .map(|(p, s)| (p.to_string(), vec![], s))
                .collect(),
            suffixes: vec![],
        }
    }

    /// Nothing violated: clean, with or without a size threshold.
    #[test]
    fn clean_when_nothing_is_violated() {
        assert!(is_clean(&stat(None, vec![], vec![])));
        assert!(is_clean(&stat(Some(1024), vec![], vec![])));
    }

    /// A file over the threshold fails the gate even with no duplicates.
    #[test]
    fn oversized_file_fails_the_gate() {
        assert!(!is_clean(&stat(Some(1024), vec![("a.bin", 2048)], vec![])));
    }

    /// Duplicates fail the gate even when the size rule passes.
    #[test]
    fn duplicates_fail_the_gate() {
        assert!(!is_clean(&stat(Some(1024), vec![], vec![("a.bin", 512)])));
    }

    #[test]
    fn suffix_is_the_last_extension() {
        assert_eq!(suffix_of("/resources/base/media/icon.png"), "png");
        assert_eq!(suffix_of("/ets/modules.abc"), "abc");
        assert_eq!(suffix_of("/noextension"), "(none)");
        assert_eq!(suffix_of("/a.b/c"), "(none)"); // the dot is in a parent segment
    }

    /// Build a zip with two identical entries, one oversized entry, and two suffixes.
    fn sample_zip(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("hvolt-stat-{}-{}", std::process::id(), name));
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("same-a.bin", opts).unwrap();
        zip.write_all(&vec![7_u8; 4096]).unwrap();
        zip.start_file("same-b.bin", opts).unwrap();
        zip.write_all(&vec![7_u8; 4096]).unwrap();
        zip.start_file("big.so", opts).unwrap();
        zip.write_all(&vec![0_u8; 8192]).unwrap();
        zip.start_file("small.txt", opts).unwrap();
        zip.write_all(b"hi").unwrap();
        zip.finish().unwrap();
        path
    }

    #[test]
    fn statistics_come_from_the_archive_contents() {
        let path = sample_zip("stats");
        let st = package_stat(&path, Some(4096)).unwrap();

        assert_eq!(st.over_limit, vec![("/big.so".to_string(), 8192)]);

        assert_eq!(st.duplicates.len(), 1, "{:?}", st.duplicates);
        let (first, rest, size) = &st.duplicates[0];
        assert_eq!(*size, 4096);
        assert!(first.ends_with("same-a.bin"), "{first}");
        assert_eq!(rest.len(), 1);
        assert!(rest[0].ends_with("same-b.bin"), "{:?}", rest);

        let bin = st.suffixes.iter().find(|(s, _, _)| s == "bin").unwrap();
        assert_eq!((bin.1, bin.2), (8192, 2));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn no_threshold_reports_no_oversized_files() {
        let path = sample_zip("nothreshold");
        let st = package_stat(&path, None).unwrap();
        assert_eq!(st.size_limit, None);
        assert!(st.over_limit.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
