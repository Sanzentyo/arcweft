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

pub(super) enum CheckedEntityReference {
    Item(CheckedProjectItem),
    Imported(crate::final_analysis::CheckedImportedProjectEntity),
}

impl CheckedEntityReference {
    pub(super) fn ty(&self) -> TypeKind {
        match self {
            Self::Item(item) => item.ty(),
            Self::Imported(entity) => entity.ty(),
        }
    }
    pub(super) fn into_value(self) -> crate::final_analysis::CheckedValueResolution {
        match self {
            Self::Item(item) => crate::final_analysis::CheckedValueResolution::ProjectItem(item),
            Self::Imported(entity) => {
                crate::final_analysis::CheckedValueResolution::ImportedProjectEntity(entity)
            }
        }
    }
    pub(super) fn into_pattern(self) -> crate::final_analysis::CheckedPatternResolution {
        match self {
            Self::Item(item) => crate::final_analysis::CheckedPatternResolution::Entity(item),
            Self::Imported(entity) => {
                crate::final_analysis::CheckedPatternResolution::ImportedProjectEntity(entity)
            }
        }
    }
    pub(super) fn into_local_item(self) -> Option<CheckedProjectItem> {
        match self {
            Self::Item(item) => Some(item),
            Self::Imported(_) => None,
        }
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn resolve_checked_entity_reference(
        &self,
        module: &HirModule,
        reference: &HirIdRef,
        source: SourceSpan,
    ) -> Result<CheckedEntityReference, EntityReferenceResolutionError> {
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
            .map(CheckedEntityReference::Item)
            .ok_or(EntityReferenceResolutionError::WrongFamily),
            ResolvedProjectSymbol::External(symbol) => {
                let owner = self
                    .catalogs
                    .world
                    .environment()
                    .bound_external_owner(self.symbols, symbol.declaration())
                    .map_err(|_| EntityReferenceResolutionError::WrongFamily)?;
                match owner {
                    crate::registration::RegisteredExternalOwner::Character(character) => Ok(
                        CheckedEntityReference::Item(CheckedProjectItem::new_external_character(
                            symbol.declaration(),
                            character.clone(),
                        )),
                    ),
                    crate::registration::RegisteredExternalOwner::ProjectEntity(entity) => {
                        Ok(CheckedEntityReference::Imported(
                            crate::final_analysis::CheckedImportedProjectEntity::new(
                                symbol.declaration(),
                                entity.clone(),
                                std::sync::Arc::clone(self.topology.generation()),
                            ),
                        ))
                    }
                    crate::registration::RegisteredExternalOwner::Environment(_) => {
                        Err(EntityReferenceResolutionError::WrongFamily)
                    }
                }
            }
            ResolvedProjectSymbol::StructuralCallable(symbol)
                if symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::Flow =>
            {
                CheckedProjectItem::new_flow(symbol.declaration().clone(), symbol.source_item())
                    .map(CheckedEntityReference::Item)
                    .ok_or(EntityReferenceResolutionError::WrongFamily)
            }
            ResolvedProjectSymbol::Callable(_)
            | ResolvedProjectSymbol::StructuralCallable(_)
            | ResolvedProjectSymbol::Nominal(_)
            | ResolvedProjectSymbol::Trait(_)
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

impl Analyzer<'_, '_, '_> {
    /// Participates in graph-domain target selection. Unknown is the empty
    /// external candidate set; ambiguity and foreign target kinds are errors.
    /// The caller combines this set with its Entry/selected-line domain before
    /// granting a value, rather than attempting a failed declaration fallback.
    pub(super) fn imported_graph_candidate(
        &self,
        module: &HirModule,
        reference: &HirIdRef,
        source: SourceSpan,
        family: arcweft_id::ProjectEntityReferenceFamily,
    ) -> Result<
        Option<crate::final_analysis::CheckedImportedProjectEntity>,
        EntityReferenceResolutionError,
    > {
        use arcweft_lang_hir::symbol::ProjectEntityReferenceLookupError;
        match self
            .symbols
            .resolve_entity_reference(module.key().path(), reference, source)
        {
            Err(ProjectEntityReferenceLookupError::Unknown { .. }) => Ok(None),
            Ok(ResolvedProjectSymbol::External(symbol)) => {
                let owner = self
                    .catalogs
                    .world
                    .environment()
                    .bound_external_owner(self.symbols, symbol.declaration())
                    .map_err(|_| EntityReferenceResolutionError::WrongFamily)?;
                match owner {
                    crate::registration::RegisteredExternalOwner::ProjectEntity(entity)
                        if entity.family() == family =>
                    {
                        Ok(Some(
                            crate::final_analysis::CheckedImportedProjectEntity::new(
                                symbol.declaration(),
                                entity.clone(),
                                std::sync::Arc::clone(self.topology.generation()),
                            ),
                        ))
                    }
                    _ => Err(EntityReferenceResolutionError::WrongFamily),
                }
            }
            Ok(_) => Err(EntityReferenceResolutionError::WrongFamily),
            Err(_) => Err(EntityReferenceResolutionError::Lookup),
        }
    }
}
