//! Backend service lifecycle commands.

use std::io::Write;

use crate::args::{is_help, single_target};
use crate::environment::{Environment, validate_environment};
use crate::lifecycle::{LifecycleKind, LifecycleOutcome, result_code, success, usage};
use crate::metadata::metadata_get;
use crate::service::{BackgroundOutput, ServiceCommand, StopStyle, start_process, stop_process};
use crate::text::write_error;

use super::services::BackendService;

pub(crate) fn backend_start<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::BackendStart, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(target) = args.first().filter(|arg| !arg.is_empty()) else {
        return Ok(usage(LifecycleKind::BackendStart, 1));
    };
    let foreground = args.get(1).is_some_and(|arg| arg == "--foreground");
    if args.len() != 1 && !(args.len() == 2 && foreground) {
        return Ok(usage(LifecycleKind::BackendStart, 1));
    }
    if target == "all" {
        if foreground {
            return Err("oqtopus backend start all does not support --foreground. Start one service at a time in foreground mode.".into());
        }
        start_all(&environment, out)
    } else {
        let code = start_backend_service(&environment, target.parse()?, foreground, out)?;
        Ok(result_code(code))
    }
}

pub(crate) fn backend_stop<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::BackendStop, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::BackendStop, 1));
    };
    if target == "all" {
        Ok(result_code(stop_all(&environment, out, err) as i32))
    } else {
        stop_backend_service(&environment, target.parse()?, out)?;
        Ok(success())
    }
}

pub(crate) fn backend_restart<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleOutcome, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::BackendRestart, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::BackendRestart, 1));
    };
    if target == "all" {
        restart_all(&environment, out, err)
    } else {
        restart_backend_service(&environment, target.parse()?, out)
    }
}

fn start_all<W: Write>(environment: &Environment, out: &mut W) -> Result<LifecycleOutcome, String> {
    for service in BackendService::START_ORDER {
        start_backend_service(environment, service, false, out)?;
    }
    Ok(success())
}

fn restart_all<W: Write, E: Write>(
    environment: &Environment,
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleOutcome, String> {
    if stop_all(environment, out, err) {
        return Ok(result_code(1));
    }
    start_all(environment, out)
}

fn restart_backend_service<W: Write>(
    environment: &Environment,
    service: BackendService,
    out: &mut W,
) -> Result<LifecycleOutcome, String> {
    stop_backend_service(environment, service, out)?;
    start_backend_service(environment, service, false, out)?;
    Ok(success())
}

fn start_backend_service<W: Write>(
    environment: &Environment,
    service: BackendService,
    foreground: bool,
    out: &mut W,
) -> Result<i32, String> {
    start_process(
        &environment.root,
        service.name(),
        || backend_command(environment, service),
        foreground,
        BackgroundOutput::Null,
        false,
        out,
    )
}

fn backend_command(
    environment: &Environment,
    service: BackendService,
) -> Result<ServiceCommand, String> {
    let (component, project_name, module) = match service {
        BackendService::Core | BackendService::SseEngine => {
            ("engine", Some("core"), "oqtopus_engine_core.app")
        }
        BackendService::Mitigator => ("engine", Some("mitigator"), "oqtopus_engine_mitigator.app"),
        BackendService::Estimator => ("engine", Some("estimator"), "oqtopus_engine_estimator.app"),
        BackendService::Combiner => ("engine", Some("combiner"), "oqtopus_engine_combiner.app"),
        BackendService::Tranqu => ("tranqu", None, "tranqu_server.proto.service"),
        BackendService::Gateway => ("gateway", None, "device_gateway.service"),
    };
    let service = service.name();
    let metadata = environment.metadata.as_str();
    let version = metadata_get(metadata, &format!("{component}_version")).ok_or_else(|| {
        format!("cannot start '{service}'. Missing {component}_version in .metadata.")
    })?;
    let mut project = if version.starts_with("branch:") {
        environment.root.join(component)
    } else {
        environment
            .install_root
            .join(format!("{component}-{version}"))
    };
    if let Some(name) = project_name {
        project.push(name);
    }
    if !project.is_dir() {
        return Err(format!(
            "cannot start '{service}'. Installed release directory not found: {}",
            project.display()
        ));
    }
    Ok(ServiceCommand::uv(vec![
        "run".into(),
        "--project".into(),
        project.display().to_string(),
        "python".into(),
        "-m".into(),
        module.into(),
        "-c".into(),
        environment
            .root
            .join(format!("config/{service}/config.yaml"))
            .display()
            .to_string(),
        "-l".into(),
        environment
            .root
            .join(format!("config/{service}/logging.yaml"))
            .display()
            .to_string(),
    ]))
}

fn stop_backend_service<W: Write>(
    environment: &Environment,
    service: BackendService,
    out: &mut W,
) -> Result<(), String> {
    stop_process(&environment.root, service.name(), StopStyle::Backend, out)
}

fn stop_all<W: Write, E: Write>(environment: &Environment, out: &mut W, err: &mut E) -> bool {
    let mut failed = false;
    for service in BackendService::START_ORDER.into_iter().rev() {
        if let Err(error) = stop_backend_service(environment, service, out) {
            let _ = write_error(err, &error);
            failed = true;
        }
    }
    failed
}
