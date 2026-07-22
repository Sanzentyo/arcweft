//! The single recursive authority for authored nominal type references.

mod engine;

use super::{TypeResolutionInput, TypeResolutionInputError, TypeResolutionReport};

/// Resolves one validated authored reference through the accepted or detached world.
pub fn resolve_type_ref(
    input: &TypeResolutionInput<'_>,
) -> Result<TypeResolutionReport, TypeResolutionInputError> {
    engine::resolve_type_ref(input)
}
