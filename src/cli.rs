//! Command routing during the incremental Rust migration.

use std::ffi::{OsStr, OsString};

/// Implementation selected for a command-line invocation.
///
/// Routing is intentionally coarse while the Rust migration is in progress: anything not listed
/// here remains the legacy CLI's responsibility.
pub(crate) enum Route {
    Help,
    Version,
    BackendInfo,
    BackendStatus,
    Legacy,
}

/// Selects the Rust implementation for migrated commands and [`Route::Legacy`] otherwise.
pub(crate) fn route(args: &[OsString]) -> Route {
    // Match OsStr values directly so non-UTF-8 arguments can still be forwarded unchanged to Bash.
    match args.first().map(OsString::as_os_str) {
        None => Route::Help,
        Some(command) if command == OsStr::new("help") || command == OsStr::new("--help") => {
            Route::Help
        }
        Some(command) if command == OsStr::new("version") || command == OsStr::new("--version") => {
            Route::Version
        }
        Some(command)
            if command == OsStr::new("backend")
                && args.get(1).is_some_and(|arg| arg == OsStr::new("info")) =>
        {
            Route::BackendInfo
        }
        Some(command)
            if command == OsStr::new("backend")
                && args.get(1).is_some_and(|arg| arg == OsStr::new("status")) =>
        {
            Route::BackendStatus
        }
        Some(_) => Route::Legacy,
    }
}
