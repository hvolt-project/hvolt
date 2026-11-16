// Regression guard for the crash class that kills the host app on a failed command.
//
// In the embedder (`ohos`) build a fatal command path must not call process::exit: appspawn
// turns a non-zero exit into SIGABRT and takes the whole IDE down with it. It has to unwind to
// hvolt_run's catch_unwind and come back as code != 0 instead.
//
// Run with the embedder posture so the `ohos` branches are the ones under test:
//   cargo test --features ohos --test ffi_exit_containment --offline

#[test]
// The FFI boundary is unsafe by construction; the calls below are the test's whole point.
#[allow(unsafe_code)]
fn a_failing_command_returns_a_code_instead_of_killing_the_caller() {
    let argv = String::from("[\"definitely-not-a-subcommand\"]");
    let c_argv = std::ffi::CString::new(argv).unwrap();
    let out = unsafe { hvolt::ffi::hvolt_run(c_argv.as_ptr(), std::ptr::null()) };
    assert!(!out.is_null(), "hvolt_run returned NULL");
    let text = unsafe { std::ffi::CStr::from_ptr(out) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe { hvolt::ffi::hvolt_free(out) };
    assert!(text.contains("\"code\":"), "envelope missing code: {text}");
    assert!(
        !text.contains("\"code\":0"),
        "a bogus subcommand must not report success: {text}"
    );
    // Reaching this line at all is the point: the process survived a fatal command path.
}
