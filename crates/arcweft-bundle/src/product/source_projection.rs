use crate::BundleCodecError;
use crate::resource_codec::{
    SourceMapSection, ValidatedViewProduct, ViewProductValidationLimits, ViewProgramResource,
    ViewStyleResource,
};

pub(super) fn validate_view_sources(
    view_program: Option<&ViewProgramResource>,
    view_style: Option<&ViewStyleResource>,
    source_map: Option<&SourceMapSection>,
) -> Result<(), BundleCodecError> {
    ValidatedViewProduct::try_from_borrowed(
        source_map,
        view_program,
        view_style,
        ViewProductValidationLimits::default(),
    )
    .map_err(|error| BundleCodecError::DecodeAwfb {
        message: error.to_string(),
    })
    .map(|_| ())
}
