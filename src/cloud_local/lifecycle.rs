//! Cloud-local service lifecycle commands.

use std::env;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::args::{is_help, single_target, validate_member};
use crate::environment::{Environment, NamedEnvironment, validate_named_environment};
use crate::lifecycle::{LifecycleKind, LifecycleOutcome, line, result_code, success, usage};
use crate::metadata::metadata_get;
use crate::service::{
    BackgroundOutput, ServiceCommand, StopStyle, ensure_command, load_env_exports, start_process,
    stop_process,
};
use crate::text::write_error;

use super::{DATABASE, SERVICES};

// Startup follows the same domain inventory used by status. Shutdown reverses dependencies.
const STOP_ORDER: [&str; 6] = ["user", "provider", "admin", "user_signup", "worker", "db"];

pub(crate) fn cloud_local_start<W: Write, E: Write>(
    args: &[String],
    out: &mut W,
    err: &mut E,
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
            start_cloud_service(&environment, service, false, out, err)?;
        }
    } else {
        validate_member(target, &SERVICES)?;
        stop_cloud_service(&environment, target, out)?;
        start_cloud_service(&environment, target, false, out, err)?;
    }
    Ok(success())
}

fn start_cloud_service<W: Write, E: Write>(
    environment: &NamedEnvironment,
    service: &str,
    foreground: bool,
    out: &mut W,
    err: &mut E,
) -> Result<i32, String> {
    validate_member(service, &SERVICES)?;
    if service == DATABASE {
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
        "worker" => {
            let mut command = ServiceCommand::uv(vec![
                "run".into(),
                "--project".into(),
                cloud.display().to_string(),
                "python".into(),
                cloud
                    .join("backend/oqtopus_cloud/worker/pending_jobs_updater/local_scheduler.py")
                    .display()
                    .to_string(),
            ]);
            command.environment = vec![
                ("POWERTOOLS_METRICS_NAMESPACE", "pending-jobs-updater"),
                ("POWERTOOLS_SERVICE_NAME", "pending-jobs-updater"),
            ];
            command
        }
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
    let mut reachable = env::var_os("OQTOPUS_TEST_MODE").is_some()
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
    if service != DATABASE {
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
    let metadata = environment.metadata.as_str();
    let version = metadata_get(metadata, "cloud_local_cloud_version").ok_or_else(|| {
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
