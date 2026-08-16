//! Projection from one exact accepted final-HIR/semantic generation.
//!
//! This is the sole production producer for source-project index data. It
//! consumes already checked identities and facts; it never reparses source,
//! reruns name/type resolution, or falls back to the retired detached HIR.

use std::sync::Arc;

use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::ItemId,
    item::HirItemKind,
    leaf::{HirIdRef, HirIdRefValue},
    module::HirModule,
    project::{HirExecutableProjectView, HirPackageModuleKey, HirProjectItemRef},
    source_index::{
        HirEntrySourcePart, HirExprSourceRole, HirFlowSourceRole, HirItemSourceRole,
        HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole, HirStyleSourceRole,
    },
    stmt::{HirSelectStmt, HirStmtKind},
    symbol::{CallableDeclarationKey, ProjectSymbolTable, ResolvedProjectSymbol},
};
use arcweft_source::{SourceAnchor, SourceSpan};

use crate::{
    callable::{CallTargetFact, CheckedCallableDeclaration},
    entry::CheckedEntryCatalog,
    final_analysis::{CheckedExpressionResolution, CheckedValueResolution, FinalSemanticAnalysis},
    types::{
        EntityKind, EntityType, GenericTypeOwnerId, GenericTypeParameterId, ProjectNominalType,
        TypeKind,
    },
};

use super::{
    AcceptedDialogueLineReference, EntitySymbol, ProgramHash, ProjectEntityId,
    ProjectFlowControlSummary, ProjectGraphDependencyRelation, ProjectGraphDependencyRelationKind,
    ProjectGraphRelation, ProjectGraphRelationKind, ProjectGraphSymbolRef, ProjectSemanticIndex,
    ProjectSemanticIndexError, SemanticHash, TypeName, entry_roles, nominal,
};

impl ProjectSemanticIndex {
    /// Publishes one project index from the exact accepted HIR, symbol, entry,
    /// and semantic generations.
    ///
    /// No partially projected index escapes on error. Consumers therefore
    /// cannot observe a mixture of accepted and reconstructed authority.
    pub fn try_from_final_project(
        program_hash: ProgramHash,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        analysis: &FinalSemanticAnalysis,
        entries: &CheckedEntryCatalog,
    ) -> Result<Self, ProjectSemanticIndexError> {
        analysis.validate_generation(project, symbols)?;

        let mut index = Self::try_new(program_hash, Arc::clone(analysis.checked_callables()))?;
        (index.entry_records, index.entry_role_edges) =
            entry_roles::checked_entry_records_and_edges(entries);
        (index.project_nominals, index.project_nominal_references) =
            nominal::checked_project_nominals(symbols, analysis)?;
        index.dialogue_line_references =
            checked_dialogue_line_references(project, analysis)?.into_boxed_slice();

        project_nominal_types(&mut index, symbols)?;
        retained_entities(&mut index, project, symbols, analysis)?;
        entry_entities(&mut index, project, analysis, entries)?;
        flow_and_style_entities(&mut index, project, symbols, analysis)?;
        callable_dependencies(&mut index, symbols, analysis)?;
        validate_relation_endpoints(&index)?;
        Ok(index)
    }
}

fn checked_dialogue_line_references(
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
) -> Result<Vec<AcceptedDialogueLineReference>, ProjectSemanticIndexError> {
    let modules = project
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut references = Vec::new();
    for (owner, checked) in analysis.expressions() {
        let CheckedExpressionResolution::DialogueLineReference(target) = checked.resolution()
        else {
            continue;
        };
        if project.dialogue_lines().get(target).is_none() {
            return Err(ProjectSemanticIndexError::MissingAcceptedDialogueLine {
                target: target.clone(),
            });
        }
        let module = modules
            .get(&owner.module())
            .copied()
            .ok_or(ProjectSemanticIndexError::MissingDialogueLineReferenceModule { owner })?;
        let lookup = module.source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Whole,
            },
        )?;
        let HirSourcePresence::Present(HirSourceSite::Span(source)) = lookup.presence() else {
            return Err(ProjectSemanticIndexError::MissingDialogueLineReferenceSource { owner });
        };
        references.push(AcceptedDialogueLineReference::new(
            target.clone(),
            source.clone(),
            HirPackageModuleKey::from(module.key()),
            owner,
        ));
    }
    Ok(references)
}

