//! Pure projection from checked View-part exports into bundle product records.

use arcweft_bundle::{
    container::BundleDigest,
    resource_codec::{
        PublicIdRef, SourceMapSourceId, SourceRangeRef,
        view::{ViewDefinitionRef, ViewExportedPart, ViewOwnedPartRef, ViewPartExportSourceRef},
    },
};
use arcweft_lang_sema::view_part::{CheckedViewPartCatalog, CheckedViewPartExport};
use arcweft_lang_syntax::ast::common::TextRange;
use std::collections::BTreeSet;
use thiserror::Error;

/// Exact normalized source snapshot used to project checked ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartSourceContext {
    id: SourceMapSourceId,
    digest: BundleDigest,
    utf8_len: usize,
}

/// Failure to project one checked exported-part inventory.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewPartLowerError {
    #[error("checked exported-part owner and target owner differ")]
    WrongOwner,
    #[error("checked exported-part source identity does not match compiler source context")]
    SourceIdentityMismatch,
    #[error("exported-part source range exceeds the u32 product range")]
    SourceRangeOverflow,
    #[error("exported-part source range exceeds the normalized source extent")]
    SourceOutOfBounds,
}

impl ViewPartSourceContext {
    pub fn from_source(id: SourceMapSourceId, source: &str) -> Self {
        Self {
            id,
            digest: BundleDigest::of(source.as_bytes()),
            utf8_len: source.len(),
        }
    }

    pub const fn id(&self) -> &SourceMapSourceId {
        &self.id
    }
}

/// Projects checked exports for the View definitions emitted in this product cut.
pub fn lower_view_part_exports(
    catalog: &CheckedViewPartCatalog,
    emitted_owners: &BTreeSet<ViewDefinitionRef>,
    source: &ViewPartSourceContext,
) -> Result<Vec<ViewExportedPart>, ViewPartLowerError> {
    let mut exports = Vec::new();
    for owner in catalog.owners() {
        let owner_ref = ViewDefinitionRef::from_public_id(owner.id().public_id().clone());
        if !emitted_owners.contains(&owner_ref) {
            continue;
        }
        for export in owner.exports() {
            exports.push(lower_export(export, owner_ref.clone(), source)?);
        }
    }
    exports.sort_by(|left, right| {
        left.target
            .view
            .cmp(&right.target.view)
            .then(left.public_name.cmp(&right.public_name))
            .then(left.target.part.cmp(&right.target.part))
    });
    Ok(exports)
}

fn lower_export(
    export: &CheckedViewPartExport,
    owner: ViewDefinitionRef,
    source: &ViewPartSourceContext,
) -> Result<ViewExportedPart, ViewPartLowerError> {
    if export.owner() != export.target().owner() {
        return Err(ViewPartLowerError::WrongOwner);
    }
    if let Some(identity) = export.source().identity()
        && (identity.document().as_str() != source.id.as_str()
            || identity.source_len() != source.utf8_len
            || identity.revision().as_bytes() != &source.digest.as_bytes())
    {
        return Err(ViewPartLowerError::SourceIdentityMismatch);
    }
    let declaration = lower_range(export.source().declaration_range(), source.utf8_len)?;
    let local_name = lower_range(export.source().local_range(), source.utf8_len)?;
    let public_name = lower_range(export.source().public_range(), source.utf8_len)?;
    Ok(ViewExportedPart {
        target: ViewOwnedPartRef::new(owner, export.local_name().clone()),
        public_name: export.public_name().clone(),
        source: ViewPartExportSourceRef {
            source_id: source.id.clone(),
            declaration,
            local_name,
            public_name,
        },
    })
}

fn lower_range(range: TextRange, source_len: usize) -> Result<SourceRangeRef, ViewPartLowerError> {
    if range.end() > source_len {
        return Err(ViewPartLowerError::SourceOutOfBounds);
    }
    Ok(SourceRangeRef {
        source: PublicIdRef(0),
        start_byte: u32::try_from(range.start())
            .map_err(|_| ViewPartLowerError::SourceRangeOverflow)?,
        end_byte: u32::try_from(range.end())
            .map_err(|_| ViewPartLowerError::SourceRangeOverflow)?,
    })
}
