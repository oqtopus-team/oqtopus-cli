use std::ffi::CString;
use std::fs::{self, Permissions};
use std::io::{BufRead, BufReader};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::harness::{EnvironmentTemplate, TestContext};

const BACKEND_SERVICES: [&str; 7] = [
    "core",
    "sse_engine",
    "mitigator",
    "estimator",
    "combiner",
    "tranqu",
    "gateway",
];

// Covers the three pid-file states in one ordered artifact: a live PID is Running, while a PID
// whose process exited and a nonnumeric PID are both Stopped. The row order and spelling are part
// of the Manager's parsing contract.
#[test]
fn backend_status_reports_services_in_manager_compatible_order() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Backend, &[]);
    let pids = context.work_dir().join("pids");
    fs::create_dir(&pids).expect("create fixture pid directory");

    // The test process remains alive while the CLI checks it.
    let running_pid = std::process::id();
    fs::write(pids.join("core.pid"), format!("{running_pid}\n")).expect("write running pid");

    // Record the PID of a process that really existed and has been reaped, matching a pid file left
    // behind after a service exits.
    let mut exited_process = Command::new("sleep")
        .arg("60")
        .spawn()
        .expect("start stale-pid fixture process");
    let stale_pid = exited_process.id();
    exited_process
        .kill()
        .expect("stop stale-pid fixture process");
    exited_process
        .wait()
        .expect("reap stale-pid fixture process");
    fs::write(pids.join("tranqu.pid"), format!("{stale_pid}\n")).expect("write stale pid");
    fs::write(pids.join("gateway.pid"), "not-a-pid\n").expect("write invalid pid");

    let output = context.run_snapshot_subject(["backend", "status"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout.clone()).expect("status should be UTF-8");
    let rows: Vec<_> = stdout.lines().collect();
    assert_eq!(rows.len(), BACKEND_SERVICES.len());
    for (row, service) in rows.iter().zip(BACKEND_SERVICES) {
        assert!(
            row.starts_with(&format!("{service}: ")),
            "service row is out of order: {row}"
        );
    }
    assert_eq!(rows[0], format!("core: Running (PID {running_pid})"));
    assert_eq!(rows[5], "tranqu: Stopped");
    assert_eq!(rows[6], "gateway: Stopped");

    let rendered = context
        .render_output(&output)
        .replace(&format!("PID {running_pid}"), "PID <PID>");
    insta::assert_snapshot!("backend_status", rendered);
}

#[test]
fn backend_status_treats_fifo_pid_paths_as_stopped() {
    // Bash checks [[ -f ]] before reading a PID path. Keep this compatibility case on the Rust
    // subject so an implementation that reads a FIFO blocks the test instead of returning status.
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Backend, &[]);
    let pids = context.work_dir().join("pids");
    fs::create_dir(&pids).expect("create fixture pid directory");

    let fifo = pids.join("core.pid");
    let fifo_path = CString::new(fifo.as_os_str().as_bytes()).expect("FIFO path has no NUL");
    // SAFETY: the path points to a temporary test directory and the mode is valid.
    let result = unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) };
    assert_eq!(
        result,
        0,
        "create FIFO: {}",
        std::io::Error::last_os_error()
    );

    let mut child = context
        .rust_command(["backend", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start Rust CLI");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child.kill().expect("stop hung Rust CLI");
                let _ = child.wait();
                panic!("backend status did not return for a FIFO PID path");
            }
            Err(error) => panic!("check Rust CLI status: {error}"),
        }
    }

    let output = child.wait_with_output().expect("wait for Rust CLI");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("status should be UTF-8"),
        BACKEND_SERVICES
            .iter()
            .map(|service| format!("{service}: Stopped\n"))
            .collect::<String>()
    );
}

#[test]
fn backend_status_rejects_arguments() {
    let context = TestContext::new();

    // No environment is created deliberately: argument validation must win over the otherwise
    // applicable missing-.metadata error, matching the Bash dispatcher.
    insta::assert_snapshot!(
        "backend_status_extra_argument",
        context.render_output(&context.run_snapshot_subject(["backend", "status", "unexpected"]))
    );
}

