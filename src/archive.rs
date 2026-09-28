//! Safe extraction of GitHub source archives with their top-level directory removed.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Cursor;
use std::path::{Component as PathComponent, Path, PathBuf};

use flate2::read::GzDecoder;

pub(crate) fn extract_github_archive(bytes: &[u8], target: &Path) -> Result<(), ()> {
    inspect_archive(bytes)?;
    if fs::symlink_metadata(target).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(());
    }
    fs::create_dir_all(target).map_err(|_| ())?;
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|_| ())?;

    for entry in entries {
        let mut entry = entry.map_err(|_| ())?;
        let path = entry.path().map_err(|_| ())?;
        let relative = stripped_path(&path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        ensure_no_existing_symlink(target, &relative)?;
        entry.unpack(target.join(relative)).map_err(|_| ())?;
    }
    Ok(())
}

/// Validates the complete entry graph before creating anything.
///
/// A safe symlink may point elsewhere inside the extracted tree, but no archive entry may use a
/// symlink as its parent. This keeps extraction from writing through links; target validation
/// separately resolves the complete link graph to catch escapes through chains and pivots.
fn inspect_archive(bytes: &[u8]) -> Result<(), ()> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|_| ())?;
    let mut paths = Vec::new();
    let mut symlinks = HashMap::new();
    let mut seen = HashSet::new();

    for entry in entries {
        let entry = entry.map_err(|_| ())?;
        let path = entry.path().map_err(|_| ())?;
        let relative = stripped_path(&path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if !seen.insert(relative.clone()) {
            return Err(());
        }

        let entry_type = entry.header().entry_type();
        if entry_type.is_hard_link() {
            // Hard-link extraction requires resolving another archive path and is unnecessary for
            // GitHub source archives. Reject it instead of risking an unchecked destination.
            return Err(());
        }
        if entry_type.is_symlink() {
            let link = entry.link_name().map_err(|_| ())?.ok_or(())?;
            symlinks.insert(relative.clone(), link.into_owned());
        } else if !entry_type.is_dir() && !entry_type.is_file() {
            return Err(());
        }
        paths.push(relative);
    }

    if paths.iter().any(|path| {
        symlinks
            .keys()
            .any(|symlink| path != symlink && path.starts_with(symlink))
    }) {
        return Err(());
    }
    for (path, link) in &symlinks {
        validate_symlink_target(path, link, &symlinks)?;
    }
    Ok(())
}

/// Removes the single top-level directory that GitHub wraps every source archive in.
///
/// The remaining path must be relative and free of `..`, so a malicious entry cannot name a
/// destination outside the extraction directory.
fn stripped_path(path: &Path) -> Result<PathBuf, ()> {
    let relative = path.components().skip(1).collect::<PathBuf>();
    if relative
        .components()
        .any(|component| !matches!(component, PathComponent::Normal(_) | PathComponent::CurDir))
    {
        return Err(());
    }
    Ok(relative)
}

/// Resolves links before applying `..`, matching filesystem traversal rather than lexical depth.
fn validate_symlink_target(
    path: &Path,
    link: &Path,
    symlinks: &HashMap<PathBuf, PathBuf>,
) -> Result<(), ()> {
    let mut resolved = path.parent().unwrap_or_else(|| Path::new("")).to_owned();
    let mut pending = link
        .components()
        .map(|part| part.as_os_str().to_owned())
        .collect::<VecDeque<_>>();
    let mut followed = 0;
    while let Some(part) = pending.pop_front() {
        match Path::new(&part).components().next().ok_or(())? {
            PathComponent::Normal(_) => {
                resolved.push(&part);
                if let Some(target) = symlinks.get(&resolved) {
                    followed += 1;
                    if followed > 40 {
                        return Err(());
                    }
                    resolved.pop();
                    for component in target.components().rev() {
                        pending.push_front(component.as_os_str().to_owned());
                    }
                }
            }
            PathComponent::CurDir => {}
            PathComponent::ParentDir => {
                if !resolved.pop() {
                    return Err(());
                }
            }
            PathComponent::RootDir | PathComponent::Prefix(_) => return Err(()),
        }
    }
    Ok(())
}

/// Revalidates a subtree before relocation: links must stay inside the copied template itself.
pub(crate) fn validate_directory_symlinks(root: &Path) -> Result<(), ()> {
    let mut pending = vec![root.to_owned()];
    let mut symlinks = HashMap::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| ())? {
            let entry = entry.map_err(|_| ())?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|_| ())?;
            if kind.is_symlink() {
                symlinks.insert(
                    path.strip_prefix(root).map_err(|_| ())?.to_owned(),
                    fs::read_link(path).map_err(|_| ())?,
                );
            } else if kind.is_dir() {
                pending.push(path);
            }
        }
    }
    for (path, link) in &symlinks {
        validate_symlink_target(path, link, &symlinks)?;
    }
    Ok(())
}

