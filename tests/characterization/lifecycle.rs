use std::fs;
use std::path::Path;

use crate::harness::{EnvironmentTemplate, TestContext};

const CLOUD_ENTRIES: [(&str, &str); 5] = [
    ("user", "oqtopus-cloud-user-api"),
    ("provider", "oqtopus-cloud-provider-api"),
    ("admin", "oqtopus-cloud-admin-api"),
    ("user_signup", "oqtopus-cloud-user-signup-api"),
    ("worker", "oqtopus-cloud-worker"),
];

fn write_cloud_entries(project: &Path) {
    let mut scripts = String::from("[project.scripts]\noqtopus-cloud-infra = 'fixture:main'\n");
    for (_, entry) in CLOUD_ENTRIES {
        scripts.push_str(&format!("{entry} = 'fixture:main'\n"));
    }
    fs::write(project.join("pyproject.toml"), scripts).expect("write cloud entry points");
}

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
            "run --no-sync --project <RELEASE> python -m oqtopus_engine_core.app -c <WORK>/config/core/config.yaml -l <WORK>/config/core/logging.yaml",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "user"],
            &["cloud-local", "stop", "user"],
            Some("logs/user"),
            "run --no-sync --project <RELEASE> oqtopus-cloud-user-api",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "provider"],
            &["cloud-local", "stop", "provider"],
            Some("logs/provider"),
            "run --no-sync --project <RELEASE> oqtopus-cloud-provider-api",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "admin"],
            &["cloud-local", "stop", "admin"],
            Some("logs/admin"),
            "run --no-sync --project <RELEASE> oqtopus-cloud-admin-api",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "user_signup"],
            &["cloud-local", "stop", "user_signup"],
            Some("logs/user_signup"),
            "run --no-sync --project <RELEASE> oqtopus-cloud-user-signup-api",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "worker", "ignored"],
            &["cloud-local", "stop", "worker"],
            Some("logs/worker"),
            "run --no-sync --project <RELEASE> oqtopus-cloud-worker",
        ),
        (
            EnvironmentTemplate::Manager,
            ("manager_version", "v1.2.3"),
            "manager/releases/manager-v1.2.3",
            &["manager", "start"],
            &["manager", "stop"],
            None,
            "run --no-sync --project <RELEASE> python -m oqtopus_manager.main -c <WORK>/config/config.yaml -l <WORK>/config/logging.yaml",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[binding]);
        let project = context.create_installed_project(release);
        if template == EnvironmentTemplate::CloudLocal {
            write_cloud_entries(&project);
        }
        if let Some(directory) = log_dir {
            fs::create_dir_all(context.work_dir().join(directory)).expect("create log fixture");
        }
        let invocation = context.root().join("uv-invocation");
        context.write_executable(
            "uv",
            b"#!/bin/sh\nset -eu\nprintf '%s\\n' \"$*\" > \"$OQTOPUS_TEST_UV_INVOCATION\"\nprintf '%s|%s|%s\\n' \"${POWERTOOLS_METRICS_NAMESPACE:-}\" \"${POWERTOOLS_SERVICE_NAME:-}\" \"${FROM_DOTENV:-}\" >> \"$OQTOPUS_TEST_UV_INVOCATION\"\nprintf '%s\\n' \"${VIRTUAL_ENV:-}\" >> \"$OQTOPUS_TEST_UV_INVOCATION\"\nexec sleep 60\n",
        );
        fs::create_dir_all(context.work_dir().join("config")).expect("create config fixture");
        fs::write(
            context.work_dir().join("config/.env"),
            "FROM_DOTENV=loaded\nVIRTUAL_ENV=/from/dotenv\nPOWERTOOLS_METRICS_NAMESPACE=custom-metrics\nPOWERTOOLS_SERVICE_NAME=custom-service\n",
        )
        .expect("write env fixture");

        let started = context
            .rust_command(start_args)
            .env("OQTOPUS_TEST_UV_INVOCATION", &invocation)
            .env("VIRTUAL_ENV", "/from/caller")
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
        // The manager never received `config/.env`; the other services do.
        let dotenv = if template == EnvironmentTemplate::Manager {
            ""
        } else {
            "loaded"
        };
        assert!(
            actual
                .lines()
                .nth(1)
                .is_some_and(|line| line.ends_with(&format!("|{dotenv}"))),
            "{actual}"
        );
        // `uv` must pick the project environment, whichever source names another one.
        assert_eq!(actual.lines().nth(2), Some(""), "{actual}");
        if template == EnvironmentTemplate::CloudLocal {
            assert_eq!(
                actual.lines().nth(1),
                Some("custom-metrics|custom-service|loaded")
            );
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
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
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
        let project = context.create_installed_project(release);
        if template == EnvironmentTemplate::CloudLocal {
            write_cloud_entries(&project);
        }
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

/// Infra owns Compose and setup; the fixture intentionally contains no compose.yaml or scripts.
fn database_context() -> (TestContext, std::path::PathBuf) {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "v1.8.0")],
    );
    let cloud = context.create_installed_project("cloud-local/releases/cloud-v1.8.0");
    fs::create_dir(cloud.join("backend")).expect("create cloud backend fixture");
    write_cloud_entries(&cloud);
    fs::create_dir(context.work_dir().join("config")).expect("create config fixture");
    fs::write(
        context.work_dir().join("config/.env"),
        [
            "STORAGE_STACK=seaweedfs\n",
            "COMPOSE_PROJECT_NAME=wrong-project\n",
            "VIRTUAL_ENV=/from/dotenv\n",
            "DB_USERNAME=custom-user\n",
            "DB_PASSWORD=custom-password\n",
        ]
        .concat(),
    )
    .expect("write cloud config");
    context.write_executable(
        "docker",
        "#!/bin/sh\necho 'unexpected docker invocation' >&2\nexit 99\n",
    );
    context.write_executable(
        "uv",
        r#"#!/bin/sh
root=$(dirname "$0")/..
{
  printf 'args=%s\n' "$*"
  printf 'cwd=%s\n' "$PWD"
  printf 'project=%s\n' "$COMPOSE_PROJECT_NAME"
  printf 'venv=%s\n' "${VIRTUAL_ENV-<unset>}"
  printf 'storage=%s\n' "$STORAGE_STACK"
  printf 'credentials=%s:%s\n' "$DB_USERNAME" "$DB_PASSWORD"
} >> "$root/infra-invocations"
printf 'uv %s\n' "$*"
if [ "${INFRA_EXIT:-0}" != 0 ]; then
  echo 'infra failed' >&2
fi
exit "${INFRA_EXIT:-0}"
"#,
    );
    (context, cloud)
}

