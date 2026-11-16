// support: cross-cutting infrastructure — config model, platform, HTTP, CLI surface, embedded docs.

pub(crate) mod cli;
pub(crate) mod config;
pub(crate) mod http;
pub(crate) mod lsp;
pub(crate) mod platform;
pub(crate) mod skill;

/// Exit the CLI. Inside the embedder (`ohos`) the process is shared with a host
/// application, so the same "fatal" becomes a panic the FFI boundary catches and
/// reports as `code != 0`.
pub fn cli_exit(code: i32) -> ! {
    #[cfg(feature = "ohos")]
    {
        panic!("hvolt command failed with exit code {code}");
    }
    #[cfg(not(feature = "ohos"))]
    {
        std::process::exit(code)
    }
}
