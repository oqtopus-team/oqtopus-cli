use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::process::Command;

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
fn non_utf8_arguments_report_an_error_before_routing() {
    // Non-UTF-8 arguments are not supported. Such an invocation must fail before any routing
    // decision, so it reaches neither an unknown-command error nor a migrated command.
    let unsupported = OsStr::from_bytes(b"\xff");
    for args in [
        vec![OsStr::new("backend"), unsupported],
        vec![OsStr::new("backend"), OsStr::new("info"), unsupported],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_oqtopus"))
            .args(&args)
            .output()
            .expect("Rust CLI should run");

        // Invalid argv is rejected once at the process boundary.
        assert_eq!(output.status.code(), Some(1), "wrong status for {args:?}");
        assert!(
            output.stdout.is_empty(),
            "stdout was not empty for {args:?}"
        );
        assert_eq!(
            output.stderr,
            b"Error: command-line arguments must be valid UTF-8.\n"
        );
    }
}

#[test]
fn empty_command_words_show_help() {
    for prefix in [
        vec![],
        vec!["backend"],
        vec!["cloud-local"],
        vec!["manager"],
    ] {
        let run = |word| {
            Command::new(env!("CARGO_BIN_EXE_oqtopus"))
                .args(&prefix)
                .arg(word)
                .output()
                .unwrap()
        };
        let empty = run("");
        let help = run("help");
        assert_eq!(empty.status.code(), Some(0));
        assert!(empty.stderr.is_empty());
        assert_eq!(empty.stdout, help.stdout);
    }
}
