//! OQTOPUS command-line entry point.
//!
//! Commands are being migrated incrementally from the legacy Bash implementation. This crate
//! handles migrated routes directly and replaces itself with the legacy CLI for all other routes,
//! preserving command-line compatibility during the transition.

mod backend;
mod cli;
mod legacy;
mod metadata;
mod text;
mod version;

use std::env;
use std::io;
use std::process;

use backend::{backend_info, backend_status};
use cli::{Route, route};
use legacy::run_legacy;
use version::version_info;

fn main() {
    // Rust ignores SIGPIPE by default. CLI pipelines expect the traditional Unix behavior: exit
    // silently when a downstream reader such as `head` or `grep -q` closes the pipe early.
    // SAFETY: installing the default disposition for SIGPIPE requires no Rust-managed callback.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args: Vec<_> = env::args_os().skip(1).collect();

    let outcome = match route(&args) {
        Route::Help => text::write_help(&mut io::stdout().lock())
            .map_err(|error| format!("failed to write help: {error}")),
        Route::Version => text::write_version(&mut io::stdout().lock(), &version_info())
            .map_err(|error| format!("failed to write version: {error}")),
        Route::BackendInfo => backend_info(&args[2..]).and_then(|info| {
            text::write_backend_info(&mut io::stdout().lock(), &info)
                .map_err(|error| format!("failed to write backend info: {error}"))
        }),
        Route::BackendStatus => backend_status(&args[2..]).and_then(|status| {
            text::write_backend_status(&mut io::stdout().lock(), &status)
                .map_err(|error| format!("failed to write backend status: {error}"))
        }),
        Route::Legacy => run_legacy(&args),
    };

    if let Err(error) = outcome {
        let _ = text::write_error(&mut io::stderr().lock(), &error);
        process::exit(1);
    }
}
