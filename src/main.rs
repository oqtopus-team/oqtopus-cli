//! OQTOPUS command-line entry point.
//!
//! Commands are being migrated incrementally from the legacy Bash implementation. This crate
//! handles migrated routes directly and replaces itself with the legacy CLI for all other routes,
//! preserving command-line compatibility during the transition.

mod backend;
mod cli;
mod environment;
mod legacy;
mod metadata;
mod service;
mod text;
mod version;

use std::env;
use std::io;
use std::process;

use backend::{backend_device_status, backend_info, backend_status};
use cli::{Route, route};
use legacy::run_legacy;
use version::version_info;

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

fn main() {
    // Rust ignores SIGPIPE by default. CLI pipelines expect the traditional Unix behavior: exit
    // silently when a downstream reader such as `head` or `grep -q` closes the pipe early.
    // SAFETY: installing the default disposition for SIGPIPE requires no Rust-managed callback.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args: Vec<String> = env::args().skip(1).collect();
    // Routing consumes at most the two leading words, so the remainder is the selected
    // command's own argument list. Help, version, and legacy routes ignore it.
    let command_args = args.get(2..).unwrap_or_default();

    // Commands report their own exit status; a returned message is always a failure.
    let outcome: Result<i32, String> = match route(&args) {
        Route::Help => text::write_help(&mut io::stdout().lock())
            .map(|()| EXIT_SUCCESS)
            .map_err(|error| format!("failed to write help: {error}")),
        Route::Version => text::write_version(&mut io::stdout().lock(), &version_info())
            .map(|()| EXIT_SUCCESS)
            .map_err(|error| format!("failed to write version: {error}")),
        Route::BackendInfo => backend_info(command_args).and_then(|info| {
            text::write_backend_info(&mut io::stdout().lock(), &info)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write backend info: {error}"))
        }),
        Route::BackendStatus => backend_status(command_args).and_then(|status| {
            text::write_backend_status(&mut io::stdout().lock(), &status)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write backend status: {error}"))
        }),
        Route::BackendDeviceStatus => backend_device_status(command_args).and_then(|status| {
            text::write_backend_device_status(&mut io::stdout().lock(), &status)
                .map(|()| status.exit_code())
                .map_err(|error| format!("failed to write backend device status: {error}"))
        }),
        Route::Legacy => run_legacy(&args),
    };

    match outcome {
        Ok(EXIT_SUCCESS) => {}
        Ok(code) => process::exit(code),
        Err(error) => {
            let _ = text::write_error(&mut io::stderr().lock(), &error);
            process::exit(EXIT_FAILURE);
        }
    }
}
