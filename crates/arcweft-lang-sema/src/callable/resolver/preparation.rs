//! Final-HIR callee preparation for the single shared callable resolver.

use super::super::PreparedCallPrefixPayload;
use super::super::PreparedCallSiteContinuation;
use super::{
    AgentIntrinsicSignatureId, BTreeMap, BuiltinCallableId, CallCalleeClassificationFact,
    CallResolverAuthority, CallableLimits, CallableName, CallablePath, CallableSignatureSchema,
    CheckedExpressionResolution, CheckedValueResolution, ExprId, FinalCallCalleeFacts,
    FunctionValueSignatureId, FxCallableSignatureId, FxResolution, HirAssociatedCallSyntax,
    HirAssociatedReceiver, HirAssociatedSeparator, HirCallCallee, HirCallExpr, HirExpr,
    HirExprKind, HirExprSourceRole, HirModule, HirPath, HirPathRoot, HirPathSegment, HirPathValue,
    HirRecoveredName, HirSelectedMember, HirSourcePresence, HirSourceQuery, HirSourceSite,
    PrepareFinalCallCalleeError, PreparedCallCallee, PreparedFinalCallCallee,
    PreparedFreeCallScope, PreparedFunctionValueCallee, PreparedFunctionValueOriginEvidence,
    PreparedFunctionValueOriginProducer, PreparedFunctionValueOriginProgress,
    PreparedFunctionValueOriginQuery, PreparedFunctionValueOriginQueryError,
    PresentationCallableId, ProjectValueLookup, PromotionCallableId, ResolveCallError,
    ResolvedAssociatedTypeReceiver, ResolvedFunctionValueSeed, TypeId, TypeKind,
    TypeResolutionReport,
};
use crate::{
    callable::{CharacterDialoguePatchContext, DialogueCallableId, DialogueCalleeIdentity},
    final_analysis::PreparedExpressionFact,
    types::{CharacterDialogueCharacterType, EntityKind},
};
/// Prepares the only callee representation admitted by the shared resolver
/// from final-HIR structure and already checked child facts.
///
/// This operation never reads source text. For an unresolved dot path it
/// always performs the typed project value lookup first. Only a definitive
/// `Absent` result, combined with the absence of a staged value resolution,
/// permits the retained nominal receiver to be projected.
pub(crate) fn prepare_final_call_callee<'a, P, U>(
    authority: CallResolverAuthority<'a>,
    expression: ExprId,
    facts: FinalCallCalleeFacts<'a, P, U>,
    dialogue_context: CharacterDialoguePatchContext,
    limits: &CallableLimits,
) -> Result<PreparedFinalCallCallee<'a>, PrepareFinalCallCalleeError>
where
    P: PreparedCallPrefixPayload<Unselected = U>,
{
    let module = authority.module();
    let expression_node = module
        .resolve_expr(expression)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidCallExpression { expression })?;
    let HirExprKind::Call(call) = expression_node.kind() else {
        return Err(PrepareFinalCallCalleeError::InvalidCallExpression { expression });
    };
    if !matches!(call.callee(), HirCallCallee::Value { .. })
        && facts.function_value_origin.is_some()
    {
        return Err(PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin { expression });
    }

    match call.callee() {
        HirCallCallee::Value { value } => {
            prepare_value_call_callee(authority, *value, facts, dialogue_context, limits)
        }
        HirCallCallee::UnresolvedDot {
            value_receiver,
            nominal_receiver,
            separator: HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback),
            member: HirRecoveredName::Valid(member),
        } => prepare_unresolved_dot_callee(
            authority,
            *value_receiver,
            nominal_receiver,
            member,
            facts,
            limits,
        ),
        HirCallCallee::Associated {
            receiver,
            separator: HirAssociatedSeparator::Present(HirAssociatedCallSyntax::ExplicitDoubleColon),
            member: HirRecoveredName::Valid(member),
        } => prepare_associated_callee(receiver, member, facts.nominal_receivers),
        HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => {
            Err(PrepareFinalCallCalleeError::RecoveredCallee)
        }
    }
}

