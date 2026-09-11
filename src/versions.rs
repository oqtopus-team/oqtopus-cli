//! Remote and locally installed component version discovery.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use crate::metadata::metadata_get;
use crate::remote::{fetch_remote_tags, remote_refs_url};

#[derive(Clone, Copy)]
pub(crate) enum VersionsKind {
    Backend,
    CloudLocal,
    Manager,
}

pub(crate) struct VersionEntry {
    pub(crate) tag: String,
    pub(crate) current: bool,
    pub(crate) installed: bool,
    pub(crate) remote: bool,
}

pub(crate) struct VersionList {
    pub(crate) component: &'static str,
    pub(crate) entries: Vec<VersionEntry>,
}

pub(crate) enum VersionsOutput {
    Usage(VersionsKind),
    List(VersionList),
}

pub(crate) struct VersionsResult {
    pub(crate) output: VersionsOutput,
    exit_code: i32,
}

impl VersionsResult {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

pub(crate) fn backend_versions(args: &[String]) -> Result<VersionsResult, String> {
    if is_help(args) {
        return Ok(usage(VersionsKind::Backend, 0));
    }
    if args.len() != 1 {
        return Ok(usage(VersionsKind::Backend, 1));
    }

    let (component, repository, binding_key) = match args[0].as_str() {
        "engine" => ("engine", "oqtopus-team/oqtopus-engine", "engine_version"),
        "tranqu" => ("tranqu", "oqtopus-team/tranqu-server", "tranqu_version"),
        "gateway" => ("gateway", "oqtopus-team/device-gateway", "gateway_version"),
        component => return Err(format!("unknown component: {component}")),
    };

    list_versions(
        component,
        repository,
        binding_key,
        OptionalEnvironment::load("backend", false),
    )
}

pub(crate) fn cloud_local_versions(args: &[String]) -> Result<VersionsResult, String> {
    if is_help(args) {
        return Ok(usage(VersionsKind::CloudLocal, 0));
    }
    if args.len() != 1 {
        return Ok(usage(VersionsKind::CloudLocal, 1));
    }

    let (component, repository, binding_key) = match args[0].as_str() {
        "cloud" => (
            "cloud",
            "oqtopus-team/oqtopus-cloud",
            "cloud_local_cloud_version",
        ),
        "frontend" => (
            "frontend",
            "oqtopus-team/oqtopus-frontend",
            "cloud_local_frontend_version",
        ),
        "admin" => (
            "admin",
            "oqtopus-team/oqtopus-admin",
            "cloud_local_admin_version",
        ),
        component => return Err(format!("unknown component: {component}")),
    };

    list_versions(
        component,
        repository,
        binding_key,
        OptionalEnvironment::load("cloud-local", true),
    )
}

pub(crate) fn manager_versions(args: &[String]) -> Result<VersionsResult, String> {
    if is_help(args) {
        return Ok(usage(VersionsKind::Manager, 0));
    }
    if !args.is_empty() {
        return Ok(usage(VersionsKind::Manager, 1));
    }

    list_versions(
        "manager",
        "oqtopus-team/oqtopus-manager",
        "manager_version",
        OptionalEnvironment::load("manager", false),
    )
}

fn is_help(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "--help"))
}

fn usage(kind: VersionsKind, exit_code: i32) -> VersionsResult {
    VersionsResult {
        output: VersionsOutput::Usage(kind),
        exit_code,
    }
}

