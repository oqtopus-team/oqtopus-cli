//! Command routing during the incremental Rust migration.

/// Implementation selected for a command-line invocation.
///
/// Routing is intentionally coarse while the Rust migration is in progress: anything not listed
/// here remains the legacy CLI's responsibility.
pub(crate) enum Route {
    Help,
    Version,
    Init,
    BackendInfo,
    BackendStatus,
    BackendDeviceStatus,
    BackendVersions,
    BackendInstall,
    BackendBuild,
    BackendUninstall,
    BackendUpdate,
    CloudLocalInfo,
    CloudLocalStatus,
    CloudLocalVersions,
    CloudLocalInstall,
    CloudLocalUninstall,
    CloudLocalUpdate,
    ManagerInfo,
    ManagerStatus,
    ManagerVersions,
    ManagerInstall,
    ManagerUninstall,
    ManagerUpdate,
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
        (Some("init"), _) => Route::Init,
        (Some("backend"), Some("info")) => Route::BackendInfo,
        (Some("backend"), Some("status")) => Route::BackendStatus,
        (Some("backend"), Some("device-status")) => Route::BackendDeviceStatus,
        (Some("backend"), Some("versions")) => Route::BackendVersions,
        (Some("backend"), Some("install")) => Route::BackendInstall,
        (Some("backend"), Some("build")) => Route::BackendBuild,
        (Some("backend"), Some("uninstall")) => Route::BackendUninstall,
        (Some("backend"), Some("update")) => Route::BackendUpdate,
        (Some("cloud-local"), Some("info")) => Route::CloudLocalInfo,
        (Some("cloud-local"), Some("status")) => Route::CloudLocalStatus,
        (Some("cloud-local"), Some("versions")) => Route::CloudLocalVersions,
        (Some("cloud-local"), Some("install")) => Route::CloudLocalInstall,
        (Some("cloud-local"), Some("uninstall")) => Route::CloudLocalUninstall,
        (Some("cloud-local"), Some("update")) => Route::CloudLocalUpdate,
        (Some("manager"), Some("info")) => Route::ManagerInfo,
        (Some("manager"), Some("status")) => Route::ManagerStatus,
        (Some("manager"), Some("versions")) => Route::ManagerVersions,
        (Some("manager"), Some("install")) => Route::ManagerInstall,
        (Some("manager"), Some("uninstall")) => Route::ManagerUninstall,
        (Some("manager"), Some("update")) => Route::ManagerUpdate,
        _ => Route::Legacy,
    }
}
