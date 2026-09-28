use crate::harness::TestContext;

#[test]
fn unknown_commands_are_rejected_at_every_level() {
    for args in [
        vec!["not-a-real-command"],
        vec!["backend", "frobnicate"],
        vec!["cloud-local", "frobnicate"],
        vec!["manager", "frobnicate"],
    ] {
        let context = TestContext::new();
        let output = context.run_snapshot_subject(&args);

        assert_eq!(output.status.code(), Some(1), "wrong status for {args:?}");
        assert!(
            output.stdout.is_empty(),
            "stdout was not empty for {args:?}"
        );
        assert_eq!(
            output.stderr,
            format!("Error: unknown {}\n", unknown_message(&args)).as_bytes(),
            "wrong stderr for {args:?}"
        );
    }
}

fn unknown_message(args: &[&str]) -> String {
    match args {
        [command] => format!("command: {command}"),
        [domain, action] => format!("{domain} command: {action}"),
        _ => unreachable!("test cases only use one or two words"),
    }
}

#[test]
fn domain_level_help_is_stable_and_covers_every_spelling() {
    for domain in ["backend", "cloud-local", "manager"] {
        let context = TestContext::new();
        let canonical = context.run_snapshot_subject([domain]);

        assert!(canonical.status.success(), "failed for bare {domain}");
        assert!(canonical.stderr.is_empty(), "stderr for bare {domain}");
        insta::assert_snapshot!(
            format!("{}_help", domain.replace('-', "_")),
            context.normalize(&String::from_utf8_lossy(&canonical.stdout))
        );

        for action in ["help", "--help"] {
            let alias = context.run_snapshot_subject([domain, action, "ignored"]);
            assert!(
                alias.status.success(),
                "failed for {domain} {action} ignored"
            );
            assert_eq!(
                alias.stdout, canonical.stdout,
                "help differed for {domain} {action}"
            );
            assert!(
                alias.stderr.is_empty(),
                "stderr for {domain} {action} ignored"
            );
        }
    }
}
