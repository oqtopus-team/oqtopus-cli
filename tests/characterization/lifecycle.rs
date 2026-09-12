use std::env;
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

use crate::harness::{EnvironmentTemplate, TestContext};

#[test]
fn lifecycle_help_is_stable() {
    let context = TestContext::new();
    for (name, args) in [
        ("backend_start_help", &["backend", "start", "help"][..]),
        ("backend_stop_help", &["backend", "stop", "--help"]),
        ("backend_restart_help", &["backend", "restart", "help"]),
        ("cloud_local_start_help", &["cloud-local", "start", "help"]),
        ("cloud_local_stop_help", &["cloud-local", "stop", "--help"]),
        (
            "cloud_local_restart_help",
            &["cloud-local", "restart", "help"],
        ),
        ("manager_start_help", &["manager", "start", "help"]),
        ("manager_stop_help", &["manager", "stop", "--help"]),
        ("manager_restart_help", &["manager", "restart", "help"]),
    ] {
        let output = context.run_snapshot_subject(args);
        assert!(output.status.success(), "{name} failed");
        assert!(output.stderr.is_empty(), "{name} wrote stderr");
        insta::assert_snapshot!(name, context.render_output(&output));
    }
}

#[test]
fn lifecycle_missing_targets_preserve_usage_and_failure() {
    for (name, template, args) in [
        (
            "backend_start_missing_target",
            EnvironmentTemplate::Backend,
            &["backend", "start"][..],
        ),
        (
            "backend_stop_missing_target",
            EnvironmentTemplate::Backend,
            &["backend", "stop"],
        ),
        (
            "backend_restart_missing_target",
            EnvironmentTemplate::Backend,
            &["backend", "restart"],
        ),
        (
            "cloud_local_start_missing_target",
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "start"],
        ),
        (
            "cloud_local_stop_missing_target",
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "stop"],
        ),
        (
            "cloud_local_restart_missing_target",
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "restart"],
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[]);
        let output = context.run_snapshot_subject(args);
        assert_eq!(output.status.code(), Some(1), "{name} succeeded");
        assert!(output.stderr.is_empty(), "{name} wrote stderr");
        insta::assert_snapshot!(name, context.render_output(&output));
    }
}

#[test]
fn stopping_absent_services_preserves_family_specific_output() {
    for (name, template, args, expected) in [
        (
            "backend_stop_absent",
            EnvironmentTemplate::Backend,
            &["backend", "stop", "core"][..],
            "core: Stopped\n",
        ),
        (
            "cloud_local_stop_absent",
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "stop", "worker"],
            "worker is not running.\n",
        ),
        (
            "manager_stop_absent",
            EnvironmentTemplate::Manager,
            &["manager", "stop"],
            "manager: Stopped\n",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[]);
        let output = context.run_snapshot_subject(args);
        assert!(output.status.success(), "{name} failed");
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
        insta::assert_snapshot!(name, context.render_output(&output));
    }
}

