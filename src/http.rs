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
