//! Complete-product validation before View resources cross a runtime boundary.

use std::sync::Arc;

use arcweft_source::{SourceRevision, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewIdentityError, ViewProgramId};
use thiserror::Error;

use crate::resource_codec::{
    ProductSourceId, ProductSourceRef, ProductSourceRefIndex, SectionCodecError, SourceMapSection,
    SourceRangeRef,
};

use super::{ViewDefinitionResource, ViewProgramResource};

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

/// Source map and optional View program accepted as one indivisible product.
#[derive(Clone, Debug)]
pub struct ValidatedViewProduct {
    source_map: Arc<SourceMapSection>,
    program: Option<Arc<ValidatedViewProgramResource>>,
}

#[derive(Clone, Debug)]
struct ValidatedViewSourceIndex {
    documents: Vec<usize>,
}

/// Failure to accept a complete View product.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewProductValidationError {
    #[error("a source-bearing View program has no SourceMap section")]
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
    #[error("invalid accepted View program revision: {0}")]
    InvalidAcceptedRevision(ViewIdentityError),
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
        limits: ViewProductValidationLimits,
    ) -> Result<Self, ViewProductValidationError> {
        let program = program
            .map(|candidate| validate_program(&candidate, source_map.as_ref(), limits))
            .transpose()?
            .map(Arc::new);
        let source_map = match source_map {
            Some(source_map) => source_map,
            None => SourceMapSection::try_from_documents(&[])
                .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?,
        };
        Ok(Self {
            source_map: Arc::new(source_map),
            program,
        })
    }

    pub fn source_map(&self) -> &SourceMapSection {
        &self.source_map
    }

    pub fn program(&self) -> Option<&ValidatedViewProgramResource> {
        self.program.as_deref()
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

    pub fn source_ref(&self, index: ProductSourceRefIndex) -> &ProductSourceRef {
        &self.resource.source_refs[index.index()]
    }

    pub const fn resource(&self) -> &ViewProgramResource {
        &self.resource
    }
}

fn validate_program(
    candidate: &ViewProgramResource,
    source_map: Option<&SourceMapSection>,
    limits: ViewProductValidationLimits,
) -> Result<ValidatedViewProgramResource, ViewProductValidationError> {
    let source_bearing =
        !candidate.source_refs.is_empty() || candidate.source_ranges().next().is_some();
    let empty_source_map;
    let source_map = match source_map {
        Some(source_map) => source_map,
        None if source_bearing => return Err(ViewProductValidationError::MissingSourceMap),
        None => {
            empty_source_map = SourceMapSection::try_from_documents(&[])
                .map_err(|_| ViewProductValidationError::ArithmeticOverflow)?;
            &empty_source_map
        }
    };
    validate_candidate_sources(candidate, source_map, limits)?;

    let encoded = candidate.encode_canonical_section()?;
    let resource = ViewProgramResource::decode_canonical_section(&encoded)?;
    validate_candidate_sources(&resource, source_map, limits)?;

    let program_id = resource.program_id.clone();
    let digest = resource.canonical_digest()?;
    let accepted_revision = AcceptedViewProgramRevision::try_from_bytes(digest.as_bytes())
        .map_err(ViewProductValidationError::InvalidAcceptedRevision)?;
    Ok(ValidatedViewProgramResource {
        resource,
        program_id,
        accepted_revision,
        source_set_revision: source_map.source_set_revision(),
    })
}

fn validate_candidate_sources(
    resource: &ViewProgramResource,
    source_map: &SourceMapSection,
    limits: ViewProductValidationLimits,
) -> Result<ValidatedViewSourceIndex, ViewProductValidationError> {
    let source_ref_count = resource.source_refs.len();
    let source_range_count = resource.source_ranges().count();
    enforce_count("source_refs", source_ref_count, limits.source_refs)?;
    enforce_count("source_ranges", source_range_count, limits.source_ranges)?;
    let relation_work = resource
        .exported_parts
        .len()
        .checked_mul(2)
        .ok_or(ViewProductValidationError::ArithmeticOverflow)?;
    let work = source_ref_count
        .checked_add(source_range_count)
        .and_then(|work| work.checked_add(relation_work))
        .ok_or(ViewProductValidationError::ArithmeticOverflow)?;
    enforce_u64("validation_work", work, limits.validation_work)?;

    let source_index = validate_source_refs(resource, source_map)?;
    for range in resource.source_ranges() {
        validate_range(range, resource, source_map, &source_index)?;
    }
    validate_export_relations(resource, source_map, &source_index)?;

    Ok(source_index)
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
    resource: &ViewProgramResource,
    source_map: &SourceMapSection,
) -> Result<ValidatedViewSourceIndex, ViewProductValidationError> {
    let documents = source_map.documents().collect::<Vec<_>>();
    let mut indexes = Vec::with_capacity(resource.source_refs.len());
    for source in &resource.source_refs {
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
    resource: &ViewProgramResource,
    source_map: &SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<(), ViewProductValidationError> {
    if range.start_byte() > range.end_byte() {
        return Err(ViewProductValidationError::ReversedRange);
    }
    let source = resolve_source(range, resource, source_map, source_index)?;
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
                    owner: resolve_source(declaration, resource, source_map, source_index)?
                        .id()
                        .clone(),
                    related: resolve_source(related, resource, source_map, source_index)?
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
    resource: &ViewProgramResource,
    source_map: &'a SourceMapSection,
    source_index: &ValidatedViewSourceIndex,
) -> Result<&'a crate::resource_codec::SourceMapDocument, ViewProductValidationError> {
    let index = range.source().index();
    if index >= resource.source_refs.len() || index >= source_index.documents.len() {
        return Err(ViewProductValidationError::InvalidSourceIndex {
            index: range.source().value(),
            count: resource.source_refs.len(),
        });
    }
    source_map
        .documents()
        .nth(source_index.documents[index])
        .ok_or(ViewProductValidationError::ArithmeticOverflow)
}
