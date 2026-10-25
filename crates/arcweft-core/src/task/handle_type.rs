//! The language carrier issued by a producer, independently of its executor.

use super::RuntimeTypeSemanticDigest;

/// Complete type of a temporal value. Program execution can produce an ordinary
/// `Need<T>`; execution backend and accepted-launch custody do not choose this
/// language type. The producer identity commits both the carrier and payload.
/// Borrowed codec preflight uses a unit payload to check the same carrier
/// grammar before materializing the original semantic type identity.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(deny_unknown_fields)]
pub enum TaskHandleType<Payload = RuntimeTypeSemanticDigest> {
    Need { payload: Payload },
    ThreadHandle { payload: Payload },
}

impl<Payload: Copy> TaskHandleType<Payload> {
    #[must_use]
    pub const fn payload_type(self) -> Payload {
        match self {
            Self::Need { payload } | Self::ThreadHandle { payload } => payload,
        }
    }

    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Need { .. } => 0,
            Self::ThreadHandle { .. } => 1,
        }
    }

    pub(crate) const fn from_semantic_tag(tag: u8, payload: Payload) -> Option<Self> {
        match tag {
            0 => Some(Self::Need { payload }),
            1 => Some(Self::ThreadHandle { payload }),
            _ => None,
        }
    }
}
