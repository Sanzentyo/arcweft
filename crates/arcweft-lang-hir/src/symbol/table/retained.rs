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

use crate::item::{
    HirCharacterSurfaceAlias, HirItem, HirItemKind, HirRetainedHeader, HirRetainedName,
};
use crate::leaf::HirIdRef;
use crate::module::HirModuleStatus;
use crate::project::HirProjectView;
use crate::proof_return::HirProofReturnHeaderProjectView;
use crate::source_index::{HirDeclarationSourceRole, HirItemSourceRole};
use crate::symbol::{
    CallableDeclarationKey, FlowPublicationKind, ProjectDeclarationId,
    ProjectEntityReferenceLookupError, ProjectRetainedSymbol, ProjectSymbol,
    ProjectSymbolLinkError, ProjectSymbolTargetId, ResolvedProjectSymbol,
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
        let Some(header) = retained_header(item.kind()) else {
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
        let (Some(public_id), HirRetainedName::Resolved(name)) =
            (header.public_id().resolved(), header.name())
        else {
            return;
        };

        let declaration_id = ProjectDeclarationId::Retained(public_id.clone());
        if let Some(ProjectSymbol::Retained(first)) = self.symbols.get(&declaration_id) {
            diagnostics.push(ProjectSymbolLinkError::DuplicatePublicId {
                public_id: public_id.clone(),
                first: first.declaration_span().clone(),
                duplicate: source,
            });
            return;
        }

        let mut binding_names = vec![name.as_str()];
        if let HirItemKind::Character(character) = item.kind()
            && let HirCharacterSurfaceAlias::Resolved(alias) = character.surface_alias()
            && alias.as_str() != name.as_str()
        {
            binding_names.push(alias.as_str());
        }
        let paths = binding_names
            .iter()
            .map(|binding| {
                ProjectSymbolPath::new(
                    ModulePathRoot::ImplicitCrate,
                    [ProjectSymbolSegment::try_new(*binding)
                        .expect("resolved retained names are project symbol segments")],
                )
                .expect("one retained name is a valid implicit project binding")
            })
            .collect::<Vec<_>>();
        for (binding, path) in binding_names.iter().zip(&paths) {
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
                    (*binding).to_owned(),
                    first,
                    source,
                ));
                return;
            }
        }

        let visibility = item.prefix().visibility().map(super::hir_visibility);
        let target = ProjectSymbolTargetId::Retained(public_id.clone());
        for path in paths {
            self.insert_scope_binding(
                module_path,
                ScopeBinding::new(
                    path,
                    target.clone(),
                    visibility,
                    module_path.clone(),
                    [source.clone()],
                ),
            );
        }
        self.symbols.insert(
            declaration_id,
            ProjectSymbol::Retained(ProjectRetainedSymbol::new(
                public_id.clone(),
                header.family(),
                name.clone(),
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

        for symbol in self.callable_symbols().filter(|symbol| {
            matches!(
                symbol.declaration(),
                CallableDeclarationKey::Flow(flow)
                    if flow.public_id() == &projection.public_id
                        && (flow.publication() == FlowPublicationKind::AuthoredAbsolute
                            || (flow.publication() == FlowPublicationKind::ModuleScoped
                                && flow.module() == requester))
            )
        }) {
            let target = ProjectSymbolTargetId::StructuralCallable(symbol.declaration().clone());
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
                if let ProjectSymbolTargetId::StructuralCallable(declaration) = target
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

fn retained_header(kind: &HirItemKind) -> Option<&HirRetainedHeader> {
    match kind {
        HirItemKind::Character(declaration) => Some(declaration.header()),
        HirItemKind::View(declaration) => Some(declaration.header()),
        HirItemKind::Action(declaration) => Some(declaration.header()),
        HirItemKind::Activity(declaration) => Some(declaration.header()),
        HirItemKind::Signal(declaration) => Some(declaration.header()),
        HirItemKind::Metric(declaration) => Some(declaration.header()),
        HirItemKind::Layer(declaration) => Some(declaration.header()),
        HirItemKind::Module(_)
        | HirItemKind::Use(_)
        | HirItemKind::Flow(_)
        | HirItemKind::Function(_)
        | HirItemKind::Predicate(_)
        | HirItemKind::Proof(_)
        | HirItemKind::Trait(_)
        | HirItemKind::Impl(_)
        | HirItemKind::Enum(_)
        | HirItemKind::Struct(_)
        | HirItemKind::TypeAlias(_)
        | HirItemKind::Resource(_)
        | HirItemKind::Entry(_)
        | HirItemKind::ExternCapability(_)
        | HirItemKind::Test(_)
        | HirItemKind::Bench(_)
        | HirItemKind::Style(_)
        | HirItemKind::Error(_) => None,
    }
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
            let Some(family) = DeclarationIdentityFamily::from_prefix(relative.family().as_str())
            else {
                return Err(ProjectEntityReferenceLookupError::Unknown {
                    reference: reference.clone(),
                    reference_span: reference_span.clone(),
                });
            };
            if family == DeclarationIdentityFamily::Asset {
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
