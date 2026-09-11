//! Native backend commands and their result data.

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

    validate_environment("backend")?;
    let services = SERVICES
        .into_iter()
        .map(|name| ServiceStatus {
            name,
            pid: running_pid(Path::new("pids").join(format!("{name}.pid")).as_path()),
        })
        .collect();

    Ok(BackendStatus { services })
}