fn prepare_value_call_callee<'a, P, U>(
    authority: CallResolverAuthority<'a>,
    value: ExprId,
    mut facts: FinalCallCalleeFacts<'a, P, U>,
    dialogue_context: CharacterDialoguePatchContext,
    limits: &CallableLimits,
) -> Result<PreparedFinalCallCallee<'a>, PrepareFinalCallCalleeError>
where
    P: PreparedCallPrefixPayload<Unselected = U>,
{
    let mut function_origin = facts.function_value_origin.take();
    let expression = authority
        .module()
        .resolve_expr(value)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidCallExpression { expression: value })?;

    if let Some(checked) = facts.expressions.get(&value)
        && let Some(callee) = character_dialogue_callee(value, checked)?
    {
        if function_origin.is_some() {
            return Err(PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin {
                expression: value,
            });
        }
        return Ok(PreparedFinalCallCallee::Dialogue {
            id: DialogueCallableId::resolve(&callee),
            callee,
            patch_context: dialogue_context,
        });
    }

    if let HirExprKind::Select(select) = expression.kind() {
        let HirSelectedMember::Name(member) = select.member() else {
            return Err(PrepareFinalCallCalleeError::RecoveredCallee);
        };
        let receiver = facts.expressions.get(&select.target()).ok_or(
            PrepareFinalCallCalleeError::MissingExpressionFact {
                expression: select.target(),
            },
        )?;
        if function_origin.is_some() {
            return Err(PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin {
                expression: value,
            });
        }
        return Ok(PreparedFinalCallCallee::Selected {
            receiver_expression: select.target(),
            receiver_type: Box::new(receiver.ty().clone()),
            method: CallableName::try_new(member.as_str())
                .map_err(|_| PrepareFinalCallCalleeError::InvalidValuePath { expression: value })?,
        });
    }

    if let HirExprKind::Path(HirPathValue::Resolved(path)) = expression.kind() {
        if let Some(checked) = facts.expressions.get(&value)
            && matches!(
                checked.checked_resolution(),
                Some(CheckedExpressionResolution::Value(
                    CheckedValueResolution::Local(_)
                ))
            )
        {
            if matches!(checked.ty(), TypeKind::Function { .. }) {
                let origin = function_origin.take().ok_or(
                    PrepareFinalCallCalleeError::MissingFunctionValueOrigin { expression: value },
                )?;
                return prepare_function_value(
                    value,
                    checked.ty(),
                    origin,
                    facts.prepared_calls,
                    limits,
                )
                .map(|value| PreparedFinalCallCallee::FunctionValue {
                    value: Box::new(value),
                });
            }
            if function_origin.is_some() {
                return Err(PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin {
                    expression: value,
                });
            }
            return Ok(PreparedFinalCallCallee::NonCallableValue {
                expression: value,
                ty: Box::new(checked.ty().clone()),
            });
        }
        let lookup = resolve_final_project_value(authority, value, path)?;
        let checked = facts.expressions.get(&value);
        let project = match checked.and_then(PreparedExpressionFact::checked_resolution) {
            Some(CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                callable,
            ))) => match lookup {
                ProjectValueLookup::Present(symbol)
                    if symbol.declaration() == callable.declaration() =>
                {
                    Some(callable.declaration().clone())
                }
                ProjectValueLookup::Present(_) | ProjectValueLookup::Absent => {
                    return Err(PrepareFinalCallCalleeError::ProjectValueFactMismatch {
                        expression: value,
                    });
                }
            },
            Some(CheckedExpressionResolution::Value(_)) => None,
            Some(_) | None => match lookup {
                ProjectValueLookup::Present(symbol) => Some(symbol.declaration().clone()),
                ProjectValueLookup::Absent => None,
            },
        };
        let scope = match path.root() {
            HirPathRoot::ImplicitCrate => PreparedFreeCallScope::Implicit,
            HirPathRoot::Crate | HirPathRoot::SelfModule | HirPathRoot::Super { .. } => {
                PreparedFreeCallScope::ExplicitProject
            }
        };
        if function_origin.is_some() {
            return Err(PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin {
                expression: value,
            });
        }
        let enum_variant = checked
            .map(|checked| {
                let project_variant = matches!(
                    checked,
                    PreparedExpressionFact::ProjectVariant(_)
                ) || matches!(
                    checked.checked_resolution(),
                    Some(CheckedExpressionResolution::Variant(variant))
                        if matches!(variant.owner(), crate::final_analysis::CheckedVariantOwner::Project { .. })
                );
                let accepted = super::AcceptedEnumVariantCase::try_from_checked(checked, limits)
                    .map_err(|_| PrepareFinalCallCalleeError::InvalidEnumVariantAuthority {
                        expression: value,
                    })?;
                if project_variant && accepted.is_none() {
                    return Err(PrepareFinalCallCalleeError::InvalidEnumVariantAuthority {
                        expression: value,
                    });
                }
                Ok(accepted)
            })
            .transpose()?
            .flatten();
        return Ok(PreparedFinalCallCallee::Free {
            path: Box::new(callable_path_from_hir(value, path, limits)?),
            project: project.map(Box::new),
            scope,
            enum_variant: enum_variant.map(Box::new),
        });
    }

    let checked = facts
        .expressions
        .get(&value)
        .ok_or(PrepareFinalCallCalleeError::MissingExpressionFact { expression: value })?;

    if matches!(checked.ty(), TypeKind::Function { .. }) {
        let origin = function_origin
            .take()
            .ok_or(PrepareFinalCallCalleeError::MissingFunctionValueOrigin { expression: value })?;
        return prepare_function_value(value, checked.ty(), origin, facts.prepared_calls, limits)
            .map(|value| PreparedFinalCallCallee::FunctionValue {
                value: Box::new(value),
            });
    }

    if function_origin.is_some() {
        return Err(PrepareFinalCallCalleeError::UnexpectedFunctionValueOrigin {
            expression: value,
        });
    }

    Ok(PreparedFinalCallCallee::NonCallableValue {
        expression: value,
        ty: Box::new(checked.ty().clone()),
    })
}

