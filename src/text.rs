//! Existing text representation of command results.
//!
//! Command logic returns data; these functions own text formatting and write to the sink chosen
//! by the entrypoint. They do not select routes or execute commands.

use std::io::{self, Write};

use crate::backend::{BackendDeviceStatus, BackendInfo, BackendStatus};
use crate::cloud_local::{CloudLocalInfo, CloudLocalStatus};
use crate::init::{InitOutput, InitResult};
use crate::manager::{ManagerInfo, ManagerStatus};
use crate::operations::{OperationKind, OperationOutput, OperationResult};
use crate::service::ServiceStatus;
use crate::version::VersionInfo;
use crate::versions::{VersionsKind, VersionsOutput, VersionsResult};

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

const INIT_USAGE: &str = "\
Usage:
  oqtopus init <env_name> --template backend [--branch <branch>]
  oqtopus init <env_name> --template cloud-local [--branch <branch>]
  oqtopus init <env_name> --template manager [--branch <branch>]

Creates a local OQTOPUS environment from the official template.

Options:
  --branch <branch>  Fetch the template from this branch of oqtopus-cli
                      instead of 'main'. Mainly useful for testing
                      in-development templates.
";

pub(crate) fn write_help(out: &mut impl Write) -> io::Result<()> {
    out.write_all(TOP_LEVEL_HELP.as_bytes())?;
    out.flush()
}

pub(crate) fn write_version(out: &mut impl Write, info: &VersionInfo) -> io::Result<()> {
    writeln!(out, "oqtopus {}", info.version)?;
    out.flush()
}

pub(crate) fn write_init(out: &mut impl Write, result: &InitResult) -> io::Result<()> {
    match &result.output {
        InitOutput::Usage => out.write_all(INIT_USAGE.as_bytes())?,
        InitOutput::Created { template, root } => {
            writeln!(
                out,
                "Created {} environment: {}",
                template.name(),
                root.display()
            )?;
        }
    }
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

pub(crate) fn write_versions(out: &mut impl Write, result: &VersionsResult) -> io::Result<()> {
    match &result.output {
        VersionsOutput::Usage(VersionsKind::Backend) => {
            out.write_all(b"Usage:\n  oqtopus backend versions <engine|tranqu|gateway>\n")?
        }
        VersionsOutput::Usage(VersionsKind::CloudLocal) => {
            out.write_all(b"Usage:\n  oqtopus cloud-local versions <cloud|frontend|admin>\n")?
        }
        VersionsOutput::Usage(VersionsKind::Manager) => {
            out.write_all(b"Usage:\n  oqtopus manager versions\n")?
        }
        VersionsOutput::List(list) => {
            writeln!(out, "{}:", list.component)?;
            for entry in &list.entries {
                let prefix = if entry.current { "* " } else { "  " };
                let annotation = match (entry.installed, entry.remote) {
                    (true, false) if !entry.tag.starts_with("branch:") => {
                        " (installed, not in remote tags)"
                    }
                    (true, _) => " (installed)",
                    (false, false) => " (not in remote tags)",
                    (false, true) => "",
                };
                writeln!(out, "{prefix}{}{annotation}", entry.tag)?;
            }
        }
    }
    out.flush()
}

pub(crate) fn write_operation(out: &mut impl Write, result: &OperationResult) -> io::Result<()> {
    if let OperationOutput::Usage(kind) = result.output {
        out.write_all(operation_usage(kind).as_bytes())?;
    }
    out.flush()
}

fn operation_usage(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::BackendInstall => {
            "Usage:\n  oqtopus backend install <engine|tranqu|gateway> [version|branch:<branch>] [--skip-sse-build]\n  oqtopus backend install all [--skip-sse-build]\n"
        }
        OperationKind::BackendBuild => "Usage:\n  oqtopus backend build sse-runtime\n",
        OperationKind::BackendUninstall => {
            "Usage:\n  oqtopus backend uninstall <engine|tranqu|gateway> <version>\n  oqtopus backend uninstall <engine|tranqu|gateway> branch:<branch>\n"
        }
        OperationKind::BackendUpdate => {
            "Usage:\n  oqtopus backend update <engine|tranqu|gateway>\n"
        }
        OperationKind::CloudLocalInstall => {
            "Usage:\n  oqtopus cloud-local install <cloud|frontend|admin> [version|branch:<branch>]\n  oqtopus cloud-local install all\n"
        }
        OperationKind::CloudLocalUninstall => {
            "Usage:\n  oqtopus cloud-local uninstall <cloud|frontend|admin> <version>\n  oqtopus cloud-local uninstall <cloud|frontend|admin> branch:<branch>\n"
        }
        OperationKind::CloudLocalUpdate => {
            "Usage:\n  oqtopus cloud-local update <cloud|frontend|admin>\n"
        }
        OperationKind::ManagerInstall => {
            "Usage:\n  oqtopus manager install [version|branch:<branch>]\n"
        }
        OperationKind::ManagerUninstall => {
            "Usage:\n  oqtopus manager uninstall <version|branch:<branch>>\n"
        }
        OperationKind::ManagerUpdate => "Usage:\n  oqtopus manager update\n",
    }
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
