use crate::resource_codec::{
    SourceMapIndex, SourceMapSection, SourceMapSourceId, ViewProgramResource, ViewStyleResource,
};
use crate::{ArcweftBundle, BundleCodecError, BundleSource};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::compact_decode_error;

pub(super) fn validate_view_sources(
    view_program: Option<&ViewProgramResource>,
    view_style: Option<&ViewStyleResource>,
    source_map: Option<&SourceMapSection>,
) -> Result<(), BundleCodecError> {
    let program = view_program.filter(|program| !program.exported_parts.is_empty());
    let style = view_style.filter(|style| {
        style
            .program
            .sheets()
            .iter()
            .flat_map(arcweft_view::style::ViewStyleSheet::rules)
            .any(|rule| rule.environment().is_some())
    });
    if program.is_none() && style.is_none() {
        return Ok(());
    }
    let source = source_map.ok_or_else(|| BundleCodecError::DecodeAwfb {
        message: "source-provenanced View data requires a product source-map section".to_owned(),
    })?;
    let source = bundle_source_from_map(source)?;
    let index =
        SourceMapIndex::from_source(&source).map_err(|error| BundleCodecError::DecodeAwfb {
            message: error.to_string(),
        })?;
    if let Some(program) = program {
        program
            .validate_export_sources(&index)
            .map_err(|error| compact_decode_error(&error))?;
    }
    if let Some(style) = style {
        let source_id = SourceMapSourceId::try_new(source.label.clone()).map_err(|error| {
            BundleCodecError::DecodeAwfb {
                message: error.to_string(),
            }
        })?;
        style
            .validate_environment_sources(&index, &source_id)
            .map_err(|error| compact_decode_error(&error))?;
    }
    Ok(())
}

pub(super) fn source_map_for_bundle(
    bundle: &ArcweftBundle,
) -> Result<SourceMapSection, BundleCodecError> {
    let document_id = SourceDocumentId::try_new(bundle.source.label.clone()).map_err(|error| {
        BundleCodecError::EncodeAwfb {
            message: error.to_string(),
        }
    })?;
    let document = SourceDocument::try_new(
        document_id,
        SourceName::path(bundle.source.label.clone()),
        bundle.source.text.clone(),
    )
    .map_err(|error| BundleCodecError::EncodeAwfb {
        message: error.to_string(),
    })?;
    SourceMapSection::try_from_documents(&[&document]).map_err(|error| {
        BundleCodecError::EncodeAwfb {
            message: error.to_string(),
        }
    })
}

pub(super) fn bundle_source_from_map(
    section: &SourceMapSection,
) -> Result<BundleSource, BundleCodecError> {
    let mut documents = section.documents();
    let Some(document) = documents.next() else {
        return Err(BundleCodecError::DecodeAwfb {
            message: "product source map contains no entry source document".to_owned(),
        });
    };
    if documents.next().is_some() {
        return Err(BundleCodecError::DecodeAwfb {
            message:
                "the current in-memory bundle boundary cannot represent multiple source documents"
                    .to_owned(),
        });
    }
    Ok(BundleSource {
        label: document.document_id().as_str().to_owned(),
        text: document.text().to_owned(),
    })
}