fn ensure_no_existing_symlink(target: &Path, relative: &Path) -> Result<(), ()> {
    let mut current = target.to_owned();
    for component in relative.components() {
        let PathComponent::Normal(part) = component else {
            continue;
        };
        current.push(part);
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::extract_github_archive;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use tar::{Builder, EntryType, Header};

    #[test]
    fn resolves_pivots_before_parent_components_and_rejects_cycles() {
        use std::collections::HashMap;
        use std::path::{Path, PathBuf};
        let links = HashMap::from([
            (PathBuf::from("pivot"), PathBuf::from(".")),
            (PathBuf::from("cycle"), PathBuf::from("cycle")),
        ]);
        assert!(
            super::validate_symlink_target(
                Path::new("escape"),
                Path::new("pivot/../outside"),
                &links
            )
            .is_err()
        );
        assert!(
            super::validate_symlink_target(Path::new("safe"), Path::new("pivot/file"), &links)
                .is_ok()
        );
        assert!(
            super::validate_symlink_target(Path::new("loop"), Path::new("cycle"), &links).is_err()
        );
    }

    #[test]
    fn relocated_template_links_must_stay_inside_the_template() {
        let root = tempfile::tempdir().unwrap();
        let template = root.path().join("templates/backend");
        fs::create_dir_all(&template).unwrap();
        std::os::unix::fs::symlink("../outside", template.join("config")).unwrap();
        assert!(super::validate_directory_symlinks(&template).is_err());
    }

    #[test]
    fn rejects_a_symlink_escape_before_writing_any_entry() {
        let sandbox = tempfile::tempdir().expect("create archive sandbox");
        let extraction = sandbox.path().join("target");
        let outside = sandbox.path().join("outside");
        fs::create_dir_all(&extraction).expect("create extraction directory");
        fs::create_dir_all(&outside).expect("create outside directory");
        let escaped = outside.join("escaped");
        fs::write(&escaped, "original").expect("write outside sentinel");
        let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = Builder::new(&mut compressed);
            let mut link = Header::new_gnu();
            link.set_entry_type(EntryType::Symlink);
            link.set_size(0);
            link.set_link_name("../outside").expect("set link target");
            link.set_cksum();
            archive
                .append_data(&mut link, "source/link", &[][..])
                .expect("append symlink");

            let contents = b"must remain inside";
            let mut file = Header::new_gnu();
            file.set_entry_type(EntryType::Regular);
            file.set_size(contents.len() as u64);
            file.set_mode(0o644);
            file.set_cksum();
            archive
                .append_data(&mut file, "source/link/escaped", &contents[..])
                .expect("append escaped file");
            archive.finish().expect("finish archive");
        }
        let bytes = compressed.finish().expect("finish compression");

        assert!(extract_github_archive(&bytes, &extraction).is_err());
        assert_eq!(fs::read_to_string(escaped).unwrap(), "original");
        assert!(
            fs::read_dir(extraction)
                .expect("read extraction directory")
                .next()
                .is_none()
        );
    }

    #[test]
    fn preserves_a_symlink_whose_target_stays_inside_the_archive() {
        let extraction = tempfile::tempdir().expect("create extraction directory");
        let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = Builder::new(&mut compressed);
            let contents = b"inside";
            let mut file = Header::new_gnu();
            file.set_entry_type(EntryType::Regular);
            file.set_size(contents.len() as u64);
            file.set_mode(0o644);
            file.set_cksum();
            archive
                .append_data(&mut file, "source/file", &contents[..])
                .expect("append file");

            let mut link = Header::new_gnu();
            link.set_entry_type(EntryType::Symlink);
            link.set_size(0);
            link.set_link_name("file").expect("set link target");
            link.set_cksum();
            archive
                .append_data(&mut link, "source/link", &[][..])
                .expect("append symlink");
            archive.finish().expect("finish archive");
        }
        let bytes = compressed.finish().expect("finish compression");

        extract_github_archive(&bytes, extraction.path()).expect("extract safe archive");
        assert_eq!(
            fs::read_link(extraction.path().join("link")).unwrap(),
            std::path::Path::new("file")
        );
        assert_eq!(
            fs::read_to_string(extraction.path().join("link")).unwrap(),
            "inside"
        );
    }
}
