//! Manager service lifecycle commands.

use std::io::Write;

use crate::args::is_help;
use crate::environment::{Environment, validate_environment};
use crate::lifecycle::{LifecycleKind, LifecycleOutcome, result_code, success, usage};
use crate::metadata::metadata_get;
use crate::service::{BackgroundOutput, ServiceCommand, StopStyle, start_process, stop_process};

pub(crate) fn manager_start<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::ManagerStart, 0));
    }
    let environment = validate_environment("manager")?;
    let foreground = args.first().is_some_and(|arg| arg == "--foreground");
    if (!foreground && !args.is_empty()) || (foreground && args.len() != 1) {
        return Ok(usage(LifecycleKind::ManagerStart, 1));
    }
    let code = start_process(
        &environment.root,
        "manager",
        || manager_command(&environment),
        foreground,
        BackgroundOutput::Null,
        true,
        out,
    )?;
    Ok(result_code(code))
}

pub(crate) fn manager_stop<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::ManagerStop, 0));
    }
    let environment = validate_environment("manager")?;
    if !args.is_empty() {
        return Ok(usage(LifecycleKind::ManagerStop, 1));
    }
    stop_process(&environment.root, "manager", StopStyle::Manager, out)?;
    Ok(success())
}

pub(crate) fn manager_restart<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::ManagerRestart, 0));
    }
    let environment = validate_environment("manager")?;
    if !args.is_empty() {
        return Ok(usage(LifecycleKind::ManagerRestart, 1));
    }
    stop_process(&environment.root, "manager", StopStyle::Manager, out)?;
    start_process(
        &environment.root,
        "manager",
        || manager_command(&environment),
        false,
        BackgroundOutput::Null,
        true,
        out,
    )?;
    Ok(success())
}

fn manager_command(environment: &Environment) -> Result<ServiceCommand, String> {
    let metadata = environment.metadata.as_str();
    let version = metadata_get(metadata, "manager_version")
        .ok_or("cannot start manager. Missing manager_version in .metadata.")?;
    let project = if version.starts_with("branch:") {
        environment.root.join("manager")
    } else {
        environment.install_root.join(format!("manager-{version}"))
    };
    if !project.is_dir() {
        return Err(format!(
            "cannot start manager. Installed release directory not found: {}",
            project.display()
        ));
    }
    Ok(ServiceCommand::uv(vec![
        "run".into(),
        "--project".into(),
        project.display().to_string(),
        "python".into(),
        "-m".into(),
        "oqtopus_manager.main".into(),
        "-c".into(),
        environment
            .root
            .join("config/config.yaml")
            .display()
            .to_string(),
        "-l".into(),
        environment
            .root
            .join("config/logging.yaml")
            .display()
            .to_string(),
    ]))
}
