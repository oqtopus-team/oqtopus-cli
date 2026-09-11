//! Existing text representation of command results.
//!
//! Command logic returns data; these functions own text formatting and write to the sink chosen
//! by the entrypoint. They do not select routes or execute commands.

use std::io::{self, Write};

use crate::backend::{BackendDeviceStatus, BackendInfo, BackendStatus};
use crate::cloud_local::{CloudLocalInfo, CloudLocalStatus};
use crate::manager::{ManagerInfo, ManagerStatus};
use crate::service::ServiceStatus;
use crate::version::VersionInfo;

const TOP_LEVEL_HELP: &str = "\
Usage:
  oqtopus <command> [args]

Commands:
  init         Create an OQTOPUS environment.
  cloud-local  Manage local cloud-local components and services.
  backend      Manage local backend components and services.
  manager      Manage the local manager component and service.
  completion   Print shell completion scripts.
  version      Print the installed CLI version.
  help         Show help.

Run 'oqtopus <command> help' for command-specific help.
";

pub(crate) fn write_help(out: &mut impl Write) -> io::Result<()> {
    out.write_all(TOP_LEVEL_HELP.as_bytes())?;
    out.flush()
}

pub(crate) fn write_version(out: &mut impl Write, info: &VersionInfo) -> io::Result<()> {
    writeln!(out, "oqtopus {}", info.version)?;
    out.flush()
}

pub(crate) fn write_backend_info(out: &mut impl Write, info: &BackendInfo) -> io::Result<()> {
    out.write_all(&info.metadata)?;
    out.flush()
}

pub(crate) fn write_backend_status(out: &mut impl Write, status: &BackendStatus) -> io::Result<()> {
    for service in &status.services {
        write_process_status(out, service)?;
    }
    out.flush()
}

const BACKEND_DEVICE_STATUS_USAGE: &str = "\
Usage:
  oqtopus backend device-status <show|active|inactive|maintenance>
";

pub(crate) fn write_backend_device_status(
    out: &mut impl Write,
    status: &BackendDeviceStatus,
) -> io::Result<()> {
    match status {
        BackendDeviceStatus::Help | BackendDeviceStatus::Invalid => {
            out.write_all(BACKEND_DEVICE_STATUS_USAGE.as_bytes())?
        }
        BackendDeviceStatus::Show(contents) => out.write_all(contents)?,
        BackendDeviceStatus::Updated(action) => writeln!(out, "{action}")?,
    }
    out.flush()
}

pub(crate) fn write_cloud_local_info(
    out: &mut impl Write,
    info: &CloudLocalInfo,
) -> io::Result<()> {
    out.write_all(&info.metadata)?;
    out.flush()
}

pub(crate) fn write_cloud_local_status(
    out: &mut impl Write,
    status: &CloudLocalStatus,
) -> io::Result<()> {
    if let Some(containers) = &status.database_containers {
        writeln!(out, "db: Running ({})", containers.join(", "))?;
    } else {
        writeln!(out, "db: Stopped")?;
    }
    for service in &status.services {
        write_process_status(out, service)?;
    }
    out.flush()
}

pub(crate) fn write_manager_info(out: &mut impl Write, info: &ManagerInfo) -> io::Result<()> {
    out.write_all(&info.metadata)?;
    out.flush()
}

pub(crate) fn write_manager_status(out: &mut impl Write, status: &ManagerStatus) -> io::Result<()> {
    write_process_status(out, &status.service)?;
    out.flush()
}

fn write_process_status(out: &mut impl Write, service: &ServiceStatus) -> io::Result<()> {
    if let Some(pid) = service.pid {
        writeln!(out, "{}: Running (PID {pid})", service.name)
    } else {
        writeln!(out, "{}: Stopped", service.name)
    }
}

pub(crate) fn write_error(out: &mut impl Write, message: &str) -> io::Result<()> {
    writeln!(out, "Error: {message}")?;
    out.flush()
}
