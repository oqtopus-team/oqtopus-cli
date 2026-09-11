use std::fs;
use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::harness::TestContext;

const CREATED_AT: &str = "2031-12-13T14:15:16Z";
const MAIN_ARCHIVE_URL: &str =
    "https://github.com/oqtopus-team/oqtopus-cli/archive/refs/heads/main.tar.gz";

#[test]
fn init_backend_creates_a_rendered_environment() {
    let context = TestContext::new();
    let archive = template_archive(&context, true);
    let output = context.run_snapshot_subject_with_template_archive(
        ["init", "demo", "--template", "backend"],
        Some(&archive),
        MAIN_ARCHIVE_URL,
        CREATED_AT,
    );

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(context.work_dir().join("demo/config/.env"))
            .expect("read rendered environment file"),
        "IMAGE=demo\nPAIR=demo-demo\n"
    );
    insta::assert_snapshot!(
        "init_backend",
        render_created_environment(&context, &output, "demo", &["config/.env", "compose.yaml"])
    );
}

#[test]
fn init_cloud_local_creates_runtime_directories() {
    let context = TestContext::new();
    let archive = template_archive(&context, true);
    let output = context.run_snapshot_subject_with_template_archive(
        ["init", "cloud", "--template", "cloud-local"],
        Some(&archive),
        MAIN_ARCHIVE_URL,
        CREATED_AT,
    );

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    insta::assert_snapshot!(
        "init_cloud_local",
        render_created_environment(&context, &output, "cloud", &["config/.env"])
    );
}

#[test]
fn init_manager_creates_template_and_runtime_directories() {
    let context = TestContext::new();
    let archive = template_archive(&context, true);
    let output = context.run_snapshot_subject_with_template_archive(
        ["init", "manager", "--template", "manager"],
        Some(&archive),
        MAIN_ARCHIVE_URL,
        CREATED_AT,
    );

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    insta::assert_snapshot!(
        "init_manager",
        render_created_environment(
            &context,
            &output,
            "manager",
            &["config/config.yaml", "assets/icon.svg"],
        )
    );
}

#[test]
fn init_branch_selects_the_requested_archive_url() {
    let context = TestContext::new();
    let archive = template_archive(&context, true);
    let output = context.run_snapshot_subject_with_template_archive(
        [
            "init",
            "branch-demo",
            "--template",
            "backend",
            "--branch",
            "feature/x",
        ],
        Some(&archive),
        "https://github.com/oqtopus-team/oqtopus-cli/archive/refs/heads/feature/x.tar.gz",
        CREATED_AT,
    );

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    insta::assert_snapshot!("init_branch", context.render_output(&output));
}

