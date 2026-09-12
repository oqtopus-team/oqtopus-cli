//! Native backend commands and their result data.

use std::fs;
use std::path::Path;

use crate::environment::validate_environment;
use crate::service::{ServiceStatus, running_pid};

/// Validated metadata. Retain source bytes because text output must preserve unknown fields,
/// line endings, and non-UTF-8 bytes. A future JSON view must define its own parsing contract.
pub(crate) struct BackendInfo {
    pub(crate) metadata: Vec<u8>,
}

/// Status of every backend service in the order consumed by the Manager.
pub(crate) struct BackendStatus {
    pub(crate) services: Vec<ServiceStatus>,
}

pub(crate) enum BackendDeviceStatus {
    Help,
    Invalid,
    Show(Vec<u8>),
    Updated(&'static str),
}

impl BackendDeviceStatus {
    /// Exit status for this outcome. An unusable action prints usage and fails, as in Bash.
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::Invalid => 1,
            Self::Help | Self::Show(_) | Self::Updated(_) => 0,
        }
    }
}

const SERVICES: [&str; 7] = [
    "core",
    "sse_engine",
    "mitigator",
    "estimator",
    "combiner",
    "tranqu",
    "gateway",
];

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
    let services = SERVICES
        .into_iter()
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
        Some(action) if action == "show" => fs::read(&path)
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
