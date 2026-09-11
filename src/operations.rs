//! Component installation, removal, update, and backend runtime builds.

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::archive::extract_github_archive;
use crate::environment::{Environment, validate_environment};
use crate::metadata::{metadata_get, set_metadata_value, unset_metadata_value};
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

pub(crate) enum OperationOutput {
    None,
    Usage(OperationKind),
}

pub(crate) struct OperationResult {
    pub(crate) output: OperationOutput,
    exit_code: i32,
}

impl OperationResult {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ComponentKind {
    Engine,
    BackendPython,
    CloudPython,
    Static,
    Manager,
}

#[derive(Clone, Copy)]
struct Component {
    name: &'static str,
    repository: &'static str,
    binding_key: &'static str,
    kind: ComponentKind,
}

const BACKEND_COMPONENTS: [Component; 3] = [
    Component {
        name: "engine",
        repository: "oqtopus-team/oqtopus-engine",
        binding_key: "engine_version",
        kind: ComponentKind::Engine,
    },
    Component {
        name: "tranqu",
        repository: "oqtopus-team/tranqu-server",
        binding_key: "tranqu_version",
        kind: ComponentKind::BackendPython,
    },
    Component {
        name: "gateway",
        repository: "oqtopus-team/device-gateway",
        binding_key: "gateway_version",
        kind: ComponentKind::BackendPython,
    },
];

const CLOUD_LOCAL_COMPONENTS: [Component; 3] = [
    Component {
        name: "cloud",
        repository: "oqtopus-team/oqtopus-cloud",
        binding_key: "cloud_local_cloud_version",
        kind: ComponentKind::CloudPython,
    },
    Component {
        name: "frontend",
        repository: "oqtopus-team/oqtopus-frontend",
        binding_key: "cloud_local_frontend_version",
        kind: ComponentKind::Static,
    },
    Component {
        name: "admin",
        repository: "oqtopus-team/oqtopus-admin",
        binding_key: "cloud_local_admin_version",
        kind: ComponentKind::Static,
    },
];

const MANAGER_COMPONENT: Component = Component {
    name: "manager",
    repository: "oqtopus-team/oqtopus-manager",
    binding_key: "manager_version",
    kind: ComponentKind::Manager,
};

pub(crate) fn backend_install<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::BackendInstall, 0));
    }
    let environment = validate_environment("backend")?;
    let Some(component_name) = args.first() else {
        return Ok(usage(OperationKind::BackendInstall, 1));
    };
    let mut version = "";
    let mut skip_sse_build = false;
    for arg in &args[1..] {
        if arg == "--skip-sse-build" {
            skip_sse_build = true;
        } else if arg.starts_with('-') {
            return Err(format!("unknown install option: {arg}"));
        } else if !version.is_empty() {
            return Ok(usage(OperationKind::BackendInstall, 1));
        } else {
            version = arg;
        }
    }

    let mut reporter = Reporter::new(out);
    if component_name == "all" {
        if !version.is_empty() {
            return Err("oqtopus backend install all does not accept a version argument.".into());
        }
        for component in BACKEND_COMPONENTS {
            install_release(&environment, component, None, skip_sse_build, &mut reporter)?;
        }
        return Ok(success());
    }
    if skip_sse_build && component_name != "engine" {
        return Err("--skip-sse-build is only supported for 'engine' and 'all'.".into());
    }
    let component = find_component(&BACKEND_COMPONENTS, component_name)?;
    install_version(
        &environment,
        component,
        (!version.is_empty()).then_some(version),
        skip_sse_build,
        &mut reporter,
    )?;
    Ok(success())
}

pub(crate) fn backend_uninstall<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::BackendUninstall, 0));
    }
    let environment = validate_environment("backend")?;
    if args.len() != 2 || args[0].is_empty() || args[1].is_empty() {
        return Ok(usage(OperationKind::BackendUninstall, 1));
    }
    let component = find_component(&BACKEND_COMPONENTS, &args[0])?;
    let mut reporter = Reporter::new(out);
    uninstall(&environment, component, &args[1], &mut reporter)?;
    Ok(success())
}

pub(crate) fn backend_update<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::BackendUpdate, 0));
    }
    let environment = validate_environment("backend")?;
    if args.len() != 1 || args[0].is_empty() {
        return Ok(usage(OperationKind::BackendUpdate, 1));
    }
    let component = find_component(&BACKEND_COMPONENTS, &args[0])?;
    install_release(
        &environment,
        component,
        None,
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}

pub(crate) fn backend_build<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::BackendBuild, 0));
    }
    let environment = validate_environment("backend")?;
    if args.len() != 1 || args[0] != "sse-runtime" {
        return Ok(usage(OperationKind::BackendBuild, 1));
    }
    let metadata = String::from_utf8_lossy(&environment.metadata);
    let version = metadata_get(&metadata, "engine_version")
        .ok_or("engine is not installed in this backend environment.")?;
    let target = if version.starts_with("branch:") {
        environment.root.join("engine")
    } else {
        environment.install_root.join(format!("engine-{version}"))
    };
    if !target.is_dir() {
        return Err(format!(
            "installed engine release not found: {}",
            target.display()
        ));
    }
    if !component_complete(ComponentKind::Engine, &target) {
        return Err(format!(
            "engine {version} is not completely installed in this backend environment."
        ));
    }
    build_sse_runtime(&environment, &target, &mut Reporter::new(out))?;
    Ok(success())
}

