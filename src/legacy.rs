//! Process handoff for commands still implemented in Bash.

use std::env;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{self, Command};

use crate::text;

const FORBID_LEGACY_FALLBACK: &str = "OQTOPUS_FORBID_LEGACY_FALLBACK";
/// Exit status used when a caller requires Rust-only execution but routing reaches the legacy CLI.
///
/// This distinguishes a deliberately blocked fallback from an ordinary command failure.
const FALLBACK_FORBIDDEN_EXIT_CODE: i32 = 125;

/// Replaces the current process with the legacy CLI, forwarding arguments unchanged.
pub(crate) fn run_legacy(args: &[String]) -> ! {
    if env::var_os(FORBID_LEGACY_FALLBACK).is_some() {
        let _ = text::write_error(
            &mut io::stderr().lock(),
            "legacy Bash fallback is forbidden for this invocation.",
        );
        process::exit(FALLBACK_FORBIDDEN_EXIT_CODE);
    }

    let legacy_cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/oqtopus");
    // exec replaces this process, preserving the legacy CLI's exit status and signal behavior.
    let error = Command::new("bash").arg(legacy_cli).args(args).exec();

    let _ = text::write_error(
        &mut io::stderr().lock(),
        &format!("failed to run the legacy OQTOPUS CLI: {error}"),
    );
    process::exit(126);
}
