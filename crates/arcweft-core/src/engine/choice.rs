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
        operation: NativeSourceOperation,
        selected: Option<usize>,
    },
    Observed {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
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
                operation,
                selected: None,
            },
        };
        state.admit_native(admission)?;
        Ok(state)
    }

    /// Authored public choice identity, if present.
    pub fn id(&self) -> Option<&str> {
        match &self.source {
            ChoiceSource::Native {
                plan, operation, ..
            } => {
                let Some(FlowOp::Choice { id, .. }) = operation.operation(plan) else {
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
            ChoiceSource::Native {
                plan, operation, ..
            } => {
                let Some(FlowOp::Choice { options, .. }) = operation.operation(plan) else {
                    unreachable!("admitted original Choice operation")
                };
                options
            }
            ChoiceSource::Observed { options, .. } => options,
        }
    }

    pub(super) fn selected(&self) -> Option<usize> {
        match self.source {
            ChoiceSource::Native { selected, .. } => selected,
            ChoiceSource::Observed { .. } => None,
        }
    }

    pub(super) fn select(&mut self, index: usize) {
        assert!(index < self.options().len(), "original option ordinal");
        let ChoiceSource::Native { selected, .. } = &mut self.source else {
            unreachable!("only an admitted Native choice can accept input")
        };
        *selected = Some(index);
    }

    pub(super) fn selected_goto(&self) -> Option<super::defer::NativeGotoOrigin> {
        let ChoiceSource::Native {
            operation,
            selected: Some(option),
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
        let invalid = || AwbcRuntimeValueSnapshotError::Message {
            message: "Native Choice requires its original Plan operation and option ordinal".into(),
        };
        let ChoiceSource::Native {
            plan,
            operation,
            selected,
        } = &self.source
        else {
            return Err(invalid());
        };
        if !admission
            .program_owner()
            .is_some_and(|owner| owner.same_program(&RuntimeProgramOwner::Plan(plan.clone())))
        {
            return Err(invalid());
        }
        operation.admit_retained(admission)?;
        admission.validate_metadata(selected)?;
        let Some(FlowOp::Choice { id, options }) = operation.operation(plan) else {
            return Err(invalid());
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
        if selected.is_some_and(|index| index >= options.len()) {
            return Err(invalid());
        }
        Ok(())
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
