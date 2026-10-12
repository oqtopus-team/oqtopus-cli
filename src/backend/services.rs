//! Backend service identities and ordering policies.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BackendService {
    Core,
    SseEngine,
    Mitigator,
    Estimator,
    Combiner,
    Tranqu,
    Gateway,
}

impl BackendService {
    /// Status order is consumed by the Manager.
    pub(super) const STATUS_ORDER: [Self; 7] = [
        Self::Core,
        Self::SseEngine,
        Self::Mitigator,
        Self::Estimator,
        Self::Combiner,
        Self::Tranqu,
        Self::Gateway,
    ];

    /// Start dependencies first; stop in the reverse order.
    pub(super) const START_ORDER: [Self; 7] = [
        Self::Gateway,
        Self::Tranqu,
        Self::Mitigator,
        Self::Estimator,
        Self::Combiner,
        Self::SseEngine,
        Self::Core,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::SseEngine => "sse_engine",
            Self::Mitigator => "mitigator",
            Self::Estimator => "estimator",
            Self::Combiner => "combiner",
            Self::Tranqu => "tranqu",
            Self::Gateway => "gateway",
        }
    }
}

impl std::str::FromStr for BackendService {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::STATUS_ORDER
            .into_iter()
            .find(|service| service.name() == name)
            .ok_or_else(|| format!("unknown service: {name}"))
    }
}