#[test]
fn backend_status_rejects_missing_metadata() {
    let context = TestContext::new();

    insta::assert_snapshot!(
        "backend_status_no_metadata",
        context.render_output(&context.run_snapshot_subject(["backend", "status"]))
    );
}

#[test]
fn backend_info_outputs_metadata() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::Backend,
        &[("engine_version", "v1.2.3")],
    );

    let output = context.run_snapshot_subject(["backend", "info"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    // The snapshot pins the complete byte-oriented output, while these focused assertions pin the
    // individual key=value fields consumed by the Manager.
    let stdout = String::from_utf8(output.stdout.clone()).expect("metadata should be UTF-8");
    let fields: std::collections::HashMap<_, _> = stdout
        .lines()
        .map(|line| line.split_once('=').expect("each metadata row has a value"))
        .collect();
    assert_eq!(fields.get("template"), Some(&"backend"));
    assert_eq!(fields.get("environment_name"), Some(&"characterization"));
    assert_eq!(
        fields.get("environment_root").copied(),
        context.work_dir().to_str()
    );
    let install_root = context.root().join("xdg-data/oqtopus/backend/releases");
    assert_eq!(fields.get("install_root").copied(), install_root.to_str());
    assert_eq!(fields.get("engine_version"), Some(&"v1.2.3"));

    insta::assert_snapshot!("backend_info", context.render_output(&output));
}

// These cases exercise intentional Rust compatibility changes, so they always run Rust even
// when the characterization subject is Bash.
#[test]
fn backend_info_rejects_bare_template_key() {
    let context = TestContext::new();
    context.write_metadata(format!(
        "template\ninstall_root={}/releases\nenvironment_root={}\n",
        context.root().display(),
        context.work_dir().display()
    ));

    let output = context
        .rust_command(["backend", "info"])
        .output()
        .expect("run Rust CLI");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"Error: invalid .metadata: missing template.\n"
    );
}

#[test]
fn backend_info_accepts_crlf_and_preserves_output_bytes() {
    // Rust accepts CRLF during validation, but `info` must still emit and retain the original bytes
    // instead of normalizing line endings or regenerating the metadata.
    let context = TestContext::new();
    let metadata = format!(
        "template=backend\r\ninstall_root={}/releases\r\nenvironment_root={}\r\nengine_version=v1.2.3\r\n",
        context.root().display(),
        context.work_dir().display()
    );
    context.write_metadata(&metadata);

    let output = context
        .rust_command(["backend", "info"])
        .output()
        .expect("run Rust CLI");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, metadata.as_bytes());
    assert_eq!(
        fs::read(context.work_dir().join(".metadata")).expect("read metadata"),
        metadata.as_bytes()
    );
}

#[test]
fn backend_info_rejects_arguments() {
    let context = TestContext::new();

    // As with `status`, the absent environment proves argument validation happens first.
    let output = context.run_snapshot_subject(["backend", "info", "unexpected"]);

    insta::assert_snapshot!(
        "backend_info_extra_argument",
        context.render_output(&output)
    );
}

#[test]
fn backend_info_rejects_missing_metadata() {
    let context = TestContext::new();

    insta::assert_snapshot!(
        "backend_info_no_metadata",
        context.render_output(&context.run_snapshot_subject(["backend", "info"]))
    );
}

#[test]
fn backend_info_rejects_wrong_template() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Manager, &[]);

    insta::assert_snapshot!(
        "backend_info_wrong_template",
        context.render_output(&context.run_snapshot_subject(["backend", "info"]))
    );
}

#[test]
fn backend_info_rejects_missing_install_root() {
    let context = TestContext::new();
    context.write_metadata(format!(
        "template=backend\nenvironment_root={}\n",
        context.work_dir().display()
    ));

    insta::assert_snapshot!(
        "backend_info_missing_install_root",
        context.render_output(&context.run_snapshot_subject(["backend", "info"]))
    );
}

