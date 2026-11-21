//! Cloud-local command modules, service inventory, and read-only results.

mod components;
mod lifecycle;
mod operations;
mod versions;

pub(crate) use lifecycle::{cloud_local_restart, cloud_local_start, cloud_local_stop};
pub(crate) use operations::{cloud_local_install, cloud_local_uninstall, cloud_local_update};
pub(crate) use versions::cloud_local_versions;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::environment::{Environment, NamedEnvironment, validate_named_environment};
use crate::metadata::metadata_get;
use crate::service::{
    ServiceStatus, ensure_project_environment, load_env_exports, running_pid, uv_run_command,
};

pub(crate) struct CloudLocalInfo {
    pub(crate) metadata: String,
}

pub(crate) struct CloudLocalStatus {
    pub(crate) database: DatabaseStatus,
    pub(crate) services: Vec<ServiceStatus>,
}

/// The combined state of the database and object storage services behind the `db` row.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DatabaseStatus {
    /// Every service reported by `oqtopus-cloud-infra status` is running, in reported order.
    Running(Vec<String>),
    /// Only some reported services are running; each keeps its reported state and order.
    PartiallyRunning(Vec<DatabaseService>),
    /// Nothing is running, or the state cannot be determined.
    Stopped,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DatabaseService {
    pub(crate) name: String,
    pub(crate) running: bool,
}

/// Every cloud-local service, in the order the Manager consumes. Startup and shutdown order are
/// separate policies and are named where they differ.
const SERVICES: [&str; 6] = ["db", "worker", "user_signup", "admin", "provider", "user"];

/// The database and object storage run as Docker Compose containers managed by
/// `oqtopus-cloud-infra` instead of a PID-backed process, so status, start, and stop all treat
/// them separately from the rest of the inventory.
const DATABASE: &str = "db";

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
    let database = database_status(&environment);
    let root = &environment.environment.root;
    let services = SERVICES
        .iter()
        .copied()
        .filter(|name| *name != DATABASE)
        .map(|name| ServiceStatus {
            name,
            pid: running_pid(&root.join("pids").join(format!("{name}.pid"))),
        })
        .collect();

    Ok(CloudLocalStatus { database, services })
}

/// Locates the installed cloud component named by the environment's binding.
///
/// A `branch:` binding marks a checkout kept inside the environment; every other binding names a
/// release in the shared install root. Install, uninstall, and build apply the same rule.
fn cloud_directory(environment: &Environment) -> Option<PathBuf> {
    let version = metadata_get(&environment.metadata, "cloud_local_cloud_version")?;
    if version.starts_with("branch:") {
        Some(environment.root.join("cloud"))
    } else {
        Some(environment.install_root.join(format!("cloud-{version}")))
    }
}

/// Asks `oqtopus-cloud-infra status` which database and storage services are running.
///
/// Any failure to obtain an answer (no installed cloud, a missing `.venv`, a release without the
/// command, a missing `uv`, or an exit status other than 0 or 3) is reported as stopped, so
/// `status` always prints every row the Manager expects.
fn database_status(environment: &NamedEnvironment) -> DatabaseStatus {
    let Ok(mut command) = infra_command(environment, "status") else {
        return DatabaseStatus::Stopped;
    };
    let output = command.stdin(Stdio::null()).stderr(Stdio::null()).output();
    let Ok(output) = output else {
        return DatabaseStatus::Stopped;
    };
    // The command follows LSB init-script conventions: 0 when every service is running and 3
    // when some or all are stopped. Any other status means the state is unknown.
    if !matches!(output.status.code(), Some(0 | 3)) {
        return DatabaseStatus::Stopped;
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return DatabaseStatus::Stopped;
    };
    classify_database(stdout.lines().filter_map(parse_infra_status_line).collect())
}

/// Constructs an infra invocation shared by start, stop, and status.
///
/// The compose project is always the environment name, even if config/.env overrides it.
/// A missing Python environment must be detected before uv can create an empty one.
fn infra_command(environment: &NamedEnvironment, action: &str) -> Result<Command, String> {
    let cloud = require_cloud_directory(&environment.environment, DATABASE, action)?;
    ensure_project_environment(&cloud).map_err(|error| format!("cannot {action} 'db': {error}"))?;
    require_cloud_entry(&cloud, "oqtopus-cloud-infra", "db", action)?;
    let backend = cloud.join("backend");
    if !backend.is_dir() {
        return Err(format!(
            "cannot {action} 'db': backend directory not found: {}",
            backend.display()
        ));
    }
    let mut command = uv_run_command(&cloud);
    command
        .args(["oqtopus-cloud-infra", action])
        .current_dir(backend)
        .envs(load_env_exports(
            &environment.environment.root.join("config/.env"),
        ))
        // Removed again after config/.env, which could set it; this only avoids uv's warning.
        .env_remove("VIRTUAL_ENV")
        .env("COMPOSE_PROJECT_NAME", &environment.name);
    Ok(command)
}

/// Like [`cloud_directory`], but reports a missing binding as an error for `action` on `service`.
fn require_cloud_directory(
    environment: &Environment,
    service: &str,
    action: &str,
) -> Result<PathBuf, String> {
    cloud_directory(environment)
        .ok_or_else(|| format!("cannot {action} '{service}': cloud component is not installed."))
}

/// Requires the cloud entry points introduced in v1.8.0.
fn require_cloud_entry(
    cloud: &Path,
    entry: &str,
    service: &str,
    action: &str,
) -> Result<(), String> {
    if fs::read_to_string(cloud.join("pyproject.toml"))
        .is_ok_and(|pyproject| declares_script(&pyproject, entry))
    {
        Ok(())
    } else {
        Err(format!(
            "cannot {action} '{service}': the installed cloud component does not provide {entry}. Update it with 'oqtopus cloud-local update cloud'."
        ))
    }
}

