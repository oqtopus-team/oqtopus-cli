//! Existing text representation of command results.
//!
//! Command logic returns data; these functions own text formatting and write to the sink chosen
//! by the entrypoint. They do not select routes or execute commands.

use std::io::{self, Write};

use crate::backend::{BackendInfo, BackendStatus};
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
        if let Some(pid) = service.pid {
            writeln!(out, "{}: Running (PID {pid})", service.name)?;
        } else {
            writeln!(out, "{}: Stopped", service.name)?;
        }
    }
    out.flush()
}

pub(crate) fn write_error(out: &mut impl Write, message: &str) -> io::Result<()> {
    writeln!(out, "Error: {message}")?;
    out.flush()
}
