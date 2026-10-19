//! Cloud-local component operation entrypoints and declarations.

use std::io::Write;

use crate::args::is_help;
use crate::environment::validate_named_environment;
use crate::operations::{
    OperationKind, OperationOutcome, find_component, install_release, install_version, success,
    uninstall, usage,
};
use crate::progress::Reporter;

use super::components::COMPONENTS;

pub(crate) fn cloud_local_install<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationOutcome, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::CloudLocalInstall, 0));
    }
    let environment = validate_named_environment("cloud-local")?.environment;
    let Some(component_name) = args.first().filter(|arg| !arg.is_empty()) else {
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
        for component in COMPONENTS {
            install_release(&environment, component, None, false, &mut reporter)?;
        }
        return Ok(success());
    }
    let component = find_component(&COMPONENTS, component_name)?;
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
) -> Result<OperationOutcome, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::CloudLocalUninstall, 0));
    }
    let environment = validate_named_environment("cloud-local")?.environment;
    if args.len() != 2 || args[0].is_empty() || args[1].is_empty() {
        return Ok(usage(OperationKind::CloudLocalUninstall, 1));
    }
    let component = find_component(&COMPONENTS, &args[0])?;
    uninstall(&environment, component, &args[1], &mut Reporter::new(out))?;
    Ok(success())
}

pub(crate) fn cloud_local_update<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationOutcome, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::CloudLocalUpdate, 0));
    }
    let environment = validate_named_environment("cloud-local")?.environment;
    if args.len() != 1 || args[0].is_empty() {
        return Ok(usage(OperationKind::CloudLocalUpdate, 1));
    }
    let component = find_component(&COMPONENTS, &args[0])?;
    install_release(
        &environment,
        component,
        None,
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}
