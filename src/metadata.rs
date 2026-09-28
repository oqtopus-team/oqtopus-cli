//! Line-oriented environment metadata and compatibility migrations.

use std::ffi::{CString, OsStr};
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Diagnostic for metadata whose bytes are not valid UTF-8.
///
/// Metadata is a UTF-8 text format. Every command that reads it rejects an undecodable file with
/// this one message instead of interpreting it lossily or silently treating it as absent.
pub(crate) const INVALID_UTF8: &str = "invalid .metadata: file is not valid UTF-8.";

/// Returns the first value for `key` from the environment's line-oriented metadata format.
pub(crate) fn metadata_get<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    // Split only once because metadata values may themselves contain '='.
    contents.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate == key).then_some(value)
    })
}

fn metadata_set(contents: &str, key: &str, value: &str) -> String {
    // Preserve ordering and unknown lines so migrating one key does not rewrite metadata owned by
    // other components.
    let mut found = false;
    let mut updated = String::new();

    for line in contents.split_inclusive('\n') {
        let text = metadata_line_text(line);
        if text
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate == key)
        {
            updated.push_str(key);
            updated.push('=');
            updated.push_str(value);
            if line.ends_with("\r\n") {
                updated.push_str("\r\n");
            } else {
                updated.push('\n');
            }
            found = true;
        } else {
            updated.push_str(line);
        }
    }

    if !found {
        let ending = appended_line_ending(contents);
        if !contents.is_empty() && !contents.ends_with('\n') {
            updated.push_str(ending);
        }
        updated.push_str(key);
        updated.push('=');
        updated.push_str(value);
        updated.push_str(ending);
    }

    updated
}

/// Line ending for a binding appended to `contents`, following its last terminated line.
///
/// An appended binding must not leave a CRLF file with mixed line endings, and a file with no
/// terminated line at all has nothing to follow, so it gets the format's default.
fn appended_line_ending(contents: &str) -> &'static str {
    match contents.rfind('\n') {
        Some(index) if contents[..index].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

fn metadata_unset(contents: &str, key: &str) -> String {
    let mut updated = String::new();

    for line in contents.split_inclusive('\n') {
        let text = metadata_line_text(line);
        if !text
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate == key)
        {
            updated.push_str(line);
        }
    }

    updated
}

fn metadata_line_text(line: &str) -> &str {
    if let Some(line) = line.strip_suffix("\r\n") {
        line
    } else {
        line.strip_suffix('\n').unwrap_or(line)
    }
}

fn migrate_key(contents: String, old_key: &str, new_key: &str) -> String {
    let Some(value) = metadata_get(&contents, old_key).map(str::to_owned) else {
        return contents;
    };

    let contents = metadata_set(&contents, new_key, &value);
    metadata_unset(&contents, old_key)
}

/// Atomically replaces `path` with newly created, owner-writable contents.
fn replace_file(path: &Path, contents: &str) -> io::Result<()> {
    // Create and sync a randomly named sibling before rename so readers never observe partially
    // migrated metadata and a stale file cannot block a later process that reuses the same PID.
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .unwrap_or_else(|| OsStr::new("oqtopus"))
        .to_string_lossy();
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(".{file_name}.tmp."))
        .tempfile_in(parent)?;

    temporary.write_all(contents.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Checks write access using the process's real user and group IDs.
///
/// This matches Bash's `[[ -w path ]]` for ordinary invocations where real and effective IDs
/// coincide. Invocations with differing real and effective IDs are outside this check's contract.
fn is_writable(path: &Path) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };

    // SAFETY: `path` is a valid, NUL-terminated C string and remains alive for the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

/// Migrates legacy metadata keys when `path` can be safely rewritten.
///
/// Migration is opportunistic: callers remain usable for read-only environments and should report
/// metadata validation errors rather than failing solely because the compatibility rewrite failed.
pub(crate) fn migrate_metadata_keys(path: &Path) {
    if !is_writable(path) {
        return;
    }

    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };
    let migrated = migrate_key(contents.clone(), "env_root", "environment_root");
    let migrated = migrate_key(migrated, "env_name", "environment_name");

    if migrated != contents {
        let _ = replace_file(path, &migrated);
    }
}

/// Sets one environment binding while preserving unknown metadata and line order.
pub(crate) fn set_metadata_value(path: &Path, key: &str, value: &str) -> io::Result<()> {
    let contents = fs::read_to_string(path)?;
    replace_file(path, &metadata_set(&contents, key, value))
}

/// Removes one environment binding while preserving every other metadata line.
pub(crate) fn unset_metadata_value(path: &Path, key: &str) -> io::Result<()> {
    let contents = fs::read_to_string(path)?;
    replace_file(path, &metadata_unset(&contents, key))
}

#[cfg(test)]
mod tests {
    use super::{metadata_set, metadata_unset};

    #[test]
    fn binding_updates_preserve_utf8_unknown_lines_and_line_endings() {
        let contents = "template=backend\r\nunknown=東京\r\nengine_version=old\r\n";
        let updated = metadata_set(contents, "engine_version", "v1.2.3");
        assert_eq!(
            updated,
            "template=backend\r\nunknown=東京\r\nengine_version=v1.2.3\r\n"
        );
        assert_eq!(
            metadata_unset(&updated, "engine_version"),
            "template=backend\r\nunknown=東京\r\n"
        );
    }

    #[test]
    fn setting_a_binding_in_empty_metadata_does_not_add_a_blank_line() {
        assert_eq!(
            metadata_set("", "engine_version", "v1.2.3"),
            "engine_version=v1.2.3\n"
        );
    }

    #[test]
    fn appending_a_binding_separates_an_unterminated_unknown_line() {
        assert_eq!(
            metadata_set("unknown=café", "engine_version", "v1.2.3"),
            "unknown=café\nengine_version=v1.2.3\n"
        );
    }

    #[test]
    fn appending_a_binding_follows_crlf_line_endings() {
        assert_eq!(
            metadata_set("template=backend\r\n", "engine_version", "v1.2.3"),
            "template=backend\r\nengine_version=v1.2.3\r\n"
        );
        assert_eq!(
            metadata_set("template=backend\r\nunknown=1", "engine_version", "v1.2.3"),
            "template=backend\r\nunknown=1\r\nengine_version=v1.2.3\r\n"
        );
    }
}
