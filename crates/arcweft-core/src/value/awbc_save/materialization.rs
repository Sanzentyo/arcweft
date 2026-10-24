//! One value mapper with a closed choice of its legitimate construction owner.
//! Literal authority is scoped to one fully admitted original source graph.

use super::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeProgramOwner};
use crate::plan::RuntimePlanLiteralMaterialization;
use crate::value::RuntimeValue;

/// The enclosing executable and its already admitted Source share the original
/// group's allowance. Recursive value mapping copies only this borrowed view.
#[derive(Clone, Copy)]
pub(crate) struct RuntimeValueRestoreContext<'a> {
    owner: &'a RuntimeProgramOwner,
    source: Option<crate::task::TaskSourceRestore<'a>>,
}

impl<'a> RuntimeValueRestoreContext<'a> {
    pub(crate) const fn new(
        owner: &'a RuntimeProgramOwner,
        source: Option<crate::task::TaskSourceRestore<'a>>,
    ) -> Self {
        Self { owner, source }
    }

    pub(crate) const fn owner(self) -> &'a RuntimeProgramOwner {
        self.owner
    }

    pub(crate) const fn source(self) -> Option<crate::task::TaskSourceRestore<'a>> {
        self.source
    }

    /// A retained task keeps its original executable while spending the
    /// enclosing restore group's existing allowance.
    pub(crate) fn rebind<'b>(
        self,
        owner: &'b RuntimeProgramOwner,
        program: &'b crate::plan::task_source::RuntimeTaskSourceProgram,
    ) -> RuntimeValueRestoreContext<'b>
    where
        'a: 'b,
    {
        RuntimeValueRestoreContext::new(
            owner,
            self.source.map(|source| crate::task::TaskSourceRestore {
                program,
                admission: source.admission,
            }),
        )
    }
}

impl<'a> From<&'a RuntimeProgramOwner> for RuntimeValueRestoreContext<'a> {
    fn from(owner: &'a RuntimeProgramOwner) -> Self {
        Self::new(owner, None)
    }
}

#[derive(Clone, Copy)]
pub(super) enum SnapshotMaterialization<'a> {
    Standalone,
    Program(RuntimeValueRestoreContext<'a>),
    Literal(&'a RuntimePlanLiteralMaterialization<'a>),
}

impl<'a> From<Option<&'a RuntimeProgramOwner>> for SnapshotMaterialization<'a> {
    fn from(owner: Option<&'a RuntimeProgramOwner>) -> Self {
        match owner {
            Some(owner) => Self::Program(owner.into()),
            None => Self::Standalone,
        }
    }
}

impl<'a> SnapshotMaterialization<'a> {
    pub(super) fn program(self) -> Option<&'a RuntimeProgramOwner> {
        match self {
            Self::Program(context) => Some(context.owner()),
            Self::Standalone | Self::Literal(_) => None,
        }
    }

    pub(super) fn restore_context(self) -> Option<RuntimeValueRestoreContext<'a>> {
        match self {
            Self::Program(context) => Some(context),
            Self::Standalone | Self::Literal(_) => None,
        }
    }
}

impl AwbcRuntimeValueSnapshot {
    pub(crate) fn materialize_admitted_plan_literal(
        self,
        authority: &RuntimePlanLiteralMaterialization<'_>,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.into_live_with_context(SnapshotMaterialization::Literal(authority))
    }
}
