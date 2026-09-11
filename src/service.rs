//! Observed process state shared by the commands that report managed services.

use std::fs;
use std::path::Path;

/// Observed process state for one managed service.
pub(crate) struct ServiceStatus {
    pub(crate) name: &'static str,
    pub(crate) pid: Option<u32>,
}

/// Returns the live PID recorded in `path`, if the PID file names a reachable process.
///
/// Invalid, missing, stale, or otherwise unreachable PID files return `None`.
pub(crate) fn running_pid(path: &Path) -> Option<u32> {
    if !path.is_file() {
        return None;
    }
    let contents = fs::read_to_string(path).ok()?;
    // Command substitution in Bash removes trailing newlines before the numeric check.
    let candidate = contents.trim_end_matches('\n');
    if candidate.is_empty() || !candidate.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let pid = candidate.parse::<libc::pid_t>().ok()?;

    // SAFETY: signal 0 does not deliver a signal; it only checks whether the PID is reachable.
    (unsafe { libc::kill(pid, 0) } == 0).then_some(pid as u32)
}
