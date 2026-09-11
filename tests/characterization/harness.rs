use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// Selects which implementation the snapshots are taken from. Unset means the Rust executable,
/// which is how tests normally run; `make record-characterization` sets it to `bash` to capture
/// the legacy contract before a command is ported.
const CHARACTERIZATION_SOURCE: &str = "OQTOPUS_CHARACTERIZATION_SOURCE";
const FORBID_LEGACY_FALLBACK: &str = "OQTOPUS_FORBID_LEGACY_FALLBACK";

pub const REMOTE_REFS_FIXTURE: &[u8] =
    b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/heads/main\n\
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/tags/v1.2.3\n\
cccccccccccccccccccccccccccccccccccccccc refs/tags/v1.10.0\n\
dddddddddddddddddddddddddddddddddddddddd refs/tags/v2.0.0\n\
eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee refs/tags/v2.0.0^{}\n\
ffffffffffffffffffffffffffffffffffffffff refs/tags/v2.0.0-rc.1\n";

/// Isolated filesystem and process environment shared by characterization tests.
///
/// Paths and locale-sensitive settings are controlled so snapshots describe CLI behavior rather
/// than properties of the developer machine running them.
pub struct TestContext {
    // Retaining the handle keeps the temporary tree alive for the lifetime of the context.
    _sandbox: tempfile::TempDir,
    root: PathBuf,
    work: PathBuf,
}

impl TestContext {
    pub fn new() -> Self {
        let sandbox = tempfile::tempdir().expect("create characterization sandbox");
        let root = fs::canonicalize(sandbox.path()).expect("resolve characterization sandbox");
        let work = root.join("work");

        for directory in [
            &work,
            &root.join("home"),
            &root.join("tmp"),
            &root.join("xdg-data"),
        ] {
            fs::create_dir_all(directory).expect("create characterization directory");
        }

        Self {
            _sandbox: sandbox,
            root,
            work,
        }
    }

    pub fn create_environment(&self, template: EnvironmentTemplate, bindings: &[(&str, &str)]) {
        let install_root = self
            .root
            .join("xdg-data/oqtopus")
            .join(template.name())
            .join("releases");
        fs::create_dir_all(&install_root).expect("create fixture install root");

        let mut metadata = format!(
            "template={}\ninstall_root={}\nenvironment_name=characterization\nenvironment_root={}\ncreated_at=2000-01-02T03:04:05Z\n",
            template.name(),
            install_root.display(),
            self.work.display(),
        );
        for (key, value) in bindings {
            metadata.push_str(&format!("{key}={value}\n"));
        }
        fs::write(self.work.join(".metadata"), metadata).expect("write fixture metadata");
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn work_dir(&self) -> &Path {
        &self.work
    }

    pub fn write_metadata(&self, contents: impl AsRef<[u8]>) {
        fs::write(self.work.join(".metadata"), contents).expect("write fixture metadata");
    }

    pub fn write_executable(&self, name: &str, contents: impl AsRef<[u8]>) {
        let bin = self.root.join("bin");
        fs::create_dir_all(&bin).expect("create fixture bin directory");
        let path = bin.join(name);
        fs::write(&path, contents).expect("write fixture executable");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("make fixture executable runnable");
    }

    pub fn run_snapshot_subject<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // Rust is the normal subject. Selecting Bash lets the same snapshots capture the legacy
        // contract before a command is migrated, without duplicating the test cases.
        match env::var(CHARACTERIZATION_SOURCE).as_deref() {
            Err(env::VarError::NotPresent) => self.run_rust(args),
            Ok("bash") => self.run_bash(args),
            Ok(source) => panic!("unsupported {CHARACTERIZATION_SOURCE} value: {source}"),
            Err(error) => panic!("invalid {CHARACTERIZATION_SOURCE} value: {error}"),
        }
    }

    /// Runs a command against a deterministic git smart-HTTP advertisement.
    ///
    /// Bash receives the fixture through a fake `curl`, while Rust receives the same bytes through
    /// a test-only file hook. Normal characterization runs therefore never depend on GitHub.
    pub fn run_snapshot_subject_with_remote_refs<I, S>(
        &self,
        args: I,
        refs: &[u8],
        fail_request: bool,
    ) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect();