/// Resolve a function value's callable origin from typed HIR ownership.  A
/// direct call and a local whose canonical initializer is a call must carry a
/// graph site; only values with no call origin remain independent.
pub(crate) fn prepare_function_value_origin_query(
    topology: std::sync::Arc<arcweft_lang_hir::project::HirProjectEvaluationTopology>,
    module: &HirModule,
    expression: ExprId,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
) -> Result<PreparedFunctionValueOriginProgress, PreparedFunctionValueOriginQueryError> {
    PreparedFunctionValueOriginQuery::start(topology, module, expression)
        .advance(module, expressions)
}

pub(crate) fn prepare_presentation_callee_id(
    module: &HirModule,
    call: &HirCallExpr,
    limits: &CallableLimits,
) -> Result<Option<PresentationCallableId>, PrepareFinalCallCalleeError> {
    let HirCallCallee::Value { value } = call.callee() else {
        return Ok(None);
    };
    let expression = module
        .resolve_expr(*value)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidCallExpression { expression: *value })?;
    let HirExprKind::Path(HirPathValue::Resolved(path)) = expression.kind() else {
        return Ok(None);
    };
    if path.root() != HirPathRoot::ImplicitCrate {
        return Ok(None);
    }
    let path = callable_path_from_hir(*value, path, limits)?;
    Ok(PresentationCallableId::resolve(&path))
}

fn character_dialogue_callee(
    expression: ExprId,
    checked: &PreparedExpressionFact,
) -> Result<Option<DialogueCalleeIdentity>, PrepareFinalCallCalleeError> {
    if let Some(CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item))) =
        checked.checked_resolution()
        && item.family() == arcweft_id::DeclarationIdentityFamily::Character
    {
        let character = item
            .character()
            .ok_or(PrepareFinalCallCalleeError::InvalidCharacterIdentity { expression })?;
        return Ok(Some(DialogueCalleeIdentity::Character {
            character: CharacterDialogueCharacterType::Exact(character),
        }));
    }
    Ok(match checked.ty() {
        TypeKind::Ref(entity) if entity.kind() == &EntityKind::Character => {
            Some(DialogueCalleeIdentity::Character {
                character: CharacterDialogueCharacterType::Any,
            })
        }
        TypeKind::CharacterDialogue(dialogue) => Some(DialogueCalleeIdentity::CharacterDialogue {
            character: dialogue.character().clone(),
        }),
        _ => None,
    })
}