#[test]
fn init_preserves_help_and_argument_errors() {
    let context = TestContext::new();
    let help = context.run_snapshot_subject(["init", "help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    insta::assert_snapshot!("init_help", context.render_output(&help));

    let alias = context.run_snapshot_subject(["init", "--help", "ignored"]);
    assert_eq!(alias.stdout, help.stdout);
    assert_eq!(alias.stderr, help.stderr);
    assert_eq!(alias.status.code(), help.status.code());

    for (name, args) in [
        ("init_no_arguments", vec!["init"]),
        ("init_empty_name", vec!["init", "", "--template", "backend"]),
        ("init_missing_template", vec!["init", "demo"]),
        (
            "init_empty_template",
            vec!["init", "demo", "--template", ""],
        ),
        (
            "init_template_without_value",
            vec!["init", "demo", "--template"],
        ),
        (
            "init_branch_without_value",
            vec!["init", "demo", "--template", "backend", "--branch"],
        ),
        (
            "init_extra_argument",
            vec!["init", "demo", "--template", "backend", "unexpected"],
        ),
    ] {
        let context = TestContext::new();
        insta::assert_snapshot!(
            name,
            context.render_output(&context.run_snapshot_subject(args))
        );
    }
}

#[test]
fn init_validates_the_name_template_and_existing_target() {
    for (name, args) in [
        (
            "init_invalid_name",
            vec!["init", "Bad_Name", "--template", "backend"],
        ),
        (
            "init_unknown_template",
            vec!["init", "demo", "--template", "unknown"],
        ),
    ] {
        let context = TestContext::new();
        let output = context.run_snapshot_subject(args);
        assert!(!context.work_dir().join("demo").exists());
        insta::assert_snapshot!(name, context.render_output(&output));
    }

    for invalid_name in ["-lead", ".lead", "_lead", "UPPER", "a b", "a/b"] {
        let context = TestContext::new();
        let output = context.run_snapshot_subject(["init", invalid_name, "--template", "backend"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .starts_with(&format!("Error: invalid env_name '{invalid_name}'.\n"))
        );
        assert!(!context.work_dir().join(invalid_name).exists());
    }

    for valid_name in ["0", "a.b-c_d"] {
        let context = TestContext::new();
        let output = context.run_snapshot_subject(["init", valid_name, "--template", "unknown"]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            output.stderr,
            b"Error: template 'unknown' is not implemented.\n"
        );
    }

    let context = TestContext::new();
    fs::create_dir(context.work_dir().join("demo")).expect("create existing target");
    insta::assert_snapshot!(
        "init_existing_target",
        context.render_output(&context.run_snapshot_subject([
            "init",
            "demo",
            "--template",
            "backend",
        ]))
    );
}

#[test]
fn init_reports_download_and_missing_template_failures() {
    let context = TestContext::new();
    let output = context.run_snapshot_subject_with_template_archive(
        ["init", "demo", "--template", "backend"],
        None,
        MAIN_ARCHIVE_URL,
        CREATED_AT,
    );
    assert!(context.work_dir().join("demo").is_dir());
    insta::assert_snapshot!(
        "init_download_failure",
        format!(
            "{}--- target tree ---\n{}",
            context.render_output(&output),
            context.render_tree("demo")
        )
    );

    let context = TestContext::new();
    let archive = template_archive(&context, false);
    let output = context.run_snapshot_subject_with_template_archive(
        ["init", "demo", "--template", "backend"],
        Some(&archive),
        MAIN_ARCHIVE_URL,
        CREATED_AT,
    );
    assert!(context.work_dir().join("demo").is_dir());
    insta::assert_snapshot!(
        "init_missing_template_in_archive",
        format!(
            "{}--- target tree ---\n{}",
            context.render_output(&output),
            context.render_tree("demo")
        )
    );
}

fn template_archive(context: &TestContext, include_backend: bool) -> Vec<u8> {
    let source = context.root().join("archive-source/oqtopus-cli-fixture");
    let templates = source.join("templates");
    if include_backend {
        write_file(
            &templates.join("backend/config/.env"),
            b"IMAGE={{ env_name }}\nPAIR={{ env_name }}-{{ env_name }}\n",
        );
        write_file(
            &templates.join("backend/config/nested/backend.toml"),
            b"enabled = true\n",
        );
        write_file(&templates.join("backend/compose.yaml"), b"services: {}\n");
    }
    write_file(&templates.join("cloud-local/config/.env"), b"ENV=local\n");
    write_file(
        &templates.join("manager/config/config.yaml"),
        b"server:\n  port: 38000\n",
    );
    write_file(&templates.join("manager/assets/icon.svg"), b"<svg></svg>\n");

    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut archive = tar::Builder::new(encoder);
    archive
        .append_dir_all("oqtopus-cli-fixture", &source)
        .expect("build template archive");
    let encoder = archive.into_inner().expect("finish tar archive");
    encoder.finish().expect("finish gzip archive")
}

fn write_file(path: &std::path::Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().expect("fixture file parent"))
        .expect("create archive fixture directory");
    fs::File::create(path)
        .expect("create archive fixture file")
        .write_all(contents)
        .expect("write archive fixture file");
}

fn render_created_environment(
    context: &TestContext,
    output: &std::process::Output,
    environment_name: &str,
    files: &[&str],
) -> String {
    let root = context.work_dir().join(environment_name);
    let metadata = fs::read_to_string(root.join(".metadata")).expect("read generated metadata");
    let mut rendered = format!(
        "{}--- tree ---\n{}--- .metadata ---\n{}",
        context.render_output(output),
        context.render_tree(environment_name),
        context.normalize(&metadata),
    );
    for file in files {
        let contents = fs::read_to_string(root.join(file)).expect("read generated fixture file");
        rendered.push_str(&format!("--- {file} ---\n{contents}"));
    }
    rendered
}
