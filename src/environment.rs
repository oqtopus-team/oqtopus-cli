//! Validation shared by commands that operate inside an OQTOPUS environment.

use std::fs;
use std::path::Path;

use crate::metadata::{metadata_get, migrate_metadata_keys};

/// A directory validated as an environment for one template.
pub(crate) struct Environment {
    pub(crate) metadata: Vec<u8>,
}

/// Validates that the current directory is an environment for `template_name`.
pub(crate) fn validate_environment(template_name: &str) -> Result<Environment, String> {
    let metadata = validated_metadata(template_name)?;
    // Presence is part of validation even where no native command reads the value yet.
    metadata.require("install_root")?;

    Ok(Environment {
        metadata: metadata.contents,
    })
}

/// Metadata of a directory whose template and `environment_root` have been validated.
struct ValidatedMetadata {
    contents: Vec<u8>,
    // Parse through a lossy view, but retain the original bytes for output. This preserves the
    // legacy command's byte-for-byte behavior after the fields required for validation are found.
    text: String,
}

impl ValidatedMetadata {
    fn require(&self, key: &str) -> Result<String, String> {
        metadata_get(&self.text, key)
            .map(str::to_owned)
            .ok_or_else(|| missing(key))
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
        .ok_or_else(|| missing("environment_root"))?;
    let current = fs::canonicalize(".")
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    if root != current.to_string_lossy() {
        return Err(format!(
            "Current directory does not match environment_root.\nenvironment_root = {root}\ncurrent          = {}",
            current.display()
        ));
    }

    Ok(ValidatedMetadata { contents, text })
}

fn missing(key: &str) -> String {
    format!("invalid .metadata: missing {key}.")
}