fn prepare_unresolved_dot_callee<'a, P, U>(
    authority: CallResolverAuthority<'a>,
    value_receiver: ExprId,
    nominal_receiver: &HirAssociatedReceiver,
    member: &arcweft_lang_hir::leaf::HirName,
    facts: FinalCallCalleeFacts<'a, P, U>,
    limits: &CallableLimits,
) -> Result<PreparedFinalCallCallee<'a>, PrepareFinalCallCalleeError> {
    let checked = facts.expressions.get(&value_receiver);
    let expression = authority
        .module()
        .resolve_expr(value_receiver)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidCallExpression {
            expression: value_receiver,
        })?;
    let staged_value = checked.is_some_and(|checked| {
        matches!(
            checked.checked_resolution(),
            Some(CheckedExpressionResolution::Value(_))
        )
    });

    let project_lookup = match expression.kind() {
        HirExprKind::Path(HirPathValue::Resolved(path)) => {
            let full_path = path.with_terminal_member(member);
            if full_path.segments().len() > limits.max_path_segments() {
                return Err(PrepareFinalCallCalleeError::InvalidValuePath {
                    expression: value_receiver,
                });
            }
            match resolve_final_project_value(authority, value_receiver, &full_path)? {
                ProjectValueLookup::Present(symbol) => {
                    if let Some(CheckedExpressionResolution::Value(
                        CheckedValueResolution::ProjectCallable(callable),
                    )) = checked.and_then(PreparedExpressionFact::checked_resolution)
                        && callable.declaration() != symbol.declaration()
                    {
                        return Err(PrepareFinalCallCalleeError::ProjectValueFactMismatch {
                            expression: value_receiver,
                        });
                    }
                    let scope = match full_path.root() {
                        HirPathRoot::ImplicitCrate => PreparedFreeCallScope::Implicit,
                        HirPathRoot::Crate
                        | HirPathRoot::SelfModule
                        | HirPathRoot::Super { .. } => PreparedFreeCallScope::ExplicitProject,
                    };
                    return Ok(PreparedFinalCallCallee::Free {
                        path: Box::new(callable_path_from_hir(value_receiver, &full_path, limits)?),
                        project: Some(Box::new(symbol.declaration().clone())),
                        scope,
                        enum_variant: None,
                    });
                }
                ProjectValueLookup::Absent => {}
            }
            Some(resolve_final_project_value(
                authority,
                value_receiver,
                path,
            )?)
        }
        HirExprKind::Path(HirPathValue::Recovered(_)) => {
            return Err(PrepareFinalCallCalleeError::RecoveredCallee);
        }
        _ => None,
    };

    match project_lookup {
        Some(ProjectValueLookup::Present(symbol)) => {
            if let Some(CheckedExpressionResolution::Value(
                CheckedValueResolution::ProjectCallable(callable),
            )) = checked.and_then(PreparedExpressionFact::checked_resolution)
                && callable.declaration() != symbol.declaration()
            {
                return Err(PrepareFinalCallCalleeError::ProjectValueFactMismatch {
                    expression: value_receiver,
                });
            }
            if !staged_value {
                return Err(PrepareFinalCallCalleeError::ProjectValueFactMismatch {
                    expression: value_receiver,
                });
            }
        }
        Some(ProjectValueLookup::Absent) if !staged_value => {
            if let Some(path) = prepare_language_free_dot_path(
                authority.world().environment().callable_catalog(),
                value_receiver,
                expression,
                member,
                limits,
            )? {
                return Ok(PreparedFinalCallCallee::Free {
                    path: Box::new(path),
                    project: None,
                    scope: PreparedFreeCallScope::Implicit,
                    enum_variant: None,
                });
            }
            return prepare_associated_callee(nominal_receiver, member, facts.nominal_receivers);
        }
        Some(ProjectValueLookup::Absent) | None => {}
    }

    let checked = checked.ok_or(PrepareFinalCallCalleeError::MissingExpressionFact {
        expression: value_receiver,
    })?;

    Ok(PreparedFinalCallCallee::Selected {
        receiver_expression: value_receiver,
        receiver_type: Box::new(checked.ty().clone()),
        method: CallableName::try_new(member.as_str()).map_err(|_| {
            PrepareFinalCallCalleeError::InvalidValuePath {
                expression: value_receiver,
            }
        })?,
    })
}

