//! Native start, stop, and restart commands for managed services.

use std::env;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::environment::{
    Environment, NamedEnvironment, validate_environment, validate_named_environment,
};
use crate::metadata::metadata_get;
use crate::service::{
    BackgroundOutput, ServiceCommand, StopStyle, ensure_command, load_env_exports, start_process,
    stop_process,
};
use crate::text::write_error;

#[derive(Clone, Copy)]
pub(crate) enum LifecycleKind {
    BackendStart,
    BackendStop,
    BackendRestart,
    CloudLocalStart,
    CloudLocalStop,
    CloudLocalRestart,
    ManagerStart,
    ManagerStop,
    ManagerRestart,
}

pub(crate) enum LifecycleOutput {
    None,
    Usage(LifecycleKind),
}

pub(crate) struct LifecycleResult {
    pub(crate) output: LifecycleOutput,
    exit_code: i32,
}

impl LifecycleResult {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

const BACKEND_SERVICES: [&str; 7] = [
    "core",
    "sse_engine",
    "mitigator",
    "estimator",
    "combiner",
    "tranqu",
    "gateway",
];
const BACKEND_START: [&str; 7] = [
    "gateway",
    "tranqu",
    "mitigator",
    "estimator",
    "combiner",
    "sse_engine",
    "core",
];
const BACKEND_STOP: [&str; 7] = [
    "core",
    "sse_engine",
    "combiner",
    "estimator",
    "mitigator",
    "tranqu",
    "gateway",
];
const CLOUD_SERVICES: [&str; 6] = ["db", "worker", "user_signup", "admin", "provider", "user"];
const CLOUD_START: [&str; 6] = ["db", "worker", "user_signup", "admin", "provider", "user"];
const CLOUD_STOP: [&str; 6] = ["user", "provider", "admin", "user_signup", "worker", "db"];

pub(crate) fn backend_start<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<LifecycleResult, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::BackendStart, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(target) = args.first() else {
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
        for service in BACKEND_START {
            start_backend_service(&environment, service, false, out)?;
        }
    } else {
        let code = start_backend_service(&environment, target, foreground, out)?;
        return Ok(result_code(code));
    }
    Ok(success())
}

pub(crate) fn backend_stop<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleResult, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::BackendStop, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::BackendStop, 1));
    };
    if target == "all" {
        let failed = stop_many(&environment, &BACKEND_STOP, out, err);
        return Ok(result_code(failed as i32));
    }
    validate_member(target, &BACKEND_SERVICES)?;
    stop_process(&environment.root, target, StopStyle::Backend, out)?;
    Ok(success())
}

pub(crate) fn backend_restart<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleResult, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::BackendRestart, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::BackendRestart, 1));
    };
    if target == "all" {
        if stop_many(&environment, &BACKEND_STOP, out, err) {
            return Ok(result_code(1));
        }
        for service in BACKEND_START {
            start_backend_service(&environment, service, false, out)?;
        }
    } else {
        validate_member(target, &BACKEND_SERVICES)?;
        stop_process(&environment.root, target, StopStyle::Backend, out)?;
        start_backend_service(&environment, target, false, out)?;
    }
    Ok(success())
}

pub(crate) fn cloud_local_start<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleResult, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::CloudLocalStart, 0));
    }
    let environment = validate_named_environment("cloud-local")?;
    let Some(target) = args.first() else {
        return Ok(usage(LifecycleKind::CloudLocalStart, 1));
    };
    let foreground = args.get(1).is_some_and(|arg| arg == "--foreground");
    if target == "all" {
        if foreground {
            return Err("--foreground is not supported for 'all'.".into());
        }
        for service in CLOUD_START {
            start_cloud_service(&environment, service, false, out, err)?;
        }
    } else {
        let code = start_cloud_service(&environment, target, foreground, out, err)?;
        return Ok(result_code(code));
    }
    Ok(success())
}

pub(crate) fn cloud_local_stop<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleResult, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::CloudLocalStop, 0));
    }
    let environment = validate_named_environment("cloud-local")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::CloudLocalStop, 1));
    };
    if target == "all" {
        let failed = stop_many_named(&environment, &CLOUD_STOP, out, err);
        return Ok(result_code(failed as i32));
    }
    validate_member(target, &CLOUD_SERVICES)?;
    stop_cloud_service(&environment, target, out)?;
    Ok(success())
}

