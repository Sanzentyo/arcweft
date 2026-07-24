//! Project-wide declaration identities and the generalized symbol table.

mod error;
mod identity;
pub mod nominal;
mod table;
#[cfg(test)]
mod tests;

pub use error::{
    ProjectSymbolDiagnosticCode, ProjectSymbolLinkError, ProjectSymbolLinkReport,
    ProjectSymbolResolutionError,
};
pub use identity::{
    CallableDeclarationId, CallableDeclarationIdError, CallableDeclarationOwner, CallablePackageId,
    CallablePackageIdError, CallableSymbol, ExternalDeclarationId, ExternalDeclarationSeed,
    ExternalDeclarationSeedError, ExternalDeclarationSeedId, ExternalSymbol, ProjectDeclarationId,
    ProjectDirectBinding, ProjectDirectBindingError, ProjectExternalDeclarations,
    ProjectExternalDeclarationsError, ProjectSymbol, ProjectSymbolRevision, ProjectSymbolWorldId,
    ProjectSymbolWorldIdError,
};
pub use table::{
    ProjectSymbolBindingCollision, ProjectSymbolLimitKind, ProjectSymbolLimits,
    ProjectSymbolLinkOutput, ProjectSymbolTable, ProjectSymbolTargetId, ProjectTypeCandidate,
    ProjectTypeLookupError, ProjectTypeTarget, ProjectValueLookup, ProjectValueLookupError,
    ResolvedProjectSymbol, VisibleProjectTypeBinding,
};

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

pub(crate) fn qualified_name(module: &CanonicalModulePath, name: &str) -> String {
    let module_len = module
        .segments()
        .iter()
        .map(|segment| segment.as_str().len() + 1)
        .sum::<usize>();
    let mut qualified = String::with_capacity(module_len + name.len());
    for segment in module.segments() {
        qualified.push_str(segment.as_str());
        qualified.push('.');
    }
    qualified.push_str(name);
    qualified
}
