//! Stable View handler program identities and mount-time capture contracts.

use serde::{Deserialize, Serialize};

pub use arcweft_id::RuntimeSemanticTypeId as ViewHandlerValueTypeId;
pub use arcweft_id::runtime_program::RuntimePureProgramId as ViewHandlerProgramId;

/// Stable identity of one checked mount-time View handler value program.
///
/// Sema authenticates the selected call application, exact admitted body
/// coordinate and execution intent. Runtime and bundle consumers compare this
/// identity directly and never reconstruct it from a label or member spelling.
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

/// One ordered parameter input consumed by a mount-time Core value program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewParameterInput {
    parameter: ViewParameterCoordinate,
    value_type: ViewHandlerValueTypeId,
}

/// Value produced by an event-time state transition before updated cells.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewHandlerTransitionValueRole {
    Unit,
    DialogueAction,
}

/// One updated input root returned after the transition value. The accepted
/// product joins this ordinal to a retained local capture and its field identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewHandlerStateWrite {
    input: u16,
    field: crate::ViewStateFieldId,
}

impl ViewHandlerStateWrite {
    pub fn try_new(input: usize, field: crate::ViewStateFieldId) -> Option<Self> {
        Some(Self {
            input: u16::try_from(input).ok()?,
            field,
        })
    }

    pub const fn input(self) -> usize {
        self.input as usize
    }

    pub const fn field(self) -> crate::ViewStateFieldId {
        self.field
    }
}

/// Complete publication role of one checked handler program result.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewHandlerResultRole {
    DialogueAction,
    StateTransition {
        value: ViewHandlerTransitionValueRole,
        writes: Box<[ViewHandlerStateWrite]>,
    },
}

/// Exact checked result and publication contract of one handler program.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
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

impl ViewParameterInput {
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
    pub const fn role(&self) -> &ViewHandlerResultRole {
        &self.role
    }

    #[must_use]
    pub const fn value_type(&self) -> ViewHandlerValueTypeId {
        self.value_type
    }

    /// Checks the input-side publication shape. Definition/lifetime and Core
    /// tuple type joins are authenticated by the accepted product boundary.
    pub fn writes_are_canonical(&self, captures: &[crate::ViewExecutionInput]) -> bool {
        let ViewHandlerResultRole::StateTransition { writes, .. } = &self.role else {
            return true;
        };
        let mut fields = std::collections::BTreeSet::new();
        writes
            .windows(2)
            .all(|pair| pair[0].input() < pair[1].input())
            && writes.iter().all(|write| {
                captures.get(write.input()).is_some_and(|input| {
                    matches!(input.source, crate::ViewExecutionInputSource::Local(_))
                }) && fields.insert(write.field())
            })
    }
}
