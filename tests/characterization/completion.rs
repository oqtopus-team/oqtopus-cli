use crate::harness::TestContext;

#[test]
fn completion_prints_the_shell_script_for_each_supported_shell() {
    for (name, shell) in [
        ("completion_bash", "bash"),
        ("completion_zsh", "zsh"),
        ("completion_fish", "fish"),
    ] {
        let context = TestContext::new();
        let output = context.run_snapshot_subject(["completion", shell]);

        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        insta::assert_snapshot!(name, context.render_output(&output));
    }
}

#[test]
fn completion_preserves_help_and_argument_errors() {
    let context = TestContext::new();
    let help = context.run_snapshot_subject(["completion", "help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    insta::assert_snapshot!("completion_help", context.render_output(&help));

    let alias = context.run_snapshot_subject(["completion", "--help", "ignored"]);
    assert_eq!(alias.stdout, help.stdout);
    assert_eq!(alias.stderr, help.stderr);
    assert_eq!(alias.status.code(), help.status.code());

    for (name, args) in [
        ("completion_no_arguments", vec!["completion"]),
        (
            "completion_extra_argument",
            vec!["completion", "bash", "unexpected"],
        ),
        (
            "completion_unsupported_shell",
            vec!["completion", "powershell"],
        ),
    ] {
        let context = TestContext::new();
        insta::assert_snapshot!(
            name,
            context.render_output(&context.run_snapshot_subject(args))
        );
    }
}
