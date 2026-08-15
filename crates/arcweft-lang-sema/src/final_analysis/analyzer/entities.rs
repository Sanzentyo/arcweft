//! Shared typed entity-reference resolution.
//!
//! Expression and pattern analysis consume this one target match.  The module
//! deliberately has no retained-first/external-second fallback and never
//! reconstructs a path from source text.

use super::{
    Analyzer, CheckedProjectItem, HirIdRef, HirItemKind, HirModule, ItemId, ResolvedProjectSymbol,
    SourceSpan, TypeKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntityReferenceResolutionError {
    Lookup,
    WrongFamily,
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn resolve_checked_entity_reference(
        &self,
        module: &HirModule,
        reference: &HirIdRef,
        source: SourceSpan,
    ) -> Result<CheckedProjectItem, EntityReferenceResolutionError> {
        let target = self
            .symbols
            .resolve_entity_reference(module.key().path(), reference, source)
            .map_err(|_| EntityReferenceResolutionError::Lookup)?;
        match target {
            ResolvedProjectSymbol::Retained(symbol) => CheckedProjectItem::try_new_retained(
                symbol.public_id().clone(),
                symbol.family(),
                symbol.owner(),
                self.retained_entity_value_type(symbol.owner())?,
            )
            .ok_or(EntityReferenceResolutionError::WrongFamily),
            ResolvedProjectSymbol::External(symbol) => {
                let character = self
                    .catalogs
                    .world
                    .environment()
                    .character_owner(self.symbols, symbol.declaration())
                    .map_err(|_| EntityReferenceResolutionError::WrongFamily)?;
                Ok(CheckedProjectItem::new_external_character(
                    symbol.declaration(),
                    character,
                ))
            }
            ResolvedProjectSymbol::StructuralCallable(symbol)
                if symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::Flow =>
            {
                CheckedProjectItem::new_flow(symbol.declaration().clone(), symbol.source_item())
                    .ok_or(EntityReferenceResolutionError::WrongFamily)
            }
            ResolvedProjectSymbol::Callable(_)
            | ResolvedProjectSymbol::StructuralCallable(_)
            | ResolvedProjectSymbol::Nominal(_)
            | ResolvedProjectSymbol::Module(_) => Err(EntityReferenceResolutionError::WrongFamily),
        }
    }

    pub(super) fn retained_entity_value_type(
        &self,
        owner: ItemId,
    ) -> Result<Option<TypeKind>, EntityReferenceResolutionError> {
        let module = self
            .modules
            .get(&owner.module())
            .ok_or(EntityReferenceResolutionError::Lookup)?;
        let item = module
            .resolve_item(owner)
            .map_err(|_| EntityReferenceResolutionError::Lookup)?;
        let value = match item.kind() {
            HirItemKind::Signal(signal) => Some(signal.observable_type()),
            HirItemKind::Metric(metric) => Some(metric.value_type()),
            _ => None,
        };
        value
            .map(|value| {
                self.types
                    .get(&value)
                    .cloned()
                    .ok_or(EntityReferenceResolutionError::Lookup)
            })
            .transpose()
    }
}