#[test]
fn cloud_local_database_lifecycle_delegates_to_infra() {
    for (action, commands) in [
        ("start", vec!["start"]),
        ("stop", vec!["stop"]),
        ("restart", vec!["stop", "start"]),
    ] {
        let (context, cloud) = database_context();
        let output = context
            .rust_command(["cloud-local", action, "db"])
            .env("VIRTUAL_ENV", "/from/caller")
            .output()
            .expect("run database lifecycle");
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let invocations = fs::read_to_string(context.root().join("infra-invocations"))
            .expect("read infra invocations");
        // The compose project and uv environment are pinned even when config/.env names others.
        let expected: Vec<String> = commands
            .iter()
            .flat_map(|command| {
                [
                    format!(
                        "args=run --no-sync --project {} oqtopus-cloud-infra {command}",
                        cloud.display()
                    ),
                    format!("cwd={}", cloud.join("backend").display()),
                    "project=characterization".into(),
                    "venv=<unset>".into(),
                    "storage=seaweedfs".into(),
                    "credentials=custom-user:custom-password".into(),
                ]
            })
            .collect();
        assert_eq!(invocations.lines().collect::<Vec<_>>(), expected);
        if action == "start" {
            insta::assert_snapshot!("cloud_local_database_start", context.render_output(&output));
        } else {
            let expected: Vec<String> = commands
                .iter()
                .flat_map(|command| {
                    let (before, after) = match *command {
                        "start" => ("Starting", "Started"),
                        _ => ("Stopping", "Stopped"),
                    };
                    [
                        format!("{before} db"),
                        format!(
                            "uv run --no-sync --project {} oqtopus-cloud-infra {command}",
                            cloud.display()
                        ),
                        format!("{after} db"),
                    ]
                })
                .collect();
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert_eq!(stdout.lines().collect::<Vec<_>>(), expected);
        }
    }
}

