//! Native backend commands and their result data.

mod components;
mod lifecycle;
mod operations;
mod services;
mod versions;

pub(crate) use lifecycle::{backend_restart, backend_start, backend_stop};
pub(crate) use operations::{backend_build, backend_install, backend_uninstall, backend_update};
pub(crate) use versions::backend_versions;

use std::fs;
use std::path::Path;

use crate::environment::validate_environment;
use crate::service::{ServiceStatus, running_pid};
use services::BackendService;

/// Validated metadata rendered by the `info` command.
pub(crate) struct BackendInfo {
    pub(crate) metadata: String,
}

/// Status of every backend service in the order consumed by the Manager.
pub(crate) struct BackendStatus {
    pub(crate) services: Vec<ServiceStatus>,
}

pub(crate) enum BackendDeviceStatus {
    Help,
    Invalid,
    Show(String),
    Updated(&'static str),
}

impl BackendDeviceStatus {
    /// Exit status for this outcome. An unusable action prints usage and fails.
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::Invalid => 1,
            Self::Help | Self::Show(_) | Self::Updated(_) => 0,
        }
    }
}

/// Validates the current backend environment and returns its metadata.
pub(crate) fn backend_info(args: &[String]) -> Result<BackendInfo, String> {
    if !args.is_empty() {
        return Err("oqtopus backend info does not accept arguments.".to_owned());
    }

    validate_environment("backend").map(|environment| BackendInfo {
        metadata: environment.metadata,
    })
}

/// Returns the observed process state of each managed backend service.
pub(crate) fn backend_status(args: &[String]) -> Result<BackendStatus, String> {
    if !args.is_empty() {
        return Err("oqtopus backend status does not accept arguments.".to_owned());
    }

    let environment = validate_environment("backend")?;
    let services = BackendService::STATUS_ORDER
        .into_iter()
        .map(BackendService::name)
        .map(|name| ServiceStatus {
            name,
            pid: running_pid(&environment.root.join("pids").join(format!("{name}.pid"))),
        })
        .collect();

    Ok(BackendStatus { services })
}

pub(crate) fn backend_device_status(args: &[String]) -> Result<BackendDeviceStatus, String> {
    if args
        .first()
        .is_some_and(|arg| arg == "help" || arg == "--help")
    {
        return Ok(BackendDeviceStatus::Help);
    }

    let environment = validate_environment("backend")?;
    let path = environment.root.join("config/gateway/device_status");
    if !path.is_file() {
        return Err(format!("device status file not found: {}", path.display()));
    }

    match args.first() {
        Some(action) if action == "show" => fs::read_to_string(&path)
            .map(BackendDeviceStatus::Show)
            .map_err(|error| format!("failed to read device status file: {error}")),
        Some(action) if action == "active" => update_device_status(&path, "active"),
        Some(action) if action == "inactive" => update_device_status(&path, "inactive"),
        Some(action) if action == "maintenance" => update_device_status(&path, "maintenance"),
        _ => Ok(BackendDeviceStatus::Invalid),
    }
}

fn update_device_status(path: &Path, status: &'static str) -> Result<BackendDeviceStatus, String> {
    fs::write(path, format!("{status}\n"))
        .map_err(|error| format!("failed to write device status file: {error}"))?;
    Ok(BackendDeviceStatus::Updated(status))
}
