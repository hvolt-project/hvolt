//! Fixture conformance against the official loader.
//!
//! Each fixture is a tiny `.ets` file whose golden output is produced by hvigor itself
//! (`scripts/oracle/run-hvigor.sh`), compared with hvolt's own emission at AST level
//! (`scripts/oracle/compare.js`). The run is `#[ignore]`d because it drives a real hvigor
//! build per fixture — minutes, not milliseconds — so it is a deliberate check rather than a
//! step in every `cargo test`.

#![cfg(test)]

use std::process::Command;

#[test]
#[ignore = "drives hvigor once per fixture; run with: cargo test -- --ignored conformance"]
fn fixtures_match_the_official_loader() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("bash")
        .arg("scripts/oracle/all.sh")
        .current_dir(root)
        .output()
        .expect("run scripts/oracle/all.sh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let passed = stdout.lines().filter(|l| l.starts_with("PASS")).count();
    let failed = stdout.lines().filter(|l| l.starts_with("FAIL")).count();
    println!("fixtures: {passed} passed, {failed} failed");
    assert!(
        output.status.success(),
        "fixture conformance failed\n{stdout}\n{stderr}"
    );
    assert!(passed > 0, "no fixture ran\n{stdout}");
}