/// Classifies the language-owned free-call identities that use dot spelling.
///
/// Dot syntax remains value-first. This classifier is consulted only after
/// both the full project value and the base value are absent, and explicit
/// project roots never enter language namespaces. Candidate construction and
/// accounting remain owned by `resolve_call_target`.
pub(crate) fn prepare_language_free_dot_path(
    accepted: &crate::callable::RegisteredCallableCatalog,
    expression: ExprId,
    receiver: &HirExpr,
    member: &arcweft_lang_hir::leaf::HirName,
    limits: &CallableLimits,
) -> Result<Option<CallablePath>, PrepareFinalCallCalleeError> {
    let HirExprKind::Path(HirPathValue::Resolved(path)) = receiver.kind() else {
        return Ok(None);
    };
    if path.root() != HirPathRoot::ImplicitCrate {
        return Ok(None);
    }
    let path = callable_path_from_hir(expression, &path.with_terminal_member(member), limits)?;
    let is_language_free = matches!(
        FxCallableSignatureId::resolve(&path),
        FxResolution::Known(_)
    ) || BuiltinCallableId::resolve(&path).is_some()
        || AgentIntrinsicSignatureId::resolve(&path).is_some()
        || PresentationCallableId::resolve(&path).is_some()
        || PromotionCallableId::resolve(&path).is_some()
        || accepted.free(&path).is_some();
    Ok(is_language_free.then_some(path))
}

fn prepare_associated_callee<'a>(
    receiver: &HirAssociatedReceiver,
    member: &arcweft_lang_hir::leaf::HirName,
    reports: &'a BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<PreparedFinalCallCallee<'a>, PrepareFinalCallCalleeError> {
    let HirAssociatedReceiver::Resolved { receiver } = receiver else {
        return Err(PrepareFinalCallCalleeError::RecoveredCallee);
    };
    let report =
        reports
            .get(receiver)
            .ok_or(PrepareFinalCallCalleeError::MissingNominalReceiver {
                receiver: *receiver,
            })?;
    let resolved = ResolvedAssociatedTypeReceiver::try_from_report(report).map_err(|_| {
        PrepareFinalCallCalleeError::InvalidNominalReceiver {
            receiver: *receiver,
        }
    })?;
    if resolved.product().root() != *receiver {
        return Err(PrepareFinalCallCalleeError::InvalidNominalReceiver {
            receiver: *receiver,
        });
    }
    Ok(PreparedFinalCallCallee::AssociatedType {
        receiver: resolved,
        member: CallableName::try_new(member.as_str()).map_err(|_| {
            PrepareFinalCallCalleeError::InvalidNominalReceiver {
                receiver: *receiver,
            }
        })?,
    })
}

