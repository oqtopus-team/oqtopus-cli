//! Manager component declaration shared by operations and version discovery.

use crate::operations::{Component, ComponentKind};

pub(super) const COMPONENT: Component = Component {
    name: "manager",
    repository: "oqtopus-team/oqtopus-manager",
    binding_key: "manager_version",
    kind: ComponentKind::Manager,
};
