//! Project-owned retained declaration identity publication and lookup.
//!
//! This is the only identity-to-`ItemId` index for authored retained values.
//! It consumes normalized final-HIR references and never reparses source text
//! or maintains a semantic side table in a consumer crate.

use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_source::SourceSpan;

use crate::item::{HirItem, HirRetainedName};
use crate::leaf::HirIdRef;
use crate::module::HirModuleStatus;
use crate::project::HirProjectView;
use crate::proof_return::HirProofReturnHeaderProjectView;
use crate::source_index::{HirDeclarationSourceRole, HirItemSourceRole};
use crate::symbol::{
    CallableDeclarationKey, ProjectDeclarationId, ProjectEntityReferenceLookupError,
    ProjectRetainedSymbol, ProjectSymbol, ProjectSymbolLinkError, ProjectSymbolTargetId,
    ResolvedProjectSymbol,
};

use super::{ImportResolutionError, ProjectSymbolModuleView, ProjectSymbolTable, ScopeBinding};

impl ProjectSymbolTable {
    pub(super) fn insert_retained_declarations(
        &mut self,
        project: HirProjectView<'_>,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        for item_ref in project.items() {
            self.insert_retained_declaration(
                item_ref.module_path(),
                ProjectSymbolModuleView::Published(item_ref.module()),
                item_ref.id(),
                item_ref.item(),
                item_ref.module().status() == HirModuleStatus::Clean,
                diagnostics,
                work,
            );
        }
    }

    /// Publishes retained declaration identities from the exact paused header
    /// transaction used to freeze Proof-return symbol authority.
    ///
    /// The final project continuation deliberately reuses this symbol table,
    /// so retained Character spellings must be present before body allocation
    /// rather than being reconstructed by a later consumer.
    pub(super) fn insert_retained_header_declarations(
        &mut self,
        project: HirProofReturnHeaderProjectView<'_, '_>,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        for item_ref in project.items() {
            let module = item_ref.module();
            self.insert_retained_declaration(
                module.key().path(),
                ProjectSymbolModuleView::ProofHeader(module),
                item_ref.id(),
                item_ref.item(),
                true,
                diagnostics,
                work,
            );
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one retained declaration row retains its typed module, source, state, and accounting owners"
    )]
    fn insert_retained_declaration(
        &mut self,
        module_path: &CanonicalModulePath,
        module: ProjectSymbolModuleView<'_, '_>,
        owner: crate::identity::ItemId,
        item: &HirItem,
        module_is_clean: bool,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        let Some(header) = item.kind().retained_header() else {
            return;
        };
        let Some(source) = module.item_source(
            owner,
            HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
        ) else {
            return;
        };
        if let Err(error) = Self::charge(work, 1, Some(source.clone())) {
            diagnostics.push(error);
            return;
        }
        let Some(public_id) = header.public_id().resolved() else {
            return;
        };
        let name = match header.name() {
            HirRetainedName::Resolved(name) => Some(name),
            HirRetainedName::IdentityOnly => None,
            HirRetainedName::Missing | HirRetainedName::Invalid => return,
        };

        // Account for every component before allocating binding and owner paths.
        let additional_components =
            u64::try_from(name.map_or(0, |name| name.segments().count().saturating_sub(1)))
                .unwrap_or(u64::MAX);
        if let Err(error) = Self::charge(work, additional_components, Some(source.clone())) {
            diagnostics.push(error);
            return;
        }

        let declaration_id = ProjectDeclarationId::Retained(public_id.clone());
        if let Some(ProjectSymbol::Retained(first)) = self.symbols.get(&declaration_id) {
            diagnostics.push(ProjectSymbolLinkError::DuplicatePublicId {
                public_id: public_id.clone(),
                first: first.declaration_span().clone(),
                duplicate: source,
            });
            return;
        }

        let visibility = item.prefix().visibility().map(super::hir_visibility);
        if let Some(name) = name {
            let path = ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                name.segments().map(|segment| {
                    ProjectSymbolSegment::try_new(segment)
                        .expect("declaration components are project symbol segments")
                }),
            )
            .expect("one declaration path owns its implicit project binding");
            let lookup_key = path.to_string();
            if let Some(first) = self
                .scopes
                .get(module_path)
                .and_then(|scope| scope.get(&lookup_key))
                .and_then(|bindings| bindings.first())
                .and_then(|binding| binding.sites.first())
                .cloned()
            {
                diagnostics.push(ProjectSymbolLinkError::duplicate_declaration(
                    module_path.clone(),
                    name.as_str().to_owned(),
                    first,
                    source,
                ));
                return;
            }
            let target = ProjectSymbolTargetId::Retained(public_id.clone());
            self.insert_scope_binding(
                module_path,
                ScopeBinding::new(
                    path,
                    target,
                    visibility,
                    module_path.clone(),
                    [source.clone()],
                ),
            );
        }
        self.symbols.insert(
            declaration_id,
            ProjectSymbol::Retained(ProjectRetainedSymbol::new(
                self.world.clone(),
                self.revision,
                public_id.clone(),
                header.family(),
                name.cloned(),
                owner,
                module_path.clone(),
                visibility,
                source,
                module_is_clean && !item.is_poisoned(),
            )),
        );
    }

    /// Resolves one normalized entity reference through a single typed target
    /// selection.
    ///
    /// Canonical retained public IDs and scope-visible external bindings enter
    /// the same candidate set before selection.  Callers therefore cannot
    /// attempt retained lookup and then fall back to an external reader.
    #[allow(
        clippy::result_large_err,
        reason = "entity-reference failures retain the complete typed reference, targets, and source identity"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "one terminal selection combines retained and external typed candidates without a fallback resolver"
    )]
    pub fn resolve_entity_reference(
        &self,
        requester: &CanonicalModulePath,
        reference: &HirIdRef,
        reference_span: SourceSpan,
    ) -> Result<ResolvedProjectSymbol<'_>, ProjectEntityReferenceLookupError> {
        let projection = entity_reference_projection(reference, &reference_span)?;
        let mut candidates = Vec::new();
        let mut inaccessible = Vec::new();

        match self.targets_for_symbol_path(requester, &projection.path) {
            Ok(bindings) => {
                candidates.extend(bindings.into_iter().map(|binding| binding.target));
            }
            Err(ImportResolutionError::Unknown) => {}
            Err(ImportResolutionError::Inaccessible(bindings)) => {
                inaccessible.extend(bindings.into_iter().map(|binding| binding.target));
            }
            Err(ImportResolutionError::Ambiguous(targets)) => candidates.extend(targets),
            Err(ImportResolutionError::InvalidPath(reason)) => {
                return Err(ProjectEntityReferenceLookupError::InvalidModulePath {
                    reference: reference.clone(),
                    reference_span,
                    reason,
                });
            }
            Err(ImportResolutionError::VisibilityEscalation) => {
                return Err(ProjectEntityReferenceLookupError::Inaccessible {
                    reference: reference.clone(),
                    reference_span,
                    candidates: Box::new([]),
                });
            }
        }

        if let Some(symbol) = self.retained(&projection.public_id) {
            let target = ProjectSymbolTargetId::Retained(projection.public_id.clone());
            if symbol.is_visible_from(requester) {
                candidates.push(target);
            } else {
                inaccessible.push(target);
            }
        }

        for symbol in self
            .callable_symbols()
            .filter(|symbol| symbol.public_id() == Some(&projection.public_id))
        {
            let target = match symbol.declaration() {
                CallableDeclarationKey::Flow(_) => {
                    ProjectSymbolTargetId::StructuralCallable(symbol.declaration().clone())
                }
                _ => ProjectSymbolTargetId::Callable(symbol.declaration().clone()),
            };
            if symbol.is_visible_from(requester) {
                candidates.push(target);
            } else {
                inaccessible.push(target);
            }
        }

        candidates.sort();
        candidates.dedup();
        match candidates.as_slice() {
            [target] => {
                if let ProjectSymbolTargetId::Retained(public_id) = target
                    && let Some(symbol) = self.retained(public_id)
                    && !symbol.is_executable()
                {
                    return Err(ProjectEntityReferenceLookupError::Poisoned {
                        reference: reference.clone(),
                        reference_span,
                        declaration: symbol.declaration_span().clone(),
                    });
                }
                if let ProjectSymbolTargetId::StructuralCallable(declaration)
                | ProjectSymbolTargetId::Callable(declaration) = target
                    && let Some(symbol) = self.callable(declaration)
                    && !symbol.is_executable()
                {
                    return Err(ProjectEntityReferenceLookupError::Poisoned {
                        reference: reference.clone(),
                        reference_span,
                        declaration: symbol.declaration_span().clone(),
                    });
                }
                self.resolve_target(target).ok_or_else(|| {
                    ProjectEntityReferenceLookupError::Unknown {
                        reference: reference.clone(),
                        reference_span,
                    }
                })
            }
            [] => {
                inaccessible.sort();
                inaccessible.dedup();
                if !inaccessible.is_empty() {
                    return Err(ProjectEntityReferenceLookupError::Inaccessible {
                        reference: reference.clone(),
                        reference_span,
                        candidates: inaccessible.into_boxed_slice(),
                    });
                }
                Err(ProjectEntityReferenceLookupError::Unknown {
                    reference: reference.clone(),
                    reference_span,
                })
            }
            candidates => Err(ProjectEntityReferenceLookupError::Ambiguous {
                reference: reference.clone(),
                reference_span,
                candidates: candidates.to_vec().into_boxed_slice(),
            }),
        }
    }
}

