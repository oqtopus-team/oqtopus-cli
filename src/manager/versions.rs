//! Manager component version command.

use crate::args::is_help;
use crate::versions::{OptionalEnvironment, VersionsKind, VersionsOutcome, list_versions, usage};

use super::components::COMPONENT;

pub(crate) fn manager_versions(args: &[String]) -> Result<VersionsOutcome, String> {
    if is_help(args) {
        return Ok(usage(VersionsKind::Manager, 0));
    }
    if !args.is_empty() {
        return Ok(usage(VersionsKind::Manager, 1));
    }

    list_versions(
        COMPONENT.name,
        COMPONENT.repository,
        COMPONENT.binding_key,
        OptionalEnvironment::load("manager", false)?,
    )
}
