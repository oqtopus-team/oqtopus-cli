use std::fs;

use crate::harness::{EnvironmentTemplate, TestContext};

#[test]
fn manager_info_outputs_metadata() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::Manager,
        &[("manager_version", "v1.2.3")],
    );
    let output = context.run_snapshot_subject(["manager", "info"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    insta::assert_snapshot!("manager_info", context.render_output(&output));
}

#[test]
fn manager_info_rejects_arguments() {
    let context = TestContext::new();
    insta::assert_snapshot!(
        "manager_info_extra_argument",
        context.render_output(&context.run_snapshot_subject(["manager", "info", "unexpected"]))
    );
}

#[test]
fn manager_status_reports_running_process() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Manager, &[]);
    let pids = context.work_dir().join("pids");
    fs::create_dir(&pids).expect("create fixture pid directory");
    let running_pid = std::process::id();
    fs::write(pids.join("manager.pid"), format!("{running_pid}\n")).expect("write running pid");
    let output = context.run_snapshot_subject(["manager", "status"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout.clone()).expect("status should be UTF-8"),
        format!("manager: Running (PID {running_pid})\n")
    );
    let rendered = context
        .render_output(&output)
        .replace(&format!("PID {running_pid}"), "PID <PID>");
    insta::assert_snapshot!("manager_status", rendered);
}

#[test]
fn manager_status_rejects_arguments_before_environment_validation() {
    let context = TestContext::new();
    insta::assert_snapshot!(
        "manager_status_extra_argument",
        context.render_output(&context.run_snapshot_subject(["manager", "status", "unexpected"]))
    );
}

#[test]
fn manager_commands_reject_missing_metadata() {
    let context = TestContext::new();
    for command in ["info", "status"] {
        let output = context.run_snapshot_subject(["manager", command]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"Error: .metadata not found.\nThis directory is not an OQTOPUS manager environment.\n"
        );
    }
}

#[test]
fn manager_status_reports_a_stopped_service_without_a_pid_file() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Manager, &[]);

    let output = context.run_snapshot_subject(["manager", "status"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"manager: Stopped\n");
}
