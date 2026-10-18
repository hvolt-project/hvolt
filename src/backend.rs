//! AppGallery Connect backend facade.
//!
//! The AGC wire protocol — login flow, token handling, certificate, device and
//! provisioning endpoints, and the remote signing pipeline — is not part of this
//! source tree. It is compiled in from a local `agc-local/` overlay when the crate
//! is built with `--features agc-local`. Without that overlay every entry point
//! here reports that the build has no AGC support, and local signing
//! (`hvolt sign --local`) stays fully usable.

use std::path::Path;

/// The overlay is spliced in with `include!` rather than `#[path] mod`: rustfmt
/// resolves `#[path]` module declarations even when they sit behind a `cfg`, which
/// made `cargo fmt --check` fail on a tree without the overlay (measured in CI,
/// 2026-10-18). `include!` is opaque to rustfmt, so the public tree formats cleanly.
#[cfg(feature = "agc-local")]
pub(crate) mod imp {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/agc-local/mod.rs"));
}

/// Message returned by every AGC entry point in a build without the overlay.
#[cfg_attr(feature = "agc-local", allow(dead_code))] // only read when the backend is absent
pub const NO_BACKEND: &str = "this build has no AppGallery Connect backend \
(see README, section \"Signing and AGC\"); local signing is available through \
`hvolt sign --local`";

/// State of the stored AGC token, for `hvolt doctor`.
#[cfg_attr(not(feature = "agc-local"), allow(dead_code))] // built by the optional backend
pub enum TokenState {
    /// No backend compiled in — the question does not apply to this build.
    #[cfg_attr(feature = "agc-local", allow(dead_code))]
    NotBuilt,
    /// No token file.
    Missing,
    Valid {
        minutes_left: u64,
    },
    ExpiredRefreshable,
    Expired,
}

/// Remote signing through AppGallery Connect.
pub fn sign_remote(project_dir: &Path) -> Result<(), String> {
    #[cfg(feature = "agc-local")]
    {
        imp::sign_remote_entry(project_dir)
    }
    #[cfg(not(feature = "agc-local"))]
    {
        let _ = project_dir;
        Err(NO_BACKEND.to_string())
    }
}

/// Interactive login (opens a browser).
pub fn login() -> Result<(), String> {
    #[cfg(feature = "agc-local")]
    {
        imp::auth::login().map(|_| ())
    }
    #[cfg(not(feature = "agc-local"))]
    {
        Err(NO_BACKEND.to_string())
    }
}

pub fn list_certs() -> Result<(), String> {
    #[cfg(feature = "agc-local")]
    {
        imp::agc::list_certs();
        Ok(())
    }
    #[cfg(not(feature = "agc-local"))]
    {
        Err(NO_BACKEND.to_string())
    }
}

pub fn delete_cert(id: &str) -> Result<(), String> {
    #[cfg(feature = "agc-local")]
    {
        imp::agc::delete_cert(id);
        Ok(())
    }
    #[cfg(not(feature = "agc-local"))]
    {
        let _ = id;
        Err(NO_BACKEND.to_string())
    }
}

pub fn list_devices() -> Result<(), String> {
    #[cfg(feature = "agc-local")]
    {
        imp::agc::list_devices();
        Ok(())
    }
    #[cfg(not(feature = "agc-local"))]
    {
        Err(NO_BACKEND.to_string())
    }
}

pub fn whoami() -> Result<(), String> {
    #[cfg(feature = "agc-local")]
    {
        imp::agc::whoami();
        Ok(())
    }
    #[cfg(not(feature = "agc-local"))]
    {
        Err(NO_BACKEND.to_string())
    }
}

/// Token state for `hvolt doctor`.
pub fn token_state() -> TokenState {
    #[cfg(feature = "agc-local")]
    {
        match imp::auth::load_token() {
            None => TokenState::Missing,
            Some(t) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let left = (t.obtained_at + t.expires_in.unwrap_or(3600)).saturating_sub(now);
                if left > 60 {
                    TokenState::Valid {
                        minutes_left: left / 60,
                    }
                } else if t.refresh_token.is_some() {
                    TokenState::ExpiredRefreshable
                } else {
                    TokenState::Expired
                }
            }
        }
    }
    #[cfg(not(feature = "agc-local"))]
    {
        TokenState::NotBuilt
    }
}
