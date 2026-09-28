//! Argument helpers shared by every command family.

/// Reports whether an invocation asks for usage text instead of an operation.
pub(crate) fn is_help(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "--help"))
}

/// Returns the sole non-empty argument, or `None` when the invocation names no single target.
pub(crate) fn single_target(args: &[String]) -> Option<&str> {
    (args.len() == 1 && !args[0].is_empty()).then(|| args[0].as_str())
}

/// Rejects a target that is not part of a domain's service inventory.
pub(crate) fn validate_member(target: &str, services: &[&str]) -> Result<(), String> {
    services
        .contains(&target)
        .then_some(())
        .ok_or_else(|| format!("unknown service: {target}"))
}
