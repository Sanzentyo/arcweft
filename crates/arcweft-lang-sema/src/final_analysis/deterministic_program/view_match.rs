//! Nonserialized View Match admission retains its actual checked execution owner.

use super::CheckedDeterministicProgram;
use arcweft_view::{
    AcceptedViewProgramRevision, CheckedViewMatchAdmissionDigest, ViewMatchSiteId, ViewProgramId,
};
use std::sync::Arc;

/// Compiler-local evidence keeps its original accepted execution owner.
///
/// ```compile_fail
/// use arcweft_lang_sema::final_analysis::CompilerLocalViewMatchCatalogRow;
/// fn serializable<T: serde::Serialize>() {}
/// serializable::<CompilerLocalViewMatchCatalogRow>();
/// ```
/// ```compile_fail
/// use arcweft_lang_sema::final_analysis::CompilerLocalViewMatchCatalogRow;
/// fn deserializable<T: serde::de::DeserializeOwned>() {}
/// deserializable::<CompilerLocalViewMatchCatalogRow>();
/// ```
#[derive(Clone, Debug)]
pub struct CompilerLocalViewMatchCatalogRow {
    program_id: ViewProgramId,
    revision: AcceptedViewProgramRevision,
    site: ViewMatchSiteId,
    admission: CheckedViewMatchAdmissionDigest,
    checked: Arc<CheckedDeterministicProgram>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompilerLocalViewMatchCatalogError {
    #[error(transparent)]
    Context(#[from] Box<crate::final_analysis::CheckedExecutionContextError>),
    #[error("the checked execution owner is not an exhaustive Match selector")]
    NotMatchSelection,
    #[error("the checked View Match coordinate exceeds its canonical length domain")]
    CoordinateLength,
}

impl CheckedDeterministicProgram {
    pub fn view_match_catalog_row(
        self: &Arc<Self>,
        analysis: &crate::final_analysis::FinalSemanticAnalysis,
        project: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
        program_id: ViewProgramId,
        revision: AcceptedViewProgramRevision,
    ) -> Result<CompilerLocalViewMatchCatalogRow, CompilerLocalViewMatchCatalogError> {
        self.input_abi()
            .environment()
            .validate_analysis(analysis)
            .map_err(Box::new)?;
        self.input_abi()
            .environment()
            .validate_project(project)
            .map_err(Box::new)?;
        let selection = self
            .input_abi()
            .match_selection()
            .ok_or(CompilerLocalViewMatchCatalogError::NotMatchSelection)?;
        let matched = selection.checked_match();
        let coordinate = matched
            .path()
            .canonical_bytes()
            .map_err(|_| CompilerLocalViewMatchCatalogError::CoordinateLength)?;
        Ok(CompilerLocalViewMatchCatalogRow {
            program_id,
            revision,
            site: ViewMatchSiteId::for_semantic_coordinate(&coordinate),
            admission: CheckedViewMatchAdmissionDigest::for_match_semantic_transcript(
                matched.semantic_digest().as_bytes(),
            ),
            checked: Arc::clone(self),
        })
    }
}

impl CompilerLocalViewMatchCatalogRow {
    pub fn validate_source(
        &self,
        analysis: &crate::final_analysis::FinalSemanticAnalysis,
        project: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    ) -> Result<(), CompilerLocalViewMatchCatalogError> {
        self.checked
            .input_abi()
            .environment()
            .validate_analysis(analysis)
            .map_err(Box::new)?;
        self.checked
            .input_abi()
            .environment()
            .validate_project(project)
            .map_err(Box::new)?;
        Ok(())
    }
    pub const fn program_id(&self) -> &ViewProgramId {
        &self.program_id
    }
    pub const fn accepted_revision(&self) -> AcceptedViewProgramRevision {
        self.revision
    }
    pub const fn site(&self) -> ViewMatchSiteId {
        self.site
    }
    pub const fn admission(&self) -> CheckedViewMatchAdmissionDigest {
        self.admission
    }
    pub fn checked_program(&self) -> &Arc<CheckedDeterministicProgram> {
        &self.checked
    }
}
