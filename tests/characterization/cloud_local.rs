use std::fs;
use std::process::Command;

use crate::harness::{EnvironmentTemplate, REMOTE_REFS_FIXTURE, TestContext};

const SERVICES: [&str; 6] = ["db", "worker", "user_signup", "admin", "provider", "user"];

/// Minimal `docker` stand-in: reports the database as running and names one container per
/// compose service, so status output does not depend on a real Docker daemon.
const DOCKER_STUB: &str = r#"#!/bin/sh
case "$*" in
  *"ps --status running --quiet db"*) echo db-id ;;
  *"service=db"*) echo characterization-db-1 ;;
  *"service=minio"*) echo characterization-minio-1 ;;
  *"service=mc"*) echo characterization-mc-1 ;;
esac
"#;

const RUNNING_DB_ROW: &str =
    "db: Running (characterization-db-1, characterization-minio-1, characterization-mc-1)";

#[test]
fn cloud_local_versions_marks_the_current_installed_version() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_frontend_version", "v1.2.3")],
    );
    let install_root = context.root().join("xdg-data/oqtopus/cloud-local/releases");
    fs::create_dir(install_root.join("frontend-v1.2.3")).expect("create installed release");

    let output = context.run_snapshot_subject_with_remote_refs(
        ["cloud-local", "versions", "frontend"],
        REMOTE_REFS_FIXTURE,
        false,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        ["frontend:", "  v2.0.0", "  v1.10.0", "* v1.2.3 (installed)",]
    );
    insta::assert_snapshot!("cloud_local_versions", context.render_output(&output));
}

#[test]
fn cloud_local_versions_rejects_missing_component() {
    let context = TestContext::new();
    insta::assert_snapshot!(
        "cloud_local_versions_missing_component",
        context.render_output(&context.run_snapshot_subject(["cloud-local", "versions"]))
    );
}

#[test]
fn cloud_local_versions_help_preserves_legacy_text() {
    let context = TestContext::new();
    let output = context.run_snapshot_subject(["cloud-local", "versions", "help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    insta::assert_snapshot!("cloud_local_versions_help", context.render_output(&output));
}

#[test]
fn cloud_local_info_outputs_metadata() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.2.3")],
    );
    let output = context.run_snapshot_subject(["cloud-local", "info"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout.clone()).expect("metadata should be UTF-8");
    assert!(stdout.contains("template=cloud-local\n"));
    assert!(stdout.contains("cloud_local_cloud_version=v1.2.3\n"));
    insta::assert_snapshot!("cloud_local_info", context.render_output(&output));
}

#[test]
fn cloud_local_info_rejects_arguments() {
    let context = TestContext::new();
    insta::assert_snapshot!(
        "cloud_local_info_extra_argument",
        context.render_output(&context.run_snapshot_subject(["cloud-local", "info", "unexpected"]))
    );
}

#[test]
fn cloud_local_status_reports_docker_and_process_services_in_manager_order() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.2.3")],
    );
    let backend = context
        .root()
        .join("xdg-data/oqtopus/cloud-local/releases/cloud-v1.2.3/backend");
    fs::create_dir_all(&backend).expect("create cloud fixture");
    fs::write(backend.join("compose.yaml"), "services: {}\n").expect("write compose fixture");
    context.write_executable("docker", DOCKER_STUB);
    let pids = context.work_dir().join("pids");
    fs::create_dir(&pids).expect("create fixture pid directory");
    let running_pid = std::process::id();
    fs::write(pids.join("worker.pid"), format!("{running_pid}\n")).expect("write running pid");

    let mut exited = Command::new("sleep")
        .arg("60")
        .spawn()
        .expect("start stale process");
    let stale_pid = exited.id();
    exited.kill().expect("kill stale process");
    exited.wait().expect("reap stale process");
    fs::write(pids.join("user.pid"), format!("{stale_pid}\n")).expect("write stale pid");

    let output = context.run_snapshot_subject(["cloud-local", "status"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout.clone()).expect("status should be UTF-8");
    let rows: Vec<_> = stdout.lines().collect();
    assert_eq!(rows.len(), SERVICES.len());
    for (row, service) in rows.iter().zip(SERVICES) {
        assert!(
            row.starts_with(&format!("{service}: ")),
            "row out of order: {row}"
        );
    }
    assert_eq!(rows[0], RUNNING_DB_ROW);
    assert_eq!(rows[1], format!("worker: Running (PID {running_pid})"));
    assert_eq!(rows[5], "user: Stopped");
    let rendered = context
        .render_output(&output)
        .replace(&format!("PID {running_pid}"), "PID <PID>");
    insta::assert_snapshot!("cloud_local_status", rendered);
}

#[test]
fn cloud_local_status_rejects_arguments_before_environment_validation() {
    let context = TestContext::new();
    insta::assert_snapshot!(
        "cloud_local_status_extra_argument",
        context.render_output(&context.run_snapshot_subject([
            "cloud-local",
            "status",
            "unexpected"
        ]))
    );
}

#[test]
fn cloud_local_commands_reject_missing_metadata() {
    let context = TestContext::new();
    for command in ["info", "status"] {
        let output = context.run_snapshot_subject(["cloud-local", command]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"Error: .metadata not found.\nThis directory is not an OQTOPUS cloud-local environment.\n"
        );
    }
}

#[test]
fn cloud_local_status_reports_all_services_stopped_without_a_cloud_binding() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::CloudLocal, &[]);

    let output = context.run_snapshot_subject(["cloud-local", "status"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("status should be UTF-8"),
        SERVICES
            .iter()
            .map(|service| format!("{service}: Stopped\n"))
            .collect::<String>()
    );
}

#[test]
fn cloud_local_info_requires_environment_name() {
    let context = TestContext::new();
    context.write_metadata(format!(
        "template=cloud-local\ninstall_root={}/releases\nenvironment_root={}\n",
        context.root().display(),
        context.work_dir().display()
    ));

    let output = context.run_snapshot_subject(["cloud-local", "info"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"Error: invalid .metadata: missing environment_name.\n"
    );
}

#[test]
fn cloud_local_status_resolves_a_branch_installation_inside_the_environment() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "branch:main")],
    );
    // A branch binding is checked out in the environment root rather than under install_root, so
    // the compose file used to probe the database comes from a different directory.
    let backend = context.work_dir().join("cloud/backend");
    fs::create_dir_all(&backend).expect("create branch cloud fixture");
    fs::write(backend.join("compose.yaml"), "services: {}\n").expect("write compose fixture");
    context.write_executable("docker", DOCKER_STUB);

    let output = context.run_snapshot_subject(["cloud-local", "status"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("status should be UTF-8");
    assert_eq!(stdout.lines().next(), Some(RUNNING_DB_ROW));
}
