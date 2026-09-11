use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CHARACTERIZATION_SOURCE: &str = "OQTOPUS_CHARACTERIZATION_SOURCE";
const FORBID_LEGACY_FALLBACK: &str = "OQTOPUS_FORBID_LEGACY_FALLBACK";

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
        // A characterization test must fail if a supposedly migrated route silently delegates to
        // Bash; otherwise it would not test the Rust implementation selected above.
        command.args(args);
        self.configure(&mut command);
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
