//! hvolt check — package-side quality gate on top of the SDK's
//! app_check_tool (the official AGC pre-submission scanner).
//! Usage discovered by decompiling CommandParser/ScanVerify on SDK 26
//! (the public docs describe an older `-mode CHECK_HAP` shape that this
//! jar rejects — measured 2026-10-10):
//!   java -jar app_check_tool.jar --input <hap|hsp|app> --out-path <dir>
//!        [--stat-duplicate true] [--stat-suffix true] [--stat-file-size <bytes>]

use crate::hap::{parse_package_stat, PackageStat};
use crate::sdk;
use crate::sign;
use std::fs;
use std::path::Path;
use std::process::Command;

pub use crate::pipeline::find_signed_hap;

/// The package-side gate verdict: only the file-size rule and the duplicate rule can fail it.
///
/// Kept pure so the verdict is testable without running app_check_tool.
pub(crate) fn is_clean(st: &PackageStat) -> bool {
    let size_ok = st.size_limit.is_none() || st.over_limit.is_empty();
    size_ok && st.duplicates.is_empty()
}

/// The app_check_tool argument list (SDK 26 shape: `--input` / `--out-path` + stats flags).
///
/// Extracted from the command runner so the flag contract is pinned by a test — this shape was
/// measured against the shipped jar and must not drift.
pub(crate) fn check_tool_args(
    jar: &Path,
    hap: &Path,
    outdir: &Path,
    size_limit: Option<u64>,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-jar".into(),
        jar.to_string_lossy().into_owned(),
        "--input".into(),
        hap.to_string_lossy().into_owned(),
        "--out-path".into(),
        outdir.to_string_lossy().into_owned(),
        "--stat-duplicate".into(),
        "true".into(),
        "--stat-suffix".into(),
        "true".into(),
    ];
    if let Some(n) = size_limit {
        args.push("--stat-file-size".into());
        args.push(n.to_string());
    }
    args
}

pub fn check_package(project_dir: &Path, size_limit: Option<u64>) -> Result<(), String> {
    let hap_path = find_signed_hap(project_dir)?;
    println!(
        "检查包: {}",
        hap_path.file_name().unwrap().to_string_lossy()
    );
    let env = sdk::detect("")?;
    let jar = sign::find_tool_jar(&env, "app_check_tool.jar")?;
    let java = sign::find_java(&env);
    let outdir = std::env::temp_dir().join(format!("hvolt-check-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outdir);
    fs::create_dir_all(&outdir).map_err(|e| e.to_string())?;

    let args = check_tool_args(&jar, &hap_path, &outdir, size_limit);
    let out = Command::new(&java)
        .args(&args)
        .output()
        .map_err(|e| format!("app_check_tool: {}", e))?;
    let stat_path = outdir.join("stat.json");
    if !stat_path.exists() {
        let so = String::from_utf8_lossy(&out.stdout);
        let se = String::from_utf8_lossy(&out.stderr);
        let _ = fs::remove_dir_all(&outdir); // no leftover output directory on failure
        return Err(format!(
            "app_check_tool produced no stat.json:\n{}{}",
            so.trim(),
            se.trim()
        ));
    }
    let raw = fs::read_to_string(&stat_path).map_err(|e| e.to_string())?;
    let st = parse_package_stat(&raw)?;
    let _ = fs::remove_dir_all(&outdir);

    let clean = is_clean(&st);
    if let Some(lim) = st.size_limit {
        if st.over_limit.is_empty() {
            println!("✓ 无超过 {} B 的大文件", lim);
        } else {
            println!("⚠ 超过 {} B 的文件 {} 个:", lim, st.over_limit.len());
            for (f, sz) in st.over_limit.iter().take(10) {
                println!("    {:>8.1} KB  {}", *sz as f64 / 1024.0, f);
            }
        }
    }
    if st.duplicates.is_empty() {
        println!("✓ 无重复文件");
    } else {
        println!("⚠ 重复文件组 {} 个:", st.duplicates.len());
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
        println!("按后缀体积分布 (top 8):");
        for (sfx, total, cnt) in st.suffixes.iter().take(8) {
            println!(
                "    {:>9.1} KB  {:>4} 个  {}",
                *total as f64 / 1024.0,
                cnt,
                sfx
            );
        }
    }
    // html twin for humans, when someone wants the official report
    println!(
        "\n{}",
        if clean {
            "✓ 包侧检查通过"
        } else {
            "结果如上——明细见 app_check_tool 官方 HTML 报告形态"
        }
    );
    Ok(())
}

#[cfg(test)]
mod gate_tests {
    use super::{check_tool_args, is_clean};
    use crate::hap::PackageStat;
    use std::path::Path;

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

    /// The flag contract measured against the shipped SDK 26 jar must not drift.
    #[test]
    fn args_carry_the_measured_flag_shape() {
        let args = check_tool_args(
            Path::new("/sdk/app_check_tool.jar"),
            Path::new("/p/output/entry-signed.hap"),
            Path::new("/tmp/out"),
            None,
        );
        let got: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec![
                "-jar",
                "/sdk/app_check_tool.jar",
                "--input",
                "/p/output/entry-signed.hap",
                "--out-path",
                "/tmp/out",
                "--stat-duplicate",
                "true",
                "--stat-suffix",
                "true",
            ]
        );
    }

    /// The size threshold is appended only when the caller asked for one.
    #[test]
    fn size_limit_is_optional_and_appended_last() {
        let base = check_tool_args(Path::new("j"), Path::new("h"), Path::new("o"), None);
        assert!(!base.iter().any(|a| a == "--stat-file-size"));

        let with = check_tool_args(Path::new("j"), Path::new("h"), Path::new("o"), Some(4096));
        assert_eq!(with.len(), base.len() + 2);
        assert_eq!(&with[with.len() - 2], "--stat-file-size");
        assert_eq!(&with[with.len() - 1], "4096");
    }
}
