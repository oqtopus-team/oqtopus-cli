//! Line-oriented environment metadata and compatibility migrations.

use std::ffi::{CString, OsStr};
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

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

    for line in contents.lines() {
        if line
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate == key)
        {
            updated.push_str(key);
            updated.push('=');
            updated.push_str(value);
            found = true;
        } else {
            updated.push_str(line);
        }
        updated.push('\n');
    }

    if !found {
        updated.push_str(key);
        updated.push('=');
        updated.push_str(value);
        updated.push('\n');
    }

    updated
}

fn metadata_unset(contents: &str, key: &str) -> String {
    let mut updated = String::new();

    for line in contents.lines() {
        if !line
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate == key)
        {
            updated.push_str(line);
            updated.push('\n');
        }
    }

    updated
}

fn migrate_key(contents: String, old_key: &str, new_key: &str) -> String {
    let Some(value) = metadata_get(&contents, old_key).map(str::to_owned) else {
        return contents;
    };

    let contents = metadata_set(&contents, new_key, &value);
    metadata_unset(&contents, old_key)
}

/// Atomically replaces `path` with newly created, owner-writable contents.
fn replace_file(path: &Path, contents: &[u8]) -> io::Result<()> {
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

    temporary.write_all(contents)?;
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
        let _ = replace_file(path, migrated.as_bytes());
    }
}

/// Sets one environment binding while preserving unknown metadata and line order.
pub(crate) fn set_metadata_value(path: &Path, key: &str, value: &str) -> io::Result<()> {
    let contents = fs::read(path)?;
    replace_file(path, &metadata_set_bytes(&contents, key, value))
}

/// Removes one environment binding while preserving every other metadata line.
pub(crate) fn unset_metadata_value(path: &Path, key: &str) -> io::Result<()> {
    let contents = fs::read(path)?;
    replace_file(path, &metadata_unset_bytes(&contents, key))
}

// The functions above rewrite metadata as text, which is what the key migration needs: it reads a
// value before writing it back, and a file it cannot decode is left alone. Binding updates run on
// metadata the CLI must not damage, so they work on bytes instead and copy every line they do not
// touch through unchanged, including lines that are not valid UTF-8.

fn metadata_set_bytes(contents: &[u8], key: &str, value: &str) -> Vec<u8> {
    let prefix = format!("{key}=");
    let replacement = format!("{key}={value}");
    let mut updated = Vec::new();
    let mut found = false;
    // Splitting on '\n' yields a trailing empty element for a file that ends in a newline; drop it
    // so a terminated final line is not mistaken for an extra blank one.
    let lines: Vec<_> = contents.split(|byte| *byte == b'\n').collect();
    let line_count = lines.len() - usize::from(contents.is_empty() || contents.ends_with(b"\n"));
    for line in &lines[..line_count] {
        if line.starts_with(prefix.as_bytes()) {
            updated.extend_from_slice(replacement.as_bytes());
            found = true;
        } else {
            updated.extend_from_slice(line);
        }
        updated.push(b'\n');
    }
    if !found {
        updated.extend_from_slice(replacement.as_bytes());
        updated.push(b'\n');
    }
    updated
}

fn metadata_unset_bytes(contents: &[u8], key: &str) -> Vec<u8> {
    let prefix = format!("{key}=");
    let mut updated = Vec::new();
    let lines: Vec<_> = contents.split(|byte| *byte == b'\n').collect();
    let line_count = lines.len() - usize::from(contents.ends_with(b"\n"));
    for line in &lines[..line_count] {
        if !line.starts_with(prefix.as_bytes()) {
            updated.extend_from_slice(line);
            updated.push(b'\n');
        }
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::{metadata_set_bytes, metadata_unset_bytes};

    #[test]
    fn binding_updates_preserve_non_utf8_unknown_lines() {
        let contents = b"template=backend\nunknown=\xff\nengine_version=old\n";
        let updated = metadata_set_bytes(contents, "engine_version", "v1.2.3");
        assert_eq!(
            updated,
            b"template=backend\nunknown=\xff\nengine_version=v1.2.3\n"
        );
        assert_eq!(
            metadata_unset_bytes(&updated, "engine_version"),
            b"template=backend\nunknown=\xff\n"
        );
    }

    #[test]
    fn setting_a_binding_in_empty_metadata_does_not_add_a_blank_line() {
        assert_eq!(
            metadata_set_bytes(b"", "engine_version", "v1.2.3"),
            b"engine_version=v1.2.3\n"
        );
    }
}
