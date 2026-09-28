//! Process state and lifecycle helpers shared by managed services.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Observed process state for one managed service.
pub(crate) struct ServiceStatus {
    pub(crate) name: &'static str,
    pub(crate) pid: Option<u32>,
}

/// Returns the live PID recorded in `path`, if the PID file names a reachable process.
///
/// Invalid, missing, stale, or otherwise unreachable PID files return `None`.
pub(crate) fn running_pid(path: &Path) -> Option<u32> {
    if !path.is_file() {
        return None;
    }
    let contents = fs::read_to_string(path).ok()?;
    // Command substitution in Bash removes trailing newlines before the numeric check.
    let candidate = contents.trim_end_matches('\n');
    if candidate.is_empty() || !candidate.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let pid = candidate.parse::<libc::pid_t>().ok()?;
    // PID 0 addresses the caller's entire process group rather than one process. A corrupted PID
    // file must never turn a service stop into a group-wide signal.
    if pid == 0 {
        return None;
    }

    // SAFETY: signal 0 does not deliver a signal; it only checks whether the PID is reachable.
    (unsafe { libc::kill(pid, 0) } == 0).then_some(pid as u32)
}

/// A directly executable managed-service command.
pub(crate) struct ServiceCommand {
    pub(crate) program: &'static str,
    pub(crate) args: Vec<String>,
    pub(crate) environment: Vec<(&'static str, &'static str)>,
}

impl ServiceCommand {
    pub(crate) fn uv(args: Vec<String>) -> Self {
        Self {
            program: "uv",
            args,
            environment: Vec::new(),
        }
    }
}

/// Output behavior of a managed child process.
pub(crate) enum BackgroundOutput {
    Null,
    Log(PathBuf),
}

#[derive(Clone, Copy)]
pub(crate) enum StopStyle {
    Backend,
    CloudLocal,
    Manager,
}

/// Serializes startup using a kernel lock that is released even after SIGKILL.
///
/// The guard file is permanent: unlinking it would allow contenders to lock different inodes.
/// The directory and its PID are retained only for diagnostics and older CLI compatibility.
struct StartLock {
    directory: PathBuf,
    _guard: fs::File,
}

impl StartLock {
    fn acquire(root: &Path, service: &str) -> Result<Self, String> {
        let pids = root.join("pids");
        fs::create_dir_all(&pids)
            .map_err(|error| format!("cannot create service PID directory: {error}"))?;
        let directory = pids.join(format!(".{service}.start.lock"));
        let guard = open_guard(&pids.join(format!(".{service}.start.guard")))
            .map_err(|error| format!("cannot open service start lock: {error}"))?;

        // SAFETY: guard owns a valid descriptor. LOCK_NB prevents waiting indefinitely.
        if unsafe { libc::flock(guard.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                let owner = running_pid(&directory.join("pid"))
                    .map(|pid| format!(" (PID {pid})"))
                    .unwrap_or_default();
                return Err(format!(
                    "cannot start '{service}'. Another start operation is already in progress{owner}."
                ));
            }
            return Err(format!("cannot acquire service start lock: {error}"));
        }

        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let lock_pid = running_pid(&directory.join("pid"));
                if let Some(pid) = lock_pid {
                    return Err(format!(
                        "cannot start '{service}'. Another start operation is already in progress (PID {pid})."
                    ));
                }
                let _ = fs::remove_file(directory.join("pid"));
                fs::remove_dir(&directory).map_err(|_| {
                    format!(
                        "cannot start '{service}'. Could not acquire start lock: {}",
                        directory.display()
                    )
                })?;
                fs::create_dir(&directory).map_err(|_| {
                    format!(
                        "cannot start '{service}'. Could not acquire start lock: {}",
                        directory.display()
                    )
                })?;
            }
            Err(_) => {
                return Err(format!(
                    "cannot start '{service}'. Could not acquire start lock: {}",
                    directory.display()
                ));
            }
        }

        if let Err(error) = fs::write(directory.join("pid"), format!("{}\n", std::process::id())) {
            let _ = fs::remove_dir(&directory);
            return Err(format!("cannot write service start lock: {error}"));
        }
        Ok(Self {
            directory,
            _guard: guard,
        })
    }
}