fn callable_dependencies(
    index: &mut ProjectSemanticIndex,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), ProjectSemanticIndexError> {
    for (_, facts) in analysis.calls() {
        let CallTargetFact::Selected { selected, .. } = facts.target() else {
            continue;
        };
        let Some(target) = selected.checked() else {
            continue;
        };
        index.checked_callables.callable(target)?;

        let Some(parent) = facts.enclosing_callable() else {
            continue;
        };
        let Some(from) = callable_parent_ref(index, symbols, parent)? else {
            continue;
        };
        let Some(to) = checked_target_ref(index, target)? else {
            continue;
        };
        let relation = ProjectGraphDependencyRelation::new(
            from,
            to,
            ProjectGraphDependencyRelationKind::CallsCallable,
        );
        if !index.dependency_relations.contains(&relation) {
            index.dependency_relations.push(relation);
        }
    }
    Ok(())
}

fn callable_parent_ref(
    index: &ProjectSemanticIndex,
    symbols: &ProjectSymbolTable,
    parent: &arcweft_lang_hir::symbol::CallableDeclarationKey,
) -> Result<Option<ProjectGraphSymbolRef>, ProjectSemanticIndexError> {
    if let arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(flow) = parent {
        symbols.callable(parent).ok_or_else(|| {
            ProjectSemanticIndexError::MissingProjectCallableParent {
                declaration: Box::new(parent.clone()),
            }
        })?;
        return Ok(Some(ProjectGraphSymbolRef::entity(
            ProjectEntityId::structural_flow(flow.clone()),
        )));
    }

    if super::project_callable_kind(parent.owner()).is_none() {
        return Ok(None);
    }
    let parent = index.project_callables.get(parent).ok_or_else(|| {
        ProjectSemanticIndexError::MissingProjectCallableParent {
            declaration: Box::new(parent.clone()),
        }
    })?;
    Ok(Some(ProjectGraphSymbolRef::callable(
        parent.checked().clone(),
    )))
}

fn checked_target_ref(
    index: &ProjectSemanticIndex,
    target: &crate::callable::CheckedCallableId,
) -> Result<Option<ProjectGraphSymbolRef>, ProjectSemanticIndexError> {
    match target.declaration() {
        CheckedCallableDeclaration::Project(declaration) => {
            if super::project_callable_kind(declaration.owner()).is_none() {
                return Ok(None);
            }
            let projected = index
                .project_callables
                .get(declaration)
                .ok_or(ProjectSemanticIndexError::InvalidProjectCallableIdentity)?;
            if projected.checked() != target {
                return Err(ProjectSemanticIndexError::InvalidProjectCallableIdentity);
            }
            Ok(Some(ProjectGraphSymbolRef::callable(target.clone())))
        }
        CheckedCallableDeclaration::Environment(declaration) => {
            let projected = index
                .environment_lowerings
                .get(declaration)
                .ok_or(ProjectSemanticIndexError::InvalidEnvironmentCallableIdentity)?;
            if projected.checked() != target {
                return Err(ProjectSemanticIndexError::InvalidEnvironmentCallableIdentity);
            }
            Ok(Some(ProjectGraphSymbolRef::callable(target.clone())))
        }
        CheckedCallableDeclaration::Detached(_) | CheckedCallableDeclaration::Standard(_) => {
            Ok(None)
        }
    }
}

