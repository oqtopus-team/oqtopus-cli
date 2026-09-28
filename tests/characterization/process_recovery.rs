use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::harness::{EnvironmentTemplate, TestContext};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[test]
fn stop_succeeds_when_foreground_cleanup_removes_the_pid_file() {
    let context = manager_context();
    let trace = context.root().join("uv-invocations");
    let mut cleanup = ServiceCleanup::new(&context, &trace);
    install_long_running_uv(&context);
    let mut command = start_command(&context, &trace, None);
    command.arg("--foreground");
    let mut foreground = ChildGuard::spawn(command);
    wait_until("foreground service readiness", PROCESS_TIMEOUT, || {
        invocation_count(&trace) == 1
    });
    stop_manager(&context);
    assert!(!foreground.wait_for_exit(PROCESS_TIMEOUT).success());
    assert!(!context.work_dir().join("pids/manager.pid").exists());
    cleanup.disarm();
}

#[test]
fn simultaneous_starts_launch_the_service_once() {
    let context = manager_context();
    let trace = context.root().join("uv-invocations");
    let mut service_cleanup = ServiceCleanup::new(&context, &trace);
    install_long_running_uv(&context);

    let mut first = spawn_start(&context, &trace, None);
    let mut second = spawn_start(&context, &trace, None);

    let first_status = first.wait_for_exit(PROCESS_TIMEOUT);
    let second_status = second.wait_for_exit(PROCESS_TIMEOUT);
    assert!(
        first_status.success() || second_status.success(),
        "both overlapping starts failed: {first_status}, {second_status}"
    );
    assert_eq!(
        invocation_count(&trace),
        1,
        "overlapping starts launched more than one service process"
    );

    stop_manager(&context);
    service_cleanup.disarm();
}

#[test]
fn killed_startup_caller_releases_lock_and_preserves_the_started_service() {
    let context = manager_context();
    let trace = context.root().join("uv-invocations");
    let ready = context.root().join("uv-ready");
    let mut service_cleanup = ServiceCleanup::new(&context, &trace);
    context.write_executable(
        "uv",
        b"#!/bin/sh\nset -eu\ntest \"$(cat pids/manager.pid)\" = \"$$\"\nmkdir -p \"$OQTOPUS_TEST_TRACE\"\n: > \"$OQTOPUS_TEST_TRACE/$$\"\nkill -STOP \"$PPID\"\n: > \"$OQTOPUS_TEST_READY\"\nexec sleep 60\n",
    );

    let mut interrupted = spawn_start(&context, &trace, Some(&ready));
    wait_until("fake uv startup readiness", PROCESS_TIMEOUT, || {
        ready.is_file()
    });
    assert!(
        interrupted
            .child
            .try_wait()
            .expect("inspect startup CLI")
            .is_none(),
        "startup CLI exited before the crash could be simulated"
    );

    let mut contending = spawn_start(&context, &trace, None);
    let contending_status = contending.wait_for_exit(PROCESS_TIMEOUT);
    assert!(
        !contending_status.success(),
        "start succeeded while another CLI held the startup lock"
    );
    assert_eq!(invocation_count(&trace), 1);

    interrupted.signal(libc::SIGKILL);
    let interrupted_status = interrupted.wait_for_exit(PROCESS_TIMEOUT);
    assert!(!interrupted_status.success());

    let mut retry = spawn_start(&context, &trace, None);
    let retry_status = retry.wait_for_exit(PROCESS_TIMEOUT);
    assert!(retry_status.success(), "retry failed: {retry_status}");
    assert_eq!(
        invocation_count(&trace),
        1,
        "retry duplicated the service whose startup caller was killed"
    );

    stop_manager(&context);
    service_cleanup.disarm();
}

#[test]
fn pid_file_creation_failure_does_not_launch_uv_and_allows_retry() {
    let context = manager_context();
    let trace = context.root().join("uv-invocations");
    let mut service_cleanup = ServiceCleanup::new(&context, &trace);
    install_long_running_uv(&context);
    let pid_file = context.work_dir().join("pids/manager.pid");
    fs::create_dir_all(&pid_file).expect("create invalid PID path");

    let mut failed = spawn_start(&context, &trace, None);
    let failed_status = failed.wait_for_exit(PROCESS_TIMEOUT);
    assert!(
        !failed_status.success(),
        "start unexpectedly accepted a directory as its PID file"
    );
    assert_eq!(
        invocation_count(&trace),
        0,
        "uv launched before its PID could be published"
    );

    fs::remove_dir(&pid_file).expect("remove invalid PID path");
    let mut retry = spawn_start(&context, &trace, None);
    let retry_status = retry.wait_for_exit(PROCESS_TIMEOUT);
    assert!(retry_status.success(), "retry failed: {retry_status}");
    assert_eq!(invocation_count(&trace), 1);

    stop_manager(&context);
    service_cleanup.disarm();
}

#[test]
fn stale_legacy_start_locks_do_not_prevent_startup() {
    for stale_pid in [None, Some("2147483647\n")] {
        let context = manager_context();
        let trace = context.root().join("uv-invocations");
        let mut service_cleanup = ServiceCleanup::new(&context, &trace);
        install_long_running_uv(&context);
        let legacy_lock = context.work_dir().join("pids/.manager.start.lock");
        fs::create_dir_all(&legacy_lock).expect("create stale legacy start lock");
        if let Some(pid) = stale_pid {
            fs::write(legacy_lock.join("pid"), pid).expect("write stale legacy lock PID");
        }

        let mut start = spawn_start(&context, &trace, None);
        let status = start.wait_for_exit(PROCESS_TIMEOUT);
        assert!(
            status.success(),
            "start with stale legacy lock failed: {status}"
        );
        assert_eq!(invocation_count(&trace), 1);

        stop_manager(&context);
        service_cleanup.disarm();
    }
}