/// Opens the persistent start-guard file for locking.
///
/// `flock` needs no write access, so an existing guard is opened read-only: the file outlives
/// every start, and one created by another user must not lock everyone else out permanently.
/// Creation still needs write access, and loses to a caller that created the file first.
fn open_guard(path: &Path) -> io::Result<fs::File> {
    match fs::File::open(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match OpenOptions::new().create_new(true).write(true).open(path) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => fs::File::open(path),
                result => result,
            }
        }
        result => result,
    }
}

impl Drop for StartLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.directory.join("pid"));
        let _ = fs::remove_dir(&self.directory);
    }
}

/// Starts one process-backed service and writes compatibility progress to `out`.
pub(crate) fn start_process<W: Write, F>(
    root: &Path,
    service: &str,
    make_command: F,
    foreground: bool,
    background_output: BackgroundOutput,
    manager_style: bool,
    out: &mut W,
) -> Result<i32, String>
where
    F: FnOnce() -> Result<ServiceCommand, String>,
{
    let lock = StartLock::acquire(root, service)?;
    let pid_file = root.join("pids").join(format!("{service}.pid"));
    if let Some(pid) = running_pid(&pid_file) {
        line(
            out,
            format!("{service} is already running (PID {pid}); skipping."),
        )?;
        return Ok(0);
    }
    if pid_file.is_file() {
        fs::remove_file(&pid_file)
            .map_err(|error| format!("failed to remove stale PID file: {error}"))?;
    }

    ensure_command("uv")?;
    let command = make_command()?;
    let mut child_command = Command::new(command.program);
    child_command
        .args(&command.args)
        .current_dir(root)
        .env_remove("VIRTUAL_ENV")
        .envs(load_env_exports(&root.join("config/.env")));
    for (key, value) in command.environment {
        child_command.env(key, value);
    }

    if foreground {
        child_command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
    } else {
        child_command.stdin(Stdio::null());
        match background_output {
            BackgroundOutput::Null => {
                child_command.stdout(Stdio::null()).stderr(Stdio::null());
            }
            BackgroundOutput::Log(path) => {
                let log = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .map_err(|error| {
                        format!("failed to open service log {}: {error}", path.display())
                    })?;
                let stderr = log
                    .try_clone()
                    .map_err(|error| format!("failed to open service log: {error}"))?;
                child_command
                    .stdout(Stdio::from(log))
                    .stderr(Stdio::from(stderr));
            }
        }
        // Match the legacy subshell's `trap '' HUP` before it execs the service.
        // SAFETY: this callback only invokes the async-signal-safe `signal` function.
        unsafe {
            child_command.pre_exec(|| {
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(())
            });
        }
    }

    out.flush()
        .map_err(|error| format!("failed to write progress: {error}"))?;
    let pid_record = fs::File::create(&pid_file)
        .map_err(|error| format!("failed to write PID file: {error}"))?;
    let pid_descriptor = pid_record.as_raw_fd();
    // Publish the child's PID before executing the service. If this CLI is killed after fork,
    // the child still records its PID, and its inherited guard descriptor excludes other starts
    // until exec. Rust opens these descriptors with close-on-exec, so the running service never
    // retains the startup lock. There is no parent-only spawn-to-PID-write recovery gap.
    // SAFETY: the callback only uses stack data and async-signal-safe getpid/write calls.
    // pid_record remains open through spawn; no allocation or Rust locks occur in the child.
    unsafe {
        child_command.pre_exec(move || publish_pid(pid_descriptor));
    }
    let spawned = child_command.spawn();
    drop(pid_record);
    let mut child = spawned.map_err(|_| {
        let _ = fs::remove_file(&pid_file);
        format!("failed to start '{service}'. The process exited immediately.")
    })?;
    let pid = child.id();

    if foreground {
        drop(lock);
        line(out, format!("Started {service} in foreground (PID {pid})"))?;
        let status = child
            .wait()
            .map_err(|error| format!("failed to wait for '{service}': {error}"))?;
        remove_matching_pid_file(&pid_file, pid);
        return Ok(exit_code(status));
    }

    thread::sleep(Duration::from_millis(200));
    if child
        .try_wait()
        .map_err(|error| format!("failed to check '{service}': {error}"))?
        .is_some()
    {
        let _ = fs::remove_file(&pid_file);
        let message = if manager_style {
            "failed to start manager. The process exited immediately.".to_owned()
        } else {
            format!("failed to start '{service}'. The process exited immediately.")
        };
        return Err(message);
    }
    line(out, format!("Started {service} (PID {pid})"))?;
    drop(lock);
    Ok(0)
}