        match env::var(CHARACTERIZATION_SOURCE).as_deref() {
            Ok("bash") => {
                let fixture = self.root.join("remote-refs.fixture");
                fs::write(&fixture, refs).expect("write remote refs fixture");
                // This and the other fake `curl` scripts exit 22 to report a failed request. That
                // is curl's own "HTTP error returned" status; only the failure matters to the CLI,
                // but a code real curl can produce keeps the fixture honest about what it imitates.
                self.write_executable(
                    "curl",
                    b"#!/usr/bin/env bash\nset -eu\n[[ ${OQTOPUS_TEST_REMOTE_FAILURE:-0} == 0 ]] || exit 22\nout=\nwhile [[ $# -gt 0 ]]; do\n  if [[ $1 == -o ]]; then out=$2; shift 2; else shift; fi\ndone\ncp \"$OQTOPUS_TEST_REMOTE_REFS\" \"$out\"\n",
                );

                let legacy_cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/oqtopus");
                let mut command = Command::new(bash_program());
                command.arg(legacy_cli).args(&args);
                self.configure(&mut command);
                command.env("OQTOPUS_TEST_REMOTE_REFS", fixture).env(
                    "OQTOPUS_TEST_REMOTE_FAILURE",
                    if fail_request { "1" } else { "0" },
                );
                command.output().expect("legacy Bash CLI should run")
            }
            Err(env::VarError::NotPresent) => {
                let fixture = if fail_request {
                    self.root.join("missing-remote-refs.fixture")
                } else {
                    let fixture = self.root.join("remote-refs.fixture");
                    fs::write(&fixture, refs).expect("write remote refs fixture");
                    fixture
                };
                let mut command = self.rust_command(&args);
                command.env("OQTOPUS_TEST_HTTP_RESPONSE_FILE", fixture);
                command.output().expect("Rust CLI should run")
            }
            Ok(source) => panic!("unsupported {CHARACTERIZATION_SOURCE} value: {source}"),
            Err(error) => panic!("invalid {CHARACTERIZATION_SOURCE} value: {error}"),
        }
    }

    pub fn run_snapshot_subject_with_template_archive<I, S>(
        &self,
        args: I,
        archive: Option<&[u8]>,
        expected_url: &str,
        created_at: &str,
    ) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect();
        let fixture = self.root.join("template.tar.gz");
        if let Some(archive) = archive {
            fs::write(&fixture, archive).expect("write template archive fixture");
        }

        match env::var(CHARACTERIZATION_SOURCE).as_deref() {
            Ok("bash") => {
                self.write_executable(
                    "curl",
                    b"#!/usr/bin/env bash\nset -eu\nout=\nurl=\nwhile [[ $# -gt 0 ]]; do\n  if [[ $1 == -o ]]; then out=$2; shift 2\n  elif [[ $1 == http://* || $1 == https://* ]]; then url=$1; shift\n  else shift\n  fi\ndone\n[[ $url == \"$OQTOPUS_TEST_EXPECTED_HTTP_URL\" ]] || exit 22\n[[ -f $OQTOPUS_TEST_HTTP_RESPONSE_FILE ]] || exit 22\ncp \"$OQTOPUS_TEST_HTTP_RESPONSE_FILE\" \"$out\"\n",
                );
                // `init` stamps a creation time into the metadata it writes, which would otherwise
                // differ on every run. Bash shells out to `date`, so the Bash subject needs this
                // stub; the Rust subject reads the same variable directly and needs no fake.
                self.write_executable(
                    "date",
                    b"#!/usr/bin/env bash\nset -eu\nprintf '%s\\n' \"$OQTOPUS_TEST_CREATED_AT\"\n",
                );
                let legacy_cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/oqtopus");
                let mut command = Command::new(bash_program());
                command.arg(legacy_cli).args(&args);
                self.configure(&mut command);
                command
                    .env("OQTOPUS_TEST_HTTP_RESPONSE_FILE", &fixture)
                    .env("OQTOPUS_TEST_EXPECTED_HTTP_URL", expected_url)
                    .env("OQTOPUS_TEST_CREATED_AT", created_at);
                command.output().expect("legacy Bash CLI should run")
            }
            Err(env::VarError::NotPresent) => {
                let mut command = self.rust_command(&args);
                command
                    .env("OQTOPUS_TEST_HTTP_RESPONSE_FILE", &fixture)
                    .env("OQTOPUS_TEST_EXPECTED_HTTP_URL", expected_url)
                    .env("OQTOPUS_TEST_CREATED_AT", created_at);
                command.output().expect("Rust CLI should run")
            }
            Ok(source) => panic!("unsupported {CHARACTERIZATION_SOURCE} value: {source}"),
            Err(error) => panic!("invalid {CHARACTERIZATION_SOURCE} value: {error}"),
        }
    }

    /// Runs a command with deterministic URL-to-response mappings.
    ///
    /// This supports operations such as `update` and branch installation that fetch refs first
    /// and an archive second. Bash receives a fake `curl`; Rust reads the same manifest through
    /// its test-only HTTP fixture hook.
    pub fn run_snapshot_subject_with_http_fixtures<I, S>(
        &self,
        args: I,
        fixtures: &[(&str, &[u8])],
    ) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect();
        let manifest = self.root.join("http-fixtures.tsv");
        let mut mappings = String::new();
        for (index, (url, contents)) in fixtures.iter().enumerate() {
            let path = self.root.join(format!("http-fixture-{index}"));
            fs::write(&path, contents).expect("write HTTP fixture");
            mappings.push_str(url);
            mappings.push('\t');
            mappings.push_str(&path.to_string_lossy());
            mappings.push('\n');
        }
        fs::write(&manifest, mappings).expect("write HTTP fixture manifest");

        match env::var(CHARACTERIZATION_SOURCE).as_deref() {
            Ok("bash") => {
                self.write_executable(
                    "curl",
                    b"#!/usr/bin/env bash\nset -eu\nout=\nurl=\nwhile [[ $# -gt 0 ]]; do\n  if [[ $1 == -o ]]; then out=$2; shift 2\n  elif [[ $1 == http://* || $1 == https://* ]]; then url=$1; shift\n  else shift\n  fi\ndone\nwhile IFS=$'\\t' read -r expected fixture; do\n  if [[ $url == \"$expected\" ]]; then cp \"$fixture\" \"$out\"; exit 0; fi\ndone < \"$OQTOPUS_TEST_HTTP_FIXTURE_MANIFEST\"\nexit 22\n",
                );
                let legacy_cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/oqtopus");
                let mut command = Command::new(bash_program());
                command.arg(legacy_cli).args(&args);
                self.configure(&mut command);
                command.env("OQTOPUS_TEST_HTTP_FIXTURE_MANIFEST", manifest);
                command.output().expect("legacy Bash CLI should run")
            }
            Err(env::VarError::NotPresent) => {
                let mut command = self.rust_command(&args);
                command.env("OQTOPUS_TEST_HTTP_FIXTURE_MANIFEST", manifest);
                command.output().expect("Rust CLI should run")
            }
            Ok(source) => panic!("unsupported {CHARACTERIZATION_SOURCE} value: {source}"),
            Err(error) => panic!("invalid {CHARACTERIZATION_SOURCE} value: {error}"),
        }
    }

    /// Installs a `uv` that echoes its arguments and creates the `.venv` the CLI looks for.
    ///
    /// Creating the directory is not cosmetic: the CLI treats it as the marker of a finished
    /// installation, so without it a second install would report the tree as incomplete.
    pub fn install_fake_uv(&self) {
        self.write_executable(
            "uv",
            b"#!/usr/bin/env bash\nset -eu\nprintf 'uv %s\\n' \"$*\"\nproject=\nwhile [[ $# -gt 0 ]]; do\n  if [[ $1 == --project ]]; then project=$2; shift 2; else shift; fi\ndone\n[[ -z $project ]] || mkdir -p \"$project/.venv\"\n",
        );
    }

    pub fn install_fake_docker(&self) {
        self.write_executable(
            "docker",
            b"#!/usr/bin/env bash\nset -eu\nprintf 'docker %s\\n' \"$*\"\n",
        );
    }

    pub fn render_tree(&self, relative_root: impl AsRef<Path>) -> String {
        fn visit(root: &Path, directory: &Path, entries: &mut Vec<String>) {
            let mut children: Vec<_> = fs::read_dir(directory)
                .expect("read tree directory")
                .map(|entry| entry.expect("read tree entry"))
                .collect();
            children.sort_by_key(fs::DirEntry::file_name);
            for entry in children {
                let path = entry.path();
                let relative = path.strip_prefix(root).expect("tree entry below root");
                let kind = entry.file_type().expect("read tree entry type");
                if kind.is_dir() {
                    entries.push(format!("{}/", relative.display()));
                    visit(root, &path, entries);
                } else if kind.is_file() {
                    entries.push(relative.display().to_string());
                } else if kind.is_symlink() {
                    entries.push(format!("{} -> <SYMLINK>", relative.display()));
                }
            }
        }

        let root = self.work.join(relative_root);
        let mut entries = Vec::new();
        if root.is_dir() {
            visit(&root, &root, &mut entries);
        }
        if entries.is_empty() {
            "<EMPTY>\n".to_owned()
        } else {
            format!("{}\n", entries.join("\n"))
        }
    }

    pub fn normalize(&self, value: &str) -> String {
        // Canonicalize platform line endings and erase the random temporary-directory component.
        value
            .replace("\r\n", "\n")
            .replace(&self.root.display().to_string(), "<TEST_ROOT>")
    }

    pub fn render_output(&self, output: &Output) -> String {
        let stdout = self.normalize(&String::from_utf8_lossy(&output.stdout));
        let stderr = self.normalize(&String::from_utf8_lossy(&output.stderr));
        let exit_code = output
            .status
            .code()
            .map_or_else(|| "<SIGNAL>".to_owned(), |code| code.to_string());

        format!("exit: {exit_code}\n--- stdout ---\n{stdout}--- stderr ---\n{stderr}--- end ---\n")
    }

    fn run_bash<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let legacy_cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/oqtopus");
        let mut command = Command::new(bash_program());
        command.arg(legacy_cli).args(args);
        self.configure(&mut command);
        command.output().expect("legacy Bash CLI should run")
    }

    fn run_rust<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.rust_command(args)
            .output()
            .expect("Rust CLI should run")
    }

    pub(crate) fn rust_command<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oqtopus"));
        command.args(args);
        self.configure(&mut command);
        // A characterization test must fail if a supposedly migrated route silently delegates to
        // Bash; otherwise it would not test the Rust implementation at all. Set after `configure`,
        // which clears the environment.
        command.env(FORBID_LEGACY_FALLBACK, "1");
        command
    }

    fn configure(&self, command: &mut Command) {
        // Start from a deliberately small, deterministic environment. PATH is retained only so the
        // legacy runner can locate Bash; all filesystem-facing variables point into the sandbox.
        let fixture_path = env::join_paths(std::iter::once(self.root.join("bin")).chain(
            env::split_paths(&env::var_os("PATH").expect("PATH should be set")),
        ))
        .expect("construct fixture PATH");
        command
            .current_dir(&self.work)
            .env_clear()
            .env("PATH", fixture_path)
            .env("HOME", self.root.join("home"))
            .env("TMPDIR", self.root.join("tmp"))
            .env("XDG_DATA_HOME", self.root.join("xdg-data"))
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("TZ", "UTC")
            .stdin(Stdio::null());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentTemplate {
    Backend,
    CloudLocal,
    Manager,
}

impl EnvironmentTemplate {
    fn name(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::CloudLocal => "cloud-local",
            Self::Manager => "manager",
        }
    }
}

