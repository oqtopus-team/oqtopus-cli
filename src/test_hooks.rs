//! Environment overrides that let the characterization suite replace network access, wall-clock
//! time, and port probing with fixed fixtures.
//!
//! This module, and every call into it, is compiled only with the `test-hooks` feature, so a
//! release binary contains no `OQTOPUS_TEST_*` handling. Even in a test build the overrides stay
//! inert unless `OQTOPUS_TEST_MODE` is set. Callers ask a question through one of the public
//! functions and never read `OQTOPUS_TEST_*` variables themselves.

use std::ffi::OsString;
use std::fs;

/// Returns the value of the test override `name`; unset unless `OQTOPUS_TEST_MODE` is set.
fn var_os(name: &str) -> Option<OsString> {
    std::env::var_os("OQTOPUS_TEST_MODE")?;
    std::env::var_os(name)
}

/// Returns the UTF-8 value of the test override `name`; a non-UTF-8 value is ignored.
fn var(name: &str) -> Option<String> {
    var_os(name)?.into_string().ok()
}

/// Answers an HTTP request from a fixture, or returns `None` to use the real transport.
///
/// Two hooks exist because commands differ in how many requests they make. The manifest maps
/// several URLs to several responses, for commands that resolve a ref and then download an
/// archive. The single-response hook answers every request with the same bytes, optionally
/// asserting the URL, which is what a one-request command needs.
pub(crate) fn http_fixture(url: &str) -> Option<Result<Vec<u8>, ()>> {
    if let Some(path) = var_os("OQTOPUS_TEST_HTTP_FIXTURE_MANIFEST") {
        return Some(from_manifest(path, url));
    }
    let path = var_os("OQTOPUS_TEST_HTTP_RESPONSE_FILE")?;
    if let Some(expected) = var("OQTOPUS_TEST_EXPECTED_HTTP_URL")
        && expected != url
    {
        return Some(Err(()));
    }
    Some(fs::read(path).map_err(|_| ()))
}

fn from_manifest(manifest: OsString, url: &str) -> Result<Vec<u8>, ()> {
    let manifest = fs::read_to_string(manifest).map_err(|_| ())?;
    let fixture = manifest.lines().find_map(|line| {
        let (candidate, fixture) = line.split_once('\t')?;
        (candidate == url).then_some(fixture)
    });
    fs::read(fixture.ok_or(())?).map_err(|_| ())
}

/// Returns the pinned creation timestamp, so snapshots do not differ on every run.
pub(crate) fn created_at() -> Option<String> {
    var("OQTOPUS_TEST_CREATED_AT")
}

/// Reports whether the database port wait should be skipped.
pub(crate) fn db_port_ready() -> bool {
    var_os("OQTOPUS_TEST_DB_PORT_READY").is_some()
}
