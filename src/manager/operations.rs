//! Manager component operation entrypoints and declaration.

use std::io::Write;

use crate::args::is_help;
use crate::environment::validate_environment;
use crate::operations::{
    OperationKind, OperationOutcome, install_release, install_version, success, uninstall, usage,
};
use crate::progress::Reporter;

use super::components::COMPONENT;

pub(crate) fn manager_install<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationOutcome, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::ManagerInstall, 0));
    }
    let environment = validate_environment("manager")?;
    let mut version = "";
    // The manager is a single component, so unlike backend and cloud-local there is no component
    // word to skip: the first positional argument is already the version.
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
        COMPONENT,
        (!version.is_empty()).then_some(version),
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}

pub(crate) fn manager_uninstall<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationOutcome, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::ManagerUninstall, 0));
    }
    let environment = validate_environment("manager")?;
    if args.len() != 1 || args[0].is_empty() {
        return Ok(usage(OperationKind::ManagerUninstall, 1));
    }
    uninstall(&environment, COMPONENT, &args[0], &mut Reporter::new(out))?;
    Ok(success())
}

pub(crate) fn manager_update<W: Write>(
    args: &[String],
    out: &mut W,
) -> Result<OperationOutcome, String> {
    if is_help(args) {
        return Ok(usage(OperationKind::ManagerUpdate, 0));
    }
    let environment = validate_environment("manager")?;
    if !args.is_empty() {
        return Ok(usage(OperationKind::ManagerUpdate, 1));
    }
    install_release(
        &environment,
        COMPONENT,
        None,
        false,
        &mut Reporter::new(out),
    )?;
    Ok(success())
}
