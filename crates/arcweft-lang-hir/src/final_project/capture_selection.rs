//! Capture ABI projection over source-validated uses and explicit choices.

use super::HirModuleEvaluationTopology;
use crate::identity::{CaptureId, ExprId, LocalId, SyntheticOwner};
use crate::scope::CaptureAccess;
use crate::source_index::HirCandidateSelectionError;

/// One capture after interpretation selection, in selected first-use order.
/// The enclosing semantic or runtime seal supplies the choice authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSelectedCapture {
    capture: CaptureId,
    local: LocalId,
    mode: CaptureAccess,
}

impl HirSelectedCapture {
    pub const fn capture(&self) -> CaptureId {
        self.capture
    }
    pub const fn local(&self) -> LocalId {
        self.local
    }
    pub const fn mode(&self) -> CaptureAccess {
        self.mode
    }
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum HirCaptureSelectionError {
    #[error("closure {owner:?} has no exact capture inventory")]
    MissingClosure { owner: ExprId },
    #[error(transparent)]
    Candidate(#[from] HirCandidateSelectionError),
}

impl HirModuleEvaluationTopology {
    /// Projects captures inside a closure's interpretation context. This does
    /// not admit the closure or its enclosing region for execution. Callers
    /// must seal their supplied decisions and admit that boundary separately.
    pub fn select_closure_captures(
        &self,
        owner: ExprId,
        mut selected: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<Box<[HirSelectedCapture]>, HirCaptureSelectionError> {
        let rows = self
            .captures()
            .captures_for_closure(owner)
            .ok_or(HirCaptureSelectionError::MissingClosure { owner })?;
        let mut captures = Vec::new();
        for row in rows {
            if row.closure() != owner {
                return Err(HirCaptureSelectionError::MissingClosure { owner });
            }
            let mut first_use = None;
            let mut mode = CaptureAccess::Read;
            for use_site in row.uses().iter() {
                if self.candidate_provenance().selects_region_within(
                    SyntheticOwner::Expr(use_site.site().owner()),
                    SyntheticOwner::Expr(owner),
                    &mut selected,
                )? {
                    first_use.get_or_insert(use_site.source().range().start());
                    mode = mode.required(use_site.access());
                }
            }
            if let Some(first_use) = first_use {
                captures.push((
                    first_use,
                    row.local(),
                    HirSelectedCapture {
                        capture: row.capture(),
                        local: row.local(),
                        mode,
                    },
                ));
            }
        }
        captures.sort_by_key(|(source, local, _)| (*source, *local));
        Ok(captures
            .into_iter()
            .map(|(_, _, capture)| capture)
            .collect())
    }
}
