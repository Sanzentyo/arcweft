//! Stable View handler program identities and mount-time capture contracts.

use serde::{Deserialize, Serialize};

pub use arcweft_id::RuntimeSemanticTypeId as ViewHandlerValueTypeId;
pub use arcweft_id::runtime_program::RuntimePureProgramId as ViewHandlerProgramId;

/// Stable identity of one checked mount-time View handler value program.
///
/// The bytes are the exact checked call-application digest. Runtime and bundle
/// consumers compare this identity directly and never reconstruct it from a
/// handler label or source member spelling.
///
/// This is an opaque semantic join identity, not a content address of Product
/// AWBC instructions. Executable-body integrity belongs to the canonical
/// bundle content-root/signature authority; View cross-section validation
/// separately proves the exact input/result ABI and runtime owners.
/// Stable semantic identity of one handler input or result value type.
/// Declaration-ordered coordinate of one captured View parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ViewParameterCoordinate(u16);

/// One ordered capture consumed when a handler program is evaluated at mount.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewHandlerCapture {
    parameter: ViewParameterCoordinate,
    value_type: ViewHandlerValueTypeId,
}

/// Closed runtime role of a mount-time handler program result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewHandlerResultRole {
    DialogueAction,
}

/// Exact checked result contract of one mount-time handler value program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewHandlerResult {
    role: ViewHandlerResultRole,
    value_type: ViewHandlerValueTypeId,
}

impl ViewParameterCoordinate {
    pub fn try_from_index(index: usize) -> Option<Self> {
        u16::try_from(index).ok().map(Self)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl ViewHandlerCapture {
    #[must_use]
    pub const fn new(
        parameter: ViewParameterCoordinate,
        value_type: ViewHandlerValueTypeId,
    ) -> Self {
        Self {
            parameter,
            value_type,
        }
    }

    #[must_use]
    pub const fn parameter(self) -> ViewParameterCoordinate {
        self.parameter
    }

    #[must_use]
    pub const fn value_type(self) -> ViewHandlerValueTypeId {
        self.value_type
    }
}

impl ViewHandlerResult {
    #[must_use]
    pub const fn new(role: ViewHandlerResultRole, value_type: ViewHandlerValueTypeId) -> Self {
        Self { role, value_type }
    }

    #[must_use]
    pub const fn role(self) -> ViewHandlerResultRole {
        self.role
    }

    #[must_use]
    pub const fn value_type(self) -> ViewHandlerValueTypeId {
        self.value_type
    }
}