fn prepare_function_value<'a, P, U>(
    expression: ExprId,
    ty: &TypeKind,
    origin: PreparedFunctionValueOriginEvidence,
    prepared_calls: super::super::PreparedCallGraphIngress<'a, P, U>,
    limits: &CallableLimits,
) -> Result<PreparedFunctionValueCallee, PrepareFinalCallCalleeError>
where
    P: PreparedCallPrefixPayload<Unselected = U>,
{
    if !matches!(ty, TypeKind::Function { .. }) {
        return Err(PrepareFinalCallCalleeError::InvalidFunctionValue { expression });
    }
    let (origin_callee, producer, captures) = origin.into_parts();
    if origin_callee != expression {
        return Err(PrepareFinalCallCalleeError::InvalidFunctionValueOrigin { expression });
    }
    let (seed, origin) = match producer {
        PreparedFunctionValueOriginProducer::Call(super::super::CheckedCallSite::HirCall(
            producer,
        )) => {
            let site = super::super::CheckedCallSite::HirCall(producer);
            let continuation = prepared_calls
                .continuation_at(site, ty)
                .map_err(PrepareFinalCallCalleeError::PreparedContinuationInvariant)?;
            match continuation {
                PreparedCallSiteContinuation::Prepared(reference) => (
                    ResolvedFunctionValueSeed::PreparedContinuation { reference },
                    PreparedFunctionValueOriginEvidence::new(
                        origin_callee,
                        PreparedFunctionValueOriginProducer::PreparedContinuation(site),
                        captures,
                    ),
                ),
                PreparedCallSiteContinuation::Independent => {
                    let ordinal = super::FunctionValueOrdinal::try_from_usize(0).map_err(|_| {
                        PrepareFinalCallCalleeError::InvalidFunctionValue { expression }
                    })?;
                    let id = FunctionValueSignatureId::new(producer, ordinal);
                    let schema = CallableSignatureSchema::for_function_value(ty, limits)
                        .map_err(|_| PrepareFinalCallCalleeError::InvalidFunctionSchema)?;
                    (
                        ResolvedFunctionValueSeed::Independent {
                            id,
                            schema,
                            effect_callable: None,
                        },
                        PreparedFunctionValueOriginEvidence::new(
                            origin_callee,
                            PreparedFunctionValueOriginProducer::IndependentExpression { producer },
                            captures,
                        ),
                    )
                }
            }
        }
        PreparedFunctionValueOriginProducer::Call(
            super::super::CheckedCallSite::DialogueApplication(_),
        ) => return Err(PrepareFinalCallCalleeError::InvalidFunctionValue { expression }),
        PreparedFunctionValueOriginProducer::Lexical { local } => {
            if !captures.is_empty() {
                return Err(PrepareFinalCallCalleeError::InvalidFunctionValue { expression });
            }
            let schema = CallableSignatureSchema::for_function_value(ty, limits)
                .map_err(|_| PrepareFinalCallCalleeError::InvalidFunctionSchema)?;
            (
                ResolvedFunctionValueSeed::Lexical {
                    id: super::LocalCallableId::from_checked_local(local),
                    schema,
                    effect_callable: None,
                },
                PreparedFunctionValueOriginEvidence::new(
                    origin_callee,
                    PreparedFunctionValueOriginProducer::Lexical { local },
                    Vec::new(),
                ),
            )
        }
        PreparedFunctionValueOriginProducer::IndependentExpression { producer } => {
            let ordinal = super::FunctionValueOrdinal::try_from_usize(0)
                .map_err(|_| PrepareFinalCallCalleeError::InvalidFunctionValue { expression })?;
            let id = FunctionValueSignatureId::new(producer, ordinal);
            let schema = CallableSignatureSchema::for_function_value(ty, limits)
                .map_err(|_| PrepareFinalCallCalleeError::InvalidFunctionSchema)?;
            (
                ResolvedFunctionValueSeed::Independent {
                    id,
                    schema,
                    effect_callable: None,
                },
                PreparedFunctionValueOriginEvidence::new(
                    origin_callee,
                    PreparedFunctionValueOriginProducer::IndependentExpression { producer },
                    captures,
                ),
            )
        }
        PreparedFunctionValueOriginProducer::PreparedContinuation(_) => {
            return Err(PrepareFinalCallCalleeError::InvalidFunctionValue { expression });
        }
    };
    Ok(PreparedFunctionValueCallee::new(ty.clone(), origin, seed))
}

fn resolve_final_project_value<'a>(
    authority: CallResolverAuthority<'a>,
    expression: ExprId,
    path: &HirPath,
) -> Result<ProjectValueLookup<'a>, PrepareFinalCallCalleeError> {
    let source = required_expression_span(authority.module(), expression)?;
    authority
        .symbols
        .resolve_hir_value_target(authority.module.key().path(), path, source)
        .map_err(|error| PrepareFinalCallCalleeError::ProjectValueLookup {
            error: Box::new(error),
        })
}

fn required_expression_span(
    module: &HirModule,
    expression: ExprId,
) -> Result<arcweft_source::SourceSpan, PrepareFinalCallCalleeError> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner: expression,
                role: HirExprSourceRole::Whole,
            },
        )
        .map_err(|_| PrepareFinalCallCalleeError::MissingValueSource { expression })?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            Err(PrepareFinalCallCalleeError::MissingValueSource { expression })
        }
    }
}

