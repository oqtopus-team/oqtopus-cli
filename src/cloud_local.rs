//! Native read-only cloud-local commands.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::environment::{Environment, NamedEnvironment, validate_named_environment};
use crate::metadata::metadata_get;
use crate::service::{ServiceStatus, running_pid};

pub(crate) struct CloudLocalInfo {
    pub(crate) metadata: Vec<u8>,
}

pub(crate) struct CloudLocalStatus {
    pub(crate) database_containers: Option<Vec<String>>,
    pub(crate) services: Vec<ServiceStatus>,
}

const PROCESS_SERVICES: [&str; 5] = ["worker", "user_signup", "admin", "provider", "user"];

pub(crate) fn cloud_local_info(args: &[String]) -> Result<CloudLocalInfo, String> {
    if !args.is_empty() {
        return Err("oqtopus cloud-local info does not accept arguments.".to_owned());
    }

    validate_named_environment("cloud-local").map(|environment| CloudLocalInfo {
        metadata: environment.environment.metadata,
    })
}

pub(crate) fn cloud_local_status(args: &[String]) -> Result<CloudLocalStatus, String> {
    if !args.is_empty() {
        return Err("oqtopus cloud-local status does not accept arguments.".to_owned());
    }

    let environment = validate_named_environment("cloud-local")?;
    let database_containers = database_containers(&environment);
    let root = &environment.environment.root;
    let services = PROCESS_SERVICES
        .into_iter()
        .map(|name| ServiceStatus {
            name,
            pid: running_pid(&root.join("pids").join(format!("{name}.pid"))),
        })
        .collect();

    Ok(CloudLocalStatus {
        database_containers,
        services,
    })
}

/// Locates the installed cloud component named by the environment's binding.
///
/// A `branch:` binding marks a checkout kept inside the environment; every other binding names a
/// release in the shared install root. Install, uninstall, and build apply the same rule.
fn cloud_directory(environment: &Environment) -> Option<PathBuf> {
    let text = String::from_utf8_lossy(&environment.metadata);
    let version = metadata_get(&text, "cloud_local_cloud_version")?;
    if version.starts_with("branch:") {
        Some(environment.root.join("cloud"))
    } else {
        Some(environment.install_root.join(format!("cloud-{version}")))
    }
}

fn database_containers(environment: &NamedEnvironment) -> Option<Vec<String>> {
    let project = environment.name.as_str();
    let compose = cloud_directory(&environment.environment)?.join("backend/compose.yaml");
    if !compose.is_file() || !database_is_running(project, &compose) {
        return None;
    }

    Some(
        ["db", "minio", "mc"]
            .into_iter()
            .filter_map(|service| container_name(project, service))
            .collect(),
    )
}

fn database_is_running(project: &str, compose: &Path) -> bool {
    Command::new("docker")
        .args(["compose", "--project-name", project, "-f"])
        .arg(compose)
        .args(["ps", "--status", "running", "--quiet", "db"])
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| {
            // `--quiet` prints one container ID per running match and nothing at all otherwise, so
            // any byte other than a line terminator means the database is up.
            output.status.success() && output.stdout.iter().any(|byte| *byte != b'\n')
        })
}

fn container_name(project: &str, service: &str) -> Option<String> {
    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("label=com.docker.compose.project={project}"),
            "--filter",
            &format!("label=com.docker.compose.service={service}"),
            "--format",
            "{{.Names}}",
        ])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_owned();
    (!name.is_empty()).then_some(name)
}