pub(crate) fn cloud_local_restart<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
) -> Result<LifecycleResult, String> {
    if is_help(args) {
        return Ok(usage(LifecycleKind::CloudLocalRestart, 0));
    }
    let environment = validate_named_environment("cloud-local")?;
    let Some(target) = single_target(args) else {
        return Ok(usage(LifecycleKind::CloudLocalRestart, 1));
    };
    if target == "all" {
        if stop_many_named(&environment, &CLOUD_STOP, out, err) {
            return Ok(result_code(1));
        }
        for service in CLOUD_START {
            start_cloud_service(&environment, service, false, out, err)?;
        }
    } else {
        validate_member(target, &CLOUD_SERVICES)?;
        stop_cloud_service(&environment, target, out)?;
        start_cloud_service(&environment, target, false, out, err)?;
    }
    Ok(success())
}

pub(crate) fn manager_start<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<LifecycleResult, String> {
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
) -> Result<LifecycleResult, String> {
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
) -> Result<LifecycleResult, String> {
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

fn start_backend_service<W: Write>(
    environment: &Environment,
    service: &str,
    foreground: bool,
    out: &mut W,
) -> Result<i32, String> {
    validate_member(service, &BACKEND_SERVICES)?;
    start_process(
        &environment.root,
        service,
        || backend_command(environment, service),
        foreground,
        BackgroundOutput::Null,
        false,
        out,
    )
}

fn backend_command(environment: &Environment, service: &str) -> Result<ServiceCommand, String> {
    let (component, project_name, module) = match service {
        "core" | "sse_engine" => ("engine", Some("core"), "oqtopus_engine_core.app"),
        "mitigator" => ("engine", Some("mitigator"), "oqtopus_engine_mitigator.app"),
        "estimator" => ("engine", Some("estimator"), "oqtopus_engine_estimator.app"),
        "combiner" => ("engine", Some("combiner"), "oqtopus_engine_combiner.app"),
        "tranqu" => ("tranqu", None, "tranqu_server.proto.service"),
        "gateway" => ("gateway", None, "device_gateway.service"),
        _ => return Err(format!("unknown service: {service}")),
    };
    let metadata = String::from_utf8_lossy(&environment.metadata);
    let version = metadata_get(&metadata, &format!("{component}_version")).ok_or_else(|| {
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

fn manager_command(environment: &Environment) -> Result<ServiceCommand, String> {
    let metadata = String::from_utf8_lossy(&environment.metadata);
    let version = metadata_get(&metadata, "manager_version")
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

fn start_cloud_service<W: Write, E: Write>(
    environment: &NamedEnvironment,
    service: &str,
    foreground: bool,
    out: &mut W,
    err: &mut E,
) -> Result<i32, String> {
    validate_member(service, &CLOUD_SERVICES)?;
    if service == "db" {
        return start_database(environment, out, err).map(|()| 0);
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
    let cloud = cloud_directory(&environment.environment, service, "start")?;
    if !cloud.is_dir() {
        return Err(format!(
            "cannot start '{service}': cloud directory not found: {}",
            cloud.display()
        ));
    }
    let command = match service {
        "worker" => ServiceCommand::uv(vec![
            "run".into(),
            "--project".into(),
            cloud.display().to_string(),
            "python".into(),
            cloud
                .join("backend/oqtopus_cloud/worker/pending_jobs_updater/local_scheduler.py")
                .display()
                .to_string(),
        ]),
        "user" | "provider" | "admin" | "user_signup" => {
            let (namespace, module, port_var, default_port) = match service {
                "user" => (
                    "user-api",
                    "oqtopus_cloud.user.lambda_function:app",
                    "USER_API_PORT",
                    "8080",
                ),
                "provider" => (
                    "provider-api",
                    "oqtopus_cloud.provider.lambda_function:app",
                    "PROVIDER_API_PORT",
                    "8888",
                ),
                "admin" => (
                    "admin-api",
                    "oqtopus_cloud.admin.lambda_function:app",
                    "ADMIN_API_PORT",
                    "8889",
                ),
                _ => (
                    "user_signup-api",
                    "oqtopus_cloud.user_signup.lambda_function:app",
                    "USER_SIGNUP_API_PORT",
                    "8890",
                ),
            };
            let port = env::var(port_var).unwrap_or_else(|_| default_port.into());
            let mut command = ServiceCommand::uv(vec![
                "run".into(),
                "--project".into(),
                cloud.display().to_string(),
                "uvicorn".into(),
                module.into(),
                "--host".into(),
                "0.0.0.0".into(),
                "--port".into(),
                port,
                "--reload".into(),
                "--log-level".into(),
                "debug".into(),
            ]);
            command.environment = vec![
                ("POWERTOOLS_METRICS_NAMESPACE", namespace),
                ("POWERTOOLS_SERVICE_NAME", namespace),
            ];
            command
        }
        _ => return Err(format!("unknown service: {service}")),
    };
    Ok(command)
}

fn start_database<W: Write, E: Write>(
    environment: &NamedEnvironment,
    out: &mut W,
    err: &mut E,
) -> Result<(), String> {
    ensure_command("docker")?;
    ensure_command("uv")?;
    let cloud = cloud_directory(&environment.environment, "db", "start")?;
    let compose = cloud.join("backend/compose.yaml");
    if !compose.is_file() {
        return Err(format!(
            "cannot start 'db': compose.yaml not found in {}",
            cloud.join("backend").display()
        ));
    }
    if database_running(&environment.name, &compose) {
        line(out, "db is already running; skipping container startup.")?;
    } else {
        line(out, "Starting db")?;
        if !docker_compose(&environment.name, &compose)
            .args(["up", "-d", "db", "minio", "mc"])
            .status()
            .is_ok_and(|status| status.success())
        {
            return Err("failed to start db with docker compose.".into());
        }
    }
    line(out, "Waiting for db to be ready")?;
    let mut ready = false;
    for attempt in 0..60 {
        let status = docker_compose(&environment.name, &compose)
            .args([
                "exec",
                "-T",
                "db",
                "mysql",
                "-h127.0.0.1",
                "--protocol=tcp",
                "-uadmin",
                "-ppassword",
                "main",
                "-e",
                "SELECT 1",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok_and(|status| status.success()) {
            ready = true;
            break;
        }
        if attempt < 59 {
            thread::sleep(Duration::from_secs(2));
        }
    }
    if !ready {
        return Err("timed out waiting for db to become ready.".into());
    }
    line(out, "Waiting for db port to be reachable from the host")?;
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3306);
    // The characterization suite cannot open loopback sockets in every sandbox. As with the
    // HTTP fixture hooks, this explicit test hook bypasses only the wait; subsequent setup still
    // executes through the same commands and paths as production.
    let mut reachable = env::var_os("OQTOPUS_FORBID_LEGACY_FALLBACK").is_some()
        && env::var_os("OQTOPUS_TEST_DB_PORT_READY").is_some();
    if !reachable {
        for attempt in 0..30 {
            if TcpStream::connect_timeout(&address.into(), Duration::from_millis(200)).is_ok() {
                reachable = true;
                break;
            }
            if attempt < 29 {
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
    if !reachable {
        return Err(
            "timed out waiting for db port 127.0.0.1:3306 to be reachable from the host.".into(),
        );
    }
    line(out, "db ready, applying migrations")?;
    let exports = load_env_exports(&environment.environment.root.join("config/.env"));
    retry_uv(
        &cloud,
        &cloud.join("backend"),
        &["alembic", "upgrade", "head"],
        &exports,
        "alembic upgrade head",
        err,
    )?;
    line(out, "Seeding db")?;
    retry_uv(
        &cloud,
        &cloud.join("backend"),
        &["python", "scripts/seed.py"],
        &exports,
        "scripts/seed.py",
        err,
    )?;
    let init = cloud.join("backend/storage/init_storage.py");
    if init.is_file() {
        let status = Command::new("uv")
            .args(["run", "--project"])
            .arg(&cloud)
            .arg("python")
            .arg(&init)
            .current_dir(&environment.environment.root)
            .env_remove("VIRTUAL_ENV")
            .envs(&exports)
            .status();
        if !status.is_ok_and(|status| status.success()) {
            return Err("failed to run storage/init_storage.py.".into());
        }
    }
    line(out, "Started db")
}

fn stop_cloud_service<W: Write>(
    environment: &NamedEnvironment,
    service: &str,
    out: &mut W,
) -> Result<(), String> {
    if service != "db" {
        return stop_process(
            &environment.environment.root,
            service,
            StopStyle::CloudLocal,
            out,
        );
    }
    ensure_command("docker")?;
    let cloud = cloud_directory(&environment.environment, "db", "stop")?;
    let compose = cloud.join("backend/compose.yaml");
    if !compose.is_file() {
        return Err(format!(
            "cannot stop 'db': compose.yaml not found in {}",
            cloud.join("backend").display()
        ));
    }
    line(out, "Stopping db")?;
    if !docker_compose(&environment.name, &compose)
        .arg("down")
        .status()
        .is_ok_and(|status| status.success())
    {
        return Err("failed to stop db with docker compose.".into());
    }
    line(out, "Stopped db")
}

fn cloud_directory(
    environment: &Environment,
    service: &str,
    operation: &str,
) -> Result<PathBuf, String> {
    let metadata = String::from_utf8_lossy(&environment.metadata);
    let version = metadata_get(&metadata, "cloud_local_cloud_version").ok_or_else(|| {
        format!("cannot {operation} '{service}': cloud component is not installed.")
    })?;
    Ok(if version.starts_with("branch:") {
        environment.root.join("cloud")
    } else {
        environment.install_root.join(format!("cloud-{version}"))
    })
}

fn database_running(project: &str, compose: &Path) -> bool {
    docker_compose(project, compose)
        .args(["ps", "--status", "running", "--quiet", "db"])
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| {
            output.status.success() && output.stdout.iter().any(|byte| *byte != b'\n')
        })
}

fn docker_compose(project: &str, compose: &Path) -> Command {
    let mut command = Command::new("docker");
    command
        .args(["compose", "--project-name", project, "-f"])
        .arg(compose);
    command
}

fn retry_uv<E: Write>(
    project: &Path,
    cwd: &Path,
    args: &[&str],
    exports: &std::collections::HashMap<String, String>,
    label: &str,
    err: &mut E,
) -> Result<(), String> {
    for attempt in 1..=5 {
        let status = Command::new("uv")
            .args(["run", "--project"])
            .arg(project)
            .args(args)
            .current_dir(cwd)
            .env_remove("VIRTUAL_ENV")
            .envs(exports)
            .status();
        if status.is_ok_and(|status| status.success()) {
            return Ok(());
        }
        if attempt < 5 {
            writeln!(
                err,
                "Warning: {label} failed; retrying ({}/5) in 2s...",
                attempt + 1
            )
            .map_err(|error| format!("failed to write warning: {error}"))?;
            err.flush()
                .map_err(|error| format!("failed to write warning: {error}"))?;
            thread::sleep(Duration::from_secs(2));
        }
    }
    Err(format!("failed to run {label}."))
}

fn stop_many<W: Write, E: Write>(
    environment: &Environment,
    services: &[&str],
    out: &mut W,
    err: &mut E,
) -> bool {
    let mut failed = false;
    for service in services {
        if let Err(error) = stop_process(&environment.root, service, StopStyle::Backend, out) {
            let _ = write_error(err, &error);
            failed = true;
        }
    }
    failed
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

fn validate_member(target: &str, services: &[&str]) -> Result<(), String> {
    services
        .contains(&target)
        .then_some(())
        .ok_or_else(|| format!("unknown service: {target}"))
}

fn single_target(args: &[String]) -> Option<&str> {
    (args.len() == 1 && !args[0].is_empty()).then(|| args[0].as_str())
}

fn is_help(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| arg == "help" || arg == "--help")
}
fn success() -> LifecycleResult {
    result_code(0)
}
fn result_code(exit_code: i32) -> LifecycleResult {
    LifecycleResult {
        output: LifecycleOutput::None,
        exit_code,
    }
}
fn usage(kind: LifecycleKind, exit_code: i32) -> LifecycleResult {
    LifecycleResult {
        output: LifecycleOutput::Usage(kind),
        exit_code,
    }
}
fn line(out: &mut impl Write, message: impl std::fmt::Display) -> Result<(), String> {
    writeln!(out, "{message}").map_err(|error| format!("failed to write progress: {error}"))?;
    out.flush()
        .map_err(|error| format!("failed to write progress: {error}"))
}
