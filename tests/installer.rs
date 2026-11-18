//! Exercises the installer against local release fixtures.

use std::fs::{self, File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::Compression;
use flate2::write::GzEncoder;

const V1: &str = "v1.1.0";
const V2: &str = "v2.0.0-beta.1";
const RELEASE: &str = "oqtopus-v2.0.0-beta.1-x86_64-unknown-linux-musl";
const BIN_DIR: &str = "~/custom bin";

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let tools = root.path().join("tools");
        fs::create_dir(&tools).unwrap();
        for (name, script) in [
            (
                "uname",
                r#"#!/bin/sh
case "$1" in
  -s) printf 'Linux\n' ;;
  -m) printf '%s\n' "${FAKE_UNAME_M:-x86_64}" ;;
  *) exit 2 ;;
esac
"#,
            ),
            (
                "curl",
                r#"#!/bin/sh
set -eu
[ "$1" = -fsSL ] && [ "$3" = -o ] && [ "$#" -eq 4 ] || exit 2
case "$2" in
  'https://github.com/oqtopus-team/oqtopus-cli.git/info/refs?service=git-upload-pack')
    fixture=refs ;;
  https://github.com/oqtopus-team/oqtopus-cli/archive/refs/tags/v1.1.0.tar.gz)
    fixture=v1.1.0.tar.gz ;;
  https://github.com/oqtopus-team/oqtopus-cli/releases/download/v2.0.0-beta.1/*)
    fixture=${2##*/} ;;
  *) printf 'Unexpected URL: %s\n' "$2" >&2; exit 2 ;;
esac
cp "$fixture" "$4"
"#,
            ),
        ] {
            let path = tools.join(name);
            fs::write(&path, script).unwrap();
            fs::set_permissions(path, Permissions::from_mode(0o755)).unwrap();
        }

        fs::write(
            root.path().join("refs"),
            b"001e# service=git-upload-pack\n0000\
0045aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/heads/main\0fixture\n\
003ebbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/tags/v1.1.0\n\
0045cccccccccccccccccccccccccccccccccccccccc refs/tags/v2.0.0-beta.1\n0000",
        )
        .unwrap();
        write_archive(
            &root.path().join("v1.1.0.tar.gz"),
            "oqtopus-cli-1.1.0/bin/oqtopus",
            "uninstalled",
        );
        let archive = root.path().join(format!("{RELEASE}.tar.gz"));
        write_archive(&archive, &format!("{RELEASE}/oqtopus"), V2);
        let checksum = Command::new("sh")
            .args([
                "-c",
                r#"if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi"#,
                "checksum",
            ])
            .arg(&archive)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert_success(&checksum);
        let digest = String::from_utf8(checksum.stdout).unwrap();
        let digest = digest.split_whitespace().next().unwrap();
        fs::write(
            root.path().join("SHA256SUMS"),
            format!("{digest}  {RELEASE}.tar.gz\n"),
        )
        .unwrap();
        Self { root }
    }

    fn install(&self, args: &[&str]) -> Output {
        self.installer(args).output().unwrap()
    }

    fn installer(&self, args: &[&str]) -> Command {
        let root = self.root.path();
        let mut command = Command::new("sh");
        command
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/install.sh"))
            .args(["--bin-dir", BIN_DIR])
            .args(args)
            .current_dir(root)
            .env_clear()
            .env("HOME", root)
            .env("TMPDIR", root)
            .env("LC_ALL", "C")
            .env("PATH", format!("{}/tools:/usr/bin:/bin", root.display()));
        command
    }

    fn binary(&self) -> PathBuf {
        self.root.path().join("custom bin/oqtopus")
    }
}

fn write_archive(path: &Path, member: &str, version: &str) {
    let contents = format!(
        "#!/bin/sh\nCLI_VERSION={version}\ncase \"$1\" in\n  version) printf '%s\\n' \"$CLI_VERSION\" ;;\n  completion) printf 'completion for %s\\n' \"$2\" ;;\nesac\n"
    );
    let encoder = GzEncoder::new(File::create(path).unwrap(), Compression::default());
    let mut archive = tar::Builder::new(encoder);
    let mut header = tar::Header::new_ustar();
    header.set_mode(0o644);
    header.set_size(contents.len() as u64);
    archive
        .append_data(&mut header, member, contents.as_bytes())
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap();
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn installs_the_stable_tag_then_upgrades_and_rolls_back() {
    let fixture = Fixture::new();
    for (args, version) in [
        (&[][..], V1),
        (&["--version", V2], V2),
        (&["--version", V1], V1),
    ] {
        assert_success(&fixture.install(args));
        let installed = Command::new(fixture.binary())
            .arg("version")
            .output()
            .unwrap();
        assert_success(&installed);
        assert_eq!(installed.stdout, format!("{version}\n").as_bytes());
    }
    let completion = fixture
        .root
        .path()
        .join(".config/fish/completions/oqtopus.fish");
    assert_eq!(
        fs::read_to_string(completion).unwrap(),
        "completion for fish\n"
    );
}

#[test]
fn checksum_mismatch_preserves_the_installed_executable() {
    let fixture = Fixture::new();
    assert_success(&fixture.install(&["--version", V1]));
    let binary = fixture.binary();
    let original = fs::read(&binary).unwrap();
    fs::write(
        fixture.root.path().join("SHA256SUMS"),
        format!("{:064}  {RELEASE}.tar.gz\n", 0),
    )
    .unwrap();

    let output = fixture.install(&["--version", V2]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("checksum mismatch"));
    assert_eq!(fs::read(binary).unwrap(), original);
}

#[test]
fn rejects_a_directory_at_the_executable_path() {
    let fixture = Fixture::new();
    let binary = fixture.binary();
    fs::create_dir_all(&binary).unwrap();
    for version in [V1, V2] {
        let output = fixture.install(&["--version", version]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("target executable path is a directory"),
            "{stderr}"
        );
        assert!(stderr.contains(binary.to_str().unwrap()), "{stderr}");
        assert_eq!(fs::read_dir(&binary).unwrap().count(), 0);
    }
}

#[test]
fn rejects_an_architecture_without_a_release_binary() {
    let fixture = Fixture::new();
    let output = fixture
        .installer(&["--version", V2])
        .env("FAKE_UNAME_M", "riscv64")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no prebuilt binary is published for Linux riscv64"),
        "{stderr}"
    );
    assert!(!fixture.binary().exists());
}
