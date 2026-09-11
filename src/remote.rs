//! Git smart-HTTP ref discovery shared by commands that read remote repositories.

use std::env;
use std::fs;
use std::io::Read;

const GITHUB_BASE_URL: &str = "https://github.com";
const FORBID_LEGACY_FALLBACK: &str = "OQTOPUS_FORBID_LEGACY_FALLBACK";

pub(crate) fn remote_refs_url(repository: &str) -> String {
    format!("{GITHUB_BASE_URL}/{repository}.git/info/refs?service=git-upload-pack")
}

pub(crate) fn fetch_remote_tags(repository: &str) -> Result<Vec<String>, ()> {
    let advertisement = fetch_url(&remote_refs_url(repository))?;
    let tags = parse_remote_refs(&advertisement)
        .into_iter()
        .filter_map(|(_, reference)| reference.strip_prefix("refs/tags/").map(str::to_owned))
        .filter(|tag| !tag.ends_with("^{}"))
        .collect::<Vec<_>>();
    let mut tags = tags;
    tags.sort();
    tags.dedup();
    (!tags.is_empty()).then_some(tags).ok_or(())
}

pub(crate) fn resolve_branch_commit(repository: &str, branch: &str) -> Result<String, ()> {
    let advertisement = fetch_url(&remote_refs_url(repository))?;
    let expected = format!("refs/heads/{branch}");
    parse_remote_refs(&advertisement)
        .into_iter()
        .find_map(|(sha, reference)| (reference == expected).then_some(sha))
        .ok_or(())
}

pub(crate) fn fetch_url(url: &str) -> Result<Vec<u8>, ()> {
    // The fixture hook is available only under the existing fallback-forbidden test mode, so it
    // does not affect a normal invocation unless that explicit test mode is also enabled.
    if env::var_os(FORBID_LEGACY_FALLBACK).is_some()
        && let Some(path) = env::var_os("OQTOPUS_TEST_HTTP_FIXTURE_MANIFEST")
    {
        let manifest = fs::read_to_string(path).map_err(|_| ())?;
        let fixture = manifest.lines().find_map(|line| {
            let (candidate, fixture) = line.split_once('\t')?;
            (candidate == url).then_some(fixture)
        });
        return fs::read(fixture.ok_or(())?).map_err(|_| ());
    }
    if env::var_os(FORBID_LEGACY_FALLBACK).is_some()
        && let Some(path) = env::var_os("OQTOPUS_TEST_HTTP_RESPONSE_FILE")
    {
        if let Ok(expected) = env::var("OQTOPUS_TEST_EXPECTED_HTTP_URL")
            && expected != url
        {
            return Err(());
        }
        return fs::read(path).map_err(|_| ());
    }
    let mut response = ureq::get(url).call().map_err(|_| ())?;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|_| ())?;
    Ok(body)
}

fn parse_remote_refs(advertisement: &[u8]) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    let marker = b" refs/";

    for marker_start in find_all(advertisement, marker) {
        if marker_start < 40
            || !advertisement[marker_start - 40..marker_start]
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            continue;
        }
        let reference_start = marker_start + 1;
        let reference_end = advertisement[reference_start..]
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
            .map_or(advertisement.len(), |offset| reference_start + offset);
        let Ok(reference) = std::str::from_utf8(&advertisement[reference_start..reference_end])
        else {
            continue;
        };
        let Ok(sha) = std::str::from_utf8(&advertisement[marker_start - 40..marker_start]) else {
            continue;
        };
        refs.push((sha.to_owned(), reference.to_owned()));
    }

    refs
}

fn find_all<'a>(haystack: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(move |(index, window)| (window == needle).then_some(index))
}

#[cfg(test)]
mod tests {
    use super::parse_remote_refs;

    #[test]
    fn parses_unpeeled_tag_refs() {
        let refs = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/tags/v1.2.3\0capability\n\
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/tags/v2.0.0^{}\n\
cccccccccccccccccccccccccccccccccccccccc refs/tags/v2.0.0-rc.1\n";

        assert_eq!(
            parse_remote_refs(refs),
            [
                (
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                    "refs/tags/v1.2.3".to_owned()
                ),
                (
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                    "refs/tags/v2.0.0^{}".to_owned()
                ),
                (
                    "cccccccccccccccccccccccccccccccccccccccc".to_owned(),
                    "refs/tags/v2.0.0-rc.1".to_owned()
                ),
            ]
        );
    }
}
