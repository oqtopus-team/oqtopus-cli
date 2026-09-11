//! Command routing during the incremental Rust migration.

/// Implementation selected for a command-line invocation.
///
/// Routing is intentionally coarse while the Rust migration is in progress: anything not listed
/// here remains the legacy CLI's responsibility.
pub(crate) enum Route {
    Help,
    Version,
    BackendInfo,
    BackendStatus,
    BackendDeviceStatus,
    CloudLocalInfo,
    CloudLocalStatus,
    ManagerInfo,
    ManagerStatus,
    Legacy,
}

/// Selects the Rust implementation for migrated commands and [`Route::Legacy`] otherwise.
pub(crate) fn route(args: &[String]) -> Route {
    // Only the two leading words select a route; the rest belongs to the command itself.
    let command = args.first().map(String::as_str);
    let action = args.get(1).map(String::as_str);

    match (command, action) {
        (None, _) | (Some("help" | "--help"), _) => Route::Help,
        (Some("version" | "--version"), _) => Route::Version,
        (Some("backend"), Some("info")) => Route::BackendInfo,
        (Some("backend"), Some("status")) => Route::BackendStatus,
        (Some("backend"), Some("device-status")) => Route::BackendDeviceStatus,
        (Some("cloud-local"), Some("info")) => Route::CloudLocalInfo,
        (Some("cloud-local"), Some("status")) => Route::CloudLocalStatus,
        (Some("manager"), Some("info")) => Route::ManagerInfo,
        (Some("manager"), Some("status")) => Route::ManagerStatus,
        _ => Route::Legacy,
    }
}
