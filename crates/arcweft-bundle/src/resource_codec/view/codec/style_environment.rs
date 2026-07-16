//! Complete-product source validation for native Style environment guards.

use super::super::model::ViewStyleResource;
use crate::resource_codec::{SourceMapSection, SourceRangeRef};
use arcweft_view::style::{ViewEnvironmentCondition, ViewStyleSourceId};
use thiserror::Error;

/// Invalid complete-product provenance for one native Style environment guard.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleEnvironmentSourceError {
    #[error("Style environment guard references an unknown product source")]
    UnknownSource,
    #[error("Style environment guard references a stale product source")]
    StaleSource,
    #[error("Style environment guard references an unknown Style source range")]
    UnknownRange,
    #[error("Style environment guard source range is empty or reversed")]
    InvalidRange,
    #[error("Style environment guard source range exceeds normalized source bounds")]
    SourceOutOfBounds,
    #[error("Style environment guard source range is not on UTF-8 boundaries")]
    InvalidUtf8Boundary,
    #[error("related Style environment ranges cross source identities")]
    WrongOwner,
    #[error("Style environment clause source is not contained by its condition source")]
    ClauseNotContained,
}

pub(super) fn validate_structure(
    style: &ViewStyleResource,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    for sheet in style.program.sheets() {
        for rule in sheet.rules() {
            let Some(condition) = rule.environment() else {
                continue;
            };
            let condition_range = owned_range(style, condition.source())?;
            owned_range(style, rule.source())?;
            for clause in condition.clauses() {
                let clause_range = owned_range(style, clause.source())?;
                ensure_same_source(condition_range, clause_range)?;
                if clause_range.start_byte() < condition_range.start_byte()
                    || clause_range.end_byte() > condition_range.end_byte()
                {
                    return Err(ViewStyleEnvironmentSourceError::ClauseNotContained);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_source_extents(
    style: &ViewStyleResource,
    sources: &SourceMapSection,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    validate_structure(style)?;
    for sheet in style.program.sheets() {
        for rule in sheet.rules() {
            let Some(condition) = rule.environment() else {
                continue;
            };
            let condition_range = checked_range(style, sources, condition.source())?;
            checked_range(style, sources, rule.source())?;
            validate_clauses(style, sources, condition, condition_range)?;
        }
    }
    Ok(())
}

fn validate_clauses(
    style: &ViewStyleResource,
    sources: &SourceMapSection,
    condition: &ViewEnvironmentCondition,
    condition_range: SourceRangeRef,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    for clause in condition.clauses() {
        let clause_range = checked_range(style, sources, clause.source())?;
        ensure_same_source(condition_range, clause_range)?;
        if clause_range.start_byte() < condition_range.start_byte()
            || clause_range.end_byte() > condition_range.end_byte()
        {
            return Err(ViewStyleEnvironmentSourceError::ClauseNotContained);
        }
    }
    Ok(())
}

fn checked_range(
    style: &ViewStyleResource,
    sources: &SourceMapSection,
    id: ViewStyleSourceId,
) -> Result<SourceRangeRef, ViewStyleEnvironmentSourceError> {
    let range = owned_range(style, id)?;
    let source = style
        .source_refs
        .get(range.source().value() as usize)
        .ok_or(ViewStyleEnvironmentSourceError::UnknownSource)?;
    let document = sources
        .get(source.id())
        .ok_or(ViewStyleEnvironmentSourceError::UnknownSource)?;
    if document.revision() != source.revision() || document.source_len() != source.source_len() {
        return Err(ViewStyleEnvironmentSourceError::StaleSource);
    }
    let start = range.start_byte() as usize;
    let end = range.end_byte() as usize;
    if end > document.text().len() {
        return Err(ViewStyleEnvironmentSourceError::SourceOutOfBounds);
    }
    if !document.text().is_char_boundary(start) || !document.text().is_char_boundary(end) {
        return Err(ViewStyleEnvironmentSourceError::InvalidUtf8Boundary);
    }
    Ok(range)
}

fn owned_range(
    style: &ViewStyleResource,
    id: ViewStyleSourceId,
) -> Result<SourceRangeRef, ViewStyleEnvironmentSourceError> {
    let range = *style
        .source_map_refs
        .get(id.value() as usize)
        .ok_or(ViewStyleEnvironmentSourceError::UnknownRange)?;
    if range.start_byte() >= range.end_byte() {
        return Err(ViewStyleEnvironmentSourceError::InvalidRange);
    }
    Ok(range)
}

fn ensure_same_source(
    owner: SourceRangeRef,
    related: SourceRangeRef,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    if owner.source() == related.source() {
        Ok(())
    } else {
        Err(ViewStyleEnvironmentSourceError::WrongOwner)
    }
}
