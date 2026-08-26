//! The single recursive authority for final-HIR nominal type graphs.

mod engine;

use super::{TypeResolutionInput, TypeResolutionInputError, TypeResolutionReport};
use arcweft_lang_hir::leaf::{HirPath, HirPathRoot, HirPathSegment};
use arcweft_lang_syntax::{ast::module_path::ModulePathRoot, types::TypePath};

/// Compares one root-preserving HIR path with one accepted canonical type path.
pub(crate) fn hir_path_matches_type_path(actual: &HirPath, expected: &TypePath) -> bool {
    let root_matches = matches!(
        (actual.root(), expected.root()),
        (HirPathRoot::ImplicitCrate, ModulePathRoot::ImplicitCrate)
            | (HirPathRoot::Crate, ModulePathRoot::Crate)
            | (HirPathRoot::SelfModule, ModulePathRoot::SelfModule)
    ) || matches!(
        (actual.root(), expected.root()),
        (HirPathRoot::Super { depth: actual }, ModulePathRoot::Super(expected)) if actual == expected
    );
    root_matches
        && actual.segments().len() == expected.segments().len()
        && actual
            .segments()
            .iter()
            .zip(expected.segments())
            .all(|(actual, expected)| {
                let actual = match actual {
                    HirPathSegment::Identifier(name) => name.as_str(),
                    HirPathSegment::ProjectSymbol(name) => name.as_str(),
                };
                actual == expected.as_str()
            })
}

/// Resolves one validated final-HIR root through the accepted or detached world.
pub fn resolve_type_ref(
    input: &TypeResolutionInput<'_>,
) -> Result<TypeResolutionReport, TypeResolutionInputError> {
    engine::resolve_type_ref(input)
}
