//! Native `init` command and environment template extraction.

use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;

use crate::remote::fetch_url;

const DEFAULT_TEMPLATE_BRANCH: &str = "main";
const FORBID_LEGACY_FALLBACK: &str = "OQTOPUS_FORBID_LEGACY_FALLBACK";

#[derive(Clone, Copy)]
pub(crate) enum EnvironmentTemplate {
    Backend,
    CloudLocal,
    Manager,
}

impl EnvironmentTemplate {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::CloudLocal => "cloud-local",
            Self::Manager => "manager",
        }
    }
}

pub(crate) enum InitOutput {
    Usage,
    Created {
        template: EnvironmentTemplate,
        root: PathBuf,
    },
}

pub(crate) struct InitResult {
    pub(crate) output: InitOutput,
    exit_code: i32,
}

impl InitResult {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

pub(crate) fn init(args: &[String]) -> Result<InitResult, String> {
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "--help"))
    {
        return Ok(usage(0));
    }
    let Some(environment_name) = args.first() else {
        return Ok(usage(1));
    };
    if environment_name.is_empty() {
        return Ok(usage(1));
    }

    let mut template = None;
    let mut branch = DEFAULT_TEMPLATE_BRANCH;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--template" if index + 1 < args.len() => {
                template = Some(args[index + 1].as_str());
                index += 2;
            }
            "--branch" if index + 1 < args.len() => {
                branch = &args[index + 1];
                index += 2;
            }
            _ => return Ok(usage(1)),
        }
    }
    let Some(template_name) = template.filter(|template| !template.is_empty()) else {
        return Ok(usage(1));
    };

    validate_environment_name(environment_name)?;
    let target = Path::new(environment_name);
    if target.exists() {
        return Err(format!(
            "target directory already exists: {environment_name}"
        ));
    }
    let template = match template_name {
        "backend" => EnvironmentTemplate::Backend,
        "cloud-local" => EnvironmentTemplate::CloudLocal,
        "manager" => EnvironmentTemplate::Manager,
        _ => return Err(format!("template '{template_name}' is not implemented.")),
    };

    create_environment(target, environment_name, template, branch)
}

fn usage(exit_code: i32) -> InitResult {
    InitResult {
        output: InitOutput::Usage,
        exit_code,
    }
}

fn validate_environment_name(name: &str) -> Result<(), String> {
    let mut bytes = name.bytes();
    let valid = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid env_name '{name}'.\nenv_name must match: ^[a-z0-9][a-z0-9_.-]*$\nUse lowercase letters, digits, '.', '_' or '-', and start with a letter or digit."
        ))
    }
}

fn create_environment(
    target: &Path,
    environment_name: &str,
    template: EnvironmentTemplate,
    branch: &str,
) -> Result<InitResult, String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("failed to create environment directory: {error}"))?;
    let root = fs::canonicalize(target)
        .map_err(|error| format!("failed to resolve environment directory: {error}"))?;

    download_template(&root, template, branch)?;
    if matches!(template, EnvironmentTemplate::Backend) {
        render_backend_environment(environment_name, &root.join("config/.env"))?;
    }
    create_runtime_directories(&root, template)?;

    let install_root = install_root(template)?;
    let created_at = created_at()?;
    let metadata = format!(
        "template={}\ninstall_root={}\nenvironment_name={environment_name}\nenvironment_root={}\ncreated_at={created_at}\n",
        template.name(),
        install_root.display(),
        root.display(),
    );
    fs::write(root.join(".metadata"), metadata)
        .map_err(|error| format!("failed to write environment metadata: {error}"))?;

    Ok(InitResult {
        output: InitOutput::Created { template, root },
        exit_code: 0,
    })
}

fn download_template(
    target: &Path,
    template: EnvironmentTemplate,
    branch: &str,
) -> Result<(), String> {
    let url =
        format!("https://github.com/oqtopus-team/oqtopus-cli/archive/refs/heads/{branch}.tar.gz");
    let archive = fetch_url(&url).map_err(|_| {
        format!(
            "failed to download {} template archive (branch '{branch}' may not exist).",
            template.name()
        )
    })?;
    let temporary = tempfile::tempdir()
        .map_err(|error| format!("failed to create temporary directory: {error}"))?;
    tar::Archive::new(GzDecoder::new(Cursor::new(archive)))
        .unpack(temporary.path())
        .map_err(|_| format!("failed to extract {} template archive.", template.name()))?;
    let source = find_template_directory(temporary.path(), template.name()).ok_or_else(|| {
        format!(
            "templates/{} was not found in the downloaded archive.",
            template.name()
        )
    })?;
    copy_directory_contents(&source, target)
        .map_err(|error| format!("failed to copy {} template: {error}", template.name()))
}

