//! Construction of the HIR-owned dialogue source-site inventory.
//!
//! This adapter intentionally stops at source/topology evidence. It must not
//! assign line IDs, derive text keys, or perform project collision handling:
//! those operations require the selected semantic expression graph and are
//! performed by the later HIR acceptance seal.

use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::SourceSpan;

use crate::dialogue_application::{HirAttachedContentApplicationFamily, HirDialogueCoordinateKind};
use crate::expr::{HirExprKind, HirNamedBlockName};
use crate::identity::{ExprId, ItemId};
use crate::item::{HirCapabilityMember, HirItemKind, HirRetainedName};
use crate::module::HirModule;
use crate::scope::HirScopeOwner;
use crate::slot::HirOrigin;
use crate::source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite};
use crate::symbol::{CallableDeclarationId, CallableDeclarationOwner};

use super::{
    DialogueLineBuildFatal, DialogueLineSourceOrder, HirDialogueFlowOwner, HirDialogueLineSite,
    HirDialogueLineSiteInventory, HirDialogueLineSiteTopology, HirDialogueLineSourceOwner,
    HirDialogueNamedScope,
};

/// Builds source-site evidence from one exact prepared HIR module.
pub(crate) fn build_site_inventory(
    module: &HirModule,
) -> Result<HirDialogueLineSiteInventory, DialogueLineBuildFatal> {
    let mut sites = Vec::new();
    let expressions = module
        .arenas()
        .expressions()
        .try_iter_prepared(module.slots())
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    for (owner, expression) in expressions {
        let metadata = module
            .slots()
            .resolve_prepared(owner)
            .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
        if !matches!(metadata.origin(), HirOrigin::Source(_)) {
            continue;
        }
        let Some((semantic_application, topology)) =
            source_site_relation(module, expression.kind(), owner)?
        else {
            continue;
        };
        let application_span = whole_expression_span(module, owner)?;
        let (source_owner, named_scopes) = source_owner_and_scopes(module, owner)?;
        let id_coordinate_span =
            coordinate_span(module, semantic_application, HirDialogueCoordinateKind::Id)?;
        let text_key_coordinate_span = coordinate_span(
            module,
            semantic_application,
            HirDialogueCoordinateKind::TextKey,
        )?;
        sites.push(HirDialogueLineSite::try_new(
            module.key().source(),
            owner,
            semantic_application,
            topology,
            source_owner,
            named_scopes,
            DialogueLineSourceOrder::try_new(1)?,
            application_span,
            id_coordinate_span,
            text_key_coordinate_span,
        )?);
    }
    HirDialogueLineSiteInventory::new(module.key().clone(), sites)
}

fn source_site_relation(
    module: &HirModule,
    kind: &HirExprKind,
    owner: ExprId,
) -> Result<Option<(ExprId, HirDialogueLineSiteTopology)>, DialogueLineBuildFatal> {
    match kind {
        HirExprKind::AttachedContentApplication(application) if application.is_dialogue_line() => {
            Ok(Some((owner, HirDialogueLineSiteTopology::Direct)))
        }
        HirExprKind::PostfixBracket(postfix) => {
            let crate::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                index,
                dialogue,
            } = postfix.candidates()
            else {
                return Ok(None);
            };
            let dialogue_expression = module
                .arenas()
                .expressions()
                .resolve_prepared(module.slots(), *dialogue)
                .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
            Ok(matches!(
                dialogue_expression.kind(),
                HirExprKind::AttachedContentApplication(application)
                    if application.is_dialogue_line()
            )
            .then_some((
                *dialogue,
                HirDialogueLineSiteTopology::OuterPostfixBracket {
                    index_candidate: *index,
                },
            )))
        }
        _ => Ok(None),
    }
}

fn coordinate_span(
    module: &HirModule,
    semantic_application: ExprId,
    kind: HirDialogueCoordinateKind,
) -> Result<Option<SourceSpan>, DialogueLineBuildFatal> {
    let expression = module
        .arenas()
        .expressions()
        .resolve_prepared(module.slots(), semantic_application)
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
        return Ok(None);
    };
    let HirAttachedContentApplicationFamily::DialogueLine { coordinates, .. } =
        application.family()
    else {
        return Ok(None);
    };
    let Some(coordinate) = coordinates.iter().find(|value| value.kind() == kind) else {
        return Ok(None);
    };
    whole_expression_span(module, coordinate.value()).map(Some)
}

fn whole_expression_span(
    module: &HirModule,
    owner: ExprId,
) -> Result<SourceSpan, DialogueLineBuildFatal> {
    let metadata = module
        .slots()
        .resolve_prepared(owner)
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    match metadata.source_site() {
        HirSourceSite::Span(span) => Ok(span.clone()),
        HirSourceSite::Insertion(_) => Err(DialogueLineBuildFatal::InvalidSourceComponent),
    }
}

