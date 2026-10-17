//! One value mapper with a closed choice of its legitimate construction owner.
//! Literal authority is scoped to one fully admitted original source graph.

use super::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeProgramOwner};
use crate::plan::RuntimePlanLiteralMaterialization;
use crate::value::RuntimeValue;

#[derive(Clone, Copy)]
pub(super) enum SnapshotMaterialization<'a> {
    Standalone,
    Program(&'a RuntimeProgramOwner),
    Literal(&'a RuntimePlanLiteralMaterialization<'a>),
}

impl<'a> From<Option<&'a RuntimeProgramOwner>> for SnapshotMaterialization<'a> {
    fn from(owner: Option<&'a RuntimeProgramOwner>) -> Self {
        match owner {
            Some(owner) => Self::Program(owner),
            None => Self::Standalone,
        }
    }
}

impl<'a> SnapshotMaterialization<'a> {
    pub(super) fn program(self) -> Option<&'a RuntimeProgramOwner> {
        match self {
            Self::Program(owner) => Some(owner),
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
