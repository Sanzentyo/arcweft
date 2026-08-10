//! Final-HIR callee preparation for the single shared callable resolver.

use super::{
    AgentIntrinsicSignatureId, BTreeMap, BuiltinCallableId, CallCalleeClassificationFact,
    CallResolverAuthority, CallableGroupIndex, CallableLimits, CallableName, CallablePath,
    CallableSignatureSchema, CheckedExpression, CheckedExpressionResolution,
    CheckedValueResolution, ExprId, FinalCallCalleeFacts, FunctionValueSignatureId,
    FxCallableSignatureId, FxResolution, HirAssociatedCallSyntax, HirAssociatedReceiver,
    HirAssociatedSeparator, HirCallCallee, HirCallExpr, HirExpr, HirExprKind, HirExprSourceRole,
    HirModule, HirPath, HirPathRoot, HirPathSegment, HirPathValue, HirRecoveredName,
    HirSelectedMember, HirSourcePresence, HirSourceQuery, HirSourceSite,
    PrepareFinalCallCalleeError, PreparedCallCallee, PreparedFinalCallCallee,
    PreparedFreeCallScope, PresentationCallableId, ProjectValueLookup, PromotionCallableId,
    ResolveCallError, ResolvedAssociatedTypeReceiver, ResolvedFunctionValueSeed, TypeId, TypeKind,
    TypeResolutionReport,
};
use crate::{
    callable::{CharacterDialoguePatchContext, DialogueCallableId, DialogueCalleeIdentity},
    types::{CharacterDialogueCharacterType, EntityKind},
};

/// Prepares the only callee representation admitted by the shared resolver
/// from final-HIR structure and already checked child facts.
///
/// This operation never reads source text. For an unresolved dot path it
/// always performs the typed project value lookup first. Only a definitive
/// `Absent` result, combined with the absence of a staged value resolution,
/// permits the retained nominal receiver to be projected.
pub(crate) fn prepare_final_call_callee<'a>(
    authority: CallResolverAuthority<'a>,
    expression: ExprId,
    facts: FinalCallCalleeFacts<'a>,
    dialogue_context: CharacterDialoguePatchContext,
    limits: &CallableLimits,
) -> Result<PreparedFinalCallCallee<'a>, PrepareFinalCallCalleeError> {
    let module = authority.module();
    let expression_node = module
        .resolve_expr(expression)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidCallExpression { expression })?;
    let HirExprKind::Call(call) = expression_node.kind() else {
        return Err(PrepareFinalCallCalleeError::InvalidCallExpression { expression });
    };

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

fn prepare_value_call_callee<'a>(
    authority: CallResolverAuthority<'a>,
    value: ExprId,
    facts: FinalCallCalleeFacts<'a>,
    dialogue_context: CharacterDialoguePatchContext,
    limits: &CallableLimits,
) -> Result<PreparedFinalCallCallee<'a>, PrepareFinalCallCalleeError> {
    let expression = authority
        .module()
        .resolve_expr(value)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidCallExpression { expression: value })?;

    if let Some(checked) = facts.expressions.get(&value)
        && let Some(callee) = character_dialogue_callee(checked)
    {
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
                checked.resolution(),
                CheckedExpressionResolution::Value(CheckedValueResolution::Local(_))
            )
        {
            if matches!(checked.ty(), TypeKind::Function { .. }) {
                return prepare_function_value(value, checked.ty(), facts.calls, limits).map(
                    |value| PreparedFinalCallCallee::FunctionValue {
                        value: Box::new(value),
                    },
                );
            }
            return Ok(PreparedFinalCallCallee::NonCallableValue {
                expression: value,
                ty: Box::new(checked.ty().clone()),
            });
        }
        let lookup = resolve_final_project_value(authority, value, path)?;
        let checked = facts.expressions.get(&value);
        let project = match checked.map(CheckedExpression::resolution) {
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
        return Ok(PreparedFinalCallCallee::Free {
            path: Box::new(callable_path_from_hir(value, path, limits)?),
            project: project.map(Box::new),
            scope,
            enum_variant: facts.enum_variants.get(&value).cloned().map(Box::new),
        });
    }

    let checked = facts
        .expressions
        .get(&value)
        .ok_or(PrepareFinalCallCalleeError::MissingExpressionFact { expression: value })?;

    if matches!(checked.ty(), TypeKind::Function { .. }) {
        return prepare_function_value(value, checked.ty(), facts.calls, limits).map(|value| {
            PreparedFinalCallCallee::FunctionValue {
                value: Box::new(value),
            }
        });
    }

    Ok(PreparedFinalCallCallee::NonCallableValue {
        expression: value,
        ty: Box::new(checked.ty().clone()),
    })
}

fn character_dialogue_callee(checked: &CheckedExpression) -> Option<DialogueCalleeIdentity> {
    if let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
        checked.resolution()
        && item.family() == arcweft_id::DeclarationIdentityFamily::Character
    {
        return Some(DialogueCalleeIdentity::Character {
            character: item.character().map_or(
                CharacterDialogueCharacterType::Any,
                CharacterDialogueCharacterType::Exact,
            ),
        });
    }
    match checked.ty() {
        TypeKind::Ref(entity) if entity.kind() == &EntityKind::Character => {
            Some(DialogueCalleeIdentity::Character {
                character: CharacterDialogueCharacterType::Any,
            })
        }
        TypeKind::CharacterDialogue(dialogue) => Some(DialogueCalleeIdentity::CharacterDialogue {
            character: dialogue.character().clone(),
        }),
        _ => None,
    }
}

fn prepare_unresolved_dot_callee<'a>(
    authority: CallResolverAuthority<'a>,
    value_receiver: ExprId,
    nominal_receiver: &HirAssociatedReceiver,
    member: &arcweft_lang_hir::leaf::HirName,
    facts: FinalCallCalleeFacts<'a>,
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
        matches!(checked.resolution(), CheckedExpressionResolution::Value(_))
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
                    )) = checked.map(CheckedExpression::resolution)
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
            )) = checked.map(CheckedExpression::resolution)
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
            if let Some(path) =
                prepare_language_free_dot_path(value_receiver, expression, member, limits)?
            {
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
        || PromotionCallableId::resolve(&path).is_some();
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

fn prepare_function_value(
    expression: ExprId,
    ty: &TypeKind,
    calls: &BTreeMap<ExprId, super::CallTargetFacts>,
    limits: &CallableLimits,
) -> Result<ResolvedFunctionValueSeed, PrepareFinalCallCalleeError> {
    if !matches!(ty, TypeKind::Function { .. }) {
        return Err(PrepareFinalCallCalleeError::InvalidFunctionValue { expression });
    }
    let schema = CallableSignatureSchema::for_function_value(ty, limits)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidFunctionSchema)?;
    let (continuation_base, next_group) =
        calls
            .get(&expression)
            .map_or((None, CallableGroupIndex::ZERO), |call| {
                let next_group = call.next_group().unwrap_or(CallableGroupIndex::ZERO);
                let base = match (call.target(), call.next_group()) {
                    (super::CallTargetFact::Selected { selected, .. }, Some(_)) => {
                        Some(selected.as_ref().clone())
                    }
                    _ => None,
                };
                (base, next_group)
            });
    let ordinal = super::FunctionValueOrdinal::try_from_usize(0)
        .map_err(|_| PrepareFinalCallCalleeError::InvalidFunctionValue { expression })?;
    Ok(ResolvedFunctionValueSeed {
        id: FunctionValueSignatureId::new(expression, ordinal),
        ty: ty.clone(),
        schema,
        effect_callable: None,
        continuation_base,
        next_group,
    })
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