fn list_versions(
    component: &'static str,
    repository: &str,
    binding_key: &str,
    environment: Option<OptionalEnvironment>,
) -> Result<VersionsResult, String> {
    let canonical_url = remote_refs_url(repository);
    let advertised_tags = fetch_remote_tags(repository)
        .map_err(|_| format!("failed to query GitHub tags API: {canonical_url}"))?;
    let remote_tags: Vec<_> = advertised_tags
        .into_iter()
        .filter(|tag| parse_stable(tag).is_some())
        .collect();
    if remote_tags.is_empty() {
        return Err(format!("no stable versions found for {component}."));
    }

    let current = environment
        .as_ref()
        .and_then(|environment| metadata_get(&environment.metadata, binding_key))
        .filter(|version| !version.is_empty())
        .map(str::to_owned);
    let installed = environment.as_ref().map_or_else(Vec::new, |environment| {
        installed_versions(&environment.install_root, component)
    });

    let mut tags = remote_tags.clone();
    if let Some(current) = &current {
        push_unique(&mut tags, current);
    }
    for version in &installed {
        push_unique(&mut tags, version);
    }
    tags.sort_by(|left, right| compare_tags(left, right).reverse());

    let entries = tags
        .into_iter()
        .map(|tag| VersionEntry {
            current: current.as_deref() == Some(tag.as_str()),
            installed: tag.starts_with("branch:") || installed.contains(&tag),
            remote: remote_tags.contains(&tag),
            tag,
        })
        .collect();

    Ok(VersionsResult {
        output: VersionsOutput::List(VersionList { component, entries }),
        exit_code: 0,
    })
}

fn installed_versions(install_root: &Path, component: &str) -> Vec<String> {
    let prefix = format!("{component}-");
    let Ok(entries) = fs::read_dir(install_root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_prefix(&prefix).map(str::to_owned))
        .filter(|version| version.starts_with('v') && !version.is_empty())
        .collect()
}

fn push_unique(tags: &mut Vec<String>, tag: &str) {
    if !tags.iter().any(|candidate| candidate == tag) {
        tags.push(tag.to_owned());
    }
}

fn compare_tags(left: &str, right: &str) -> Ordering {
    let left_branch = left.starts_with("branch:");
    let right_branch = right.starts_with("branch:");
    left_branch
        .cmp(&right_branch)
        .then_with(|| match (parse_stable(left), parse_stable(right)) {
            (Some(left), Some(right)) => compare_version_parts(left, right),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| left.cmp(right))
}

fn parse_stable(tag: &str) -> Option<[&str; 3]> {
    let mut parts = tag.strip_prefix('v')?.split('.');
    let version = [parts.next()?, parts.next()?, parts.next()?];
    if parts.next().is_some()
        || version
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(version)
}

pub(crate) fn latest_stable(tags: &[String]) -> Option<String> {
    tags.iter()
        .filter(|tag| parse_stable(tag).is_some())
        .max_by(|left, right| compare_tags(left, right))
        .cloned()
}

fn compare_version_parts(left: [&str; 3], right: [&str; 3]) -> Ordering {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| compare_decimal(left, right))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

fn compare_decimal(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

struct OptionalEnvironment {
    metadata: String,
    install_root: PathBuf,
}

impl OptionalEnvironment {
    /// Loads context only when every field used by the legacy `try_load_*_env` helper is valid.
    fn load(template_name: &str, require_name: bool) -> Option<Self> {
        let path = Path::new(".metadata");
        if !path.is_file() {
            return None;
        }
        let metadata = String::from_utf8_lossy(&fs::read(path).ok()?).into_owned();
        if metadata_get(&metadata, "template")? != template_name {
            return None;
        }
        let root = metadata_get(&metadata, "environment_root")
            .or_else(|| metadata_get(&metadata, "env_root"))?;
        if require_name
            && metadata_get(&metadata, "environment_name")
                .or_else(|| metadata_get(&metadata, "env_name"))
                .is_none()
        {
            return None;
        }
        let install_root = PathBuf::from(metadata_get(&metadata, "install_root")?);
        let current = fs::canonicalize(".").ok()?;
        if root != current.to_string_lossy() {
            return None;
        }

        Some(Self {
            metadata,
            install_root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::compare_tags;

    #[test]
    fn orders_branches_then_semver_without_integer_overflow() {
        let mut tags = [
            "v2.0.0",
            "v10.0.0",
            "branch:develop",
            "v999999999999999999999999.0.0",
            "custom",
        ];
        tags.sort_by(|left, right| compare_tags(left, right).reverse());

        assert_eq!(
            tags,
            [
                "branch:develop",
                "v999999999999999999999999.0.0",
                "v10.0.0",
                "v2.0.0",
                "custom",
            ]
        );
    }
}
