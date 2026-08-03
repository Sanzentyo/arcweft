//! Candidate-only Call lowering for the E34 ordinary-index interpretation.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{
    AttachedCandidateExpressionChild, AttachedCandidateNode, AttachedCandidateTypeRoot,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxAssociatedCallSyntax,
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
    SyntaxCallCalleeProjection, SyntaxCallProjection, SyntaxCallTypeApplicationSpelling,
    SyntaxCallTypeApplicationTerminator, SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole,
    SyntaxRequiredTokenState,
};
use arcweft_lang_syntax::name::SyntaxNameIssue;

use crate::expr::{
    HirAssociatedCallSyntax, HirAssociatedReceiver, HirCallArgument, HirCallArgumentListTerminator,
    HirCallArgumentOrdinal, HirCallBuildError, HirCallCallee, HirCallChildPoison,
    HirCallChildStates, HirCallExpr, HirCallTypeApplication, HirCallTypeApplicationSpelling,
    HirCallTypeApplicationTerminator, HirCallTypeArgument, HirCallTypeArgumentOrdinal,
    HirCallValue, HirPoisonState, HirRecoveredName, HirRecoveryIssue, HirRequiredTokenState,
};
use crate::identity::{ExprId, ScopeId, TypeId};
use crate::lower::{HirInvariantFailure, HirLimitError, HirLowerFailure};
use crate::source_index::expression_component_role;

use super::CandidateCursor;
use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::name_projection::{name, recovered_name, require_attempted_name_limit};

#[derive(Clone, Copy)]
struct LoweredCandidateCallChild {
    expression: ExprId,
    missing: bool,
    state: HirCallChildPoison,
}