#[test]
fn backend_info_rejects_mismatched_environment_root() {
    let context = TestContext::new();
    context.write_metadata(format!(
        "template=backend\ninstall_root={}/releases\nenvironment_root={}/elsewhere\n",
        context.root().display(),
        context.root().display()
    ));

    insta::assert_snapshot!(
        "backend_info_mismatched_root",
        context.render_output(&context.run_snapshot_subject(["backend", "info"]))
    );
}

#[test]
fn backend_info_migrates_legacy_metadata_keys() {
    // A read-only command still performs the compatibility migration, so this test checks both the
    // rewritten file and the metadata bytes printed after that rewrite.
    let context = TestContext::new();
    context.write_metadata(format!(
        "template=backend\ninstall_root={}/releases\nenv_name=legacy\nenv_root={}\nengine_version=v1.2.3\n",
        context.root().display(),
        context.work_dir().display()
    ));

    let output = context.run_snapshot_subject(["backend", "info"]);
    let migrated = fs::read(context.work_dir().join(".metadata")).expect("read migrated metadata");
    let expected = format!(
        "template=backend\ninstall_root={}/releases\nengine_version=v1.2.3\nenvironment_root={}\nenvironment_name=legacy\n",
        context.root().display(),
        context.work_dir().display()
    );

    // Check the persisted side effect directly; the snapshot independently covers stdout.
    assert_eq!(migrated, expected.as_bytes());
    insta::assert_snapshot!(
        "backend_info_legacy_metadata",
        context.render_output(&output)
    );
}

#[test]
fn backend_info_does_not_migrate_metadata_unwritable_by_an_unprivileged_owner() {
    // Exercise ordinary invocations with matching real/effective IDs. Root bypasses the file
    // permissions, and differing IDs are outside this fixture's scope.
    // SAFETY: these ID getters have no arguments or memory-safety preconditions.
    if unsafe {
        libc::getuid() == 0
            || libc::getuid() != libc::geteuid()
            || libc::getgid() != libc::getegid()
    } {
        return;
    }

    let context = TestContext::new();
    let original = format!(
        "template=backend\ninstall_root={}/releases\nenv_name=legacy\nenv_root={}\n",
        context.root().display(),
        context.work_dir().display()
    );
    let path = context.work_dir().join(".metadata");
    context.write_metadata(&original);
    // The owner may read but not write. Group/other write bits expose the difference between
    // `access(W_OK)` and merely checking whether any write bit is present.
    fs::set_permissions(&path, Permissions::from_mode(0o422))
        .expect("make metadata unwritable by its owner");

    let output = context.run_snapshot_subject(["backend", "info"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, original.as_bytes());
    assert_eq!(
        fs::read(path).expect("read unchanged metadata"),
        original.as_bytes()
    );
}

#[test]
fn backend_info_exits_silently_on_broken_stdout_pipe() {
    // The large value prevents the entire response from fitting in the pipe before its reader is
    // closed. The CLI should then receive SIGPIPE without printing a secondary write error.
    let context = TestContext::new();
    let metadata = format!(
        "template=backend\ninstall_root={}/releases\nenvironment_root={}\npadding={}\n",
        context.root().display(),
        context.work_dir().display(),
        "x".repeat(8 * 1024 * 1024)
    );
    context.write_metadata(metadata);

    let mut child = context
        .rust_command(["backend", "info"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start Rust CLI");
    let stdout = child.stdout.take().expect("capture stdout");
    let mut reader = BufReader::new(stdout);
    let mut first_line = String::new();
    reader.read_line(&mut first_line).expect("read first line");
    assert_eq!(first_line, "template=backend\n");
    drop(reader);

    let output = child.wait_with_output().expect("wait for Rust CLI");
    assert_eq!(output.status.signal(), Some(libc::SIGPIPE));
    assert!(output.stderr.is_empty());
}
