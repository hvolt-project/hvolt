// lib: the hvolt crate as a library, so an embedder (the on-device IDE) can call
// the same command surface the CLI exposes without spawning a process.
//
// The binary in `main.rs` is a thin shim over [`main_entry`]; every module keeps
// its `crate::` paths because they now resolve inside this crate.
pub mod build;
pub mod cli;
pub mod device;
pub mod ffi;
pub mod project;
pub mod sdk;
pub mod sign;
pub mod support;
pub mod translator;

/// The CLI entry point: signal conventions plus one dispatch call.
pub fn main_entry() {
    #[cfg(unix)]
    support::platform::reset_sigpipe();
    #[cfg(windows)]
    support::platform::install_pipe_quiet_exit();

    let args = support::cli::parse();
    cli::dispatch(args.command);
}
