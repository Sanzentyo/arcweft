//! Move-only Entry roots prepared before contextual statement checking.

use std::collections::BTreeMap;

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    identity::{ItemId, TypeId},
    item::{HirEntryKind, HirEntryMember, HirEntryTarget, HirItemKind},
    leaf::HirIdRef,
    project::HirExecutableProjectView,
    source_index::{
        HirEntrySourcePart, HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
    },
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, FlowDeclarationId,
        ProjectEntityReferenceLookupError, ProjectSymbolTable, ProjectSymbolTargetId,
        ResolvedProjectSymbol,
    },
};
use arcweft_source::SourceSpan;

use crate::types::{SemanticTypeDigest, TypeKind};

use super::{CheckedEntryDiagnostic, CheckedFlowId};

/// Exact accepted Flow target resolved once from an Entry member.
///
/// This proof deliberately has no `Clone` implementation. The early ingress
/// worklist borrows it and the late Entry checker consumes it.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedEntryFlowTarget {
    source_item: ItemId,
    declaration: FlowDeclarationId,
    id: CheckedFlowId,
    source: SourceSpan,
}

impl PreparedEntryFlowTarget {
    pub(crate) const fn declaration(&self) -> &FlowDeclarationId {
        &self.declaration
    }

    pub(crate) const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub(crate) fn into_parts(self) -> (ItemId, FlowDeclarationId, CheckedFlowId, SourceSpan) {
        (self.source_item, self.declaration, self.id, self.source)
    }
}

/// One stateful Entry's exact Event type and initial Flow root.
///
/// The semantic type is selected by its already-resolved `TypeId`; no owned
/// `TypeKind`, source spelling, or fallback type is retained here.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedEntryRootSeed {
    entry: ItemId,
    event_type: TypeId,
    event_digest: SemanticTypeDigest,
    target: PreparedEntryFlowTarget,
}

impl PreparedEntryRootSeed {
    pub(crate) const fn entry(&self) -> ItemId {
        self.entry
    }

    pub(crate) const fn event_type(&self) -> TypeId {
        self.event_type
    }

    pub(crate) const fn event_digest(&self) -> SemanticTypeDigest {
        self.event_digest
    }

    pub(crate) const fn target(&self) -> &PreparedEntryFlowTarget {
        &self.target
    }

    pub(crate) fn into_target(self) -> PreparedEntryFlowTarget {
        self.target
    }
}

/// Deterministically keyed move-only root inventory.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct PreparedEntryRootCatalog {
    roots: BTreeMap<ItemId, PreparedEntryRootSeed>,
}

impl PreparedEntryRootCatalog {
    pub(crate) fn roots(&self) -> impl ExactSizeIterator<Item = &PreparedEntryRootSeed> {
        self.roots.values()
    }

    pub(crate) fn get(&self, entry: ItemId) -> Option<&PreparedEntryRootSeed> {
        self.roots.get(&entry)
    }

    pub(crate) fn take(&mut self, entry: ItemId) -> Option<PreparedEntryRootSeed> {
        self.roots.remove(&entry)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// Resolves every stateful Entry root before any contextual Event pattern is
/// seeded. A failure publishes neither a partial catalog nor a fallback root.
pub(crate) fn prepare_entry_root_seeds(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    types: &BTreeMap<TypeId, TypeKind>,
) -> Result<PreparedEntryRootCatalog, Vec<CheckedEntryDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut roots = BTreeMap::new();

    for item in project.items() {
        let HirItemKind::Entry(entry) = item.item().kind() else {
            continue;
        };
        if !matches!(
            entry.kind(),
            HirEntryKind::Game | HirEntryKind::Editor | HirEntryKind::Test
        ) {
            continue;
        }

        let events = entry
            .members()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, member)| match member {
                HirEntryMember::EventType(binding) => Some((ordinal, binding)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let event = match events.as_slice() {
            [(ordinal, binding)] => Some((*ordinal, *binding)),
            [] => {
                diagnostics.push(CheckedEntryDiagnostic::new(
                    "sema.entry.missing_role",
                    "entry is missing required `event` role",
                    entry_source(item.module(), item.id(), HirEntrySourcePart::Whole),
                ));
                None
            }
            [(first, _), (duplicate, _), ..] => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.duplicate_role",
                        "entry declares `event` more than once",
                        entry_member_source(item.module(), item.id(), *duplicate),
                    )
                    .with_related([entry_member_source(
                        item.module(),
                        item.id(),
                        *first,
                    )]),
                );
                None
            }
        };