fn find_template_directory(root: &Path, template_name: &str) -> Option<PathBuf> {
    let suffix = Path::new("templates").join(template_name);
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        let mut entries: Vec<_> = fs::read_dir(&directory)
            .ok()?
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if !entry.file_type().ok()?.is_dir() {
                continue;
            }
            if path.ends_with(&suffix) {
                return Some(path);
            }
            pending.push(path);
        }
    }
    None
}

fn copy_directory_contents(source: &Path, target: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&target_path)?;
            copy_directory_contents(&source_path, &target_path)?;
        } else if file_type.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(&source_path)?, target_path)?;
        } else if file_type.is_file() {
            fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

fn render_backend_environment(environment_name: &str, path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let contents = fs::read(path)
        .map_err(|error| format!("failed to read backend environment template: {error}"))?;
    let rendered = replace_all(&contents, b"{{ env_name }}", environment_name.as_bytes());
    fs::write(path, rendered)
        .map_err(|error| format!("failed to render backend environment template: {error}"))
}

fn replace_all(contents: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut rendered = Vec::with_capacity(contents.len());
    let mut remaining = contents;
    while let Some(index) = remaining
        .windows(needle.len())
        .position(|window| window == needle)
    {
        rendered.extend_from_slice(&remaining[..index]);
        rendered.extend_from_slice(replacement);
        remaining = &remaining[index + needle.len()..];
    }
    rendered.extend_from_slice(remaining);
    rendered
}

fn create_runtime_directories(root: &Path, template: EnvironmentTemplate) -> Result<(), String> {
    let directories: &[&str] = match template {
        EnvironmentTemplate::Backend => &[
            "pids",
            "sse_work",
            "logs/core",
            "logs/sse_engine",
            "logs/mitigator",
            "logs/estimator",
            "logs/combiner",
            "logs/tranqu",
            "logs/gateway",
        ],
        EnvironmentTemplate::CloudLocal => &[
            "pids",
            "logs/db",
            "logs/user",
            "logs/provider",
            "logs/admin",
            "logs/user_signup",
            "logs/worker",
        ],
        EnvironmentTemplate::Manager => &["pids", "logs"],
    };
    for directory in directories {
        fs::create_dir_all(root.join(directory))
            .map_err(|error| format!("failed to create runtime directories: {error}"))?;
    }
    Ok(())
}

fn install_root(template: EnvironmentTemplate) -> Result<PathBuf, String> {
    let base = env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| "HOME is not set.".to_owned())?;
    Ok(base.join("oqtopus").join(template.name()).join("releases"))
}

fn created_at() -> Result<String, String> {
    if env::var_os(FORBID_LEGACY_FALLBACK).is_some()
        && let Ok(value) = env::var("OQTOPUS_TEST_CREATED_AT")
    {
        return Ok(value);
    }

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("failed to read system time: {error}"))?
        .as_secs();
    let timestamp: libc::time_t = seconds
        .try_into()
        .map_err(|_| "system time is outside the supported range.".to_owned())?;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `timestamp` and `broken_down` are valid for the duration of the call. A successful
    // `gmtime_r` initializes the complete `tm` value before it is assumed initialized.
    if unsafe { libc::gmtime_r(&timestamp, broken_down.as_mut_ptr()) }.is_null() {
        return Err("failed to convert system time to UTC.".to_owned());
    }
    // SAFETY: the successful `gmtime_r` call above initialized `broken_down`.
    let broken_down = unsafe { broken_down.assume_init() };
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        broken_down.tm_year + 1900,
        broken_down.tm_mon + 1,
        broken_down.tm_mday,
        broken_down.tm_hour,
        broken_down.tm_min,
        broken_down.tm_sec,
    ))
}