/// Reports whether `entry` is a key of the `[project.scripts]` table.
///
/// Only the table-header form that the cloud's pyproject uses is recognized; a name that appears
/// anywhere else (a comment, a dependency, another table) does not count. Unrecognized TOML
/// spellings, such as an inline `scripts = { ... }` table, are rejected rather than guessed at,
/// which surfaces as the "update cloud" error instead of a failing `uv run`.
/// The input is assumed to be valid TOML, as `uv sync` at install time has already parsed it.
fn declares_script(pyproject: &str, entry: &str) -> bool {
    // A minimal line scan to avoid a TOML dependency for one lookup. Replace it with a real
    // TOML parser if the crate ever depends on one.
    let mut in_scripts = false;
    for line in pyproject.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_scripts = line
                .strip_prefix('[')
                .and_then(|rest| rest.split_once(']'))
                .is_some_and(|(header, _)| {
                    !header.starts_with('[') && header.trim() == "project.scripts"
                });
            continue;
        }
        if !in_scripts {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let key = ['"', '\'']
            .iter()
            .find_map(|quote| key.strip_prefix(*quote)?.strip_suffix(*quote))
            .unwrap_or(key);
        if key == entry {
            return true;
        }
    }
    false
}

/// Parses one `<service>: running|stopped` row. Any other line, such as a diagnostic, is skipped.
fn parse_infra_status_line(line: &str) -> Option<DatabaseService> {
    let (name, state) = line.split_once(':')?;
    let running = match state.trim_start() {
        "running" => true,
        "stopped" => false,
        _ => return None,
    };
    (!name.is_empty()).then(|| DatabaseService {
        name: name.to_owned(),
        running,
    })
}

fn classify_database(services: Vec<DatabaseService>) -> DatabaseStatus {
    if services.is_empty() || services.iter().all(|service| !service.running) {
        DatabaseStatus::Stopped
    } else if services.iter().all(|service| service.running) {
        DatabaseStatus::Running(services.into_iter().map(|service| service.name).collect())
    } else {
        DatabaseStatus::PartiallyRunning(services)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(output: &str) -> DatabaseStatus {
        classify_database(output.lines().filter_map(parse_infra_status_line).collect())
    }

    #[test]
    fn every_running_service_is_named_in_reported_order() {
        assert_eq!(
            parse("db: running\nseaweedfs: running\n"),
            DatabaseStatus::Running(vec!["db".into(), "seaweedfs".into()])
        );
    }

    #[test]
    fn a_mix_keeps_each_reported_state() {
        assert_eq!(
            parse("db: running\nseaweedfs: stopped\n"),
            DatabaseStatus::PartiallyRunning(vec![
                DatabaseService {
                    name: "db".into(),
                    running: true
                },
                DatabaseService {
                    name: "seaweedfs".into(),
                    running: false
                },
            ])
        );
    }

    #[test]
    fn nothing_running_or_nothing_reported_is_stopped() {
        assert_eq!(
            parse("db: stopped\nminio: stopped\n"),
            DatabaseStatus::Stopped
        );
        assert_eq!(parse(""), DatabaseStatus::Stopped);
    }

    #[test]
    fn lines_that_are_not_service_rows_are_ignored() {
        for line in [
            "Starting compose project",
            ": running",
            "db: Running",
            "db: running (healthy)",
            "db: exited",
        ] {
            assert_eq!(parse_infra_status_line(line), None, "{line:?}");
        }
        assert_eq!(
            parse_infra_status_line("db:running"),
            Some(DatabaseService {
                name: "db".into(),
                running: true
            })
        );
    }

    #[test]
    fn script_keys_are_matched_exactly_inside_project_scripts() {
        let pyproject = "[project]\r\nname = \"oqtopus-cloud\"\r\n\r\n[ project.scripts ] # entry points\r\n# oqtopus-cloud-admin-api = \"disabled\"\r\noqtopus-cloud-infra = \"a:main\"\r\n\"oqtopus-cloud-worker\" = \"b:main\"\r\n'oqtopus-cloud-user-api'='c:main'\r\n";
        for entry in [
            "oqtopus-cloud-infra",
            "oqtopus-cloud-worker",
            "oqtopus-cloud-user-api",
        ] {
            assert!(declares_script(pyproject, entry), "{entry}");
        }
        for entry in [
            "oqtopus-cloud-admin-api",
            "oqtopus-cloud",
            "oqtopus-cloud-inf",
        ] {
            assert!(!declares_script(pyproject, entry), "{entry}");
        }
    }

    #[test]
    fn names_outside_project_scripts_are_not_entry_points() {
        for pyproject in [
            "# oqtopus-cloud-infra\n[project]\nname = 'oqtopus-cloud-infra'\n",
            "[project]\ndependencies = [\n  \"oqtopus-cloud-infra-sdk\",\n  \"oqtopus-cloud-infra\",\n]\n",
            "[project.scripts]\nother = 'a:main'\n[tool.uv.sources]\noqtopus-cloud-infra = { path = '.' }\n",
            "[project.gui-scripts]\noqtopus-cloud-infra = 'a:main'\n",
            "[[project.scripts]]\noqtopus-cloud-infra = 'a:main'\n",
            "[project]\nscripts = { oqtopus-cloud-infra = 'a:main' }\n",
        ] {
            assert!(
                !declares_script(pyproject, "oqtopus-cloud-infra"),
                "{pyproject:?}"
            );
        }
    }
}