#[derive(Clone, Copy)]
struct LoweredCandidateCallType {
    ty: TypeId,
    state: HirCallChildPoison,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_candidate_call(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        projection: &SyntaxCallProjection,
    ) -> Result<(HirCallExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let expression_projection = node
            .expression_projection()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let mut children = candidate_call_children(node)?;
        let mut types = candidate_call_types(node)?;

        if let SyntaxCallProjection::CallbackBlock(callback) = projection {
            if !types.is_empty() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let callee = self.lower_required_candidate_call_child(
                expression_projection,
                &mut children,
                ExpressionComponentRole::CallCallee,
                scope,
                cursor,
            )?;
            if callee.missing {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let argument = self.lower_required_candidate_call_child(
                expression_projection,
                &mut children,
                ExpressionComponentRole::CallArgument {
                    argument: 0,
                    part: SyntaxCallArgumentPart::Value,
                },
                scope,
                cursor,
            )?;
            if !children.is_empty() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let argument_states = [argument.state];
            return build_candidate_call(
                HirCallCallee::value(callee.expression),
                callee.state,
                HirCallTypeApplication::absent(),
                Box::new([HirCallArgument::Positional {
                    value: candidate_call_value(argument),
                }]),
                callback.terminator(),
                &argument_states,
                &[],
            );
        }

        let SyntaxCallProjection::Parenthesized(call) = projection else {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        let (callee, callee_state) = match call.callee() {
            SyntaxCallCalleeProjection::Ordinary => {
                let child = self.lower_required_candidate_call_child(
                    expression_projection,
                    &mut children,
                    ExpressionComponentRole::CallCallee,
                    scope,
                    cursor,
                )?;
                if child.missing {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                (HirCallCallee::value(child.expression), child.state)
            }
            SyntaxCallCalleeProjection::UnresolvedDot { member } => {
                let value = self.lower_required_candidate_call_child(
                    expression_projection,
                    &mut children,
                    ExpressionComponentRole::CallAssociatedReceiver,
                    scope,
                    cursor,
                )?;
                if value.missing {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                let nominal = self.lower_required_candidate_call_type(
                    &mut types,
                    SyntaxCallTypeChildRole::DotNominalReceiver,
                    scope,
                    cursor,
                )?;
                (
                    HirCallCallee::unresolved_dot(
                        value.expression,
                        candidate_associated_receiver(nominal),
                        recovered_name(member)?,
                    ),
                    value.state,
                )
            }
            SyntaxCallCalleeProjection::Associated { syntax, member } => {
                let receiver = self.lower_required_candidate_call_type(
                    &mut types,
                    SyntaxCallTypeChildRole::AssociatedReceiver,
                    scope,
                    cursor,
                )?;
                (
                    HirCallCallee::associated(
                        candidate_associated_receiver(receiver),
                        recovered_name(member)?,
                        match syntax {
                            SyntaxAssociatedCallSyntax::DotFallback => {
                                HirAssociatedCallSyntax::DotFallback
                            }
                            SyntaxAssociatedCallSyntax::ExplicitDoubleColon => {
                                HirAssociatedCallSyntax::ExplicitDoubleColon
                            }
                        },
                    ),
                    HirCallChildPoison::Clean,
                )
            }
        };

        let mut type_argument_states = Vec::new();
        let explicit_type_application = match call.explicit_type_application() {
            None => HirCallTypeApplication::absent(),
            Some(application) => {
                let mut arguments = Vec::with_capacity(application.arguments().len());
                for (position, projection) in application.arguments().iter().enumerate() {
                    HirCallTypeArgumentOrdinal::try_new(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    if matches!(projection, SyntaxCallTypeArgumentProjection::Missing) {
                        arguments.push(HirCallTypeArgument::Missing);
                        continue;
                    }
                    let ordinal = u16::try_from(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    let child = self.lower_required_candidate_call_type(
                        &mut types,
                        SyntaxCallTypeChildRole::ExplicitCallTypeArgument { ordinal },
                        scope,
                        cursor,
                    )?;
                    let expected_poison =
                        matches!(projection, SyntaxCallTypeArgumentProjection::InvalidPresent);
                    if (child.state == HirCallChildPoison::Poisoned) != expected_poison {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                    type_argument_states.push(child.state);
                    arguments.push(if expected_poison {
                        HirCallTypeArgument::InvalidPresent { poisoned: child.ty }
                    } else {
                        HirCallTypeArgument::Resolved { ty: child.ty }
                    });
                }
                HirCallTypeApplication::present(
                    match application.spelling() {
                        SyntaxCallTypeApplicationSpelling::DirectAngle => {
                            HirCallTypeApplicationSpelling::DirectAngle
                        }
                        SyntaxCallTypeApplicationSpelling::Turbofish => {
                            HirCallTypeApplicationSpelling::Turbofish
                        }
                    },
                    arguments.into_boxed_slice(),
                    match application.terminator() {
                        SyntaxCallTypeApplicationTerminator::Closed => {
                            HirCallTypeApplicationTerminator::Closed
                        }
                        SyntaxCallTypeApplicationTerminator::RecoveredMissing => {
                            HirCallTypeApplicationTerminator::RecoveredMissing
                        }
                        SyntaxCallTypeApplicationTerminator::InvalidPresent => {
                            HirCallTypeApplicationTerminator::InvalidPresent
                        }
                    },
                )
            }
        };
        if !types.is_empty() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        let mut arguments = Vec::with_capacity(call.arguments().len());
        let mut argument_states = Vec::with_capacity(call.arguments().len());
        for (position, projection) in call.arguments().iter().enumerate() {
            HirCallArgumentOrdinal::try_new(position)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let syntax_ordinal =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let child = self.lower_required_candidate_call_child(
                expression_projection,
                &mut children,
                ExpressionComponentRole::CallArgument {
                    argument: syntax_ordinal,
                    part: SyntaxCallArgumentPart::Value,
                },
                scope,
                cursor,
            )?;
            argument_states.push(child.state);
            let value = candidate_call_value(child);
            arguments.push(match projection {
                SyntaxCallArgumentProjection::Positional { .. } => {
                    HirCallArgument::Positional { value }
                }
                SyntaxCallArgumentProjection::Named {
                    name: source_name,
                    equals,
                    ..
                } => {
                    let call_name = match source_name {
                        Ok(source_name) => HirRecoveredName::Valid(name(source_name)?),
                        Err(SyntaxNameIssue::Missing) => HirRecoveredName::Missing,
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            HirRecoveredName::InvalidPresent
                        }
                    };
                    HirCallArgument::Named {
                        name: call_name,
                        equals: required_token_state(*equals),
                        value,
                    }
                }
                SyntaxCallArgumentProjection::Spread { ellipsis, .. } => HirCallArgument::Spread {
                    value,
                    ellipsis: required_token_state(*ellipsis),
                },
            });
        }
        if !children.is_empty() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }

        build_candidate_call(
            callee,
            callee_state,
            explicit_type_application,
            arguments.into_boxed_slice(),
            call.terminator(),
            &argument_states,
            &type_argument_states,
        )
    }

    fn lower_required_candidate_call_child(
        &mut self,
        projection: &ExpressionProjection,
        children: &mut BTreeMap<ExpressionComponentRole, AttachedCandidateExpressionChild<'_>>,
        role: ExpressionComponentRole,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<LoweredCandidateCallChild, HirLowerFailure> {
        let child = required_call_child(children, role)?;
        let source_role = expression_component_role(projection, role)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        match child {
            AttachedCandidateExpressionChild::Authored { node, .. }
            | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                let expression = self.lower_candidate_expression(node, scope, cursor)?;
                Ok(LoweredCandidateCallChild {
                    expression,
                    missing: false,
                    state: if self.staged_expression_is_poisoned(expression)? {
                        HirCallChildPoison::Poisoned
                    } else {
                        HirCallChildPoison::Clean
                    },
                })
            }
            AttachedCandidateExpressionChild::Missing { source, .. } => {
                let expression =
                    self.lower_missing_candidate_expression(scope, cursor, source_role, &source)?;
                Ok(LoweredCandidateCallChild {
                    expression,
                    missing: true,
                    state: HirCallChildPoison::Poisoned,
                })
            }
        }
    }

    fn lower_required_candidate_call_type(
        &mut self,
        types: &mut BTreeMap<SyntaxCallTypeChildRole, AttachedCandidateTypeRoot<'_>>,
        role: SyntaxCallTypeChildRole,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<LoweredCandidateCallType, HirLowerFailure> {
        let root = required_call_type(types, role)?;
        let ty = self.lower_candidate_type(root.node(), scope, cursor)?;
        let state = if self
            .arenas
            .types()
            .resolve_staged(&self.slots, ty)
            .map_err(HirLowerFailure::from)?
            .is_poisoned()
        {
            HirCallChildPoison::Poisoned
        } else {
            HirCallChildPoison::Clean
        };
        Ok(LoweredCandidateCallType { ty, state })
    }
}

fn candidate_call_children<'a>(
    node: AttachedCandidateNode<'a>,
) -> Result<BTreeMap<ExpressionComponentRole, AttachedCandidateExpressionChild<'a>>, HirLowerFailure>
{
    let mut children = BTreeMap::new();
    for child in node.semantic_expression_children() {
        if children.insert(child.component_role(), child).is_some() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
    }
    Ok(children)
}

fn candidate_call_types<'a>(
    node: AttachedCandidateNode<'a>,
) -> Result<BTreeMap<SyntaxCallTypeChildRole, AttachedCandidateTypeRoot<'a>>, HirLowerFailure> {
    let mut types = BTreeMap::new();
    for root in node.direct_semantic_type_roots() {
        if types.insert(root.role(), root).is_some() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
    }
    Ok(types)
}

fn required_call_child<'a>(
    children: &mut BTreeMap<ExpressionComponentRole, AttachedCandidateExpressionChild<'a>>,
    role: ExpressionComponentRole,
) -> Result<AttachedCandidateExpressionChild<'a>, HirLowerFailure> {
    children
        .remove(&role)
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}

fn required_call_type<'a>(
    children: &mut BTreeMap<SyntaxCallTypeChildRole, AttachedCandidateTypeRoot<'a>>,
    role: SyntaxCallTypeChildRole,
) -> Result<AttachedCandidateTypeRoot<'a>, HirLowerFailure> {
    children
        .remove(&role)
        .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
}

