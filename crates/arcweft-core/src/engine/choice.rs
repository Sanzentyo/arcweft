//! A Native choice retains its original immutable operation, not executable copies.

use std::sync::Arc;

use super::code_cursor::NativeSourceOperation;
use crate::plan::{ChoiceRuntimeOption, FlowOp, RuntimePlan};
use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

/// Suspended choice, with a borrowed presentation of its original options.
#[derive(Clone)]
pub struct ChoiceState {
    source: ChoiceSource,
}

#[derive(Clone)]
enum ChoiceSource {
    Native {
        plan: Arc<RuntimePlan>,
        continuation: NativeChoiceContinuation,
    },
    Observed {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeChoiceContinuation {
    pub(super) operation: NativeSourceOperation,
    pub(super) selected: Option<usize>,
}

impl ChoiceState {
    /// Describes a choice owned by another executor. This grants no Native
    /// execution or restore authority.
    pub fn observed(id: Option<String>, options: Vec<ChoiceRuntimeOption>) -> Self {
        Self {
            source: ChoiceSource::Observed { id, options },
        }
    }

    pub(super) fn native(
        plan: Arc<RuntimePlan>,
        operation: NativeSourceOperation,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let state = Self {
            source: ChoiceSource::Native {
                plan,
                continuation: NativeChoiceContinuation {
                    operation,
                    selected: None,
                },
            },
        };
        state.admit_native(admission)?;
        Ok(state)
    }

    /// Authored public choice identity, if present.
    pub fn id(&self) -> Option<&str> {
        match &self.source {
            ChoiceSource::Native { plan, continuation } => {
                let Some(FlowOp::Choice { id, .. }) = continuation.operation.operation(plan) else {
                    unreachable!("admitted original Choice operation")
                };
                id.as_deref()
            }
            ChoiceSource::Observed { id, .. } => id.as_deref(),
        }
    }

    /// Original options in source order, without copying executable effects.
    pub fn options(&self) -> &[ChoiceRuntimeOption] {
        match &self.source {
            ChoiceSource::Native { plan, continuation } => {
                let Some(FlowOp::Choice { options, .. }) = continuation.operation.operation(plan)
                else {
                    unreachable!("admitted original Choice operation")
                };
                options
            }
            ChoiceSource::Observed { options, .. } => options,
        }
    }

    pub(super) fn selected(&self) -> Option<usize> {
        match &self.source {
            ChoiceSource::Native { continuation, .. } => continuation.selected,
            ChoiceSource::Observed { .. } => None,
        }
    }

    pub(super) fn select(&mut self, index: usize) {
        assert!(index < self.options().len(), "original option ordinal");
        let ChoiceSource::Native { continuation, .. } = &mut self.source else {
            unreachable!("only an admitted Native choice can accept input")
        };
        continuation.selected = Some(index);
    }

    pub(super) fn selected_goto(&self) -> Option<super::defer::NativeGotoOrigin> {
        let ChoiceSource::Native {
            continuation:
                NativeChoiceContinuation {
                    operation,
                    selected: Some(option),
                },
            ..
        } = &self.source
        else {
            return None;
        };
        self.options()[*option]
            .target
            .as_ref()
            .map(|_| super::defer::NativeGotoOrigin::Choice {
                source: operation.clone(),
                option: *option,
            })
    }

    pub(super) fn selected_has_target(&self) -> bool {
        self.selected()
            .is_some_and(|index| self.options()[index].target.is_some())
    }

    pub(super) fn admit_native(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let ChoiceSource::Native { plan, continuation } = &self.source else {
            return Err(invalid_choice());
        };
        if !admission
            .program_owner()
            .is_some_and(|owner| owner.same_program(&RuntimeProgramOwner::Plan(plan.clone())))
        {
            return Err(invalid_choice());
        }
        continuation.admit_retained(admission)
    }

    pub(super) fn continuation(&self) -> Option<&NativeChoiceContinuation> {
        match &self.source {
            ChoiceSource::Native { continuation, .. } => Some(continuation),
            ChoiceSource::Observed { .. } => None,
        }
    }
}

fn invalid_choice() -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: "Native Choice requires its original Plan operation and option ordinal".into(),
    }
}

impl NativeChoiceContinuation {
    pub(super) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.operation.admit_retained(admission)?;
        admission.validate_metadata(&self.selected)?;
        let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner().cloned() else {
            return Err(invalid_choice());
        };
        let Some(FlowOp::Choice { id, options }) = self.operation.operation(&plan) else {
            return Err(invalid_choice());
        };
        admission.validate_metadata(id)?;
        admission.validate_metadata_collection(options.len())?;
        for option in options {
            admission.validate_metadata(&(
                &option.id,
                &option.label,
                &option.target,
                &option.out,
            ))?;
            // Effect code and its literal values remain in the admitted,
            // immutable Plan. No effect payload is retained in this state.
            admission.validate_metadata_collection(option.effects.len())?;
        }
        if self.selected.is_some_and(|index| index >= options.len()) {
            return Err(invalid_choice());
        }
        Ok(())
    }

    pub(super) fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<ChoiceState, String> {
        let RuntimeProgramOwner::Plan(plan) = owner else {
            return Err(invalid_choice().to_string());
        };
        Ok(ChoiceState {
            source: ChoiceSource::Native {
                plan: Arc::clone(plan),
                continuation: self,
            },
        })
    }
}

impl std::fmt::Debug for ChoiceState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChoiceState")
            .field("id", &self.id())
            .field("options", &self.options())
            .field("selected", &self.selected())
            .finish()
    }
}

impl PartialEq for ChoiceState {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
            && self.options() == other.options()
            && self.selected() == other.selected()
    }
}