/// Writes a decimal PID without allocation; called between fork and exec.
fn publish_pid(descriptor: RawFd) -> io::Result<()> {
    let mut buffer = [0_u8; 12];
    let mut start = buffer.len() - 1;
    buffer[start] = b'\n';
    // SAFETY: getpid has no preconditions and is async-signal-safe.
    let mut pid = unsafe { libc::getpid() } as u32;
    loop {
        start -= 1;
        buffer[start] = b'0' + (pid % 10) as u8;
        pid /= 10;
        if pid == 0 {
            break;
        }
    }
    while start < buffer.len() {
        // SAFETY: the descriptor is open and the buffer remains valid for the given length.
        let written = unsafe {
            libc::write(
                descriptor,
                buffer[start..].as_ptr().cast(),
                buffer.len() - start,
            )
        };
        if written < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if written == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        start += written as usize;
    }
    Ok(())
}

// Foreground cleanup may have removed the PID file before the stop caller observes exit.
fn remove_pid_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Stops one process-backed service using the wording of its command family.
pub(crate) fn stop_process<W: Write>(
    root: &Path,
    service: &str,
    style: StopStyle,
    out: &mut W,
) -> Result<(), String> {
    let pid_file = root.join("pids").join(format!("{service}.pid"));
    let Some(pid) = running_pid(&pid_file) else {
        if pid_file.is_file() {
            remove_pid_file(&pid_file)
                .map_err(|error| format!("failed to remove stale PID file: {error}"))?;
        }
        let message = match style {
            StopStyle::Backend | StopStyle::Manager => format!("{service}: Stopped"),
            StopStyle::CloudLocal => format!("{service} is not running."),
        };
        return line(out, message);
    };

    // SAFETY: a positive PID was parsed from the service's PID file and SIGTERM is valid.
    if unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } != 0 {
        return Err(match style {
            StopStyle::Backend | StopStyle::Manager => {
                format!("failed to stop '{service}'. Could not send TERM to PID {pid}.")
            }
            StopStyle::CloudLocal => format!("failed to send TERM to {service} (PID {pid})."),
        });
    }
    for _ in 0..5 {
        thread::sleep(Duration::from_secs(1));
        if running_pid(&pid_file).is_none() {
            remove_pid_file(&pid_file)
                .map_err(|error| format!("failed to remove PID file: {error}"))?;
            let message = match style {
                StopStyle::CloudLocal => format!("Stopped {service} (PID {pid})"),
                StopStyle::Backend | StopStyle::Manager => format!("Stopped {service}"),
            };
            return line(out, message);
        }
    }
    Err(match style {
        StopStyle::CloudLocal => format!("{service} (PID {pid}) did not stop within 5 seconds."),
        StopStyle::Backend | StopStyle::Manager => format!(
            "failed to stop '{service}'. The process did not exit within 5 seconds after TERM was sent."
        ),
    })
}

pub(crate) fn ensure_command(name: &str) -> Result<(), String> {
    let found = env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|directory| {
            directory.join(name).metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
    });
    found
        .then_some(())
        .ok_or_else(|| format!("'{name}' is required but was not found on PATH."))
}

pub(crate) fn load_env_exports(path: &Path) -> HashMap<String, String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    contents
        .lines()
        .filter_map(parse_env_line)
        .collect::<HashMap<_, _>>()
}

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.trim().is_empty() || line.trim_start().starts_with('#') {
        return None;
    }
    let (key, raw_value) = line.split_once('=')?;
    let valid_key = !key.is_empty()
        && key.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
        });
    if !valid_key {
        return None;
    }
    let quoted = raw_value
        .strip_prefix('"')
        .and_then(|rest| rest.split_once('"').map(|(value, _)| value))
        .or_else(|| {
            raw_value
                .strip_prefix('\'')
                .and_then(|rest| rest.split_once('\'').map(|(value, _)| value))
        });
    let value = if let Some(value) = quoted {
        value
    } else {
        let comment = raw_value
            .as_bytes()
            .windows(2)
            .position(|pair| pair[0].is_ascii_whitespace() && pair[1] == b'#')
            .unwrap_or(raw_value.len());
        raw_value[..comment].trim_end_matches(|character: char| character.is_ascii_whitespace())
    };
    Some((key.to_owned(), value.to_owned()))
}

fn remove_matching_pid_file(path: &Path, pid: u32) {
    if running_pid_text(path).is_some_and(|recorded| recorded == pid) {
        let _ = fs::remove_file(path);
    }
}

