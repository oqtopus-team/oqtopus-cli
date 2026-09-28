//! Cloud-local component declarations shared by operations and version discovery.

use crate::operations::{Component, ComponentKind};

pub(super) const COMPONENTS: [Component; 3] = [
    Component {
        name: "cloud",
        repository: "oqtopus-team/oqtopus-cloud",
        binding_key: "cloud_local_cloud_version",
        kind: ComponentKind::CloudPython,
    },
    Component {
        name: "frontend",
        repository: "oqtopus-team/oqtopus-frontend",
        binding_key: "cloud_local_frontend_version",
        kind: ComponentKind::Static,
    },
    Component {
        name: "admin",
        repository: "oqtopus-team/oqtopus-admin",
        binding_key: "cloud_local_admin_version",
        kind: ComponentKind::Static,
    },
];
