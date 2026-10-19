//! Component installation, removal, update, and backend runtime builds.

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::archive::extract_github_archive;
use crate::environment::Environment;
use crate::metadata::{set_metadata_value, unset_metadata_value};
use crate::progress::Reporter;
use crate::remote::{fetch_remote_tags, fetch_url, remote_refs_url, resolve_branch_commit};
use crate::versions::latest_stable;

const ENGINE_PROJECTS: [&str; 4] = ["core", "combiner", "estimator", "mitigator"];

#[derive(Clone, Copy)]
pub(crate) enum OperationKind {
    BackendInstall,
    BackendBuild,
    BackendUninstall,
    BackendUpdate,
    CloudLocalInstall,
    CloudLocalUninstall,
    CloudLocalUpdate,
    ManagerInstall,
    ManagerUninstall,
    ManagerUpdate,
}

pub(crate) struct OperationOutcome {
    pub(crate) usage: Option<OperationKind>,
    exit_code: i32,
}

impl OperationOutcome {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ComponentKind {
    Engine,
    BackendPython,
    CloudPython,
    Static,
    Manager,
}

#[derive(Clone, Copy)]
pub(crate) struct Component {
    pub(crate) name: &'static str,
    pub(crate) repository: &'static str,
    pub(crate) binding_key: &'static str,
    pub(crate) kind: ComponentKind,
}

/// Installs an explicitly requested version, which may name a branch instead of a release.
///
/// The `update` commands deliberately call [`install_release`] directly rather than going through
/// here: update means "move to the latest release", so it never accepts or preserves a branch.
pub(crate) fn install_version<W: Write>(
    environment: &Environment,
    component: Component,
    version: Option<&str>,
    skip_sse_build: bool,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    let version = version.filter(|version| !version.is_empty());
    if let Some(branch) = version.and_then(|version| version.strip_prefix("branch:")) {
        if branch.is_empty() {
            return Err("invalid branch version format: branch: (expected branch:<branch>)".into());
        }
        install_branch(environment, component, branch, skip_sse_build, reporter)
    } else {
        install_release(environment, component, version, skip_sse_build, reporter)
    }
}

pub(crate) fn install_release<W: Write>(
    environment: &Environment,
    component: Component,
    requested_version: Option<&str>,
    skip_sse_build: bool,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    ensure_command("uv")?;
    let version = match requested_version.filter(|version| !version.is_empty()) {
        Some(version) => version.to_owned(),
        None => resolve_latest(component.repository)?,
    };
    fs::create_dir_all(&environment.install_root)
        .map_err(|error| format!("failed to create install root: {error}"))?;
    let target = environment
        .install_root
        .join(format!("{}-{version}", component.name));

    if component_complete(component.kind, &target) {
        progress(
            reporter,
            format!(
                "{} {version} is already installed; reusing {}",
                component.name,
                target.display()
            ),
        )?;
    } else {
        if path_exists(&target) {
            progress(
                reporter,
                format!("Removing incomplete installation: {}", target.display()),
            )?;
            remove_path(&target).map_err(|error| {
                format!(
                    "failed to remove incomplete installation {}: {error}",
                    target.display()
                )
            })?;
        }
        progress(reporter, format!("Installing {} {version}", component.name))?;
        download_release(component, &version, &target)?;
        sync_component(component, &version, &target, reporter)?;
    }
    finish_install(
        environment,
        component,
        &version,
        &target,
        skip_sse_build,
        format!("Skipping sse_runtime Docker image build for engine {version}."),
        reporter,
    )
}

fn install_branch<W: Write>(
    environment: &Environment,
    component: Component,
    branch: &str,
    skip_sse_build: bool,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    ensure_command("uv")?;
    let target = environment.root.join(component.name);
    if path_exists(&target) {
        progress(
            reporter,
            format!(
                "Removing existing branch installation: {}",
                target.display()
            ),
        )?;
        remove_path(&target).map_err(|error| {
            format!(
                "failed to remove existing branch installation {}: {error}",
                target.display()
            )
        })?;
    }
    progress(
        reporter,
        format!("Downloading {} from branch '{branch}'", component.name),
    )?;
    let sha = resolve_branch_commit(component.repository, branch).map_err(|_| {
        format!(
            "failed to resolve branch '{branch}' for {}.",
            component.name
        )
    })?;
    let url = format!(
        "https://github.com/{}/archive/{sha}.tar.gz",
        component.repository
    );
    let archive = fetch_url(&url).map_err(|_| {
        format!(
            "failed to download {} from branch '{branch}'.",
            component.name
        )
    })?;
    extract_github_archive(&archive, &target).map_err(|_| {
        format!(
            "failed to download {} from branch '{branch}'.",
            component.name
        )
    })?;
    let version = format!("branch:{branch}");
    sync_component(component, &version, &target, reporter)?;
    finish_install(
        environment,
        component,
        &version,
        &target,
        skip_sse_build,
        format!("Skipping sse_runtime Docker image build for engine branch '{branch}'."),
        reporter,
    )
}

fn finish_install<W: Write>(
    environment: &Environment,
    component: Component,
    version: &str,
    target: &Path,
    skip_sse_build: bool,
    skip_build_message: String,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    if component.kind == ComponentKind::Engine {
        if skip_sse_build {
            progress(reporter, skip_build_message)?;
        } else {
            build_sse_runtime(environment, target, reporter)?;
        }
    }
    set_binding(environment, component.binding_key, version)?;
    progress(
        reporter,
        format!("Bound {}={version}", component.binding_key),
    )
}

/// Removes one installed component, and for a branch checkout its metadata binding as well.
///
/// The asymmetry is deliberate and matches the legacy CLI. Releases live in a shared install root
/// and several environments may be bound to the same one, so removing a release here says nothing
/// about what this environment should point at. A branch checkout belongs to this environment
/// alone, so removing it leaves the binding naming a directory that no longer exists.
pub(crate) fn uninstall<W: Write>(
    environment: &Environment,
    component: Component,
    version: &str,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    let branch = version.starts_with("branch:");
    let target = if branch {
        environment.root.join(component.name)
    } else {
        environment
            .install_root
            .join(format!("{}-{version}", component.name))
    };
    if !target.is_dir() {
        let kind = if branch {
            "branch install not found"
        } else {
            "installed release not found"
        };
        return Err(format!("{kind}: {}", target.display()));
    }
    remove_path(&target)
        .map_err(|error| format!("failed to remove {}: {error}", target.display()))?;
    if branch {
        unset_metadata_value(&environment.root.join(".metadata"), component.binding_key)
            .map_err(|error| format!("failed to update .metadata: {error}"))?;
    }
    progress(reporter, format!("Removed {}", target.display()))
}

fn resolve_latest(repository: &str) -> Result<String, String> {
    let tags = fetch_remote_tags(repository).map_err(|_| {
        format!(
            "failed to query GitHub tags API: {}",
            remote_refs_url(repository)
        )
    })?;
    latest_stable(&tags).ok_or_else(|| {
        format!("could not resolve a latest stable version from GitHub tags for {repository}.")
    })
}

fn download_release(component: Component, version: &str, target: &Path) -> Result<(), String> {
    let url = format!(
        "https://github.com/{}/archive/refs/tags/{version}.tar.gz",
        component.repository
    );
    let archive = fetch_url(&url).map_err(|_| {
        format!(
            "failed to download {} {version} from {url}.",
            component.name
        )
    })?;
    extract_github_archive(&archive, target)
        .map_err(|_| format!("failed to extract {} {version}.", component.name))
}

fn sync_component<W: Write>(
    component: Component,
    version: &str,
    target: &Path,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    match component.kind {
        ComponentKind::Engine => {
            for project in ENGINE_PROJECTS {
                let project_dir = target.join(project);
                if !project_dir.join("pyproject.toml").is_file() {
                    return Err(format!(
                        "engine {version} is missing {project}/pyproject.toml."
                    ));
                }
                run_uv(&project_dir, reporter).map_err(|_| {
                    format!("failed to synchronize engine {version} project '{project}' with uv.")
                })?;
            }
        }
        ComponentKind::CloudPython => {
            if !target.join("pyproject.toml").is_file() {
                return Err(format!("cloud {version} is missing pyproject.toml."));
            }
            run_uv(target, reporter)
                .map_err(|_| format!("failed to synchronize cloud {version} with uv."))?;
        }
        ComponentKind::BackendPython | ComponentKind::Manager => {
            run_uv(target, reporter).map_err(|_| {
                format!(
                    "failed to synchronize {} {version} with uv.",
                    component.name
                )
            })?;
        }
        ComponentKind::Static => {}
    }
    Ok(())
}

fn run_uv<W: Write>(target: &Path, reporter: &mut Reporter<'_, W>) -> Result<(), ()> {
    reporter.flush().map_err(|_| ())?;
    let status = Command::new("uv")
        .args(["sync", "--frozen", "--no-dev", "--project"])
        .arg(target)
        .status()
        .map_err(|_| ())?;
    status.success().then_some(()).ok_or(())
}

pub(crate) fn build_sse_runtime<W: Write>(
    environment: &Environment,
    engine: &Path,
    reporter: &mut Reporter<'_, W>,
) -> Result<(), String> {
    ensure_command("docker")?;
    let runtime = engine.join("sse_runtime");
    let dockerfile = runtime.join("Dockerfile");
    if !dockerfile.is_file() {
        return Err(format!(
            "sse_runtime Dockerfile not found: {}",
            dockerfile.display()
        ));
    }
    let image = load_config_value(&environment.root.join("config/.env"), "SSE_CONTAINER_IMAGE")
        .ok_or("SSE_CONTAINER_IMAGE is missing from config/.env.")?;
    // Match `id -u` and `id -g` without adding an external command dependency.
    // SAFETY: both calls take no arguments, always succeed, and return a plain integer.
    let uid = unsafe { libc::getuid() };
    // SAFETY: as above.
    let gid = unsafe { libc::getgid() };
    progress(
        reporter,
        format!("Building sse_runtime Docker image as {image}"),
    )?;
    reporter
        .flush()
        .map_err(|error| format!("failed to write progress: {error}"))?;
    let status = Command::new("docker")
        .arg("build")
        .arg(&runtime)
        .args([
            "-t",
            &image,
            "--build-arg",
            &format!("UID={uid}"),
            "--build-arg",
            &format!("GID={gid}"),
        ])
        .status()
        .map_err(|_| "failed to build sse_runtime Docker image.".to_owned())?;
    if !status.success() {
        return Err("failed to build sse_runtime Docker image.".into());
    }
    progress(reporter, format!("Built sse_runtime Docker image: {image}"))
}

/// Reads one `KEY=value` setting from an environment's `config/.env`.
///
/// Deliberately matches the legacy reader rather than a general dotenv parser: leading whitespace
/// only marks a line as blank, it is never stripped from the key, so an indented assignment does
/// not match. The value keeps everything after the first '=' with one layer of surrounding quotes
/// removed.
fn load_config_value(path: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    contents.lines().find_map(|line| {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        let (candidate, value) = line.split_once('=')?;
        if candidate != key {
            return None;
        }
        let value = value.strip_prefix(['\'', '"']).unwrap_or(value);
        let value = value.strip_suffix(['\'', '"']).unwrap_or(value);
        Some(value.to_owned())
    })
}

/// Reports whether an existing directory holds a finished installation.
///
/// An interrupted install leaves a downloaded but unsynchronized tree behind, so the marker is the
/// product of the synchronization step: the virtual environment `uv sync` creates, one per project
/// for the multi-project engine. A static component runs no synchronization step, so extracting it
/// is all there is to complete.
pub(crate) fn component_complete(kind: ComponentKind, target: &Path) -> bool {
    match kind {
        ComponentKind::Engine => ENGINE_PROJECTS
            .iter()
            .all(|project| target.join(project).join(".venv").is_dir()),
        ComponentKind::Static => target.is_dir(),
        ComponentKind::BackendPython | ComponentKind::CloudPython | ComponentKind::Manager => {
            target.join(".venv").is_dir()
        }
    }
}

pub(crate) fn find_component(
    components: &'static [Component],
    name: &str,
) -> Result<Component, String> {
    components
        .iter()
        .copied()
        .find(|component| component.name == name)
        .ok_or_else(|| format!("unknown component: {name}"))
}

fn set_binding(environment: &Environment, key: &str, value: &str) -> Result<(), String> {
    set_metadata_value(&environment.root.join(".metadata"), key, value)
        .map_err(|error| format!("failed to update .metadata: {error}"))
}

fn ensure_command(name: &str) -> Result<(), String> {
    let found = env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|directory| {
            let candidate = directory.join(name);
            candidate.metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
    });
    found
        .then_some(())
        .ok_or_else(|| format!("'{name}' is required but was not found on PATH."))
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
}

