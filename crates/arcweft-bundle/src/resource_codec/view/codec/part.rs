//! Canonical exported-part inventory and provenance validation.

use super::super::model::{ViewExportedPart, ViewProgramInstruction, ViewProgramResource};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Typed failure produced while validating exported View-part records.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewExportValidationError {
    #[error("exported-part owner does not exist")]
    UnknownOwner,
    #[error("exported-part target does not exist")]
    MissingTarget,
    #[error("exported-part target belongs to another owner")]
    WrongOwner,
    #[error("a nested View call cannot be exported")]
    ReexportNotSupported,
    #[error("one static local part labels more than one instruction")]
    DuplicateStaticTarget,
    #[error("one local target is exported more than once")]
    DuplicateTarget,
    #[error("one View has duplicate exported public names")]
    DuplicatePublicName,
    #[error("exported parts are not in canonical order")]
    NonCanonicalOrder,
    #[error("exported-part source reference is unknown")]
    UnknownSource,
    #[error("exported-part source range is empty or reversed")]
    InvalidSourceRange,
    #[error("exported-part source range exceeds normalized source bounds")]
    SourceOutOfBounds,
    #[error("exported-part name ranges are not contained in the declaration")]
    SourceNotContained,
    #[error("exported-part local and public name ranges overlap")]
    SourceOverlap,
}

pub(super) fn validate_exports(
    program: &ViewProgramResource,
) -> Result<(), ViewExportValidationError> {
    validate_canonical_order(&program.exported_parts)?;
    let targets = owner_targets(program)?;
    let mut exported_targets = BTreeSet::new();
    let mut public_names = BTreeSet::new();

    for exported in &program.exported_parts {
        let owner = exported.target.view.view_id().as_str();
        let part = exported.target.part.as_public_id().as_str();
        let owner_targets = targets
            .get(owner)
            .ok_or(ViewExportValidationError::UnknownOwner)?;
        let is_view_call = owner_targets.get(part).copied().ok_or_else(|| {
            if targets.values().any(|parts| parts.contains_key(part)) {
                ViewExportValidationError::WrongOwner
            } else {
                ViewExportValidationError::MissingTarget
            }
        })?;
        if is_view_call {
            return Err(ViewExportValidationError::ReexportNotSupported);
        }
        if !exported_targets.insert((owner, part)) {
            return Err(ViewExportValidationError::DuplicateTarget);
        }
        if !public_names.insert((owner, exported.public_name.as_public_id().as_str())) {
            return Err(ViewExportValidationError::DuplicatePublicName);
        }
        validate_source_structure(exported)?;
    }
    Ok(())
}

fn validate_canonical_order(exports: &[ViewExportedPart]) -> Result<(), ViewExportValidationError> {
    if exports.windows(2).all(|pair| {
        let left = &pair[0];
        let right = &pair[1];
        (&left.target.view, &left.public_name, &left.target.part)
            < (&right.target.view, &right.public_name, &right.target.part)
    }) {
        Ok(())
    } else {
        Err(ViewExportValidationError::NonCanonicalOrder)
    }
}

fn owner_targets(
    program: &ViewProgramResource,
) -> Result<BTreeMap<&str, BTreeMap<&str, bool>>, ViewExportValidationError> {
    let mut owners = BTreeMap::new();
    for definition in &program.definitions {
        let instructions = program
            .instructions
            .get(
                definition.body.start_instruction as usize
                    ..definition.body.end_instruction as usize,
            )
            .ok_or(ViewExportValidationError::MissingTarget)?;
        let mut parts = BTreeMap::new();
        for instruction in instructions {
            let Some(part) = instruction.part() else {
                continue;
            };
            let part = part.as_public_id().as_str();
            let is_view_call = matches!(instruction, ViewProgramInstruction::CallView { .. });
            if parts.insert(part, is_view_call).is_some() {
                return Err(ViewExportValidationError::DuplicateStaticTarget);
            }
        }
        owners.insert(definition.public_id.as_str(), parts);
    }
    Ok(owners)
}

fn validate_source_structure(exported: &ViewExportedPart) -> Result<(), ViewExportValidationError> {
    let ranges = exported.source.ranges();
    let source = ranges[0].source();
    if ranges.iter().any(|range| range.source() != source) {
        return Err(ViewExportValidationError::UnknownSource);
    }
    if ranges
        .iter()
        .any(|range| range.start_byte() >= range.end_byte())
    {
        return Err(ViewExportValidationError::InvalidSourceRange);
    }
    let declaration = *ranges[0];
    if ranges[1..].iter().any(|range| {
        range.start_byte() < declaration.start_byte() || range.end_byte() > declaration.end_byte()
    }) {
        return Err(ViewExportValidationError::SourceNotContained);
    }
    let local = *ranges[1];
    let public = *ranges[2];
    if local.start_byte() < public.end_byte() && public.start_byte() < local.end_byte() {
        return Err(ViewExportValidationError::SourceOverlap);
    }
    Ok(())
}
