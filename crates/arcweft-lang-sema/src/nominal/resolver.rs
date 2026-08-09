//! The single recursive authority for final-HIR nominal type graphs.

mod engine;

use super::{TypeResolutionInput, TypeResolutionInputError, TypeResolutionReport};

/// Resolves one validated final-HIR root through the accepted or detached world.
pub fn resolve_type_ref(
    input: &TypeResolutionInput<'_>,
) -> Result<TypeResolutionReport, TypeResolutionInputError> {
    engine::resolve_type_ref(input)
}
