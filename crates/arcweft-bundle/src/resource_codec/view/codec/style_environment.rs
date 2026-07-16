//! Complete-product source validation for native Style environment guards.

use super::super::model::ViewStyleResource;
use crate::resource_codec::{PublicIdTable, SourceMapIndex, SourceMapSourceId, SourceRangeRef};
use arcweft_view::style::{ViewEnvironmentCondition, ViewStyleSourceId};
use thiserror::Error;

/// Invalid complete-product provenance for one native Style environment guard.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleEnvironmentSourceError {
    #[error("Style environment guard references an unknown product source")]
    UnknownSource,
    #[error("Style environment guard references an unknown Style source range")]
    UnknownRange,
    #[error("Style environment guard source range is empty or reversed")]
    InvalidRange,
    #[error("Style environment guard source range exceeds normalized source bounds")]
    SourceOutOfBounds,
    #[error("Style environment guard source range is not on UTF-8 boundaries")]
    InvalidUtf8Boundary,
    #[error("Style environment guard source does not belong to its Style sheet")]
    WrongOwner,
    #[error("Style environment clause source is not contained by its condition source")]
    ClauseNotContained,
}

pub(super) fn validate_structure(
    style: &ViewStyleResource,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    let table = style
        .public_id_table()
        .map_err(|_| ViewStyleEnvironmentSourceError::UnknownRange)?;
    for sheet in style.program.sheets() {
        let owner = sheet.id().public_id().as_str();
        for rule in sheet.rules() {
            let Some(condition) = rule.environment() else {
                continue;
            };
            let condition_range = owned_range(style, &table, owner, condition.source())?;
            owned_range(style, &table, owner, rule.source())?;
            for clause in condition.clauses() {
                let clause_range = owned_range(style, &table, owner, clause.source())?;
                if clause_range.start_byte < condition_range.start_byte
                    || clause_range.end_byte > condition_range.end_byte
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
    sources: &SourceMapIndex,
    source_id: &SourceMapSourceId,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    validate_structure(style)?;
    let entry = sources
        .entry(source_id)
        .ok_or(ViewStyleEnvironmentSourceError::UnknownSource)?;
    let table = style
        .public_id_table()
        .map_err(|_| ViewStyleEnvironmentSourceError::UnknownRange)?;

    for sheet in style.program.sheets() {
        let owner = sheet.id().public_id().as_str();
        for rule in sheet.rules() {
            let Some(condition) = rule.environment() else {
                continue;
            };
            let condition_range = checked_range(
                style,
                &table,
                sources,
                source_id,
                entry.utf8_len(),
                owner,
                condition.source(),
            )?;
            checked_range(
                style,
                &table,
                sources,
                source_id,
                entry.utf8_len(),
                owner,
                rule.source(),
            )?;
            validate_clauses(
                style,
                &table,
                sources,
                source_id,
                entry.utf8_len(),
                owner,
                condition,
                condition_range,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_clauses(
    style: &ViewStyleResource,
    table: &PublicIdTable,
    sources: &SourceMapIndex,
    source_id: &SourceMapSourceId,
    source_len: usize,
    owner: &str,
    condition: &ViewEnvironmentCondition,
    condition_range: SourceRangeRef,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    for clause in condition.clauses() {
        let clause_range = checked_range(
            style,
            table,
            sources,
            source_id,
            source_len,
            owner,
            clause.source(),
        )?;
        if clause_range.start_byte < condition_range.start_byte
            || clause_range.end_byte > condition_range.end_byte
        {
            return Err(ViewStyleEnvironmentSourceError::ClauseNotContained);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn checked_range(
    style: &ViewStyleResource,
    table: &PublicIdTable,
    sources: &SourceMapIndex,
    source_id: &SourceMapSourceId,
    source_len: usize,
    owner: &str,
    id: ViewStyleSourceId,
) -> Result<SourceRangeRef, ViewStyleEnvironmentSourceError> {
    let range = owned_range(style, table, owner, id)?;
    let start = range.start_byte as usize;
    let end = range.end_byte as usize;
    if end > source_len {
        return Err(ViewStyleEnvironmentSourceError::SourceOutOfBounds);
    }
    if !sources.is_utf8_boundary(source_id, start).unwrap_or(false)
        || !sources.is_utf8_boundary(source_id, end).unwrap_or(false)
    {
        return Err(ViewStyleEnvironmentSourceError::InvalidUtf8Boundary);
    }
    Ok(range)
}

fn owned_range(
    style: &ViewStyleResource,
    table: &PublicIdTable,
    owner: &str,
    id: ViewStyleSourceId,
) -> Result<SourceRangeRef, ViewStyleEnvironmentSourceError> {
    let range = *style
        .source_map_refs
        .get(id.value() as usize)
        .ok_or(ViewStyleEnvironmentSourceError::UnknownRange)?;
    let actual_owner = table
        .get(range.source)
        .map_err(|_| ViewStyleEnvironmentSourceError::UnknownRange)?;
    if actual_owner != owner {
        return Err(ViewStyleEnvironmentSourceError::WrongOwner);
    }
    if range.start_byte >= range.end_byte {
        return Err(ViewStyleEnvironmentSourceError::InvalidRange);
    }
    Ok(range)
}