#[test]
fn process_services_start_and_stop_without_fallback() {
    for (template, binding, release, start_args, stop_args, log_dir, expected_args) in [
        (
            EnvironmentTemplate::Backend,
            ("engine_version", "v1.2.3"),
            "backend/releases/engine-v1.2.3/core",
            &["backend", "start", "core"][..],
            &["backend", "stop", "core"][..],
            None,
            "run --project <RELEASE> python -m oqtopus_engine_core.app -c <WORK>/config/core/config.yaml -l <WORK>/config/core/logging.yaml",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.2.3"),
            "cloud-local/releases/cloud-v1.2.3",
            &["cloud-local", "start", "user"],
            &["cloud-local", "stop", "user"],
            Some("logs/user"),
            "run --project <RELEASE> uvicorn oqtopus_cloud.user.lambda_function:app --host 0.0.0.0 --port 8080 --reload --log-level debug",
        ),
        (
            EnvironmentTemplate::Manager,
            ("manager_version", "v1.2.3"),
            "manager/releases/manager-v1.2.3",
            &["manager", "start"],
            &["manager", "stop"],
            None,
            "run --project <RELEASE> python -m oqtopus_manager.main -c <WORK>/config/config.yaml -l <WORK>/config/logging.yaml",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[binding]);
        fs::create_dir_all(context.root().join("xdg-data/oqtopus").join(release))
            .expect("create release fixture");
        if let Some(directory) = log_dir {
            fs::create_dir_all(context.work_dir().join(directory)).expect("create log fixture");
        }
        let invocation = context.root().join("uv-invocation");
        context.write_executable(
            "uv",
            b"#!/bin/sh\nset -eu\nprintf '%s\\n' \"$*\" > \"$OQTOPUS_TEST_UV_INVOCATION\"\nprintf '%s|%s|%s\\n' \"${POWERTOOLS_METRICS_NAMESPACE:-}\" \"${POWERTOOLS_SERVICE_NAME:-}\" \"${FROM_DOTENV:-}\" >> \"$OQTOPUS_TEST_UV_INVOCATION\"\nexec sleep 60\n",
        );
        fs::create_dir_all(context.work_dir().join("config")).expect("create config fixture");
        fs::write(
            context.work_dir().join("config/.env"),
            "FROM_DOTENV=loaded\n",
        )
        .expect("write env fixture");

        let started = context
            .rust_command(start_args)
            .env("OQTOPUS_TEST_UV_INVOCATION", &invocation)
            .output()
            .expect("start service");
        assert!(
            started.status.success(),
            "{}",
            String::from_utf8_lossy(&started.stderr)
        );
        assert!(String::from_utf8_lossy(&started.stdout).starts_with("Started "));
        let actual = fs::read_to_string(&invocation).expect("read uv invocation");
        let release_path = context.root().join("xdg-data/oqtopus").join(release);
        let expected = expected_args
            .replace("<RELEASE>", &release_path.display().to_string())
            .replace("<WORK>", &context.work_dir().display().to_string());
        assert_eq!(actual.lines().next(), Some(expected.as_str()));
        assert!(
            actual
                .lines()
                .nth(1)
                .is_some_and(|line| line.ends_with("|loaded"))
        );
        if template == EnvironmentTemplate::CloudLocal {
            assert_eq!(actual.lines().nth(1), Some("user-api|user-api|loaded"));
        }

        let stopped = context
            .rust_command(stop_args)
            .output()
            .expect("stop service");
        assert!(
            stopped.status.success(),
            "{}",
            String::from_utf8_lossy(&stopped.stderr)
        );
        assert!(String::from_utf8_lossy(&stopped.stdout).contains("Stopped"));
    }
}

