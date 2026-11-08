// main: the binary entry point — module tree, signal conventions, and the dispatch call.
mod build;
mod cli;
mod device;
mod project;
mod sdk;
mod sign;
mod support;
mod translator;

fn main() {
    #[cfg(unix)]
    reset_sigpipe();
    #[cfg(windows)]
    install_pipe_quiet_exit();

    #[cfg(unix)]
    reset_sigpipe();

    let args = crate::support::cli::parse();
    crate::cli::dispatch(args.command);
}

/// CLI convention: a broken stdout pipe (head/less closing early) exits
/// silently with the default SIGPIPE action instead of a Rust panic.
#[cfg(unix)]
fn reset_sigpipe() {
    // The only `unsafe` in the crate: restoring the default SIGPIPE action so
    // `hvolt ... | head` exits quietly instead of panicking on EPIPE.
    #[allow(unsafe_code)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// Windows has no SIGPIPE: ERROR_PIPE_BUSY(232) surfaces as a println!
/// panic. Same CLI convention: a closed pipe means "consumer done",
/// exit silently.
#[cfg(windows)]
fn install_pipe_quiet_exit() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        if msg.contains("failed printing to stdout") {
            std::process::exit(0);
        }
        eprintln!("{}", msg);
        std::process::exit(101);
    }));
}
