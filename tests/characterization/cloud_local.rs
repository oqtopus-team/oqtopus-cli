use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::harness::{EnvironmentTemplate, REMOTE_REFS_FIXTURE, TestContext};

const SERVICES: [&str; 6] = ["db", "worker", "user_signup", "admin", "provider", "user"];

/// Records how `oqtopus-cloud-infra status` was launched, then replays `$INFRA_STDOUT_FILE` and exits
/// with the code in `$INFRA_EXIT_FILE`. The stderr line checks that diagnostics never reach the
/// CLI's own stderr.
const INFRA_UV_STUB: &str = r#"#!/bin/sh
root=$(dirname "$0")/..
{
  printf 'cwd=%s\n' "$PWD"
  printf 'args=%s\n' "$*"
  printf 'COMPOSE_PROJECT_NAME=%s\n' "${COMPOSE_PROJECT_NAME-<unset>}"
  printf 'VIRTUAL_ENV=%s\n' "${VIRTUAL_ENV-<unset>}"
  printf 'STORAGE_STACK=%s\n' "${STORAGE_STACK-<unset>}"
} > "$root/uv-invocation"
echo 'infra: diagnostic on stderr' >&2
cat "$root/infra-stdout"
exit "$(cat "$root/infra-exit")"
"#;

const RUNNING_DB_ROW: &str = "db: Running (db, seaweedfs)";

/// Installs a cloud release that provides `oqtopus-cloud-infra` and a fake `uv` that answers its
/// `status` subcommand with `stdout` and `exit`.
fn install_infra_cloud(context: &TestContext, release: &Path, stdout: &str, exit: i32) {
    fs::create_dir_all(release.join("backend")).expect("create cloud fixture");
    fs::create_dir_all(release.join(".venv")).expect("create cloud environment");
    fs::write(
        release.join("pyproject.toml"),
        "[project.scripts]\noqtopus-cloud-infra = \"oqtopus_cloud.infra:main\"\n",
    )
    .expect("write pyproject fixture");
    set_infra_reply(context, stdout, exit);
    context.write_executable("uv", INFRA_UV_STUB);
}

fn set_infra_reply(context: &TestContext, stdout: &str, exit: i32) {
    fs::write(context.root().join("infra-stdout"), stdout).expect("write infra stdout");
    fs::write(context.root().join("infra-exit"), exit.to_string()).expect("write infra exit");
}

fn release_dir(context: &TestContext) -> PathBuf {
    context
        .root()
        .join("xdg-data/oqtopus/cloud-local/releases/cloud-v1.8.0")
}

fn db_row(output: &Output) -> String {
    assert!(output.status.success(), "status should succeed: {output:?}");
    assert!(
        output.stderr.is_empty(),
        "stderr should stay empty: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("status should be UTF-8");
    let rows: Vec<_> = stdout.lines().collect();
    assert_eq!(rows.len(), SERVICES.len(), "every row should be printed");
    rows[0].to_owned()
}

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
fn cloud_local_status_reports_infra_and_process_services_in_manager_order() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    let release = release_dir(&context);
    install_infra_cloud(&context, &release, "db: running\nseaweedfs: running\n", 0);
    let config = context.work_dir().join("config");
    fs::create_dir(&config).expect("create config directory");
    fs::write(
        config.join(".env"),
        "STORAGE_STACK=seaweedfs\nCOMPOSE_PROJECT_NAME=from-dotenv\nVIRTUAL_ENV=/from/dotenv\n",
    )
    .expect("write config/.env");
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

    let output = context
        .rust_command(["cloud-local", "status"])
        .env("VIRTUAL_ENV", "/should/not/leak")
        .output()
        .expect("Rust CLI should run");

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

    // The infra command runs from the cloud backend without syncing, with config/.env exported,
    // the caller's virtualenv removed, and the compose project pinned to the environment name.
    let invocation =
        fs::read_to_string(context.root().join("uv-invocation")).expect("uv should be invoked");
    let release = "<TEST_ROOT>/xdg-data/oqtopus/cloud-local/releases/cloud-v1.8.0";
    assert_eq!(
        context.normalize(&invocation).lines().collect::<Vec<_>>(),
        [
            format!("cwd={release}/backend"),
            format!("args=run --no-sync --project {release} oqtopus-cloud-infra status"),
            "COMPOSE_PROJECT_NAME=characterization".into(),
            "VIRTUAL_ENV=<unset>".into(),
            "STORAGE_STACK=seaweedfs".into(),
        ]
    );
}

#[test]
fn cloud_local_status_reports_partially_running_db() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    install_infra_cloud(
        &context,
        &release_dir(&context),
        "db: running\nseaweedfs: stopped\n",
        3,
    );

    let output = context.run_snapshot_subject(["cloud-local", "status"]);

    // OQTOPUS Manager parses this exact form into its `partial` state and per-service details.
    assert_eq!(
        db_row(&output),
        "db: Partially running (db: running, seaweedfs: stopped)"
    );
    insta::assert_snapshot!(
        "cloud_local_status_partially_running",
        context.render_output(&output)
    );
}