#[test]
fn already_running_service_skips_before_dependencies_and_bindings() {
    for (template, args, service) in [
        (
            EnvironmentTemplate::Backend,
            &["backend", "start", "core"][..],
            "core",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "start", "worker"],
            "worker",
        ),
        (
            EnvironmentTemplate::Manager,
            &["manager", "start"],
            "manager",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[]);
        let pids = context.work_dir().join("pids");
        fs::create_dir_all(&pids).expect("create PID fixture");
        fs::write(
            pids.join(format!("{service}.pid")),
            format!("{}\n", std::process::id()),
        )
        .expect("write live PID");

        let output = context.rust_command(args).output().expect("start service");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8 output"),
            format!(
                "{service} is already running (PID {}); skipping.\n",
                std::process::id()
            )
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn lifecycle_routes_reject_unknown_services_and_extra_arguments() {
    for (template, args, expected) in [
        (
            EnvironmentTemplate::Backend,
            &["backend", "start", "unknown"][..],
            "Error: unknown service: unknown\n",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "stop", "unknown"],
            "Error: unknown service: unknown\n",
        ),
        (
            EnvironmentTemplate::Manager,
            &["manager", "restart", "unexpected"],
            "",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[]);

        let output = context
            .rust_command(args)
            .output()
            .expect("run lifecycle command");

        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stderr, expected.as_bytes());
        if expected.is_empty() {
            assert_eq!(output.stdout, b"Usage:\n  oqtopus manager restart\n");
        } else {
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn restart_starts_absent_process_services_without_fallback() {
    for (template, binding, release, restart_args, stop_args, log_dir, stopped_message) in [
        (
            EnvironmentTemplate::Backend,
            ("engine_version", "v1.2.3"),
            "backend/releases/engine-v1.2.3/core",
            &["backend", "restart", "core"][..],
            &["backend", "stop", "core"][..],
            None,
            "core: Stopped\n",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.2.3"),
            "cloud-local/releases/cloud-v1.2.3",
            &["cloud-local", "restart", "worker"],
            &["cloud-local", "stop", "worker"],
            Some("logs/worker"),
            "worker is not running.\n",
        ),
        (
            EnvironmentTemplate::Manager,
            ("manager_version", "v1.2.3"),
            "manager/releases/manager-v1.2.3",
            &["manager", "restart"],
            &["manager", "stop"],
            None,
            "manager: Stopped\n",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[binding]);
        fs::create_dir_all(context.root().join("xdg-data/oqtopus").join(release))
            .expect("create release fixture");
        if let Some(directory) = log_dir {
            fs::create_dir_all(context.work_dir().join(directory)).expect("create log fixture");
        }
        context.write_executable("uv", b"#!/bin/sh\nexec sleep 60\n");

        let restarted = context
            .rust_command(restart_args)
            .output()
            .expect("restart service");
        assert!(
            restarted.status.success(),
            "{}",
            String::from_utf8_lossy(&restarted.stderr)
        );
        let stdout = String::from_utf8(restarted.stdout).expect("UTF-8 output");
        assert!(stdout.starts_with(stopped_message));
        assert!(stdout.contains("Started "));

        let stopped = context
            .rust_command(stop_args)
            .output()
            .expect("stop service");
        assert!(stopped.status.success());
    }
}

#[test]
fn cloud_local_database_start_runs_setup_sequence() {
    let bash_subject = env::var("OQTOPUS_CHARACTERIZATION_SOURCE").as_deref() == Ok("bash");
    let listener = bash_subject.then(|| {
        TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3306))
            .expect("reserve the cloud-local database port")
    });
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.2.3")],
    );
    let cloud = context
        .root()
        .join("xdg-data/oqtopus/cloud-local/releases/cloud-v1.2.3");
    fs::create_dir_all(cloud.join("backend")).expect("create cloud backend fixture");
    fs::write(cloud.join("backend/compose.yaml"), "services: {}\n").expect("write compose fixture");
    context.write_executable(
        "docker",
        b"#!/bin/sh\nset -eu\ncase \"$*\" in\n  *\"ps --status running --quiet db\"*) exit 0 ;;\n  *\"exec -T db mysql\"*) exit 0 ;;\n  *) printf 'docker %s\\n' \"$*\" ;;\nesac\n",
    );
    context.write_executable("uv", b"#!/bin/sh\nset -eu\nprintf 'uv %s\\n' \"$*\"\n");

    let output = if bash_subject {
        context.run_snapshot_subject(["cloud-local", "start", "db"])
    } else {
        context
            .rust_command(["cloud-local", "start", "db"])
            .env("OQTOPUS_TEST_DB_PORT_READY", "1")
            .output()
            .expect("start cloud-local database")
    };
    drop(listener);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    insta::assert_snapshot!("cloud_local_database_start", context.render_output(&output));
}

#[test]
fn foreground_services_propagate_exit_status_and_clean_pid() {
    for (template, binding, release, args, service) in [
        (
            EnvironmentTemplate::Backend,
            ("engine_version", "v1.2.3"),
            "backend/releases/engine-v1.2.3/core",
            &["backend", "start", "core", "--foreground"][..],
            "core",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.2.3"),
            "cloud-local/releases/cloud-v1.2.3",
            &["cloud-local", "start", "worker", "--foreground"],
            "worker",
        ),
        (
            EnvironmentTemplate::Manager,
            ("manager_version", "v1.2.3"),
            "manager/releases/manager-v1.2.3",
            &["manager", "start", "--foreground"],
            "manager",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[binding]);
        fs::create_dir_all(context.root().join("xdg-data/oqtopus").join(release))
            .expect("create release fixture");
        context.write_executable("uv", b"#!/bin/sh\nexit 7\n");

        let output = context
            .rust_command(args)
            .output()
            .expect("start foreground service");

        assert_eq!(output.status.code(), Some(7));
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .starts_with(&format!("Started {service} in foreground (PID "))
        );
        assert!(
            !context
                .work_dir()
                .join(format!("pids/{service}.pid"))
                .exists()
        );
        assert!(
            !context
                .work_dir()
                .join(format!("pids/.{service}.start.lock"))
                .exists()
        );
    }
}
