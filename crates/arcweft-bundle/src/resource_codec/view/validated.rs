//! Complete-product validation before View resources cross a runtime boundary.

use std::sync::Arc;

use arcweft_source::{SourceRevision, SourceSetRevision};
use arcweft_view::{
    AcceptedViewProgramRevision, ViewIdentityError, ViewProgramId,
    style::{
        ViewEnvironmentCondition, ViewStyleDeclaration, ViewStyleProgram, ViewStyleRule,
        ViewStyleSheet, ViewStyleToken,
    },
};
use thiserror::Error;

use crate::resource_codec::{
    ProductSourceId, ProductSourceRef, ProductSourceRefIndex, SectionCodecError, SourceMapSection,
    SourceRangeRef,
};

use super::{
    ViewDefinitionResource, ViewProgramResource, ViewStyleEnvironmentSourceError, ViewStyleResource,
};

/// Exact candidate limits applied before complete-product validation work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewProductValidationLimits {
    pub source_refs: usize,
    pub source_ranges: usize,
    pub validation_work: u64,
}

/// View program whose complete source dependencies and source ranges are valid.
#[derive(Clone, Debug)]
pub struct ValidatedViewProgramResource {
    resource: ViewProgramResource,
    program_id: ViewProgramId,
    accepted_revision: AcceptedViewProgramRevision,
    source_set_revision: SourceSetRevision,
}

/// Native Style resource accepted against the same complete source product.
#[derive(Clone, Debug)]
pub struct ValidatedViewStyleResource {
    resource: ViewStyleResource,
    source_set_revision: SourceSetRevision,
}

/// Source map and optional View resources accepted as one indivisible product.
#[derive(Clone, Debug)]
pub struct ValidatedViewProduct {
    source_map: Arc<SourceMapSection>,
    program: Option<Arc<ValidatedViewProgramResource>>,
    style: Option<Arc<ValidatedViewStyleResource>>,
}

#[derive(Clone, Debug)]
struct ValidatedViewSourceIndex {
    documents: Vec<usize>,
}

