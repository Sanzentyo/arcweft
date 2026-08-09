//! Shared typed name projection into final HIR.

use arcweft_lang_syntax::name::{SyntaxName, SyntaxNameIssue};

use crate::expr::HirRecoveredName;
use crate::identity::HirLimit;
use crate::leaf::{HirName, HirNameInvariantError};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::require_limit;

pub(crate) fn name(value: &SyntaxName) -> Result<HirName, HirLowerFailure> {
    require_limit(HirLimit::NameBytes, value.as_str().len())?;
    HirName::try_new(value.as_str().into())
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

pub(crate) const fn name_issue(_: &SyntaxNameIssue) -> HirNameInvariantError {
    HirNameInvariantError::InvalidIdentifier
}

pub(super) fn recovered_name(
    source: &Result<SyntaxName, SyntaxNameIssue>,
) -> Result<HirRecoveredName, HirLowerFailure> {
    match source {
        Ok(source) => name(source).map(HirRecoveredName::Valid),
        Err(SyntaxNameIssue::Missing) => Ok(HirRecoveredName::Missing),
        Err(issue) => {
            require_attempted_name_limit(issue)?;
            Ok(HirRecoveredName::InvalidPresent)
        }
    }
}

pub(super) fn require_attempted_name_limit(issue: &SyntaxNameIssue) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::NameBytes, attempted_name_bytes(issue))
}

pub(super) fn attempted_name_bytes(issue: &SyntaxNameIssue) -> usize {
    match issue {
        SyntaxNameIssue::Missing => 0,
        SyntaxNameIssue::InvalidStart { spelling }
        | SyntaxNameIssue::InvalidContinuation { spelling } => spelling.len(),
    }
}
