//! Result data shared by lifecycle commands.

use std::io::Write;

#[derive(Clone, Copy)]
pub(crate) enum LifecycleKind {
    BackendStart,
    BackendStop,
    BackendRestart,
    CloudLocalStart,
    CloudLocalStop,
    CloudLocalRestart,
    ManagerStart,
    ManagerStop,
    ManagerRestart,
}

pub(crate) enum LifecycleOutput {
    None,
    Usage(LifecycleKind),
}

pub(crate) struct LifecycleOutcome {
    pub(crate) output: LifecycleOutput,
    exit_code: i32,
}

impl LifecycleOutcome {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

pub(crate) fn success() -> LifecycleOutcome {
    result_code(0)
}
pub(crate) fn result_code(exit_code: i32) -> LifecycleOutcome {
    LifecycleOutcome {
        output: LifecycleOutput::None,
        exit_code,
    }
}
pub(crate) fn usage(kind: LifecycleKind, exit_code: i32) -> LifecycleOutcome {
    LifecycleOutcome {
        output: LifecycleOutput::Usage(kind),
        exit_code,
    }
}
pub(crate) fn line(out: &mut impl Write, message: impl std::fmt::Display) -> Result<(), String> {
    writeln!(out, "{message}").map_err(|error| format!("failed to write progress: {error}"))?;
    out.flush()
        .map_err(|error| format!("failed to write progress: {error}"))
}
