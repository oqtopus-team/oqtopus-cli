mod backend;
mod cloud_local;
mod harness;
mod init;
mod manager;
mod operations;

use harness::TestContext;

#[test]
fn top_level_help() {
    let context = TestContext::new();
    // The no-argument form is the canonical output; every accepted help spelling must remain
    // byte-for-byte equivalent so aliases cannot drift independently.
    let canonical = context.run_snapshot_subject::<_, &str>([]);

    assert!(canonical.status.success());
    assert!(canonical.stderr.is_empty());

    let stdout = context.normalize(&String::from_utf8_lossy(&canonical.stdout));
    insta::assert_snapshot!(stdout);

    for args in [
        &[][..],
        &["help"][..],
        &["--help"],
        &["help", "ignored"],
        &["--help", "ignored"],
    ] {
        let alias = context.run_snapshot_subject(args);

        assert!(alias.status.success(), "CLI failed for arguments {args:?}");
        assert!(alias.stderr.is_empty(), "CLI wrote stderr for {args:?}");
        assert_eq!(alias.stdout, canonical.stdout, "help differed for {args:?}");
    }
}
