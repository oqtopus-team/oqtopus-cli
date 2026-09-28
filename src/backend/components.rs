//! Backend component declarations shared by operations and version discovery.

use crate::operations::{Component, ComponentKind};

pub(super) const COMPONENTS: [Component; 3] = [
    Component {
        name: "engine",
        repository: "oqtopus-team/oqtopus-engine",
        binding_key: "engine_version",
        kind: ComponentKind::Engine,
    },
    Component {
        name: "tranqu",
        repository: "oqtopus-team/tranqu-server",
        binding_key: "tranqu_version",
        kind: ComponentKind::BackendPython,
    },
    Component {
        name: "gateway",
        repository: "oqtopus-team/device-gateway",
        binding_key: "gateway_version",
        kind: ComponentKind::BackendPython,
    },
];
