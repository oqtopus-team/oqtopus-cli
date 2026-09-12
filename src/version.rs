//! Version information from build metadata.

pub(crate) struct VersionInfo {
    pub(crate) version: &'static str,
}

pub(crate) fn version_info() -> VersionInfo {
    VersionInfo {
        version: env!("CARGO_PKG_VERSION"),
    }
}
