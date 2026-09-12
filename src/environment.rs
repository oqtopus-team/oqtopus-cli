//! Validation shared by commands that operate inside an OQTOPUS environment.

use std::fs;
use std::path::{Path, PathBuf};

use crate::metadata::{metadata_get, migrate_metadata_keys};

/// A directory validated as an environment for one template.
pub(crate) struct Environment {
    pub(crate) metadata: Vec<u8>,
    pub(crate) root: PathBuf,
    pub(crate) install_root: PathBuf,
}

/// An [`Environment`] whose `environment_name` was required during validation.
///
/// Only [`validate_named_environment`] constructs this, so a caller cannot read the name of an
/// environment that was never checked for one.
pub(crate) struct NamedEnvironment {
    pub(crate) environment: Environment,
    pub(crate) name: String,
}

/// Validates that the current directory is an environment for `template_name`.
pub(crate) fn validate_environment(template_name: &str) -> Result<Environment, String> {
    let metadata = validated_metadata(template_name)?;
    let install_root = metadata.require("install_root")?;

    Ok(metadata.into_environment(install_root))
}

/// Validates the current directory and additionally requires `environment_name`.
///
/// The name is looked up before `install_root` so that metadata missing both reports the same
/// field as the legacy CLI.
pub(crate) fn validate_named_environment(template_name: &str) -> Result<NamedEnvironment, String> {
    let metadata = validated_metadata(template_name)?;
    let name = metadata.require_compat("environment_name", "env_name")?;
    let install_root = metadata.require("install_root")?;

    Ok(NamedEnvironment {
        environment: metadata.into_environment(install_root),
        name,
    })
}

/// Metadata of a directory whose template and `environment_root` have been validated.
struct ValidatedMetadata {
    contents: Vec<u8>,
    // Parse through a lossy view, but retain the original bytes for output. This preserves the
    // legacy command's byte-for-byte behavior after the fields required for validation are found.
    text: String,
    root: PathBuf,
}

impl ValidatedMetadata {
    fn require(&self, key: &str) -> Result<String, String> {
        metadata_get(&self.text, key)
            .map(str::to_owned)
            .ok_or_else(|| missing(key))
    }

    /// Reads `key`, falling back to a pre-migration spelling for metadata that could not be
    /// rewritten in place.
    fn require_compat(&self, key: &str, legacy_key: &str) -> Result<String, String> {
        metadata_get(&self.text, key)
            .or_else(|| metadata_get(&self.text, legacy_key))
            .map(str::to_owned)
            .ok_or_else(|| missing(key))
    }

    fn into_environment(self, install_root: String) -> Environment {
        Environment {
            metadata: self.contents,
            root: self.root,
            install_root: PathBuf::from(install_root),
        }
    }
}

fn validated_metadata(template_name: &str) -> Result<ValidatedMetadata, String> {
    let path = Path::new(".metadata");
    if !path.is_file() {
        return Err(format!(
            ".metadata not found.\nThis directory is not an OQTOPUS {template_name} environment."
        ));
    }

    migrate_metadata_keys(path);
    // The legacy metadata lookup also reports an unreadable file as a missing required key.
    let contents = fs::read(path).map_err(|_| missing("template"))?;
    let text = String::from_utf8_lossy(&contents).into_owned();

    let template = metadata_get(&text, "template").ok_or_else(|| missing("template"))?;
    if template != template_name {
        return Err(format!(
            "invalid environment template. Found template='{template}', but 'oqtopus {template_name}' requires template='{template_name}'."
        ));
    }

    let root = metadata_get(&text, "environment_root")
        .or_else(|| metadata_get(&text, "env_root"))
        .ok_or_else(|| missing("environment_root"))?
        .to_owned();
    let current = fs::canonicalize(".")
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    if root != current.to_string_lossy() {
        return Err(format!(
            "Current directory does not match environment_root.\nenvironment_root = {root}\ncurrent          = {}",
            current.display()
        ));
    }

    Ok(ValidatedMetadata {
        contents,
        text,
        root: PathBuf::from(root),
    })
}

fn missing(key: &str) -> String {
    format!("invalid .metadata: missing {key}.")
}
