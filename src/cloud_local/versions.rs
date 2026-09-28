//! Cloud-local component version command.

use crate::args::is_help;
use crate::operations::find_component;
use crate::versions::{OptionalEnvironment, VersionsKind, VersionsOutcome, list_versions, usage};

use super::components::COMPONENTS;

pub(crate) fn cloud_local_versions(args: &[String]) -> Result<VersionsOutcome, String> {
    if is_help(args) {
        return Ok(usage(VersionsKind::CloudLocal, 0));
    }
    if args.len() != 1 {
        return Ok(usage(VersionsKind::CloudLocal, 1));
    }

    let component = find_component(&COMPONENTS, &args[0])?;
    list_versions(
        component.name,
        component.repository,
        component.binding_key,
        OptionalEnvironment::load("cloud-local", true)?,
    )
}