fn source_owner_and_scopes(
    module: &HirModule,
    application: ExprId,
) -> Result<(HirDialogueLineSourceOwner, Arc<[HirDialogueNamedScope]>), DialogueLineBuildFatal> {
    let expression = module
        .arenas()
        .expressions()
        .resolve_prepared(module.slots(), application)
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    let mut scope = Some(expression.scope());
    let mut item = None;
    let mut scope_chain = Vec::new();
    let mut named_scopes = Vec::new();
    while let Some(scope_id) = scope {
        let payload = module
            .arenas()
            .scopes()
            .resolve_prepared(module.slots(), scope_id)
            .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
        scope_chain.push(scope_id);
        match *payload.owner() {
            HirScopeOwner::Item(owner) => {
                if item.is_none() {
                    item = Some(owner);
                }
            }
            HirScopeOwner::Expr(owner) => {
                let expression = module
                    .arenas()
                    .expressions()
                    .resolve_prepared(module.slots(), owner)
                    .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
                if let HirExprKind::NamedBlock(block) = expression.kind()
                    && block.scope() == scope_id
                    && let HirNamedBlockName::Resolved(name) = block.name()
                {
                    let segment = ModuleSegment::new(name.as_str())
                        .map_err(|_| DialogueLineBuildFatal::InvalidInternalPrefix)?;
                    let declaration =
                        expression_component_span(module, owner, HirExprSourceRole::Name)?;
                    named_scopes.push(HirDialogueNamedScope::new(scope_id, segment, declaration));
                }
            }
            HirScopeOwner::Module(_) | HirScopeOwner::Stmt(_) => {}
        }
        scope = payload.parent();
    }
    named_scopes.reverse();
    let owner = item
        .map(|item| source_owner(module, item, &scope_chain))
        .transpose()?
        .flatten()
        .unwrap_or(HirDialogueLineSourceOwner::Ownerless);
    Ok((owner, Arc::from(named_scopes)))
}

fn expression_component_span(
    module: &HirModule,
    owner: ExprId,
    role: HirExprSourceRole,
) -> Result<SourceSpan, DialogueLineBuildFatal> {
    match module
        .source_components()
        .component_presence(&HirSourceQuery::Expr { owner, role })
        .ok_or(DialogueLineBuildFatal::InvalidSourceComponent)?
    {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => Err(DialogueLineBuildFatal::InvalidSourceComponent),
    }
}

fn source_owner(
    module: &HirModule,
    item: ItemId,
    scope_chain: &[crate::identity::ScopeId],
) -> Result<Option<HirDialogueLineSourceOwner>, DialogueLineBuildFatal> {
    let item = module
        .arenas()
        .items()
        .resolve_prepared(module.slots(), item)
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    match item.kind() {
        HirItemKind::Flow(flow) => flow
            .identity()
            .accepted_publication()
            .map(|(id, _)| HirDialogueFlowOwner::try_new(id))
            .transpose()
            .map_err(|_| DialogueLineBuildFatal::InvalidInternalPrefix)
            .map(|owner| owner.map(HirDialogueLineSourceOwner::Flow)),
        HirItemKind::Function(function) => ordinary_callable(
            module,
            CallableDeclarationOwner::Function,
            function.name().resolved().map(crate::leaf::HirName::as_str),
            std::iter::empty(),
        ),
        HirItemKind::Predicate(predicate) => ordinary_callable(
            module,
            CallableDeclarationOwner::Predicate,
            predicate
                .name()
                .resolved()
                .map(crate::leaf::HirName::as_str),
            std::iter::empty(),
        ),
        HirItemKind::Proof(proof) => ordinary_callable(
            module,
            CallableDeclarationOwner::Proof,
            proof.name().resolved().map(crate::leaf::HirName::as_str),
            std::iter::empty(),
        ),
        HirItemKind::View(view) => {
            let name = match view.header().name() {
                HirRetainedName::Resolved(name) => Some(name.as_str()),
                HirRetainedName::Missing | HirRetainedName::Invalid => None,
            };
            ordinary_callable(
                module,
                CallableDeclarationOwner::View,
                name,
                std::iter::empty(),
            )
        }
        HirItemKind::ExternCapability(capability) => {
            let Some(capability_name) = capability.name().resolved() else {
                return Ok(None);
            };
            for member in capability.members() {
                let HirCapabilityMember::Function(function) = member else {
                    continue;
                };
                if !scope_chain.contains(&function.callable_scope()) {
                    continue;
                }
                let Some(name) = function.name().resolved() else {
                    return Ok(None);
                };
                return ordinary_callable(
                    module,
                    CallableDeclarationOwner::ExternCapability,
                    Some(name.as_str()),
                    [ModuleSegment::new(capability_name.as_str())
                        .map_err(|_| DialogueLineBuildFatal::InvalidInternalPrefix)?],
                );
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn ordinary_callable(
    module: &HirModule,
    owner: CallableDeclarationOwner,
    name: Option<&str>,
    owner_path: impl IntoIterator<Item = ModuleSegment>,
) -> Result<Option<HirDialogueLineSourceOwner>, DialogueLineBuildFatal> {
    let Some(name) = name else {
        return Ok(None);
    };
    CallableDeclarationId::try_new_in_owner_path(
        module.key().package().clone(),
        module.key().path().clone(),
        owner,
        owner_path,
        name,
    )
    .map(HirDialogueLineSourceOwner::Callable)
    .map(Some)
    .map_err(|_| DialogueLineBuildFatal::InvalidInternalPrefix)
}
