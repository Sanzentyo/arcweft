use serde::{Deserialize, Serialize};

/// Agent-specific publication controls kept outside the generic policy profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPublicationPolicy {
    /// Object-id and mask attachments are internal policy auxiliaries by default.
    pub publish_auxiliary_images: bool,
}

impl AgentPublicationPolicy {
    pub const fn strict_default() -> Self {
        Self {
            publish_auxiliary_images: false,
        }
    }
}

impl Default for AgentPublicationPolicy {
    fn default() -> Self {
        Self::strict_default()
    }
}