#[test]
fn cloud_local_status_maps_infra_replies_to_the_db_row() {
    let cases: [(&str, &str, i32, &str); 11] = [
        (
            "all running",
            "db: running\nminio: running\n",
            0,
            "db: Running (db, minio)",
        ),
        (
            "stopped services listed in reported order",
            "seaweedfs: stopped\ndb: running\n",
            3,
            "db: Partially running (seaweedfs: stopped, db: running)",
        ),
        (
            "all stopped",
            "db: stopped\nseaweedfs: stopped\n",
            3,
            "db: Stopped",
        ),
        (
            "exit status is not trusted over the rows",
            "db: running\nseaweedfs: stopped\n",
            0,
            "db: Partially running (db: running, seaweedfs: stopped)",
        ),
        (
            "non-row lines are skipped",
            "Checking compose project\ndb: running\nnote: something\nseaweedfs: running\n",
            0,
            "db: Running (db, seaweedfs)",
        ),
        (
            "no trailing newline",
            "db: running\nseaweedfs: running",
            0,
            "db: Running (db, seaweedfs)",
        ),
        ("no rows", "", 0, "db: Stopped"),
        (
            "generic failure",
            "db: running\nseaweedfs: running\n",
            1,
            "db: Stopped",
        ),
        ("usage error", "db: running\n", 2, "db: Stopped"),
        ("status unknown", "db: running\n", 4, "db: Stopped"),
        ("command not found", "", 127, "db: Stopped"),
    ];
    for (label, stdout, exit, expected) in cases {
        let context = TestContext::new();
        context.create_environment(
            EnvironmentTemplate::CloudLocal,
            &[("cloud_local_cloud_version", "v1.8.0")],
        );
        install_infra_cloud(&context, &release_dir(&context), stdout, exit);

        let output = context.run_snapshot_subject(["cloud-local", "status"]);

        assert_eq!(db_row(&output), expected, "case: {label}");
    }
}

#[test]
fn cloud_local_status_reports_db_stopped_when_infra_is_unavailable() {
    // An installed cloud release that predates oqtopus-cloud-infra.
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    let release = release_dir(&context);
    install_infra_cloud(&context, &release, "db: running\n", 0);
    fs::write(
        release.join("pyproject.toml"),
        "[project]\nname = \"oqtopus-cloud\"\n",
    )
    .expect("write legacy pyproject");
    assert_eq!(
        db_row(&context.run_snapshot_subject(["cloud-local", "status"])),
        "db: Stopped"
    );
    assert!(!context.root().join("uv-invocation").exists());

    // The release directory is bound in metadata but missing from disk.
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    set_infra_reply(&context, "db: running\n", 0);
    context.write_executable("uv", INFRA_UV_STUB);
    assert_eq!(
        db_row(&context.run_snapshot_subject(["cloud-local", "status"])),
        "db: Stopped"
    );
    assert!(!context.root().join("uv-invocation").exists());

    // `uv` cannot be executed at all.
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    install_infra_cloud(&context, &release_dir(&context), "db: running\n", 0);
    context.write_executable("uv", "not an executable format");
    fs::set_permissions(
        context.root().join("bin/uv"),
        std::os::unix::fs::PermissionsExt::from_mode(0o644),
    )
    .expect("make uv non-executable");
    assert_eq!(
        db_row(&context.run_snapshot_subject(["cloud-local", "status"])),
        "db: Stopped"
    );
}

#[test]
fn cloud_local_status_does_not_create_a_missing_python_environment() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    let release = release_dir(&context);
    install_infra_cloud(&context, &release, "db: running\n", 0);
    fs::remove_dir(release.join(".venv")).expect("remove Python environment");
    assert_eq!(
        db_row(&context.run_snapshot_subject(["cloud-local", "status"])),
        "db: Stopped"
    );
    assert!(!context.root().join("uv-invocation").exists());
    assert!(!release.join(".venv").exists());
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
    // the infra command runs from a different project directory.
    install_infra_cloud(
        &context,
        &context.work_dir().join("cloud"),
        "db: running\nseaweedfs: running\n",
        0,
    );

    let output = context.run_snapshot_subject(["cloud-local", "status"]);

    assert_eq!(db_row(&output), RUNNING_DB_ROW);
    let invocation =
        fs::read_to_string(context.root().join("uv-invocation")).expect("uv should be invoked");
    assert!(
        context.normalize(&invocation).contains(
            "args=run --no-sync --project <TEST_ROOT>/work/cloud oqtopus-cloud-infra status\n"
        ),
        "{invocation}"
    );
}
