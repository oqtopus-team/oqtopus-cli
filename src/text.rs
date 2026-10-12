//! Existing text representation of command results.
//!
//! Command logic returns data; these functions own text formatting and write to the sink chosen
//! by the entrypoint. They do not select routes or execute commands.

use std::io::{self, Write};

use crate::backend::{BackendDeviceStatus, BackendInfo, BackendStatus};
use crate::cloud_local::{CloudLocalInfo, CloudLocalStatus};
use crate::completion::{CompletionOutcome, CompletionOutput};
use crate::init::{InitOutcome, InitOutput};
use crate::lifecycle::{LifecycleKind, LifecycleOutcome, LifecycleOutput};
use crate::manager::{ManagerInfo, ManagerStatus};
use crate::operations::{OperationKind, OperationOutcome};
use crate::service::ServiceStatus;
use crate::version::VersionInfo;
use crate::versions::{VersionsKind, VersionsOutcome, VersionsOutput};

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

const BACKEND_HELP: &str = "\
Usage:
  oqtopus backend <command> [args]

Commands:
  install         Install backend component releases.
  build           Build backend runtime artifacts.
  versions        List available component versions.
  uninstall       Remove an installed release.
  update          Install the latest release of a component.
  start           Start a managed service.
  stop            Stop a managed service.
  restart         Restart a managed service.
  status          Show process status.
  device-status   Show or update gateway device status.
  info            Show backend environment information.
  help            Show help.
";

const CLOUD_LOCAL_HELP: &str = "\
Usage:
  oqtopus cloud-local <command> [args]

Commands:
  install         Install cloud-local component releases.
  versions        List available component versions.
  uninstall       Remove an installed release.
  update          Install the latest release of a component.
  start           Start a managed service.
  stop            Stop a managed service.
  restart         Restart a managed service.
  status          Show process status.
  info            Show environment metadata.
  help            Show this help.
";

const MANAGER_HELP: &str = "\
Usage:
  oqtopus manager <command> [args]

Commands:
  install    Install a manager release.
  uninstall  Remove an installed release.
  update     Install the latest release and update the environment binding.
  versions   List available manager versions.
  start      Start the manager service.
  stop       Stop the manager service.
  restart    Restart the manager service.
  status     Show process status.
  info       Show manager environment information.
  help       Show this help.
";

pub(crate) fn write_backend_help(out: &mut impl Write) -> io::Result<()> {
    out.write_all(BACKEND_HELP.as_bytes())?;
    out.flush()
}

pub(crate) fn write_cloud_local_help(out: &mut impl Write) -> io::Result<()> {
    out.write_all(CLOUD_LOCAL_HELP.as_bytes())?;
    out.flush()
}

pub(crate) fn write_manager_help(out: &mut impl Write) -> io::Result<()> {
    out.write_all(MANAGER_HELP.as_bytes())?;
    out.flush()
}

pub(crate) fn write_version(out: &mut impl Write, info: &VersionInfo) -> io::Result<()> {
    writeln!(out, "oqtopus {}", info.version)?;
    out.flush()
}

pub(crate) fn write_init(out: &mut impl Write, result: &InitOutcome) -> io::Result<()> {
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
    out.write_all(info.metadata.as_bytes())?;
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
        BackendDeviceStatus::Show(contents) => out.write_all(contents.as_bytes())?,
        BackendDeviceStatus::Updated(action) => writeln!(out, "{action}")?,
    }
    out.flush()
}

pub(crate) fn write_cloud_local_info(
    out: &mut impl Write,
    info: &CloudLocalInfo,
) -> io::Result<()> {
    out.write_all(info.metadata.as_bytes())?;
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
    out.write_all(info.metadata.as_bytes())?;
    out.flush()
}

pub(crate) fn write_manager_status(out: &mut impl Write, status: &ManagerStatus) -> io::Result<()> {
    write_process_status(out, &status.service)?;
    out.flush()
}

pub(crate) fn write_versions(out: &mut impl Write, result: &VersionsOutcome) -> io::Result<()> {
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
                // A tag reaches this list because it is advertised remotely, installed locally, or
                // bound in metadata, so the annotation names whichever of those is worth pointing
                // out. "not in remote tags" warns that a local version has no counterpart upstream;
                // it is suppressed for branch checkouts, which are never expected to be tags.
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

/// Writes an operation's final output, which exists only when the operation refused to run.
///
/// A successful install, build, uninstall, or update has already streamed its progress through the
/// reporter, so there is nothing left to render here beyond flushing.
pub(crate) fn write_operation(out: &mut impl Write, result: &OperationOutcome) -> io::Result<()> {
    if let Some(kind) = result.usage {
        out.write_all(operation_usage(kind).as_bytes())?;
    }
    out.flush()
}

pub(crate) fn write_lifecycle(out: &mut impl Write, result: &LifecycleOutcome) -> io::Result<()> {
    if let LifecycleOutput::Usage(kind) = result.output {
        out.write_all(lifecycle_usage(kind).as_bytes())?;
    }
    out.flush()
}

fn lifecycle_usage(kind: LifecycleKind) -> &'static str {
    match kind {
        LifecycleKind::BackendStart => {
            "Usage:\n  oqtopus backend start <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway|all>\n  oqtopus backend start <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway> --foreground\n"
        }
        LifecycleKind::BackendStop => {
            "Usage:\n  oqtopus backend stop <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway|all>\n"
        }
        LifecycleKind::BackendRestart => {
            "Usage:\n  oqtopus backend restart <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway|all>\n"
        }
        LifecycleKind::CloudLocalStart => {
            "Usage:\n  oqtopus cloud-local start <db|user|provider|admin|user_signup|worker|all>\n  oqtopus cloud-local start <db|user|provider|admin|user_signup|worker> --foreground\n"
        }
        LifecycleKind::CloudLocalStop => {
            "Usage:\n  oqtopus cloud-local stop <db|user|provider|admin|user_signup|worker|all>\n"
        }
        LifecycleKind::CloudLocalRestart => {
            "Usage:\n  oqtopus cloud-local restart <db|user|provider|admin|user_signup|worker|all>\n"
        }
        LifecycleKind::ManagerStart => {
            "Usage:\n  oqtopus manager start\n  oqtopus manager start --foreground\n"
        }
        LifecycleKind::ManagerStop => "Usage:\n  oqtopus manager stop\n",
        LifecycleKind::ManagerRestart => "Usage:\n  oqtopus manager restart\n",
    }
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

const COMPLETION_USAGE: &str = "\
Usage:
  oqtopus completion <bash|zsh|fish>
";

pub(crate) fn write_completion(out: &mut impl Write, result: &CompletionOutcome) -> io::Result<()> {
    match result.output {
        CompletionOutput::Usage => out.write_all(COMPLETION_USAGE.as_bytes())?,
        CompletionOutput::Script(script) => out.write_all(script.as_bytes())?,
    }
    out.flush()
}

pub(crate) fn write_error(out: &mut impl Write, message: &str) -> io::Result<()> {
    writeln!(out, "Error: {message}")?;
    out.flush()
}