fn running_pid_text(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()?
        .trim_end_matches('\n')
        .parse()
        .ok()
}

fn line(out: &mut impl Write, message: impl std::fmt::Display) -> Result<(), String> {
    writeln!(out, "{message}").map_err(|error| format!("failed to write progress: {error}"))?;
    out.flush()
        .map_err(|error| format!("failed to write progress: {error}"))
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{StartLock, load_env_exports, running_pid};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn pid_cleanup_tolerates_absence_but_preserves_other_errors() {
        let root = tempfile::tempdir().unwrap();
        let pid = root.path().join("service.pid");
        super::remove_pid_file(&pid).unwrap();
        fs::write(&pid, "123").unwrap();
        super::remove_pid_file(&pid).unwrap();
        super::remove_pid_file(&pid).unwrap();
        fs::create_dir(&pid).unwrap();
        assert!(super::remove_pid_file(&pid).is_err());
    }

    #[test]
    fn missing_owner_pid_does_not_allow_stealing_a_live_start_lock() {
        let directory = tempfile::tempdir().expect("create lock fixture");
        let first = StartLock::acquire(directory.path(), "manager").expect("acquire start lock");
        // Model the interval before owner PID publication, or an interrupted diagnostic write.
        fs::remove_file(first.directory.join("pid")).expect("remove diagnostic PID");

        let contender = StartLock::acquire(directory.path(), "manager");
        assert!(matches!(contender, Err(message) if message.contains("already in progress")));

        drop(first);
        let retry = StartLock::acquire(directory.path(), "manager");
        assert!(retry.is_ok(), "normal release must permit another startup");
    }

    #[test]
    fn a_guard_file_without_write_permission_still_locks() {
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            // Permission bits do not restrict root, so this case cannot be modeled here.
            return;
        }
        let directory = tempfile::tempdir().expect("create lock fixture");
        let pids = directory.path().join("pids");
        fs::create_dir_all(&pids).expect("create PID directory");
        let guard = pids.join(".manager.start.guard");
        fs::write(&guard, b"").expect("create guard fixture");
        // Model a guard left by another user: readable, but not writable by this caller.
        fs::set_permissions(&guard, fs::Permissions::from_mode(0o444))
            .expect("drop guard write permission");

        let lock = StartLock::acquire(directory.path(), "manager");
        assert!(
            lock.is_ok(),
            "a read-only guard must not lock out startup: {:?}",
            lock.err()
        );
        let contender = StartLock::acquire(directory.path(), "manager");
        assert!(matches!(contender, Err(message) if message.contains("already in progress")));
    }

    #[test]
    fn environment_exports_match_the_legacy_reader() {
        let directory = tempfile::tempdir().expect("create environment fixture");
        let path = directory.path().join(".env");
        fs::write(
            &path,
            "# ignored\nEMPTY=\nDOUBLE=\"quoted value\" ignored\nSINGLE='single value' ignored\nSEPARATORS=a=b=c\nCOMMENT=4194304  # 4MB\nNO_SPACE=value#kept\n   INDENTED=no\n1INVALID=no\nCRLF=accepted\r\nUNCLOSED=\"kept\n",
        )
        .expect("write environment fixture");

        let exports = load_env_exports(&path);
        assert_eq!(exports.get("EMPTY").map(String::as_str), Some(""));
        assert_eq!(
            exports.get("DOUBLE").map(String::as_str),
            Some("quoted value")
        );
        assert_eq!(
            exports.get("SINGLE").map(String::as_str),
            Some("single value")
        );
        assert_eq!(exports.get("SEPARATORS").map(String::as_str), Some("a=b=c"));
        assert_eq!(exports.get("COMMENT").map(String::as_str), Some("4194304"));
        assert_eq!(
            exports.get("NO_SPACE").map(String::as_str),
            Some("value#kept")
        );
        assert_eq!(exports.get("CRLF").map(String::as_str), Some("accepted"));
        assert_eq!(exports.get("UNCLOSED").map(String::as_str), Some("\"kept"));
        assert!(!exports.contains_key("INDENTED"));
        assert!(!exports.contains_key("1INVALID"));
    }

    #[test]
    fn pid_zero_is_never_treated_as_a_service_process() {
        let directory = tempfile::tempdir().expect("create PID fixture");
        let path = directory.path().join("service.pid");
        fs::write(&path, "0\n").expect("write PID fixture");

        assert_eq!(running_pid(&path), None);
    }
}
