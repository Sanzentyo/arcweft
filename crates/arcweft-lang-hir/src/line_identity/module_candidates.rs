use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::SourceSpan;

use crate::dialogue_application::{HirDialogueContentApplication, HirDialogueCoordinateKind};
use crate::expr::{HirExprKind, HirNamedBlockName};
use crate::identity::{ExprId, ItemId};
use crate::item::{HirCapabilityMember, HirItemKind, HirRetainedName};
use crate::leaf::{HirIdRef, HirIdRefValue};
use crate::module::{HirModule, HirModuleStatus};
use crate::scope::HirScopeOwner;
use crate::slot::HirOrigin;
use crate::source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite};
use crate::symbol::{CallableDeclarationId, CallableDeclarationOwner};

use super::builder::HirDialogueLineCandidateBuilder;
use super::{
    DialogueIdentityCoordinateKind, DialogueLineBuildFatal, DialogueLineDiagnostic,
    DialogueLineSourceOrder, HirDialogueFlowOwner, HirDialogueLineCandidates,
    HirDialogueLineSourceOwner, HirDialogueLineSourceSite, HirDialogueNamedScope,
    InvalidCoordinateReason,
};

pub(crate) fn build_module_candidates(
    module: &HirModule,
) -> Result<(HirDialogueLineCandidates, Arc<[DialogueLineDiagnostic]>), DialogueLineBuildFatal> {
    if module.status() != HirModuleStatus::Clean {
        return Ok((
            HirDialogueLineCandidates::empty(module.key().clone()),
            Arc::from([]),
        ));
    }

    let mut applications = Vec::new();
    let expressions = module
        .arenas()
        .expressions()
        .try_iter_prepared(module.slots())
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    for (owner, expression) in expressions {
        let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
            continue;
        };
        let metadata = module
            .slots()
            .resolve_prepared(owner)
            .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
        if !matches!(metadata.origin(), HirOrigin::Source(_)) {
            continue;
        }
        let span = whole_expression_span(module, owner)?;
        applications.push((span.range().start(), span.range().end(), owner, application));
    }
    applications.sort_by_key(|(start, end, owner, _)| (*start, *end, *owner));

    let mut builder = HirDialogueLineCandidateBuilder::new(module.key());
    for (position, (_, _, owner, application)) in applications.into_iter().enumerate() {
        let source_order = u32::try_from(position)
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(DialogueLineBuildFatal::ArithmeticOverflow {
                operation: super::DialogueLineBuildOperation::SourceOrder,
            })?;
        let source_order = DialogueLineSourceOrder::try_new(source_order)?;
        let application_span = whole_expression_span(module, owner)?;
        let id = coordinate_evidence(module, application, HirDialogueCoordinateKind::Id)?;
        let text_key =
            coordinate_evidence(module, application, HirDialogueCoordinateKind::TextKey)?;
        let (source_owner, named_scopes) = source_owner_and_scopes(module, owner)?;
        let site = HirDialogueLineSourceSite::try_new(
            module.key().source(),
            owner,
            source_owner,
            named_scopes,
            source_order,
            application_span,
            id.span().cloned(),
            text_key.span().cloned(),
        )?;

        if application.has_recovery()
            || matches!(id, CoordinateEvidence::ExistingRecovery { .. })
            || matches!(text_key, CoordinateEvidence::ExistingRecovery { .. })
        {
            builder.skip(&site)?;
            continue;
        }
        if let Some(diagnostic) = id
            .diagnostic()
            .cloned()
            .or_else(|| text_key.diagnostic().cloned())
        {
            builder.reject(&site, diagnostic)?;
            continue;
        }
        builder.push(site, id.reference(), text_key.reference())?;
    }
    Ok(builder.finish())
}

enum CoordinateEvidence {
    Absent,
    Resolved {
        reference: HirIdRef,
        span: SourceSpan,
    },
    ExistingRecovery {
        span: SourceSpan,
    },
    Invalid {
        diagnostic: Box<DialogueLineDiagnostic>,
        span: SourceSpan,
    },
}

impl CoordinateEvidence {
    fn span(&self) -> Option<&SourceSpan> {
        match self {
            Self::Absent => None,
            Self::Resolved { span, .. }
            | Self::ExistingRecovery { span }
            | Self::Invalid { span, .. } => Some(span),
        }
    }

    const fn reference(&self) -> Option<&HirIdRef> {
        match self {
            Self::Resolved { reference, .. } => Some(reference),
            Self::Absent | Self::ExistingRecovery { .. } | Self::Invalid { .. } => None,
        }
    }

    fn diagnostic(&self) -> Option<&DialogueLineDiagnostic> {
        match self {
            Self::Invalid { diagnostic, .. } => Some(diagnostic),
            Self::Absent | Self::Resolved { .. } | Self::ExistingRecovery { .. } => None,
        }
    }
}

fn coordinate_evidence(
    module: &HirModule,
    application: &HirDialogueContentApplication,
    kind: HirDialogueCoordinateKind,
) -> Result<CoordinateEvidence, DialogueLineBuildFatal> {
    let mut coordinates = application
        .coordinates()
        .iter()
        .filter(|coordinate| coordinate.kind() == kind);
    let Some(first) = coordinates.next() else {
        return Ok(CoordinateEvidence::Absent);
    };
    let first_span = whole_expression_span(module, first.value())?;
    if let Some(duplicate) = coordinates.next() {
        let duplicate_span = whole_expression_span(module, duplicate.value())?;
        return Ok(CoordinateEvidence::Invalid {
            diagnostic: Box::new(DialogueLineDiagnostic::DuplicateLineIdentityCoordinate {
                coordinate: coordinate_kind(kind),
                first: first_span.clone(),
                duplicate: duplicate_span,
            }),
            span: first_span,
        });
    }
    let expression = module
        .arenas()
        .expressions()
        .resolve_prepared(module.slots(), first.value())
        .map_err(|_| DialogueLineBuildFatal::InvalidSourceComponent)?;
    match expression.kind() {
        HirExprKind::EntityReference(HirIdRefValue::Resolved(reference))
            if !expression.is_poisoned() =>
        {
            Ok(CoordinateEvidence::Resolved {
                reference: reference.clone(),
                span: first_span,
            })
        }
        HirExprKind::EntityReference(HirIdRefValue::Recovered(_)) | HirExprKind::Error(_) => {
            Ok(CoordinateEvidence::ExistingRecovery { span: first_span })
        }
        _ if expression.is_poisoned() => {
            Ok(CoordinateEvidence::ExistingRecovery { span: first_span })
        }
        _ => Ok(CoordinateEvidence::Invalid {
            diagnostic: Box::new(DialogueLineDiagnostic::InvalidLineIdentityCoordinate {
                coordinate: coordinate_kind(kind),
                reason: InvalidCoordinateReason::RuntimeExpression,
                span: first_span.clone(),
            }),
            span: first_span,
        }),
    }
}

const fn coordinate_kind(kind: HirDialogueCoordinateKind) -> DialogueIdentityCoordinateKind {
    match kind {
        HirDialogueCoordinateKind::Id => DialogueIdentityCoordinateKind::LineId,
        HirDialogueCoordinateKind::TextKey => DialogueIdentityCoordinateKind::TextKey,
    }
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
