//! Native read-only Manager commands.

mod components;
mod lifecycle;
mod operations;
mod versions;

pub(crate) use lifecycle::{manager_restart, manager_start, manager_stop};
pub(crate) use operations::{manager_install, manager_uninstall, manager_update};
pub(crate) use versions::manager_versions;

use crate::environment::validate_environment;
use crate::service::{ServiceStatus, running_pid};

pub(crate) struct ManagerInfo {
    pub(crate) metadata: String,
}

pub(crate) struct ManagerStatus {
    pub(crate) service: ServiceStatus,
}

pub(crate) fn manager_info(args: &[String]) -> Result<ManagerInfo, String> {
    if !args.is_empty() {
        return Err("oqtopus manager info does not accept arguments.".to_owned());
    }

    validate_environment("manager").map(|environment| ManagerInfo {
        metadata: environment.metadata,
    })
}

pub(crate) fn manager_status(args: &[String]) -> Result<ManagerStatus, String> {
    if !args.is_empty() {
        return Err("oqtopus manager status does not accept arguments.".to_owned());
    }

    let environment = validate_environment("manager")?;
    Ok(ManagerStatus {
        service: ServiceStatus {
            name: "manager",
            pid: running_pid(&environment.root.join("pids/manager.pid")),
        },
    })
}
