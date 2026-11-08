//! Shared HTTP client configuration.
//!
//! Every request hvolt makes sets a connect and a read timeout: without them a
//! half-open connection hangs the command indefinitely (ureq's defaults have no
//! read timeout). Downloads that legitimately take longer still complete, because
//! the timeout applies per read operation, not to the whole transfer.

use std::time::Duration;

const CONNECT: Duration = Duration::from_secs(30);
const READ: Duration = Duration::from_secs(60);
const WRITE: Duration = Duration::from_secs(60);

/// Agent with explicit timeouts.
pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT)
        .timeout_read(READ)
        .timeout_write(WRITE)
        .build()
}

/// Download `url` into `dest`, continuing a partial file when `resume` is set.
///
/// Mirrors what `curl -L -C - -o <dest> <url>` did: follow redirects (ureq does this by
/// default), ask for the remaining range when a partial file is on disk, and append.
/// A server that ignores `Range` answers 200 with the whole body, so the file is truncated
/// first in that case — otherwise the two copies would be concatenated.
pub fn download_to_file(url: &str, dest: &std::path::Path, resume: bool) -> Result<(), String> {
    use std::io::Write;

    let mut offset = 0u64;
    if resume {
        if let Ok(meta) = std::fs::metadata(dest) {
            offset = meta.len();
        }
    }
    let mut request = agent().get(url);
    if offset > 0 {
        request = request.set("Range", &format!("bytes={offset}-"));
    }
    let response = request.call().map_err(|e| format!("GET {}: {}", url, e))?;
    let partial = response.status() == 206 && offset > 0;
    let mut file = if partial {
        std::fs::OpenOptions::new()
            .append(true)
            .open(dest)
            .map_err(|e| format!("open {}: {}", dest.display(), e))?
    } else {
        std::fs::File::create(dest).map_err(|e| format!("create {}: {}", dest.display(), e))?
    };
    let mut reader = response.into_reader();
    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("write {}: {}", dest.display(), e))?;
    file.flush()
        .map_err(|e| format!("flush {}: {}", dest.display(), e))?;
    Ok(())
}
