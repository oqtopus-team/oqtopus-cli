//! Cloud-local service lifecycle commands.

use crate::args::{is_help, single_target, validate_member};
use crate::environment::{NamedEnvironment, validate_named_environment};
use crate::lifecycle::{LifecycleKind, LifecycleOutcome, line, result_code, success, usage};
use crate::service::{
    BackgroundOutput, ServiceCommand, StopStyle, ensure_command, ensure_project_environment,
    start_process, stop_process,
};
use crate::text::write_error;
use std::io::Write;

use super::{DATABASE, SERVICES, infra_command, require_cloud_directory, require_cloud_entry};

// Startup follows the same domain inventory used by status. Shutdown reverses dependencies.
const STOP_ORDER: [&str; 6] = ["user", "provider", "admin", "user_signup", "worker", "db"];

pub(crate) fn cloud_local_start<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    _err: &mut E,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::CloudLocalStart, 0));
    }
    let environment = validate_named_environment("cloud-local")?;
    let Some(target) = args.first().filter(|arg| !arg.is_empty()) else {
        return Ok(usage(LifecycleKind::CloudLocalStart, 1));
    };
    // Trailing start arguments remain ignored for compatibility with the established CLI behavior.
    let foreground = args.get(1).is_some_and(|arg| arg == "--foreground");
    if target == "all" {
        if foreground {
            return Err("--foreground is not supported for 'all'.".into());
        }
        for service in SERVICES {
            start_cloud_service(&environment, service, false, out)?;
        }
    } else {
        let code = start_cloud_service(&environment, target, foreground, out)?;
        return Ok(result_code(code));
    }
    Ok(success())
}

pub(crate) fn cloud_local_stop<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::CloudLocalStop, 0));
    }
    let environment = validate_named_environment("cloud-local")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::CloudLocalStop, 1));
    };
    if target == "all" {
        let failed = stop_many_named(&environment, &STOP_ORDER, out, err);
        return Ok(result_code(failed as i32));
    }
    validate_member(target, &SERVICES)?;
    stop_cloud_service(&environment, target, out)?;
    Ok(success())
}

pub(crate) fn cloud_local_restart<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::CloudLocalRestart, 0));
    }
    let environment = validate_named_environment("cloud-local")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::CloudLocalRestart, 1));
    };
    if target == "all" {
        if stop_many_named(&environment, &STOP_ORDER, out, err) {
            return Ok(result_code(1));
        }
        for service in SERVICES {
            start_cloud_service(&environment, service, false, out)?;
        }
    } else {
        validate_member(target, &SERVICES)?;
        stop_cloud_service(&environment, target, out)?;
        start_cloud_service(&environment, target, false, out)?;
    }
    Ok(success())
}

fn start_cloud_service<W: Write>(
    environment: &NamedEnvironment,
    service: &str,
    foreground: bool,
    out: &mut W,
) -> Result<i32, String> {
    validate_member(service, &SERVICES)?;
    if service == DATABASE {
        return run_database(environment, "start", out).map(|()| 0);
    }
    let log = environment
        .environment
        .root
        .join(format!("logs/{service}/service.log"));
    start_process(
        &environment.environment.root,
        service,
        || cloud_command(environment, service),
        foreground,
        BackgroundOutput::Log(log),
        false,
        out,
    )
}

fn cloud_command(environment: &NamedEnvironment, service: &str) -> Result<ServiceCommand, String> {
    let cloud = require_cloud_directory(&environment.environment, service, "start")?;
    if !cloud.is_dir() {
        return Err(format!(
            "cannot start '{service}': cloud directory not found: {}",
            cloud.display()
        ));
    }
    ensure_project_environment(&cloud)
        .map_err(|error| format!("cannot start '{service}': {error}"))?;
    let entry = match service {
        "user" => "oqtopus-cloud-user-api",
        "provider" => "oqtopus-cloud-provider-api",
        "admin" => "oqtopus-cloud-admin-api",
        "user_signup" => "oqtopus-cloud-user-signup-api",
        "worker" => "oqtopus-cloud-worker",
        _ => return Err(format!("unknown service: {service}")),
    };
    require_cloud_entry(&cloud, entry, service, "start")?;
    Ok(ServiceCommand::uv_run(vec![
        "--project".into(),
        cloud.display().to_string(),
        entry.into(),
    ])
    .with_config_env())
}

/// Delegates database and storage lifecycle, including readiness and setup, to the cloud.
fn run_database<W: Write>(
    environment: &NamedEnvironment,
    action: &str,
    out: &mut W,
) -> Result<(), String> {
    ensure_command("docker")?;
    ensure_command("uv")?;
    let mut command = infra_command(environment, action)?;
    let (before, after) = match action {
        "start" => ("Starting db", "Started db"),
        _ => ("Stopping db", "Stopped db"),
    };
    line(out, before)?;
    if !command.status().is_ok_and(|status| status.success()) {
        return Err(format!("failed to {action} db."));
    }
    line(out, after)
}

fn stop_cloud_service<W: Write>(
    environment: &NamedEnvironment,
    service: &str,
    out: &mut W,
) -> Result<(), String> {
    if service != DATABASE {
        return stop_process(
            &environment.environment.root,
            service,
            StopStyle::CloudLocal,
            out,
        );
    }
    run_database(environment, "stop", out)
}

fn stop_many_named<W: Write, E: Write>(
    environment: &NamedEnvironment,
    services: &[&str],
    out: &mut W,
    err: &mut E,
) -> bool {
    let mut failed = false;
    for service in services {
        if let Err(error) = stop_cloud_service(environment, service, out) {
            let _ = write_error(err, &error);
            failed = true;
        }
    }
    failed
}
