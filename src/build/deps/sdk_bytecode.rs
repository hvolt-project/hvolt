// sdk_bytecode: the ark bytecode version each API level compiles to.
//
// The SDK answers this with `ts2abc.js --target-api-version <api>`, which only forwards the
// level to es2abc as `--target-bc-version`. Reading that answer by running Node makes a
// process spawn the price of a constant, so the answers are measured once and kept here.
//
// Every row was measured on 2026-11-08 against SDK 26.0.0.23 (the version `~/.hvolt/sdk/
// default` points at), running
//
//     node ~/.hvolt/sdk/default/openharmony/ets/build-tools/ets-loader/bin/ark/ts2abc.js \
//         --target-api-version <api>
//
// The mapping is SDK-versioned: a newer SDK can raise a level's bytecode version.
// `the_table_matches_the_installed_sdk` re-measures every row and fails on a mismatch, so
// the table cannot drift away from the SDK hvolt is installed against.

/// API level → bytecode version, in the exact form es2abc prints it.
///
/// The measured range is API 12..=30. Out-of-range levels are deliberately absent: es2abc
/// silently clamps them to its own newest version, which is not a mapping worth caching.
pub(crate) const TABLE: &[(u32, &str)] = &[
    (12, "12.0.2.0"), // ts2abc.js --target-api-version 12 → 12.0.2.0 (SDK 26.0.0.23)
    (13, "12.0.6.0"), // ts2abc.js --target-api-version 13 → 12.0.6.0 (SDK 26.0.0.23)
    (14, "12.0.6.0"), // ts2abc.js --target-api-version 14 → 12.0.6.0 (SDK 26.0.0.23)
    (15, "12.0.6.0"), // ts2abc.js --target-api-version 15 → 12.0.6.0 (SDK 26.0.0.23)
    (16, "12.0.6.0"), // ts2abc.js --target-api-version 16 → 12.0.6.0 (SDK 26.0.0.23)
    (17, "12.0.6.0"), // ts2abc.js --target-api-version 17 → 12.0.6.0 (SDK 26.0.0.23)
    (18, "13.0.1.0"), // ts2abc.js --target-api-version 18 → 13.0.1.0 (SDK 26.0.0.23)
    (19, "13.0.1.0"), // ts2abc.js --target-api-version 19 → 13.0.1.0 (SDK 26.0.0.23)
    (20, "13.0.1.0"), // ts2abc.js --target-api-version 20 → 13.0.1.0 (SDK 26.0.0.23)
    (21, "13.0.1.0"), // ts2abc.js --target-api-version 21 → 13.0.1.0 (SDK 26.0.0.23)
    (22, "13.0.1.0"), // ts2abc.js --target-api-version 22 → 13.0.1.0 (SDK 26.0.0.23)
    (23, "13.0.1.0"), // ts2abc.js --target-api-version 23 → 13.0.1.0 (SDK 26.0.0.23)
    (24, "24.0.0.0"), // ts2abc.js --target-api-version 24 → 24.0.0.0 (SDK 26.0.0.23)
    (25, "24.0.0.0"), // ts2abc.js --target-api-version 25 → 24.0.0.0 (SDK 26.0.0.23)
    (26, "24.0.0.0"), // ts2abc.js --target-api-version 26 → 24.0.0.0 (SDK 26.0.0.23)
    (27, "24.0.0.0"), // ts2abc.js --target-api-version 27 → 24.0.0.0 (SDK 26.0.0.23)
    (28, "24.0.0.0"), // ts2abc.js --target-api-version 28 → 24.0.0.0 (SDK 26.0.0.23)
    (29, "24.0.0.0"), // ts2abc.js --target-api-version 29 → 24.0.0.0 (SDK 26.0.0.23)
    (30, "24.0.0.0"), // ts2abc.js --target-api-version 30 → 24.0.0.0 (SDK 26.0.0.23)
];

/// The bytecode version `api` compiles to, or `None` when no row covers that level.
pub(crate) fn bytecode_version(api: u32) -> Option<&'static str> {
    TABLE
        .iter()
        .find(|(level, _)| *level == api)
        .map(|(_, version)| *version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_unique_and_reachable() {
        for (index, (api, version)) in TABLE.iter().enumerate() {
            assert!(
                !version.is_empty(),
                "API {api} has an empty bytecode version"
            );
            assert_eq!(bytecode_version(*api), Some(*version), "API {api}");
            assert!(
                !TABLE[..index].iter().any(|(seen, _)| seen == api),
                "API {api} appears twice"
            );
        }
        assert_eq!(bytecode_version(11), None);
        assert_eq!(bytecode_version(31), None);
    }

    /// Re-measures the table against the installed SDK; skips when either the SDK's script or
    /// Node is missing, since the point is to compare the table with the toolchain.
    #[test]
    fn the_table_matches_the_installed_sdk() {
        let env = match crate::sdk::manage::detect("") {
            Ok(env) => env,
            Err(_) => return,
        };
        let node = crate::sdk::manage::node_bin(&env.node_home);
        if !node.is_file() {
            println!("skip: no node at {}", node.display());
            return;
        }
        let platform = crate::build::deps::sdk_facts::platform_root(&env.ohos_sdk);
        let script = platform.join("ets/build-tools/ets-loader/bin/ark/ts2abc.js");
        if !script.is_file() {
            println!("skip: no ts2abc.js at {}", script.display());
            return;
        }
        for (api, expected) in TABLE {
            let output = std::process::Command::new(&node)
                .arg(&script)
                .arg("--target-api-version")
                .arg(api.to_string())
                .output()
                .unwrap_or_else(|e| panic!("run {} for API {api}: {e}", node.display()));
            assert!(
                output.status.success(),
                "ts2abc.js failed for API {api}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
            assert_eq!(
                actual,
                *expected,
                "API {api} disagrees with {}",
                script.display()
            );
        }
    }
}