        let gotos = entry
            .members()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, member)| match member {
                HirEntryMember::Goto(target) => Some((ordinal, target)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let goto = match gotos.as_slice() {
            [(ordinal, target)] => Some((*ordinal, *target)),
            _ => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.goto_cardinality",
                        "stateful entry must contain exactly one initial `goto` target",
                        entry_source(item.module(), item.id(), HirEntrySourcePart::Whole),
                    )
                    .with_related(gotos.iter().map(|(ordinal, _)| {
                        entry_member_source(item.module(), item.id(), *ordinal)
                    })),
                );
                None
            }
        };

        let Some((_, event)) = event else {
            continue;
        };
        let Some((goto_ordinal, goto)) = goto else {
            continue;
        };
        let Some(event_type) = types.get(&event.ty()) else {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.missing_nominal_resolution",
                "event role type is absent from the prepared semantic type map",
                entry_member_source(item.module(), item.id(), events[0].0),
            ));
            continue;
        };
        if event_type.contains_nominal_poison()
            || !matches!(event_type, TypeKind::ProjectNominal(_))
        {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.role_not_direct_nominal",
                "event role must name one resolved project enum",
                entry_member_source(item.module(), item.id(), events[0].0),
            ));
            continue;
        }
        let source = entry_member_source(item.module(), item.id(), goto_ordinal);
        let Some(target) = resolve_entry_flow_target(
            project,
            symbols,
            item.module(),
            goto.target(),
            source,
            &mut diagnostics,
        ) else {
            continue;
        };
        let seed = PreparedEntryRootSeed {
            entry: item.id(),
            event_type: event.ty(),
            event_digest: event_type.semantic_identity_digest(),
            target,
        };
        if roots.insert(item.id(), seed).is_some() {
            diagnostics.push(CheckedEntryDiagnostic::new(
                "sema.entry.duplicate_root",
                "stateful Entry root occurs more than once in the accepted project",
                entry_source(item.module(), item.id(), HirEntrySourcePart::Whole),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(PreparedEntryRootCatalog { roots })
    } else {
        Err(diagnostics)
    }
}

fn resolve_entry_flow_target(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    module: &arcweft_lang_hir::module::HirModule,
    target: &HirEntryTarget,
    source: SourceSpan,
    diagnostics: &mut Vec<CheckedEntryDiagnostic>,
) -> Option<PreparedEntryFlowTarget> {
    let HirEntryTarget::Authored(value) = target else {
        diagnostics.push(invalid_flow_id(source));
        return None;
    };
    let Some(reference) = value.as_resolved() else {
        diagnostics.push(invalid_flow_id(source));
        return None;
    };
    let Some(public_id) = absolute_public_id(reference) else {
        diagnostics.push(invalid_flow_id(source));
        return None;
    };
    if !public_id.as_str().starts_with("flow.") {
        diagnostics.push(CheckedEntryDiagnostic::new(
            "sema.entry.invalid_flow_family",
            format!("Entry target `{public_id}` must use the `flow.*` family"),
            source,
        ));
        return None;
    }
    let symbol =
        match symbols.resolve_entity_reference(module.key().path(), reference, source.clone()) {
            Ok(ResolvedProjectSymbol::StructuralCallable(symbol))
                if symbol.owner() == CallableDeclarationOwner::Flow =>
            {
                symbol
            }
            Ok(other) => {
                diagnostics.push(
                    CheckedEntryDiagnostic::new(
                        "sema.entry.invalid_flow_family",
                        format!("Entry target `{public_id}` does not denote a Flow"),
                        source,
                    )
                    .with_related(resolved_symbol_source(&other)),
                );
                return None;
            }
            Err(error) => {
                let code = match &error {
                    ProjectEntityReferenceLookupError::Ambiguous { .. } => {
                        "sema.entry.ambiguous_flow"
                    }
                    _ => "sema.entry.unknown_flow",
                };
                diagnostics.push(
                    CheckedEntryDiagnostic::new(code, error.to_string(), source)
                        .with_related(entity_lookup_sources(symbols, &error)),
                );
                return None;
            }
        };
    let CallableDeclarationKey::Flow(declaration) = symbol.declaration() else {
        unreachable!("the structural Flow target owns a Flow declaration key")
    };
    let flow_module = project
        .modules()
        .map(|(_, module)| module)
        .find(|candidate| candidate.snapshot_id() == symbol.source_snapshot());
    let Some(flow_module) = flow_module else {
        diagnostics.push(CheckedEntryDiagnostic::new(
            "sema.entry.unknown_flow",
            format!("Entry target Flow `{public_id}` is absent from its accepted HIR snapshot"),
            source,
        ));
        return None;
    };
    let source_item = symbol.source_item();
    if !matches!(
        flow_module
            .resolve_item(source_item)
            .map(|item| item.kind()),
        Ok(HirItemKind::Flow(_))
    ) {
        diagnostics.push(CheckedEntryDiagnostic::new(
            "sema.entry.unknown_flow",
            format!("Entry target Flow `{public_id}` is absent from its accepted HIR snapshot"),
            source,
        ));
        return None;
    }
    Some(PreparedEntryFlowTarget {
        source_item,
        declaration: declaration.clone(),
        id: CheckedFlowId::from_declaration(declaration),
        source,
    })
}

fn invalid_flow_id(source: SourceSpan) -> CheckedEntryDiagnostic {
    CheckedEntryDiagnostic::new(
        "sema.entry.invalid_flow_id",
        "Entry target must be one complete absolute Flow ID",
        source,
    )
}

fn absolute_public_id(reference: &HirIdRef) -> Option<PublicId> {
    let HirIdRef::Absolute(reference) = reference else {
        return None;
    };
    PublicId::try_new(reference.as_str().to_owned()).ok()
}

fn project_target_source(
    symbols: &ProjectSymbolTable,
    target: &ProjectSymbolTargetId,
) -> Option<SourceSpan> {
    match target {
        ProjectSymbolTargetId::Callable(id) | ProjectSymbolTargetId::StructuralCallable(id) => {
            symbols
                .callable(id)
                .map(|symbol| symbol.declaration_span().clone())
        }
        ProjectSymbolTargetId::External(id) => symbols
            .external(*id)
            .map(|symbol| symbol.declaration_span().clone()),
        ProjectSymbolTargetId::Nominal(id) => symbols
            .nominal(id)
            .map(|declaration| declaration.source().whole().clone()),
        ProjectSymbolTargetId::Retained(id) => symbols
            .retained(id)
            .map(|symbol| symbol.declaration_span().clone()),
        ProjectSymbolTargetId::Module(_) => None,
    }
}

fn resolved_symbol_source(symbol: &ResolvedProjectSymbol<'_>) -> Option<SourceSpan> {
    match symbol {
        ResolvedProjectSymbol::Callable(symbol)
        | ResolvedProjectSymbol::StructuralCallable(symbol) => {
            Some(symbol.declaration_span().clone())
        }
        ResolvedProjectSymbol::External(symbol) => Some(symbol.declaration_span().clone()),
        ResolvedProjectSymbol::Nominal(symbol) => Some(symbol.source().whole().clone()),
        ResolvedProjectSymbol::Retained(symbol) => Some(symbol.declaration_span().clone()),
        ResolvedProjectSymbol::Module(_) => None,
    }
}

fn entity_lookup_sources(
    symbols: &ProjectSymbolTable,
    error: &ProjectEntityReferenceLookupError,
) -> Vec<SourceSpan> {
    match error {
        ProjectEntityReferenceLookupError::Ambiguous { candidates, .. }
        | ProjectEntityReferenceLookupError::Inaccessible { candidates, .. } => candidates
            .iter()
            .filter_map(|candidate| project_target_source(symbols, candidate))
            .collect(),
        ProjectEntityReferenceLookupError::Poisoned { declaration, .. } => {
            vec![declaration.clone()]
        }
        ProjectEntityReferenceLookupError::Unknown { .. }
        | ProjectEntityReferenceLookupError::RelativeRequiresFamily { .. }
        | ProjectEntityReferenceLookupError::UnsupportedParentDepth { .. }
        | ProjectEntityReferenceLookupError::InvalidIdentity { .. }
        | ProjectEntityReferenceLookupError::InvalidReferencePath { .. }
        | ProjectEntityReferenceLookupError::InvalidModulePath { .. }
        | ProjectEntityReferenceLookupError::CatalogOwned { .. } => Vec::new(),
    }
}

fn entry_source(
    module: &arcweft_lang_hir::module::HirModule,
    owner: ItemId,
    part: HirEntrySourcePart,
) -> SourceSpan {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item {
                owner,
                role: HirItemSourceRole::Entry(part),
            },
        )
        .expect("accepted final-HIR Entry owns its validated source role");
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => span.clone(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            unreachable!("executable Entry roles retain authored source")
        }
    }
}

fn entry_member_source(
    module: &arcweft_lang_hir::module::HirModule,
    owner: ItemId,
    ordinal: usize,
) -> SourceSpan {
    let member = u32::try_from(ordinal).expect("accepted Entry member ordinal fits u32");
    entry_source(module, owner, HirEntrySourcePart::MemberValue { member })
}
