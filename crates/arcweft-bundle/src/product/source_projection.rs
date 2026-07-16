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
    ValidatedViewProduct::try_new(
        source_map.cloned(),
        view_program.cloned(),
        ViewProductValidationLimits::default(),
    )
    .map_err(|error| BundleCodecError::DecodeAwfb {
        message: error.to_string(),
    })?;
    if let (Some(style), Some(source_map)) = (view_style, source_map) {
        style
            .validate_environment_sources(source_map)
            .map_err(|error| BundleCodecError::DecodeAwfb {
                message: error.to_string(),
            })?;
    }
    Ok(())
}
