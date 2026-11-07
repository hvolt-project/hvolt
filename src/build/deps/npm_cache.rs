//! Package cache: where a dependency resolved from a registry is unpacked.
//!
//! One layout for every source, so a build reads the same path whether the package came from
//! a public registry or a local directory served as one:
//!
//! ```text
//! ~/.hvolt/cache/packages/<registry>/<name>/<version>/
//! ```
//!
//! Nothing here touches the project tree: a package that compiles as it lies never leaves the
//! cache, and the flat project keeps only `hvolt.toml` as its configuration.

use std::path::PathBuf;

/// Root of the package cache (`~/.hvolt/cache/packages`).
pub fn root() -> PathBuf {
    crate::sdk::manage::home_dir()
        .join(".hvolt")
        .join("cache")
        .join("packages")
}

/// Directory one package version occupies.
pub fn package_dir(registry: &str, name: &str, version: &str) -> PathBuf {
    root().join(registry).join(name).join(version)
}