fn progress<W: Write>(
    reporter: &mut Reporter<'_, W>,
    message: impl std::fmt::Display,
) -> Result<(), String> {
    reporter
        .line(message)
        .map_err(|error| format!("failed to write progress: {error}"))
}

pub(crate) fn success() -> OperationOutcome {
    OperationOutcome {
        usage: None,
        exit_code: 0,
    }
}

pub(crate) fn usage(kind: OperationKind, exit_code: i32) -> OperationOutcome {
    OperationOutcome {
        usage: Some(kind),
        exit_code,
    }
}

#[cfg(test)]
mod tests {
    use super::load_config_value;
    use std::fs;

    #[test]
    fn config_values_match_the_legacy_env_reader() {
        let directory = tempfile::tempdir().expect("create config fixture");
        let path = directory.path().join(".env");
        fs::write(
            &path,
            "# ignored\nEMPTY=\nDOUBLE=\"image:1\"\nSINGLE='image:2'\nSEPARATORS=a=b=c\n   INDENTED=no\n",
        )
        .expect("write config fixture");

        assert_eq!(load_config_value(&path, "EMPTY").as_deref(), Some(""));
        assert_eq!(
            load_config_value(&path, "DOUBLE").as_deref(),
            Some("image:1")
        );
        assert_eq!(
            load_config_value(&path, "SINGLE").as_deref(),
            Some("image:2")
        );
        assert_eq!(
            load_config_value(&path, "SEPARATORS").as_deref(),
            Some("a=b=c")
        );
        assert_eq!(load_config_value(&path, "INDENTED"), None);
    }
}
