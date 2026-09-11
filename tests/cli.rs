use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Output};

const FORBID_LEGACY_FALLBACK: &str = "OQTOPUS_FORBID_LEGACY_FALLBACK";

// These tests exercise only behavior implemented by the Rust binary. Falling back would make a
// passing assertion ambiguous: it could be validating the legacy CLI instead.
fn run_rust(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oqtopus"))
        .args(args)
        .env(FORBID_LEGACY_FALLBACK, "1")
        .output()
        .expect("Rust CLI should run")
}

#[test]
fn version_uses_the_compiled_package_version() {
    let expected = format!("oqtopus {}\n", env!("CARGO_PKG_VERSION"));

    // Both spellings intentionally ignore trailing arguments for compatibility with the legacy
    // command. The environment override is poisoned to prove that the build metadata wins.
    for args in [
        &["version"][..],
        &["--version"],
        &["version", "ignored"],
        &["--version", "ignored"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_oqtopus"))
            .args(args)
            .env(FORBID_LEGACY_FALLBACK, "1")
            .env("OQTOPUS_CLI_VERSION", "must-not-be-used")
            .output()
            .expect("Rust CLI should run");

        assert!(
            output.status.success(),
            "Rust failed for arguments {args:?}"
        );
        assert!(output.stderr.is_empty(), "Rust wrote stderr for {args:?}");
        assert_eq!(
            output.stdout,
            expected.as_bytes(),
            "wrong version for {args:?}"
        );
    }
}

#[test]
fn unmigrated_route_honors_fallback_forbidden() {
    // Cover both an unknown top-level command and a known command whose subcommand is not migrated.
    for args in [&["not-yet-migrated"][..], &["backend", "device-status"]] {
        let output = run_rust(args);

        assert_eq!(output.status.code(), Some(125), "wrong status for {args:?}");
        assert!(
            output.stdout.is_empty(),
            "stdout was not empty for {args:?}"
        );
        assert_eq!(
            output.stderr, b"Error: legacy Bash fallback is forbidden for this invocation.\n",
            "wrong stderr for {args:?}"
        );
    }
}

#[test]
fn non_utf8_arguments_abort_before_routing() {
    // Non-UTF-8 arguments are not supported. Such an invocation must fail before any routing
    // decision, so it reaches neither a migrated command nor the legacy CLI.
    let unsupported = OsStr::from_bytes(b"\xff");
    for args in [
        // Would route to the legacy CLI, and to a migrated command, respectively.
        vec![OsStr::new("backend"), unsupported],
        vec![OsStr::new("backend"), OsStr::new("info"), unsupported],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_oqtopus"))
            .args(&args)
            .env(FORBID_LEGACY_FALLBACK, "1")
            .output()
            .expect("Rust CLI should run");

        // Argument collection panics; with the default unwind strategy that is exit status 101.
        assert_eq!(output.status.code(), Some(101), "wrong status for {args:?}");
        assert!(
            output.stdout.is_empty(),
            "stdout was not empty for {args:?}"
        );
        // Neither a routed command nor the forbidden-fallback path reported this failure.
        assert!(
            !output.stderr.starts_with(b"Error: "),
            "failure was reported as a command error for {args:?}"
        );
    }
}