/// Failure to accept a complete View product.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewProductValidationError {
    #[error("a source-bearing View program or Style resource has no SourceMap section")]
    MissingSourceMap,
    #[error("unknown product source {id:?}")]
    MissingSource { id: ProductSourceId },
    #[error("product source reference is stale")]
    StaleSource {
        id: ProductSourceId,
        expected_revision: SourceRevision,
        actual_revision: SourceRevision,
        expected_len: u64,
        actual_len: u64,
    },
    #[error("source reference index {index} is out of bounds")]
    InvalidSourceIndex { index: u32, count: usize },
    #[error("related View source ranges cross source identities")]
    CrossSource {
        owner: ProductSourceId,
        related: ProductSourceId,
    },
    #[error("source range is reversed")]
    ReversedRange,
    #[error("source range is out of bounds")]
    OutOfBoundsRange,
    #[error("source range is not on UTF-8 boundaries")]
    NonUtf8Boundary,
    #[error("source operand range is not contained in its declaration")]
    RangeNotContained,
    #[error("View product source budget exceeded")]
    BudgetExceeded {
        resource: &'static str,
        actual: u64,
        limit: u64,
    },
    #[error("View product validation arithmetic overflow")]
    ArithmeticOverflow,
    #[error("non-canonical {resource} candidate")]
    NonCanonicalCandidate { resource: &'static str },
    #[error("invalid accepted View program revision: {0}")]
    InvalidAcceptedRevision(ViewIdentityError),
    #[error(transparent)]
    StyleEnvironment(#[from] ViewStyleEnvironmentSourceError),
    #[error(transparent)]
    View(#[from] SectionCodecError),
}

impl Default for ViewProductValidationLimits {
    fn default() -> Self {
        Self {
            source_refs: 65_536,
            source_ranges: 1_048_576,
            validation_work: 2_097_152,
        }
    }
}

impl ValidatedViewProduct {
    pub fn try_new(
        source_map: Option<SourceMapSection>,
        program: Option<ViewProgramResource>,
        style: Option<ViewStyleResource>,
        limits: ViewProductValidationLimits,
    ) -> Result<Self, ViewProductValidationError> {
        validate_product_limits(program.as_ref(), style.as_ref(), limits)?;

        let source_bearing = program.as_ref().is_some_and(program_is_source_bearing)
            || style.as_ref().is_some_and(style_is_source_bearing);
        let source_map = match source_map {
            Some(source_map) => source_map,
            None if source_bearing => return Err(ViewProductValidationError::MissingSourceMap),
            None => SourceMapSection::try_from_documents(&[])
                .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?,
        };

        let program_index = program
            .as_ref()
            .map(|resource| validate_source_refs(&resource.source_refs, &source_map))
            .transpose()?;
        let style_index = style
            .as_ref()
            .map(|resource| validate_source_refs(&resource.source_refs, &source_map))
            .transpose()?;
        if let Some(resource) = style.as_ref() {
            resource.validate_source_ids()?;
        }
        if let (Some(resource), Some(index)) = (program.as_ref(), program_index.as_ref()) {
            validate_program_ranges(resource, &source_map, index)?;
        }
        if let (Some(resource), Some(index)) = (style.as_ref(), style_index.as_ref()) {
            validate_style_ranges(resource, &source_map, index)?;
        }
        if let (Some(resource), Some(index)) = (program.as_ref(), program_index.as_ref()) {
            validate_export_relations(resource, &source_map, index)?;
        }
        if let Some(resource) = style.as_ref() {
            super::codec::style_environment::validate_structure(resource)?;
        }

        let program = program.map(canonical_program).transpose()?;
        let style = style.map(canonical_style).transpose()?;

        let program_index = program
            .as_ref()
            .map(|resource| validate_source_refs(&resource.source_refs, &source_map))
            .transpose()?;
        let style_index = style
            .as_ref()
            .map(|resource| validate_source_refs(&resource.source_refs, &source_map))
            .transpose()?;
        if let Some(resource) = style.as_ref() {
            resource.validate_source_ids()?;
        }
        if let (Some(resource), Some(index)) = (program.as_ref(), program_index.as_ref()) {
            validate_program_ranges(resource, &source_map, index)?;
        }
        if let (Some(resource), Some(index)) = (style.as_ref(), style_index.as_ref()) {
            validate_style_ranges(resource, &source_map, index)?;
        }
        if let (Some(resource), Some(index)) = (program.as_ref(), program_index.as_ref()) {
            validate_export_relations(resource, &source_map, index)?;
        }
        if let Some(resource) = style.as_ref() {
            super::codec::style_environment::validate_structure(resource)?;
        }

        let source_set_revision = source_map.source_set_revision();
        let program = program
            .map(|resource| -> Result<_, ViewProductValidationError> {
                let program_id = resource.program_id.clone();
                let accepted_revision = super::semantic::accepted_revision(&resource)?;
                Ok(Arc::new(ValidatedViewProgramResource {
                    resource,
                    program_id,
                    accepted_revision,
                    source_set_revision,
                }))
            })
            .transpose()?;
        let style = style.map(|resource| {
            Arc::new(ValidatedViewStyleResource {
                resource,
                source_set_revision,
            })
        });
        Ok(Self {
            source_map: Arc::new(source_map),
            program,
            style,
        })
    }

    pub fn source_map(&self) -> &SourceMapSection {
        &self.source_map
    }

    pub fn program(&self) -> Option<&ValidatedViewProgramResource> {
        self.program.as_deref()
    }

    pub fn style(&self) -> Option<&ValidatedViewStyleResource> {
        self.style.as_deref()
    }
}

impl ValidatedViewProgramResource {
    pub const fn program_id(&self) -> &ViewProgramId {
        &self.program_id
    }

    pub const fn accepted_revision(&self) -> AcceptedViewProgramRevision {
        self.accepted_revision
    }

    pub const fn source_set_revision(&self) -> SourceSetRevision {
        self.source_set_revision
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &ViewDefinitionResource> {
        self.resource.definitions.iter()
    }

    /// Resolves one accepted definition by its nominal public View identity.
    pub fn definition(&self, id: &arcweft_view::ViewId) -> Option<&ViewDefinitionResource> {
        self.resource
            .definitions
            .iter()
            .find(|definition| definition.public_id.view_id() == id)
    }

    pub fn source_ref(&self, index: ProductSourceRefIndex) -> &ProductSourceRef {
        &self.resource.source_refs[index.index()]
    }

    pub const fn resource(&self) -> &ViewProgramResource {
        &self.resource
    }
}

impl ValidatedViewStyleResource {
    pub const fn resource(&self) -> &ViewStyleResource {
        &self.resource
    }

    pub const fn program(&self) -> &ViewStyleProgram {
        &self.resource.program
    }

    pub const fn source_set_revision(&self) -> SourceSetRevision {
        self.source_set_revision
    }

    /// Compares source-free runtime Style semantics through the typed model.
    pub fn has_same_runtime_semantics(&self, other: &Self) -> bool {
        self.resource.style_program_id == other.resource.style_program_id
            && self.resource.adapter_requirements == other.resource.adapter_requirements
            && style_program_semantics_equal(&self.resource.program, &other.resource.program)
    }
}

fn validate_product_limits(
    program: Option<&ViewProgramResource>,
    style: Option<&ViewStyleResource>,
    limits: ViewProductValidationLimits,
) -> Result<(), ViewProductValidationError> {
    let source_ref_count = program
        .map_or(0, |resource| resource.source_refs.len())
        .checked_add(style.map_or(0, |resource| resource.source_refs.len()))
        .ok_or(ViewProductValidationError::ArithmeticOverflow)?;
    let source_range_count = program
        .map_or(0, |resource| resource.source_ranges().count())
        .checked_add(style.map_or(0, |resource| resource.source_map_refs.len()))
        .ok_or(ViewProductValidationError::ArithmeticOverflow)?;
    enforce_count("source_refs", source_ref_count, limits.source_refs)?;
    enforce_count("source_ranges", source_range_count, limits.source_ranges)?;

    let exported_part_count = program.map_or(0, |resource| resource.exported_parts.len());
    let (condition_count, wrapper_count, clause_count, nesting_edge_count) = style
        .into_iter()
        .flat_map(|resource| resource.program.sheets())
        .flat_map(ViewStyleSheet::rules)
        .filter_map(ViewStyleRule::environment)
        .try_fold(
            (0_usize, 0_usize, 0_usize, 0_usize),
            |(conditions, wrappers, clauses, nesting_edges), condition| {
                let condition_wrappers = condition.wrappers().len();
                Ok::<_, ViewProductValidationError>((
                    conditions
                        .checked_add(1)
                        .ok_or(ViewProductValidationError::ArithmeticOverflow)?,
                    wrappers
                        .checked_add(condition_wrappers)
                        .ok_or(ViewProductValidationError::ArithmeticOverflow)?,
                    clauses
                        .checked_add(condition.clauses().len())
                        .ok_or(ViewProductValidationError::ArithmeticOverflow)?,
                    nesting_edges
                        .checked_add(condition_wrappers.saturating_sub(1))
                        .ok_or(ViewProductValidationError::ArithmeticOverflow)?,
                ))
            },
        )?;
    let relation_work = exported_part_count
        .checked_mul(2)
        .and_then(|work| {
            wrapper_count
                .checked_mul(3)
                .and_then(|count| work.checked_add(count))
        })
        .and_then(|work| {
            clause_count
                .checked_mul(2)
                .and_then(|count| work.checked_add(count))
        })
        .and_then(|work| work.checked_add(nesting_edge_count))
        .and_then(|work| work.checked_add(condition_count))
        .ok_or(ViewProductValidationError::ArithmeticOverflow)?;
    let work = source_ref_count
        .checked_add(source_range_count)
        .and_then(|work| work.checked_add(relation_work))
        .ok_or(ViewProductValidationError::ArithmeticOverflow)?;
    enforce_u64("validation_work", work, limits.validation_work)
}

fn canonical_program(
    candidate: ViewProgramResource,
) -> Result<ViewProgramResource, ViewProductValidationError> {
    let encoded = candidate.encode_canonical_section()?;
    drop(candidate);
    ViewProgramResource::decode_canonical_section(&encoded).map_err(Into::into)
}

fn canonical_style(
    candidate: ViewStyleResource,
) -> Result<ViewStyleResource, ViewProductValidationError> {
    let encoded = candidate.encode_canonical_section()?;
    let canonical = ViewStyleResource::decode_canonical_section(&encoded)?;
    let is_canonical = candidate == canonical;
    drop(candidate);
    if is_canonical {
        Ok(canonical)
    } else {
        Err(ViewProductValidationError::NonCanonicalCandidate {
            resource: "ViewStyle",
        })
    }
}

fn program_is_source_bearing(resource: &ViewProgramResource) -> bool {
    !resource.source_refs.is_empty() || resource.source_ranges().next().is_some()
}

fn style_is_source_bearing(resource: &ViewStyleResource) -> bool {
    !resource.source_refs.is_empty()
        || !resource.source_map_refs.is_empty()
        || resource
            .program
            .sheets()
            .iter()
            .any(|sheet| !sheet.tokens().is_empty() || !sheet.rules().is_empty())
        || resource
            .program
            .patches()
            .iter()
            .any(|patch| !patch.declarations().is_empty())
}

fn validate_program_ranges(
    resource: &ViewProgramResource,
    source_map: &SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<(), ViewProductValidationError> {
    for range in resource.source_ranges() {
        validate_range(range, &resource.source_refs, source_map, source_index)?;
    }
    Ok(())
}

fn validate_style_ranges(
    resource: &ViewStyleResource,
    source_map: &SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<(), ViewProductValidationError> {
    for range in &resource.source_map_refs {
        validate_range(range, &resource.source_refs, source_map, source_index)?;
    }
    Ok(())
}

fn enforce_count(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), ViewProductValidationError> {
    if actual <= limit {
        Ok(())
    } else {
        Err(ViewProductValidationError::BudgetExceeded {
            resource,
            actual: u64::try_from(actual)
                .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?,
            limit: u64::try_from(limit)
                .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?,
        })
    }
}

fn enforce_u64(
    resource: &'static str,
    actual: usize,
    limit: u64,
) -> Result<(), ViewProductValidationError> {
    let actual =
        u64::try_from(actual).map_err(|_| ViewProductValidationError::ArithmeticOverflow)?;
    if actual <= limit {
        Ok(())
    } else {
        Err(ViewProductValidationError::BudgetExceeded {
            resource,
            actual,
            limit,
        })
    }
}

fn validate_source_refs(
    source_refs: &[ProductSourceRef],
    source_map: &SourceMapSection,
) -> Result<ValidatedViewSourceIndex, ViewProductValidationError> {
    let documents = source_map.documents().collect::<Vec<_>>();
    let mut indexes = Vec::with_capacity(source_refs.len());
    for source in source_refs {
        let document = source_map.get(source.id()).ok_or_else(|| {
            ViewProductValidationError::MissingSource {
                id: source.id().clone(),
            }
        })?;
        if source.revision() != document.revision() || source.source_len() != document.source_len()
        {
            return Err(ViewProductValidationError::StaleSource {
                id: source.id().clone(),
                expected_revision: source.revision(),
                actual_revision: document.revision(),
                expected_len: source.source_len(),
                actual_len: document.source_len(),
            });
        }
        let index = documents
            .binary_search_by(|candidate| candidate.id().cmp(source.id()))
            .map_err(|_| ViewProductValidationError::MissingSource {
                id: source.id().clone(),
            })?;
        indexes.push(index);
    }
    Ok(ValidatedViewSourceIndex { documents: indexes })
}

fn validate_range(
    range: &SourceRangeRef,
    source_refs: &[ProductSourceRef],
    source_map: &SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<(), ViewProductValidationError> {
    if range.start_byte() > range.end_byte() {
        return Err(ViewProductValidationError::ReversedRange);
    }
    let source = resolve_source(range, source_refs, source_map, source_index)?;
    let start = usize::try_from(range.start_byte())
        .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?;
    let end = usize::try_from(range.end_byte())
        .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?;
    if u64::from(range.end_byte()) > source.source_len() {
        return Err(ViewProductValidationError::OutOfBoundsRange);
    }
    if !source.text().is_char_boundary(start) || !source.text().is_char_boundary(end) {
        return Err(ViewProductValidationError::NonUtf8Boundary);
    }
    Ok(())
}

fn validate_export_relations(
    resource: &ViewProgramResource,
    source_map: &SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<(), ViewProductValidationError> {
    for export in &resource.exported_parts {
        let declaration = &export.source.declaration;
        for related in [&export.source.local_name, &export.source.public_name] {
            if declaration.source() != related.source() {
                return Err(ViewProductValidationError::CrossSource {
                    owner: resolve_source(
                        declaration,
                        &resource.source_refs,
                        source_map,
                        source_index,
                    )?
                    .id()
                    .clone(),
                    related: resolve_source(
                        related,
                        &resource.source_refs,
                        source_map,
                        source_index,
                    )?
                    .id()
                    .clone(),
                });
            }
            if related.start_byte() < declaration.start_byte()
                || related.end_byte() > declaration.end_byte()
            {
                return Err(ViewProductValidationError::RangeNotContained);
            }
        }
    }
    Ok(())
}

fn resolve_source<'a>(
    range: &SourceRangeRef,
    source_refs: &[ProductSourceRef],
    source_map: &'a SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<&'a crate::resource_codec::SourceMapDocument, ViewProductValidationError> {
    let index = range.source().index();
    if index >= source_refs.len() || index >= source_index.documents.len() {
        return Err(ViewProductValidationError::InvalidSourceIndex {
            index: range.source().value(),
            count: source_refs.len(),
        });
    }
    source_map
        .documents()
        .nth(source_index.documents[index])
        .ok_or(ViewProductValidationError::ArithmeticOverflow)
}

fn style_program_semantics_equal(left: &ViewStyleProgram, right: &ViewStyleProgram) -> bool {
    left.sheets().len() == right.sheets().len()
        && left
            .sheets()
            .iter()
            .zip(right.sheets())
            .all(|(left, right)| style_sheet_semantics_equal(left, right))
        && left.patches().len() == right.patches().len()
        && left
            .patches()
            .iter()
            .zip(right.patches())
            .all(|(left, right)| {
                left.id() == right.id()
                    && declarations_semantics_equal(left.declarations(), right.declarations())
            })
}

fn style_sheet_semantics_equal(left: &ViewStyleSheet, right: &ViewStyleSheet) -> bool {
    left.id() == right.id()
        && left.tokens().len() == right.tokens().len()
        && left
            .tokens()
            .iter()
            .zip(right.tokens())
            .all(|(left, right)| style_token_semantics_equal(left, right))
        && left.rules().len() == right.rules().len()
        && left
            .rules()
            .iter()
            .zip(right.rules())
            .all(|(left, right)| style_rule_semantics_equal(left, right))
}

fn style_token_semantics_equal(left: &ViewStyleToken, right: &ViewStyleToken) -> bool {
    left.id() == right.id()
        && left.value_kind() == right.value_kind()
        && left.value() == right.value()
}

fn style_rule_semantics_equal(left: &ViewStyleRule, right: &ViewStyleRule) -> bool {
    left.selector() == right.selector()
        && environment_semantics_equal(left.environment(), right.environment())
        && declarations_semantics_equal(left.declarations(), right.declarations())
        && left.source_order() == right.source_order()
}

fn environment_semantics_equal(
    left: Option<&ViewEnvironmentCondition>,
    right: Option<&ViewEnvironmentCondition>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.clauses().len() == right.clauses().len()
                && left
                    .clauses()
                    .iter()
                    .zip(right.clauses())
                    .all(|(left, right)| left.test() == right.test())
        }
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn declarations_semantics_equal(
    left: &[ViewStyleDeclaration],
    right: &[ViewStyleDeclaration],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.property() == right.property()
                && left.value() == right.value()
                && left.op() == right.op()
        })
}