fn flow_and_style_entities(
    index: &mut ProjectSemanticIndex,
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), ProjectSemanticIndexError> {
    for item in project.items() {
        match item.item().kind() {
            HirItemKind::Flow(_) => index_flow(index, item, symbols, analysis)?,
            HirItemKind::Style(style) => {
                let id = resolved_public_id(
                    style.id(),
                    DeclarationIdentityFamily::Style,
                    "style declaration",
                )?;
                index_nonretained_entity(
                    index,
                    item,
                    ProjectEntityId::public(id),
                    EntityKind::Style,
                    HirItemSourceRole::Style(HirStyleSourceRole::ItemId),
                    analysis,
                )?;
            }
            // `Source` is intentionally not reintroduced here. Lang-01.3
            // removes that old producer/reader family, and final HIR has no
            // accepted Source entity source role to project from.
            _ => {}
        }
    }
    Ok(())
}

fn index_flow(
    index: &mut ProjectSemanticIndex,
    item: HirProjectItemRef<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), ProjectSemanticIndexError> {
    let symbol = symbols
        .flow_symbol_for_item(item.id())
        .ok_or(ProjectSemanticIndexError::MissingFlowSymbol { owner: item.id() })?;
    let arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(flow) = symbol.declaration() else {
        unreachable!("Flow item lookup only publishes structural Flow declarations")
    };
    let identity = ProjectEntityId::structural_flow(flow.clone());
    index_nonretained_entity(
        index,
        item,
        identity.clone(),
        EntityKind::Flow,
        HirItemSourceRole::Flow(HirFlowSourceRole::Whole),
        analysis,
    )?;

    let HirItemKind::Flow(flow) = item.item().kind() else {
        return Err(ProjectSemanticIndexError::WrongEntityOwner { owner: item.id() });
    };
    let summary = summarize_flow(
        index,
        item.module(),
        flow.body().scope(),
        &identity,
        symbols,
        analysis,
    )?;
    index.flow_control_summaries.insert(identity, summary);
    Ok(())
}

fn index_nonretained_entity(
    index: &mut ProjectSemanticIndex,
    item: HirProjectItemRef<'_>,
    identity: ProjectEntityId,
    kind: EntityKind,
    source_role: HirItemSourceRole,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), ProjectSemanticIndexError> {
    let owner = item.id();
    let checked = analysis
        .item(owner)
        .ok_or(ProjectSemanticIndexError::MissingCheckedItem { owner })?;
    if checked.role().family() != item.item().family() {
        return Err(ProjectSemanticIndexError::WrongEntityOwner { owner });
    }
    let public_id = identity.public_id().clone();
    let source = authored_item_span(item, source_role, &public_id)?;
    let semantic_hash = entity_semantic_hash(&public_id, &kind, None, checked.effects(), None);
    let ty = EntityType::new(kind, None);
    let source = SourceAnchor::from_span(source);
    insert_entity(
        index,
        EntitySymbol::new(identity, ty, source, semantic_hash),
    )
}

fn resolved_public_id(
    value: &HirIdRefValue,
    family: DeclarationIdentityFamily,
    label: &'static str,
) -> Result<PublicId, ProjectSemanticIndexError> {
    let reference =
        value
            .as_resolved()
            .ok_or_else(|| ProjectSemanticIndexError::InvalidEntityIdentity {
                id: "<recovered>".to_owned(),
                family: family.prefix(),
                message: format!("{label} has a recovered identity"),
            })?;
    absolute_public_id(reference, family, label)
}

fn absolute_public_id(
    reference: &HirIdRef,
    family: DeclarationIdentityFamily,
    label: &'static str,
) -> Result<PublicId, ProjectSemanticIndexError> {
    reference.declaration_public_id(family).ok_or_else(|| {
        ProjectSemanticIndexError::InvalidEntityIdentity {
            id: "<relative>".to_owned(),
            family: family.prefix(),
            message: format!(
                "{label} requires an absolute or same-family root declaration identity"
            ),
        }
    })
}