#[test]
fn immediate_startup_failure_clears_pid_and_allows_retry() {
    let context = manager_context();
    let trace = context.root().join("uv-invocations");
    let state = context.root().join("uv-state");
    let mut service_cleanup = ServiceCleanup::new(&context, &trace);
    context.write_executable(
        "uv",
        b"#!/bin/sh\nset -eu\nmkdir -p \"$OQTOPUS_TEST_TRACE\" \"$OQTOPUS_TEST_STATE\"\n: > \"$OQTOPUS_TEST_TRACE/$$\"\nif mkdir \"$OQTOPUS_TEST_STATE/first\" 2>/dev/null; then exit 7; fi\nexec sleep 60\n",
    );

    let mut first = start_command(&context, &trace, None);
    first.env("OQTOPUS_TEST_STATE", &state);
    let mut first = ChildGuard::spawn(first);
    let first_status = first.wait_for_exit(PROCESS_TIMEOUT);
    assert!(
        !first_status.success(),
        "immediate failure was reported as success"
    );
    assert!(
        !context.work_dir().join("pids/manager.pid").exists(),
        "failed startup left a service PID file"
    );

    let mut retry = start_command(&context, &trace, None);
    retry.env("OQTOPUS_TEST_STATE", &state);
    let mut retry = ChildGuard::spawn(retry);
    let retry_status = retry.wait_for_exit(PROCESS_TIMEOUT);
    assert!(retry_status.success(), "retry failed: {retry_status}");
    assert_eq!(invocation_count(&trace), 2);

    stop_manager(&context);
    service_cleanup.disarm();
}

fn manager_context() -> TestContext {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::Manager,
        &[("manager_version", "v1.2.3")],
    );
    fs::create_dir_all(
        context
            .root()
            .join("xdg-data/oqtopus/manager/releases/manager-v1.2.3"),
    )
    .expect("create manager release fixture");
    context
}

fn install_long_running_uv(context: &TestContext) {
    context.write_executable(
        "uv",
        b"#!/bin/sh\nset -eu\nmkdir -p \"$OQTOPUS_TEST_TRACE\"\n: > \"$OQTOPUS_TEST_TRACE/$$\"\nexec sleep 60\n",
    );
}

fn start_command(context: &TestContext, trace: &Path, ready: Option<&Path>) -> Command {
    let mut command = context.rust_command(["manager", "start"]);
    command
        .env("OQTOPUS_TEST_TRACE", trace)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(ready) = ready {
        command.env("OQTOPUS_TEST_READY", ready);
    }
    command
}

fn spawn_start(context: &TestContext, trace: &Path, ready: Option<&Path>) -> ChildGuard {
    ChildGuard::spawn(start_command(context, trace, ready))
}

fn stop_manager(context: &TestContext) {
    let mut command = context.rust_command(["manager", "stop"]);
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut stop = ChildGuard::spawn(command);
    let status = stop.wait_for_exit(PROCESS_TIMEOUT + Duration::from_secs(1));
    assert!(
        status.success(),
        "manager cleanup failed with status {status}"
    );
}

fn invocation_count(trace: &Path) -> usize {
    fs::read_dir(trace)
        .map(|entries| entries.count())
        .unwrap_or(0)
}

fn wait_until<F>(description: &str, timeout: Duration, mut condition: F)
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        if condition() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

struct ChildGuard {
    child: Child,
    finished: bool,
}

impl ChildGuard {
    fn spawn(mut command: Command) -> Self {
        let child = command.spawn().expect("spawn Rust CLI");
        Self {
            child,
            finished: false,
        }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().expect("wait for Rust CLI") {
                self.finished = true;
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "Rust CLI did not exit within {timeout:?}"
            );
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn signal(&self, signal: libc::c_int) {
        // SAFETY: `Child::id` is a positive PID belonging to this test's spawned process.
        let result = unsafe { libc::kill(self.child.id() as libc::pid_t, signal) };
        assert_eq!(result, 0, "failed to signal Rust CLI");
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct ServiceCleanup {
    pid_file: PathBuf,
    trace: PathBuf,
    active: bool,
}

impl ServiceCleanup {
    fn new(context: &TestContext, trace: &Path) -> Self {
        Self {
            pid_file: context.work_dir().join("pids/manager.pid"),
            trace: trace.to_owned(),
            active: true,
        }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for ServiceCleanup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut pids = HashSet::new();
        if let Ok(pid) = fs::read_to_string(&self.pid_file)
            && let Ok(pid) = pid.trim().parse::<libc::pid_t>()
            && pid > 0
        {
            pids.insert(pid);
        }
        if let Ok(entries) = fs::read_dir(&self.trace) {
            for entry in entries.flatten() {
                if let Ok(pid) = entry.file_name().to_string_lossy().parse::<libc::pid_t>()
                    && pid > 0
                {
                    pids.insert(pid);
                }
            }
        }
        for pid in pids {
            // SAFETY: every positive PID came from this test's isolated PID or invocation files.
            let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
}