pub(crate) fn cloud_local_install<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::CloudLocalInstall, 0));
    }
    let environment = validate_environment("cloud-local")?;
    let Some(component_name) = args.first() else {
        return Ok(usage(OperationKind::CloudLocalInstall, 1));
    };
    let mut version = "";
    for arg in &args[1..] {
        if arg.starts_with('-') {
            return Err(format!("unknown install option: {arg}"));
        } else if !version.is_empty() {
            return Ok(usage(OperationKind::CloudLocalInstall, 1));
        } else {
            version = arg;
        }
    }
    let mut reporter = Reporter::new(out);
    if component_name == "all" {
        if !version.is_empty() {
            return Err(
                "oqtopus cloud-local install all does not accept a version argument.".into(),
            );
        }
        for component in CLOUD_LOCAL_COMPONENTS {
            install_release(&environment, component, None, false, &mut reporter)?;
        }
        return Ok(success());
    }
    let component = find_component(&CLOUD_LOCAL_COMPONENTS, component_name)?;
    install_version(
        &environment,
        component,
        (!version.is_empty()).then_some(version),
        false,
        &mut reporter,
    )?;
    Ok(success())
}

pub(crate) fn cloud_local_uninstall<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::CloudLocalUninstall, 0));
    }
    let environment = validate_environment("cloud-local")?;
    if args.len() != 2 || args[0].is_empty() || args[1].is_empty() {
        return Ok(usage(OperationKind::CloudLocalUninstall, 1));
    }
    let component = find_component(&CLOUD_LOCAL_COMPONENTS, &args[0])?;
    uninstall(&environment, component, &args[1], &mut Reporter::new(out))?;
    Ok(success())
}

pub(crate) fn cloud_local_update<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::CloudLocalUpdate, 0));
    }
    let environment = validate_environment("cloud-local")?;
    if args.len() != 1 || args[0].is_empty() {
        return Ok(usage(OperationKind::CloudLocalUpdate, 1));
    }
    let component = find_component(&CLOUD_LOCAL_COMPONENTS, &args[0])?;
    install_release(
        &environment,
        component,
        None,
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}

pub(crate) fn manager_install<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::ManagerInstall, 0));
    }
    let environment = validate_environment("manager")?;
    let mut version = "";
    for arg in args {
        if arg.starts_with('-') {
            return Err(format!("unknown install option: {arg}"));
        } else if !version.is_empty() {
            return Ok(usage(OperationKind::ManagerInstall, 1));
        } else {
            version = arg;
        }
    }
    install_version(
        &environment,
        MANAGER_COMPONENT,
        (!version.is_empty()).then_some(version),
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}

pub(crate) fn manager_uninstall<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::ManagerUninstall, 0));
    }
    let environment = validate_environment("manager")?;
    if args.len() != 1 || args[0].is_empty() {
        return Ok(usage(OperationKind::ManagerUninstall, 1));
    }
    uninstall(
        &environment,
        MANAGER_COMPONENT,
        &args[0],
        &mut Reporter::new(out),
    )?;
    Ok(success())
}

pub(crate) fn manager_update<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationResult, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::ManagerUpdate, 0));
    }
    let environment = validate_environment("manager")?;
    if !args.is_empty() {
        return Ok(usage(OperationKind::ManagerUpdate, 1));
    }
    install_release(
        &environment,
        MANAGER_COMPONENT,
        None,
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}

fn install_version<W: Write>(
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

fn install_release<W: Write>(
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
    if component.kind == ComponentKind::Engine {
        if skip_sse_build {
            progress(
                reporter,
                format!("Skipping sse_runtime Docker image build for engine {version}."),
            )?;
        } else {
            build_sse_runtime(environment, &target, reporter)?;
        }
    }
    set_binding(environment, component.binding_key, &version)?;
    progress(
        reporter,
        format!("Bound {}={version}", component.binding_key),
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
    if component.kind == ComponentKind::Engine {
        if skip_sse_build {
            progress(
                reporter,
                format!("Skipping sse_runtime Docker image build for engine branch '{branch}'."),
            )?;
        } else {
            build_sse_runtime(environment, &target, reporter)?;
        }
    }
    set_binding(environment, component.binding_key, &version)?;
    progress(
        reporter,
        format!("Bound {}={version}", component.binding_key),
    )
}

fn uninstall<W: Write>(
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

fn build_sse_runtime<W: Write>(
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
    let uid = unsafe { libc::getuid() };
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

fn component_complete(kind: ComponentKind, target: &Path) -> bool {
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

fn find_component(components: &'static [Component], name: &str) -> Result<Component, String> {
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

fn is_help(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "--help"))
}

fn progress<W: Write>(
    reporter: &mut Reporter<'_, W>,
    message: impl std::fmt::Display,
) -> Result<(), String> {
    reporter
        .line(message)
        .map_err(|error| format!("failed to write progress: {error}"))
}

fn success() -> OperationResult {
    OperationResult {
        output: OperationOutput::None,
        exit_code: 0,
    }
}

fn usage(kind: OperationKind, exit_code: i32) -> OperationResult {
    OperationResult {
        output: OperationOutput::Usage(kind),
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
