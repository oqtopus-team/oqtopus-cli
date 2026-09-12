use std::fs;
use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;

use crate::harness::{EnvironmentTemplate, REMOTE_REFS_FIXTURE, TestContext};

const MANAGER_REFS_URL: &str =
    "https://github.com/oqtopus-team/oqtopus-manager.git/info/refs?service=git-upload-pack";
const MANAGER_RELEASE_URL: &str =
    "https://github.com/oqtopus-team/oqtopus-manager/archive/refs/tags/v2.0.0.tar.gz";
const TRANQU_BRANCH_URL: &str = "https://github.com/oqtopus-team/tranqu-server/archive/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tar.gz";

fn archive(top: &str, files: &[(&str, &str)]) -> Vec<u8> {
    let temporary = tempfile::tempdir().expect("create archive source");
    let root = temporary.path().join(top);
    fs::create_dir_all(&root).expect("create archive root");
    for (path, contents) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create archive parent");
        }
        fs::write(path, contents).expect("write archive file");
    }

    let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut compressed);
        tar.append_dir_all(top, &root).expect("append archive tree");
        tar.finish().expect("finish tar archive");
    }
    compressed.flush().expect("flush gzip archive");
    compressed.finish().expect("finish gzip archive")
}

#[test]
fn operation_help_is_stable() {
    for (name, args) in [
        ("backend_install", &["backend", "install", "--help"][..]),
        ("backend_build", &["backend", "build", "help"][..]),
        ("backend_uninstall", &["backend", "uninstall", "help"][..]),
        ("backend_update", &["backend", "update", "--help"][..]),
        (
            "cloud_local_install",
            &["cloud-local", "install", "help"][..],
        ),
        (
            "cloud_local_uninstall",
            &["cloud-local", "uninstall", "--help"][..],
        ),
        ("cloud_local_update", &["cloud-local", "update", "help"][..]),
        ("manager_install", &["manager", "install", "--help"][..]),
        ("manager_uninstall", &["manager", "uninstall", "help"][..]),
        ("manager_update", &["manager", "update", "--help"][..]),
    ] {
        let context = TestContext::new();
        let output = context.run_snapshot_subject(args);
        insta::assert_snapshot!(name, context.render_output(&output));
    }
}

#[test]
fn manager_update_installs_latest_and_binds_it() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::Manager,
        &[("manager_version", "v1.2.3")],
    );
    context.install_fake_uv();
    let checkout = archive(
        "oqtopus-manager-2.0.0",
        &[("pyproject.toml", "[project]\n")],
    );

    let output = context.run_snapshot_subject_with_http_fixtures(
        ["manager", "update"],
        &[
            (MANAGER_REFS_URL, REMOTE_REFS_FIXTURE),
            (MANAGER_RELEASE_URL, &checkout),
        ],
    );

    let observation = format!(
        "{}--- metadata ---\n{}--- release tree ---\n{}",
        context.render_output(&output),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
        context.render_tree("../xdg-data/oqtopus/manager/releases/manager-v2.0.0"),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn backend_branch_install_and_uninstall_update_binding() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Backend, &[]);
    context.install_fake_uv();
    let checkout = archive("tranqu-main", &[("pyproject.toml", "[project]\n")]);

    let install = context.run_snapshot_subject_with_http_fixtures(
        ["backend", "install", "tranqu", "branch:main"],
        &[
            (
                "https://github.com/oqtopus-team/tranqu-server.git/info/refs?service=git-upload-pack",
                REMOTE_REFS_FIXTURE,
            ),
            (TRANQU_BRANCH_URL, &checkout),
        ],
    );
    let uninstall = context.run_snapshot_subject(["backend", "uninstall", "tranqu", "branch:main"]);
    let observation = format!(
        "--- install ---\n{}--- uninstall ---\n{}--- metadata ---\n{}--- branch tree ---\n{}",
        context.render_output(&install),
        context.render_output(&uninstall),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
        context.render_tree("tranqu"),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn cloud_local_frontend_release_install_does_not_run_uv() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::CloudLocal, &[]);
    context.install_fake_uv();
    let checkout = archive("frontend-1.2.3", &[("package.json", "{}\n")]);
    let url = "https://github.com/oqtopus-team/oqtopus-frontend/archive/refs/tags/v1.2.3.tar.gz";

    let output = context.run_snapshot_subject_with_http_fixtures(
        ["cloud-local", "install", "frontend", "v1.2.3"],
        &[(url, &checkout)],
    );
    let observation = format!(
        "{}--- metadata ---\n{}--- release tree ---\n{}",
        context.render_output(&output),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
        context.render_tree("../xdg-data/oqtopus/cloud-local/releases/frontend-v1.2.3"),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn backend_build_streams_docker_output_in_order() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::Backend,
        &[("engine_version", "v1.2.3")],
    );
    let release = context
        .root()
        .join("xdg-data/oqtopus/backend/releases/engine-v1.2.3");
    for project in ["core", "combiner", "estimator", "mitigator"] {
        fs::create_dir_all(release.join(project).join(".venv")).unwrap();
    }
    fs::create_dir_all(release.join("sse_runtime")).unwrap();
    fs::write(release.join("sse_runtime/Dockerfile"), "FROM scratch\n").unwrap();
    fs::create_dir_all(context.work_dir().join("config")).unwrap();
    fs::write(
        context.work_dir().join("config/.env"),
        "SSE_CONTAINER_IMAGE='example/sse:1'\n",
    )
    .unwrap();
    context.install_fake_docker();

    let output = context.run_snapshot_subject(["backend", "build", "sse-runtime"]);
    let normalized = context
        .normalize(&context.render_output(&output))
        .replace(|character: char| character.is_ascii_digit(), "<N>");
    insta::assert_snapshot!(normalized);
}