#[test]
fn cloud_local_all_preserves_lifecycle_order() {
    let (context, _) = database_context();
    for (service, _) in CLOUD_ENTRIES {
        fs::create_dir_all(context.work_dir().join(format!("logs/{service}"))).unwrap();
    }
    context.write_executable(
        "uv",
        r#"#!/bin/sh
root=$(dirname "$0")/..
if [ "$5" = oqtopus-cloud-infra ]; then
  printf 'infra %s\n' "$6" >> "$root/calls"
else
  printf '%s\n' "$5" >> "$root/calls"
  exec sleep 60
fi
"#,
    );
    let stop_rows = || {
        let mut rows: Vec<_> = ["user", "provider", "admin", "user_signup", "worker"]
            .iter()
            .map(|service| {
                let pid =
                    fs::read_to_string(context.work_dir().join(format!("pids/{service}.pid")))
                        .unwrap_or_default();
                format!("Stopped {service} (PID {})", pid.trim())
            })
            .collect();
        rows.extend(["Stopping db".into(), "Stopped db".into()]);
        rows
    };
    let started = context.run_snapshot_subject(["cloud-local", "start", "all"]);
    let restart_stop_rows = stop_rows();
    let restarted = context.run_snapshot_subject(["cloud-local", "restart", "all"]);
    let final_stop_rows = stop_rows();
    // Stop before asserting so a failed assertion cannot leave the fixture's services running.
    let stopped = context.run_snapshot_subject(["cloud-local", "stop", "all"]);
    assert!(started.status.success(), "{started:?}");
    assert!(restarted.status.success(), "{restarted:?}");
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(started.stderr.is_empty() && restarted.stderr.is_empty() && stopped.stderr.is_empty());
    let startup = [
        "infra start",
        "oqtopus-cloud-worker",
        "oqtopus-cloud-user-signup-api",
        "oqtopus-cloud-admin-api",
        "oqtopus-cloud-provider-api",
        "oqtopus-cloud-user-api",
    ];
    // start all, then restart all (stop + start), then the final stop all.
    let expected_calls = [&startup[..], &["infra stop"], &startup, &["infra stop"]].concat();
    assert_eq!(
        fs::read_to_string(context.root().join("calls"))
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        expected_calls
    );
    assert_eq!(
        String::from_utf8(stopped.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        final_stop_rows
    );
    let restart_stdout = String::from_utf8(restarted.stdout).unwrap();
    assert_eq!(
        restart_stdout
            .lines()
            .take(restart_stop_rows.len())
            .collect::<Vec<_>>(),
        restart_stop_rows
    );
    for (service, _) in CLOUD_ENTRIES {
        assert!(
            !context
                .work_dir()
                .join(format!("pids/{service}.pid"))
                .exists()
        );
    }
}

#[test]
fn cloud_local_entry_points_receive_template_and_custom_startup_settings() {
    let (context, _) = database_context();
    let template = include_str!("../../templates/cloud-local/config/.env");
    fs::write(context.work_dir().join("config/.env"), format!(
        "{template}USER_API_HOST=127.0.0.2\nUSER_API_PORT=18080\nUSER_API_RELOAD=false\nUSER_API_LOG_LEVEL=warning\nUSER_API_WORKERS=2\n"
    )).unwrap();
    context.write_executable(
        "uv",
        r#"#!/bin/sh
root=$(dirname "$0")/..
env > "$root/service-env"
"#,
    );
    let output = context.run_snapshot_subject(["cloud-local", "start", "user", "--foreground"]);
    assert!(output.status.success(), "{output:?}");
    let exports = fs::read_to_string(context.root().join("service-env")).unwrap();
    for expected in [
        "USER_API_HOST=127.0.0.2",
        "USER_API_PORT=18080",
        "USER_API_RELOAD=false",
        "USER_API_LOG_LEVEL=warning",
        "USER_API_WORKERS=2",
        "USER_API_POWERTOOLS_SERVICE_NAME=user-api",
        "USER_API_POWERTOOLS_METRICS_NAMESPACE=user-api",
        "PROVIDER_API_HOST=0.0.0.0",
        "PROVIDER_API_PORT=8888",
        "PROVIDER_API_RELOAD=true",
        "ADMIN_API_PORT=8889",
        "USER_SIGNUP_API_PORT=8890",
        "STORAGE_STACK=seaweedfs",
        "DB_USERNAME=admin",
        "DB_PASSWORD=password",
        "STORAGE_SEAWEEDFS_ENDPOINT_URL=http://localhost:8333",
    ] {
        assert!(
            exports.lines().any(|line| line == expected),
            "missing {expected}: {exports}"
        );
    }
    assert!(
        !exports
            .lines()
            .any(|line| line.starts_with("STORAGE_DRIVER=")
                || line.starts_with("POWERTOOLS_SERVICE_NAME=")
                || line.starts_with("POWERTOOLS_METRICS_NAMESPACE="))
    );
}

#[test]
fn cloud_local_database_failure_does_not_report_success_or_restart() {
    for (action, target) in [
        ("start", "db"),
        ("stop", "db"),
        ("restart", "db"),
        ("start", "all"),
        ("restart", "all"),
    ] {
        let (context, _) = database_context();
        let output = context
            .rust_command(["cloud-local", action, target])
            .env("INFRA_EXIT", "7")
            .output()
            .expect("run failing infra");
        assert_eq!(output.status.code(), Some(1));
        let command = if action == "start" { "start" } else { "stop" };
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("infra failed\nError: failed to {command} db.\n")
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(!stdout.contains("Started db") && !stdout.contains("Stopped db"));
        let calls = fs::read_to_string(context.root().join("infra-invocations")).unwrap();
        assert_eq!(
            calls
                .lines()
                .filter(|line| line.starts_with("args="))
                .count(),
            1
        );
    }
}

#[test]
fn cloud_local_database_lifecycle_requires_installed_environment() {
    for action in ["start", "stop"] {
        let (context, cloud) = database_context();
        fs::remove_dir(cloud.join(".venv")).expect("remove environment fixture");
        let output = context.run_snapshot_subject(["cloud-local", action, "db"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!(
                "Error: cannot {action} 'db': Python environment not found: {}. Install the component again to recreate it.\n",
                cloud.join(".venv").display()
            )
        );
        assert!(!context.root().join("infra-invocations").exists());
        assert!(!cloud.join(".venv").exists());
    }
}

#[test]
fn cloud_local_lifecycle_rejects_missing_entry_points() {
    for (service, entry, action) in CLOUD_ENTRIES
        .into_iter()
        .map(|(service, entry)| (service, entry, "start"))
        .chain([
            ("db", "oqtopus-cloud-infra", "start"),
            ("db", "oqtopus-cloud-infra", "stop"),
        ])
    {
        let (context, cloud) = database_context();
        fs::write(
            cloud.join("pyproject.toml"),
            "[project]\nname = 'old-cloud'\n",
        )
        .unwrap();
        let output = context.run_snapshot_subject(["cloud-local", action, service]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!(
                "Error: cannot {action} '{service}': the installed cloud component does not provide {entry}. Update it with 'oqtopus cloud-local update cloud'.\n"
            )
        );
        assert!(!context.root().join("infra-invocations").exists());
        assert!(
            !context
                .work_dir()
                .join(format!("pids/{service}.pid"))
                .exists()
        );
    }
}

#[test]
fn cloud_local_database_lifecycle_resolves_branch_installations() {
    let (context, cloud) = database_context();
    context.create_environment(
        EnvironmentTemplate::CloudLocal,
        &[("cloud_local_cloud_version", "branch:main")],
    );
    let branch = context.work_dir().join("cloud");
    fs::rename(&cloud, &branch).expect("move cloud to branch checkout");
    let output = context.run_snapshot_subject(["cloud-local", "start", "db"]);
    assert!(output.status.success(), "{output:?}");
    let calls = fs::read_to_string(context.root().join("infra-invocations")).unwrap();
    assert!(calls.contains(&format!(
        "--project {} oqtopus-cloud-infra start",
        branch.display()
    )));
    assert!(calls.contains(&format!("cwd={}\n", branch.join("backend").display())));
}

#[test]
fn process_services_require_installed_environment() {
    for (template, binding, release, args, prefix) in [
        (
            EnvironmentTemplate::Backend,
            ("engine_version", "v1.2.3"),
            "backend/releases/engine-v1.2.3/core",
            &["backend", "start", "core"][..],
            "cannot start 'core'. ",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "worker"],
            "cannot start 'worker': ",
        ),
        (
            EnvironmentTemplate::CloudLocal,
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
            &["cloud-local", "start", "user"],
            "cannot start 'user': ",
        ),
        (
            EnvironmentTemplate::Manager,
            ("manager_version", "v1.2.3"),
            "manager/releases/manager-v1.2.3",
            &["manager", "start"],
            "cannot start manager. ",
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[binding]);
        let project = context.root().join("xdg-data/oqtopus").join(release);
        fs::create_dir_all(&project).expect("create release fixture without environment");
        let invoked = context.root().join("uv-invoked");
        context.write_executable(
            "uv",
            format!("#!/bin/sh\ntouch '{}'\n", invoked.display()).as_bytes(),
        );

        let output = context.rust_command(args).output().expect("start service");

        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8 output"),
            format!(
                "Error: {prefix}Python environment not found: {}. Install the component again to recreate it.\n",
                project.join(".venv").display()
            ),
            "{args:?}"
        );
        // `uv run --no-sync` would have created an empty `.venv`, making the release look installed.
        assert!(!invoked.exists(), "{args:?}: uv must not run");
        assert!(!project.join(".venv").exists(), "{args:?}");
        let service = if args[0] == "manager" {
            "manager"
        } else {
            args[2]
        };
        assert!(
            !context
                .work_dir()
                .join(format!("pids/{service}.pid"))
                .exists(),
            "{args:?}"
        );
    }
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
            ("cloud_local_cloud_version", "v1.8.0"),
            "cloud-local/releases/cloud-v1.8.0",
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
        let project = context.create_installed_project(release);
        if template == EnvironmentTemplate::CloudLocal {
            write_cloud_entries(&project);
        }
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

#[test]
fn empty_start_and_install_targets_show_usage() {
    for (template, domain) in [
        (EnvironmentTemplate::Backend, "backend"),
        (EnvironmentTemplate::CloudLocal, "cloud-local"),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[]);
        for action in ["start", "install"] {
            for suffix in [vec![""], vec!["", "extra"]] {
                let output = context
                    .rust_command([vec![domain, action], suffix].concat())
                    .output()
                    .unwrap();
                let help = context
                    .rust_command([domain, action, "help"])
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(1));
                assert_eq!(output.stdout, help.stdout);
                assert!(output.stderr.is_empty());
            }
        }
    }
}

#[test]
fn cloud_operations_require_name_before_install_root() {
    for action in ["install", "uninstall", "update"] {
        let context = TestContext::new();
        context.write_metadata(format!(
            "template=cloud-local\nenvironment_root={}\n",
            context.work_dir().display()
        ));
        let output = context
            .rust_command(["cloud-local", action, "cloud"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"Error: invalid .metadata: missing environment_name.\n"
        );
    }
}