fn summarize_flow(
    index: &mut ProjectSemanticIndex,
    module: &HirModule,
    root_scope: arcweft_lang_hir::identity::ScopeId,
    flow_id: &ProjectEntityId,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<ProjectFlowControlSummary, ProjectSemanticIndexError> {
    let mut summary = ProjectFlowControlSummary::default();
    let mut statement_owned_control = std::collections::BTreeSet::new();

    for (statement_id, statement) in module.statements() {
        if !scope_is_within(module, statement.scope(), root_scope) {
            continue;
        }
        match statement.kind() {
            HirStmtKind::Goto { target } => {
                if let Some(target) = checked_flow_target(analysis, *target)? {
                    summary.record_static_goto();
                    push_relation(
                        index,
                        ProjectGraphRelation::new(
                            flow_id.clone(),
                            target,
                            ProjectGraphRelationKind::FlowGoto,
                        ),
                    );
                } else {
                    summary.record_dynamic_goto();
                }
            }
            HirStmtKind::Include(include) => {
                let target =
                    resolved_include_flow_target(module, statement_id, include.target(), symbols)?;
                push_relation(
                    index,
                    ProjectGraphRelation::new(
                        flow_id.clone(),
                        target,
                        ProjectGraphRelationKind::FlowInclude,
                    ),
                );
            }
            HirStmtKind::LetChoice { .. }
            | HirStmtKind::Choice { .. }
            | HirStmtKind::If(_)
            | HirStmtKind::IfLet(_)
            | HirStmtKind::Match(_)
            | HirStmtKind::LetElse { .. } => summary.record_branch(),
            HirStmtKind::Loop(_)
            | HirStmtKind::While(_)
            | HirStmtKind::WhileLet(_)
            | HirStmtKind::For(_)
            | HirStmtKind::LetLoop { .. } => summary.record_loop(),
            HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
                summary.record_branch();
                summary.add_select_branches(branches.len());
            }
            HirStmtKind::Wait { .. } => summary.record_await(),
            HirStmtKind::LetActionReceive { action, .. } => {
                summary.record_await();
                statement_owned_control.insert(*action);
            }
            _ => {}
        }
    }

    for (owner, expression) in module.expressions() {
        if statement_owned_control.contains(&owner)
            || !scope_is_within(module, expression.scope(), root_scope)
        {
            continue;
        }
        match expression.kind() {
            HirExprKind::Await(_) => summary.record_await(),
            HirExprKind::Thread(_) => summary.record_thread(),
            HirExprKind::If(_) | HirExprKind::IfLet(_) | HirExprKind::Match(_) => {
                summary.record_branch();
            }
            _ => {}
        }
    }
    Ok(summary)
}

fn checked_flow_target(
    analysis: &FinalSemanticAnalysis,
    owner: arcweft_lang_hir::identity::ExprId,
) -> Result<Option<ProjectEntityId>, ProjectSemanticIndexError> {
    let checked = analysis
        .expression(owner)
        .ok_or(ProjectSemanticIndexError::MissingFlowExpression { owner })?;
    let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
        checked.resolution()
    else {
        return Ok(None);
    };
    let Some((declaration, _)) = item.flow_owner() else {
        return Ok(None);
    };
    let CallableDeclarationKey::Flow(declaration) = declaration else {
        return Ok(None);
    };
    Ok(Some(ProjectEntityId::structural_flow(declaration.clone())))
}

fn resolved_include_flow_target(
    module: &HirModule,
    owner: arcweft_lang_hir::identity::StmtId,
    target: &HirIdRefValue,
    symbols: &ProjectSymbolTable,
) -> Result<ProjectEntityId, ProjectSemanticIndexError> {
    let reference =
        target
            .as_resolved()
            .ok_or_else(|| ProjectSemanticIndexError::InvalidEntityIdentity {
                id: "<recovered>".to_owned(),
                family: DeclarationIdentityFamily::Flow.prefix(),
                message: "include target has a recovered identity".to_owned(),
            })?;
    let source = module.source_site(
        module.provenance().source_identity(),
        HirSourceQuery::Stmt {
            owner,
            role: HirStmtSourceRole::Whole,
        },
    )?;
    let HirSourcePresence::Present(HirSourceSite::Span(source)) = source.presence() else {
        return Err(ProjectSemanticIndexError::MissingFlowStatementSource { owner });
    };
    let resolved =
        symbols.resolve_entity_reference(module.key().path(), reference, source.clone())?;
    let ResolvedProjectSymbol::StructuralCallable(symbol) = resolved else {
        return Err(ProjectSemanticIndexError::InvalidEntityIdentity {
            id: format!("{reference:?}"),
            family: DeclarationIdentityFamily::Flow.prefix(),
            message: "include target does not resolve to a structural Flow".to_owned(),
        });
    };
    let CallableDeclarationKey::Flow(declaration) = symbol.declaration() else {
        return Err(ProjectSemanticIndexError::InvalidEntityIdentity {
            id: format!("{reference:?}"),
            family: DeclarationIdentityFamily::Flow.prefix(),
            message: "include target does not resolve to a structural Flow".to_owned(),
        });
    };
    Ok(ProjectEntityId::structural_flow(declaration.clone()))
}