#[test]
fn backend_engine_install_syncs_every_project_and_can_skip_the_image_build() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Backend, &[]);
    context.install_fake_uv();
    let checkout = archive(
        "engine-1.2.3",
        &[
            ("core/pyproject.toml", "[project]\n"),
            ("combiner/pyproject.toml", "[project]\n"),
            ("estimator/pyproject.toml", "[project]\n"),
            ("mitigator/pyproject.toml", "[project]\n"),
        ],
    );
    let output = context.run_snapshot_subject_with_http_fixtures(
        ["backend", "install", "engine", "--skip-sse-build", "v1.2.3"],
        &[(
            "https://github.com/oqtopus-team/oqtopus-engine/archive/refs/tags/v1.2.3.tar.gz",
            &checkout,
        )],
    );
    let observation = format!(
        "{}--- metadata ---\n{}",
        context.render_output(&output),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn backend_update_then_release_uninstall_keeps_the_binding() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Backend, &[]);
    context.install_fake_uv();
    let checkout = archive("gateway-2.0.0", &[("pyproject.toml", "[project]\n")]);
    let update = context.run_snapshot_subject_with_http_fixtures(
        ["backend", "update", "gateway"],
        &[
            (
                "https://github.com/oqtopus-team/device-gateway.git/info/refs?service=git-upload-pack",
                REMOTE_REFS_FIXTURE,
            ),
            (
                "https://github.com/oqtopus-team/device-gateway/archive/refs/tags/v2.0.0.tar.gz",
                &checkout,
            ),
        ],
    );
    let uninstall = context.run_snapshot_subject(["backend", "uninstall", "gateway", "v2.0.0"]);
    let observation = format!(
        "--- update ---\n{}--- uninstall ---\n{}--- metadata ---\n{}",
        context.render_output(&update),
        context.render_output(&uninstall),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn cloud_local_update_then_release_uninstall_keeps_the_binding() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::CloudLocal, &[]);
    context.install_fake_uv();
    let checkout = archive("cloud-2.0.0", &[("pyproject.toml", "[project]\n")]);
    let update = context.run_snapshot_subject_with_http_fixtures(
        ["cloud-local", "update", "cloud"],
        &[
            (
                "https://github.com/oqtopus-team/oqtopus-cloud.git/info/refs?service=git-upload-pack",
                REMOTE_REFS_FIXTURE,
            ),
            (
                "https://github.com/oqtopus-team/oqtopus-cloud/archive/refs/tags/v2.0.0.tar.gz",
                &checkout,
            ),
        ],
    );
    let uninstall = context.run_snapshot_subject(["cloud-local", "uninstall", "cloud", "v2.0.0"]);
    let observation = format!(
        "--- update ---\n{}--- uninstall ---\n{}--- metadata ---\n{}",
        context.render_output(&update),
        context.render_output(&uninstall),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn manager_branch_install_and_uninstall_update_binding() {
    let context = TestContext::new();
    context.create_environment(EnvironmentTemplate::Manager, &[]);
    context.install_fake_uv();
    let checkout = archive("manager-main", &[("pyproject.toml", "[project]\n")]);
    let install = context.run_snapshot_subject_with_http_fixtures(
        ["manager", "install", "branch:main"],
        &[
            (MANAGER_REFS_URL, REMOTE_REFS_FIXTURE),
            (
                "https://github.com/oqtopus-team/oqtopus-manager/archive/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tar.gz",
                &checkout,
            ),
        ],
    );
    let uninstall = context.run_snapshot_subject(["manager", "uninstall", "branch:main"]);
    let observation = format!(
        "--- install ---\n{}--- uninstall ---\n{}--- metadata ---\n{}",
        context.render_output(&install),
        context.render_output(&uninstall),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn operation_argument_errors_are_stable() {
    for (name, template, args) in [
        (
            "backend_all_version",
            EnvironmentTemplate::Backend,
            &["backend", "install", "all", "v1.2.3"][..],
        ),
        (
            "cloud_local_unknown_option",
            EnvironmentTemplate::CloudLocal,
            &["cloud-local", "install", "cloud", "--offline"][..],
        ),
        (
            "manager_empty_branch",
            EnvironmentTemplate::Manager,
            &["manager", "install", "branch:"][..],
        ),
        (
            "backend_build_without_engine",
            EnvironmentTemplate::Backend,
            &["backend", "build", "sse-runtime"][..],
        ),
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[]);
        let output = context.run_snapshot_subject(args);
        insta::assert_snapshot!(name, context.render_output(&output));
    }
}

#[test]
fn failed_sync_does_not_change_the_existing_binding() {
    let context = TestContext::new();
    context.create_environment(
        EnvironmentTemplate::Manager,
        &[("manager_version", "v1.2.3")],
    );
    context.write_executable(
        "uv",
        b"#!/usr/bin/env bash\nset -eu\nprintf 'uv %s\\n' \"$*\"\nexit 7\n",
    );
    let checkout = archive("manager-2.0.0", &[("pyproject.toml", "[project]\n")]);
    let output = context.run_snapshot_subject_with_http_fixtures(
        ["manager", "install", "v2.0.0"],
        &[(MANAGER_RELEASE_URL, &checkout)],
    );
    let observation = format!(
        "{}--- metadata ---\n{}",
        context.render_output(&output),
        context.normalize(&fs::read_to_string(context.work_dir().join(".metadata")).unwrap()),
    );
    insta::assert_snapshot!(context.normalize(&observation));
}

#[test]
fn empty_install_version_resolves_latest_for_every_template() {
    {
        let context = TestContext::new();
        context.create_environment(EnvironmentTemplate::Backend, &[]);
        context.install_fake_uv();
        let checkout = archive("gateway-2.0.0", &[("pyproject.toml", "[project]\n")]);
        let output = context.run_snapshot_subject_with_http_fixtures(
            ["backend", "install", "gateway", ""],
            &[
                (
                    "https://github.com/oqtopus-team/device-gateway.git/info/refs?service=git-upload-pack",
                    REMOTE_REFS_FIXTURE,
                ),
                (
                    "https://github.com/oqtopus-team/device-gateway/archive/refs/tags/v2.0.0.tar.gz",
                    &checkout,
                ),
            ],
        );
        insta::assert_snapshot!(
            "backend_empty_version",
            context.normalize(&context.render_output(&output))
        );
    }
    {
        let context = TestContext::new();
        context.create_environment(EnvironmentTemplate::CloudLocal, &[]);
        context.install_fake_uv();
        let checkout = archive("frontend-2.0.0", &[("package.json", "{}\n")]);
        let output = context.run_snapshot_subject_with_http_fixtures(
            ["cloud-local", "install", "frontend", ""],
            &[
                (
                    "https://github.com/oqtopus-team/oqtopus-frontend.git/info/refs?service=git-upload-pack",
                    REMOTE_REFS_FIXTURE,
                ),
                (
                    "https://github.com/oqtopus-team/oqtopus-frontend/archive/refs/tags/v2.0.0.tar.gz",
                    &checkout,
                ),
            ],
        );
        insta::assert_snapshot!(
            "cloud_local_empty_version",
            context.normalize(&context.render_output(&output))
        );
    }
    {
        let context = TestContext::new();
        context.create_environment(EnvironmentTemplate::Manager, &[]);
        context.install_fake_uv();
        let checkout = archive("manager-2.0.0", &[("pyproject.toml", "[project]\n")]);
        let output = context.run_snapshot_subject_with_http_fixtures(
            ["manager", "install", ""],
            &[
                (MANAGER_REFS_URL, REMOTE_REFS_FIXTURE),
                (MANAGER_RELEASE_URL, &checkout),
            ],
        );
        insta::assert_snapshot!(
            "manager_empty_version",
            context.normalize(&context.render_output(&output))
        );
    }
}
