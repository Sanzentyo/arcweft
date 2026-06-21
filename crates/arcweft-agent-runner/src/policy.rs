use std::collections::BTreeSet;

/// Runtime policy resolved from compiled effects and launch profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAgentPolicy {
    allowed: BTreeSet<RuntimeAgentCapability>,
}

/// Capability that may be granted to an Agent controller at runtime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentCapability {
    Observe,
    Act,
    ActPhysical,
    Capture,
    ResourceRead,
    DebugRead,
    DebugRecord,
    Rag,
}

impl Default for RuntimeAgentPolicy {
    fn default() -> Self {
        Self::new([RuntimeAgentCapability::Observe])
    }
}

impl RuntimeAgentPolicy {
    pub fn new(capabilities: impl IntoIterator<Item = RuntimeAgentCapability>) -> Self {
        Self {
            allowed: capabilities.into_iter().collect(),
        }
    }

    pub fn allows(&self, capability: RuntimeAgentCapability) -> bool {
        self.allowed.contains(&capability)
    }
}

impl RuntimeAgentCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "agent.observe",
            Self::Act => "agent.act.semantic",
            Self::ActPhysical => "agent.act.physical",
            Self::Capture => "agent.capture",
            Self::ResourceRead => "agent.resource.read",
            Self::DebugRead => "debug.read",
            Self::DebugRecord => "debug.record",
            Self::Rag => "agent.rag.query",
        }
    }
}