fn callable_path_from_hir(
    expression: ExprId,
    path: &HirPath,
    limits: &CallableLimits,
) -> Result<CallablePath, PrepareFinalCallCalleeError> {
    let segments = path
        .segments()
        .iter()
        .map(|segment| {
            CallableName::try_new(match segment {
                HirPathSegment::Identifier(name) => name.as_str(),
                HirPathSegment::ProjectSymbol(name) => name.as_str(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PrepareFinalCallCalleeError::InvalidValuePath { expression })?;
    CallablePath::try_new_with_limits(segments, limits)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidValuePath { expression })
}

pub(super) fn classify_prepared_callee(
    prepared: &PreparedCallCallee<'_>,
    call: &HirCallExpr,
    module: &HirModule,
) -> Result<CallCalleeClassificationFact, ResolveCallError> {
    let module_id = module.module_id();
    match (prepared, call.callee()) {
        (
            PreparedCallCallee::Free { .. }
            | PreparedCallCallee::Dialogue { .. }
            | PreparedCallCallee::FunctionValue { .. }
            | PreparedCallCallee::NonCallableValue { .. },
            HirCallCallee::Value { value },
        ) if value.module() == module_id => {
            Ok(CallCalleeClassificationFact::Value { expression: *value })
        }
        (
            PreparedCallCallee::Selected {
                receiver_expression,
                method,
                ..
            },
            HirCallCallee::Value { value },
        ) if value.module() == module_id
            && module.resolve_expr(*value).is_ok_and(|expression| {
                matches!(
                    expression.kind(),
                    HirExprKind::Select(select)
                        if select.target() == *receiver_expression
                            && matches!(select.member(), HirSelectedMember::Name(member) if member.as_str() == method.as_str())
                )
            }) =>
        {
            Ok(CallCalleeClassificationFact::Value { expression: *value })
        }
        (PreparedCallCallee::Free { .. }, HirCallCallee::UnresolvedDot { value_receiver, .. })
            if value_receiver.module() == module_id =>
        {
            Ok(CallCalleeClassificationFact::Value {
                expression: *value_receiver,
            })
        }
        (
            PreparedCallCallee::Selected {
                receiver_expression,
                method,
                ..
            },
            HirCallCallee::UnresolvedDot {
                value_receiver,
                separator: HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback),
                member: HirRecoveredName::Valid(member),
                ..
            },
        ) if receiver_expression == value_receiver && method.as_str() == member.as_str() => {
            Ok(CallCalleeClassificationFact::Value {
                expression: *value_receiver,
            })
        }
        (
            PreparedCallCallee::AssociatedType { receiver, member },
            HirCallCallee::UnresolvedDot {
                nominal_receiver: HirAssociatedReceiver::Resolved { receiver: owner },
                separator,
                member: HirRecoveredName::Valid(authored_member),
                ..
            },
        ) if *separator
            == HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback)
            && *owner == receiver.product().root()
            && receiver.product().recovered() == receiver.ty()
            && receiver.root().recovered() == Some(receiver.ty())
            && owner.module() == module_id
            && member.as_str() == authored_member.as_str() =>
        {
            Ok(CallCalleeClassificationFact::AssociatedType {
                receiver: *owner,
                separator: *separator,
            })
        }
        (
            PreparedCallCallee::AssociatedType { receiver, member },
            HirCallCallee::Associated {
                receiver: HirAssociatedReceiver::Resolved { receiver: owner },
                separator,
                member: HirRecoveredName::Valid(authored_member),
            },
        ) if *separator
            == HirAssociatedSeparator::Present(HirAssociatedCallSyntax::ExplicitDoubleColon)
            && *owner == receiver.product().root()
            && receiver.product().recovered() == receiver.ty()
            && receiver.root().recovered() == Some(receiver.ty())
            && owner.module() == module_id
            && member.as_str() == authored_member.as_str() =>
        {
            Ok(CallCalleeClassificationFact::AssociatedType {
                receiver: *owner,
                separator: *separator,
            })
        }
        _ => Err(ResolveCallError::InvalidResolvedCallable),
    }
}