fn bash_program() -> PathBuf {
    let path = env::var_os("PATH").expect("PATH should be set");
    env::split_paths(&path)
        .map(|directory| directory.join("bash"))
        .find(|candidate| candidate.is_file())
        .expect("bash should be available")
}

#[test]
fn environment_fixtures_have_stable_metadata() {
    // Keep fixture construction under test because every command snapshot relies on its exact
    // metadata shape and normalization.
    for template in [
        EnvironmentTemplate::Backend,
        EnvironmentTemplate::CloudLocal,
        EnvironmentTemplate::Manager,
    ] {
        let context = TestContext::new();
        context.create_environment(template, &[("component_version", "v1.2.3")]);

        let metadata =
            fs::read_to_string(context.work.join(".metadata")).expect("read fixture metadata");
        let normalized = context.normalize(&metadata);
        let expected = format!(
            "template={}\ninstall_root=<TEST_ROOT>/xdg-data/oqtopus/{}/releases\nenvironment_name=characterization\nenvironment_root=<TEST_ROOT>/work\ncreated_at=2000-01-02T03:04:05Z\ncomponent_version=v1.2.3\n",
            template.name(),
            template.name(),
        );

        assert_eq!(normalized, expected);
    }
}

#[test]
fn rust_subject_forbids_legacy_fallback_after_clearing_the_environment() {
    let context = TestContext::new();
    let output = context.run_rust(["not-yet-migrated"]);

    assert_eq!(output.status.code(), Some(125));
    assert_eq!(
        output.stderr,
        b"Error: legacy Bash fallback is forbidden for this invocation.\n"
    );
}