fn candidate_associated_receiver(child: LoweredCandidateCallType) -> HirAssociatedReceiver {
    if child.state == HirCallChildPoison::Poisoned {
        HirAssociatedReceiver::invalid_present(child.ty)
    } else {
        HirAssociatedReceiver::resolved(child.ty)
    }
}

fn candidate_call_value(child: LoweredCandidateCallChild) -> HirCallValue {
    if child.missing {
        HirCallValue::Missing {
            recovery: child.expression,
        }
    } else {
        HirCallValue::Present {
            value: child.expression,
        }
    }
}

fn required_token_state(state: SyntaxRequiredTokenState) -> HirRequiredTokenState {
    match state {
        SyntaxRequiredTokenState::Present => HirRequiredTokenState::Present,
        SyntaxRequiredTokenState::Missing => HirRequiredTokenState::Missing,
        SyntaxRequiredTokenState::InvalidPresent => HirRequiredTokenState::InvalidPresent,
    }
}

fn build_candidate_call(
    callee: HirCallCallee,
    callee_state: HirCallChildPoison,
    explicit_type_application: HirCallTypeApplication,
    arguments: Box<[HirCallArgument]>,
    terminator: SyntaxCallArgumentListTerminator,
    argument_states: &[HirCallChildPoison],
    type_argument_states: &[HirCallChildPoison],
) -> Result<(HirCallExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
    let (call, state) = HirCallExpr::try_new(
        callee,
        explicit_type_application,
        arguments,
        match terminator {
            SyntaxCallArgumentListTerminator::Closed => HirCallArgumentListTerminator::Closed,
            SyntaxCallArgumentListTerminator::RecoveredMissing => {
                HirCallArgumentListTerminator::RecoveredMissing
            }
        },
        HirCallChildStates::new(callee_state, argument_states, type_argument_states),
        false,
    )
    .map_err(|error| match error {
        HirCallBuildError::LimitExceeded { limit, observed } => HirLowerFailure::Limit(
            HirLimitError::with_maximum(limit, observed, limit.maximum()),
        ),
        HirCallBuildError::ChildStateShapeMismatch | HirCallBuildError::ChildIdentityMismatch => {
            HirInvariantFailure::InvalidArenaCommit.into()
        }
    })?;
    let recovery = match state {
        HirPoisonState::Clean => None,
        HirPoisonState::Poisoned(issue) => Some(issue),
    };
    Ok((call, recovery))
}
