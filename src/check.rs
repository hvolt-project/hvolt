//! hvolt check — package-side quality gate on top of the SDK's
//! app_check_tool (the official AGC pre-submission scanner).
//! Usage discovered by decompiling CommandParser/ScanVerify on SDK 26
//! (the public docs describe an older `-mode CHECK_HAP` shape that this
//! jar rejects — measured 2026-10-10):
//!   java -jar app_check_tool.jar --input <hap|hsp|app> --out-path <dir>
//!        [--stat-duplicate true] [--stat-suffix true] [--stat-file-size <bytes>]

use crate::hap::parse_package_stat;
use crate::sdk;
use crate::sign;
use std::fs;
use std::path::Path;
use std::process::Command;

pub use crate::pipeline::find_signed_hap;

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

    let mut args: Vec<String> = vec![
        "-jar".into(),
        jar.to_string_lossy().into_owned(),
        "--input".into(),
        hap_path.to_string_lossy().into_owned(),
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

    let mut clean = true;
    if let Some(lim) = st.size_limit {
        if st.over_limit.is_empty() {
            println!("✓ 无超过 {} B 的大文件", lim);
        } else {
            clean = false;
            println!("⚠ 超过 {} B 的文件 {} 个:", lim, st.over_limit.len());
            for (f, sz) in st.over_limit.iter().take(10) {
                println!("    {:>8.1} KB  {}", *sz as f64 / 1024.0, f);
            }
        }
    }
    if st.duplicates.is_empty() {
        println!("✓ 无重复文件");
    } else {
        clean = false;
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