fn scope_is_within(
    module: &HirModule,
    mut candidate: arcweft_lang_hir::identity::ScopeId,
    root: arcweft_lang_hir::identity::ScopeId,
) -> bool {
    loop {
        if candidate == root {
            return true;
        }
        let Ok(scope) = module.resolve_scope(candidate) else {
            return false;
        };
        let Some(parent) = scope.parent() else {
            return false;
        };
        candidate = parent;
    }
}

fn push_relation(index: &mut ProjectSemanticIndex, relation: ProjectGraphRelation) {
    if !index.relations.contains(&relation) {
        index.relations.push(relation);
    }
}

fn project_nominal_types(
    index: &mut ProjectSemanticIndex,
    symbols: &ProjectSymbolTable,
) -> Result<(), ProjectSemanticIndexError> {
    for declaration in symbols.nominal_symbols() {
        let name = TypeName::new(declaration.id().qualified_name());
        let arguments = declaration
            .type_parameters()
            .iter()
            .map(|parameter| {
                TypeKind::GenericParam(GenericTypeParameterId::new(
                    GenericTypeOwnerId::Nominal(declaration.id().clone()),
                    parameter.ordinal(),
                ))
            })
            .collect::<Vec<_>>();
        let ty =
            TypeKind::ProjectNominal(ProjectNominalType::new(declaration.id().clone(), arguments));
        if index.types.insert(name.clone(), ty).is_some() {
            return Err(ProjectSemanticIndexError::DuplicateType {
                name: name.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn retained_entities(
    index: &mut ProjectSemanticIndex,
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
) -> Result<(), ProjectSemanticIndexError> {
    for symbol in symbols.retained_symbols() {
        let owner = symbol.owner();
        let item = project_item(project, owner)?;
        let checked = analysis
            .item(owner)
            .ok_or(ProjectSemanticIndexError::MissingCheckedItem { owner })?;
        if checked.role().family() != item.item().family() {
            return Err(ProjectSemanticIndexError::WrongEntityOwner { owner });
        }
        let Some(kind) = EntityKind::from_declaration_identity_family(symbol.family()) else {
            return Err(ProjectSemanticIndexError::WrongEntityOwner { owner });
        };
        symbol
            .family()
            .validate_public_id(symbol.public_id())
            .map_err(|error| ProjectSemanticIndexError::InvalidEntityIdentity {
                id: symbol.public_id().as_str().to_owned(),
                family: symbol.family().prefix(),
                message: error.to_string(),
            })?;

        let value = retained_value_type(item.item().kind(), analysis)?;
        let semantic_hash = entity_semantic_hash(
            symbol.public_id(),
            &kind,
            value.as_ref(),
            checked.effects(),
            None,
        );
        insert_entity(
            index,
            EntitySymbol::new(
                ProjectEntityId::public(symbol.public_id().clone()),
                EntityType::new(kind, value),
                SourceAnchor::from_span(symbol.declaration_span().clone()),
                semantic_hash,
            ),
        )?;
    }
    Ok(())
}

fn retained_value_type(
    kind: &HirItemKind,
    analysis: &FinalSemanticAnalysis,
) -> Result<Option<TypeKind>, ProjectSemanticIndexError> {
    let owner = match kind {
        HirItemKind::Signal(signal) => Some(signal.observable_type()),
        HirItemKind::Metric(metric) => Some(metric.value_type()),
        _ => None,
    };
    owner
        .map(|owner| {
            analysis.ty(owner).cloned().ok_or_else(|| {
                ProjectSemanticIndexError::MissingCheckedType {
                    root: owner,
                    reason: "the accepted entity declaration has no checked value type".to_owned(),
                }
            })
        })
        .transpose()
}

fn entry_entities(
    index: &mut ProjectSemanticIndex,
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    entries: &CheckedEntryCatalog,
) -> Result<(), ProjectSemanticIndexError> {
    for binding in entries.entries() {
        let owner = binding.source_item();
        let item = project_item(project, owner)?;
        let checked = analysis
            .item(owner)
            .ok_or(ProjectSemanticIndexError::MissingCheckedItem { owner })?;
        if !matches!(item.item().kind(), HirItemKind::Entry(_))
            || checked.role().family() != item.item().family()
        {
            return Err(ProjectSemanticIndexError::WrongEntityOwner { owner });
        }
        let id = binding.id().public_id().clone();
        let source = authored_item_span(
            item,
            HirItemSourceRole::Entry(HirEntrySourcePart::Whole),
            &id,
        )?;
        let kind = EntityKind::Entry;
        let semantic_hash = entity_semantic_hash(
            &id,
            &kind,
            None,
            checked.effects(),
            Some(binding.binding_digest().as_bytes()),
        );
        insert_entity(
            index,
            EntitySymbol::new(
                ProjectEntityId::public(id),
                EntityType::new(kind, None),
                SourceAnchor::from_span(source),
                semantic_hash,
            ),
        )?;
    }
    Ok(())
}

fn project_item(
    project: HirExecutableProjectView<'_>,
    owner: ItemId,
) -> Result<HirProjectItemRef<'_>, ProjectSemanticIndexError> {
    project
        .items()
        .find(|item| item.id() == owner)
        .ok_or(ProjectSemanticIndexError::MissingProjectItem { owner })
}

fn authored_item_span(
    item: HirProjectItemRef<'_>,
    role: HirItemSourceRole,
    id: &PublicId,
) -> Result<SourceSpan, ProjectSemanticIndexError> {
    let module = item.module();
    let lookup = module.source_site(
        module.provenance().source_identity(),
        HirSourceQuery::Item {
            owner: item.id(),
            role,
        },
    )?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            Err(ProjectSemanticIndexError::MissingEntitySource { id: id.clone() })
        }
    }
}

fn insert_entity(
    index: &mut ProjectSemanticIndex,
    symbol: EntitySymbol,
) -> Result<(), ProjectSemanticIndexError> {
    let id = symbol.identity().clone();
    if index.entities.insert(id.clone(), symbol).is_some() {
        return Err(ProjectSemanticIndexError::DuplicateEntity { id });
    }
    Ok(())
}

fn validate_relation_endpoints(
    index: &ProjectSemanticIndex,
) -> Result<(), ProjectSemanticIndexError> {
    for relation in &index.relations {
        for id in [relation.from(), relation.to()] {
            if !index.entities.contains_key(id) {
                return Err(ProjectSemanticIndexError::MissingRelationEndpoint { id: id.clone() });
            }
        }
    }
    Ok(())
}

fn entity_semantic_hash(
    id: &PublicId,
    kind: &EntityKind,
    value: Option<&TypeKind>,
    effects: &crate::effects::EffectSet,
    extra: Option<&[u8]>,
) -> SemanticHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.project-entity.v1\0");
    hash_bytes(&mut hasher, id.as_str().as_bytes());
    hash_bytes(&mut hasher, kind.as_str().as_bytes());
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_bytes(&mut hasher, value.semantic_identity_digest().as_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(&(effects.len() as u64).to_le_bytes());
    for effect in effects.iter() {
        hash_bytes(&mut hasher, effect.to_string().as_bytes());
    }
    match extra {
        Some(extra) => {
            hasher.update(&[1]);
            hash_bytes(&mut hasher, extra);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    SemanticHash::new(hasher.finalize().to_hex().to_string())
}

fn hash_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
