//! Pure projection from checked View-part exports into bundle product records.

use arcweft_bundle::resource_codec::{
    ProductSourceRef, SourceMapSection, SourceRangeRef, ViewProductBuildError,
    view::{ViewDefinitionRef, ViewExportedPart, ViewOwnedPartRef, ViewPartExportSourceRef},
};
use arcweft_lang_sema::view_part::{CheckedViewPartCatalog, CheckedViewPartExport};
use arcweft_source::{SourceDocumentId, SourceSpan};
use std::collections::BTreeSet;
use thiserror::Error;

/// Canonical source table and exported-part records produced together.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LoweredViewPartExports {
    source_refs: Vec<ProductSourceRef>,
    exports: Vec<ViewExportedPart>,
}

/// Failure to project one checked exported-part inventory.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewPartLowerError {
    #[error("checked exported-part owner and target owner differ")]
    WrongOwner,
    #[error("checked exported-part source `{0}` is absent from the product source map")]
    MissingSource(SourceDocumentId),
    #[error("checked exported-part source identity does not match the product source map")]
    SourceIdentityMismatch,
    #[error("exported-part source range exceeds the u32 product range")]
    SourceRangeOverflow,
    #[error("exported-part source range exceeds its source document extent")]
    SourceOutOfBounds,
    #[error(transparent)]
    ProductSource(#[from] ViewProductBuildError),
}

impl LoweredViewPartExports {
    pub fn into_parts(self) -> (Vec<ProductSourceRef>, Vec<ViewExportedPart>) {
        (self.source_refs, self.exports)
    }

    pub fn source_refs(&self) -> &[ProductSourceRef] {
        &self.source_refs
    }

    pub fn exports(&self) -> &[ViewExportedPart] {
        &self.exports
    }
}

/// Projects checked exports for the View definitions emitted in this product cut.
pub fn lower_view_part_exports(
    catalog: &CheckedViewPartCatalog,
    emitted_owners: &BTreeSet<ViewDefinitionRef>,
    source_map: &SourceMapSection,
) -> Result<LoweredViewPartExports, ViewPartLowerError> {
    let selected = catalog
        .owners()
        .iter()
        .filter_map(|owner| {
            let owner_ref = ViewDefinitionRef::from_public_id(owner.id().public_id().clone());
            emitted_owners
                .contains(&owner_ref)
                .then_some((owner_ref, owner.exports()))
        })
        .flat_map(|(owner, exports)| exports.iter().map(move |export| (owner.clone(), export)))
        .collect::<Vec<_>>();

    let mut source_refs = selected
        .iter()
        .flat_map(|(_, export)| {
            [
                export.source().declaration_span(),
                export.source().local_operand_span(),
                export.source().public_operand_span(),
            ]
        })
        .map(|span| source_ref_for_span(span, source_map))
        .collect::<Result<Vec<_>, _>>()?;
    source_refs.sort();
    source_refs.dedup();

    let mut exports = selected
        .into_iter()
        .map(|(owner, export)| lower_export(export, owner, source_map, &source_refs))
        .collect::<Result<Vec<_>, _>>()?;
    exports.sort_by(|left, right| {
        left.target
            .view
            .cmp(&right.target.view)
            .then(left.public_name.cmp(&right.public_name))
            .then(left.target.part.cmp(&right.target.part))
    });
    Ok(LoweredViewPartExports {
        source_refs,
        exports,
    })
}

fn lower_export(
    export: &CheckedViewPartExport,
    owner: ViewDefinitionRef,
    source_map: &SourceMapSection,
    source_refs: &[ProductSourceRef],
) -> Result<ViewExportedPart, ViewPartLowerError> {
    if export.owner() != export.target().owner() {
        return Err(ViewPartLowerError::WrongOwner);
    }
    let declaration = lower_range(export.source().declaration_span(), source_map, source_refs)?;
    let local_name = lower_range(
        export.source().local_operand_span(),
        source_map,
        source_refs,
    )?;
    let public_name = lower_range(
        export.source().public_operand_span(),
        source_map,
        source_refs,
    )?;
    Ok(ViewExportedPart {
        target: ViewOwnedPartRef::new(owner, export.local_name().clone()),
        public_name: export.public_name().clone(),
        source: ViewPartExportSourceRef {
            declaration,
            local_name,
            public_name,
        },
    })
}

fn lower_range(
    span: &SourceSpan,
    source_map: &SourceMapSection,
    source_refs: &[ProductSourceRef],
) -> Result<SourceRangeRef, ViewPartLowerError> {
    let source = source_ref_for_span(span, source_map)?;
    let range = span.range();
    if u64::try_from(range.end()).unwrap_or(u64::MAX) > source.source_len() {
        return Err(ViewPartLowerError::SourceOutOfBounds);
    }
    SourceRangeRef::try_for_source(
        source_refs,
        &source,
        u32::try_from(range.start()).map_err(|_| ViewPartLowerError::SourceRangeOverflow)?,
        u32::try_from(range.end()).map_err(|_| ViewPartLowerError::SourceRangeOverflow)?,
    )
    .map_err(Into::into)
}

fn source_ref_for_span(
    span: &SourceSpan,
    source_map: &SourceMapSection,
) -> Result<ProductSourceRef, ViewPartLowerError> {
    let identity = span.source();
    let document = source_map
        .documents()
        .find(|document| document.document_id() == identity.id())
        .ok_or_else(|| ViewPartLowerError::MissingSource(identity.id().clone()))?;
    if document.revision() != identity.revision() || document.source_len() != identity.source_len()
    {
        return Err(ViewPartLowerError::SourceIdentityMismatch);
    }
    Ok(ProductSourceRef::from_document(document))
}
