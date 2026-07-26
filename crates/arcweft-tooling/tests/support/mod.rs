use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_tooling::{
    format::format_document,
    model::{FormatOptions, ToolingEditReport, ToolingError},
};
use std::sync::Arc;

pub fn format_fixture(
    source: &str,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/format.arcw")
                .expect("fixture source ID"),
            SourceName::path("format.arcw"),
            source,
        )
        .expect("fixture source document"),
    );
    format_document(document, options)
}