struct EntityReferenceProjection {
    path: SymbolPath,
    public_id: PublicId,
}

#[allow(
    clippy::result_large_err,
    reason = "projection failures preserve the full typed reference and exact source evidence"
)]
fn entity_reference_projection(
    reference: &HirIdRef,
    reference_span: &SourceSpan,
) -> Result<EntityReferenceProjection, ProjectEntityReferenceLookupError> {
    let text = match reference {
        HirIdRef::Absolute(reference) => reference.as_str().to_owned(),
        HirIdRef::Relative(_) => {
            return Err(ProjectEntityReferenceLookupError::RelativeRequiresFamily {
                reference: reference.clone(),
                reference_span: reference_span.clone(),
            });
        }
        HirIdRef::FamilyRelative(relative) => {
            let parent_depth = relative.relative().parent_depth();
            if parent_depth != 0 {
                return Err(ProjectEntityReferenceLookupError::UnsupportedParentDepth {
                    reference: reference.clone(),
                    reference_span: reference_span.clone(),
                    parent_depth,
                });
            }
            let Some(family) =
                arcweft_id::ProjectEntityReferenceFamily::from_prefix(relative.family().as_str())
            else {
                return Err(ProjectEntityReferenceLookupError::Unknown {
                    reference: reference.clone(),
                    reference_span: reference_span.clone(),
                });
            };
            if family == arcweft_id::ProjectEntityReferenceFamily::Asset {
                return Err(ProjectEntityReferenceLookupError::CatalogOwned {
                    reference: reference.clone(),
                    reference_span: reference_span.clone(),
                });
            }
            format!(
                "{}.{}",
                family.prefix(),
                relative.relative().suffix().as_str()
            )
        }
    };
    let public_id = PublicId::try_new(text).map_err(|reason| {
        ProjectEntityReferenceLookupError::InvalidIdentity {
            reference: reference.clone(),
            reference_span: reference_span.clone(),
            reason,
        }
    })?;
    if public_id
        .as_str()
        .split('.')
        .next()
        .and_then(DeclarationIdentityFamily::from_prefix)
        == Some(DeclarationIdentityFamily::Asset)
    {
        return Err(ProjectEntityReferenceLookupError::CatalogOwned {
            reference: reference.clone(),
            reference_span: reference_span.clone(),
        });
    }
    let segments = public_id
        .as_str()
        .split('.')
        .map(|segment| ProjectSymbolSegment::try_new(segment.to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(
            |_| ProjectEntityReferenceLookupError::InvalidReferencePath {
                reference: reference.clone(),
                reference_span: reference_span.clone(),
            },
        )?;
    let path = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, segments).map_err(|_| {
        ProjectEntityReferenceLookupError::InvalidReferencePath {
            reference: reference.clone(),
            reference_span: reference_span.clone(),
        }
    })?;
    let path = SymbolPath::try_from(&path).map_err(|_| {
        ProjectEntityReferenceLookupError::InvalidReferencePath {
            reference: reference.clone(),
            reference_span: reference_span.clone(),
        }
    })?;
    Ok(EntityReferenceProjection { path, public_id })
}
