//! Final expression-payload applicability for the sole typed source query.

use super::{
    HirCallArgumentSourcePart, HirCallTypeApplicationSourceRole, HirCallTypeArgumentSourcePart,
    HirClosureParameterSourcePart, HirDialogueNodeSourcePart, HirExprSourceRole,
    HirIdRefSourcePart, HirMatchArmSourcePart, HirRecordFieldSourcePart,
    HirRichTextArgumentSourcePart, HirRichTextTagSourcePart, HirSourceQueryError,
};
use crate::dialogue_application::{
    HirDialogueContentApplication, HirDialogueNodeKind, HirRichTextArgument,
};
use crate::expr::{
    HirCallArgument, HirCallCallee, HirCallTypeApplication, HirCallTypeApplicationSpelling,
    HirCallTypeApplicationTerminator, HirCallTypeArgument, HirExprKind, HirRecordField,
};
use crate::identity::ExprId;
use crate::leaf::{
    HirIdRef, HirIdRefShape, HirIdRefValue, HirLifetimePathValue, HirLiteral, HirPathValue,
};

impl HirExprKind {
    /// Rejects an inapplicable role or one-over ordinal from the resolved
    /// semantic family before the source manifest and source identity are read.
    #[allow(
        clippy::too_many_lines,
        reason = "the closed thirty-six-family expression source-role matrix is exhaustive"
    )]
    #[allow(
        clippy::match_same_arms,
        reason = "the exhaustive role matrix keeps independently named expression families visible at the typed boundary"
    )]
    pub(crate) fn validate_source_role(
        &self,
        owner: ExprId,
        role: HirExprSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        if role == HirExprSourceRole::Whole {
            return Ok(());
        }

        match self {
            Self::Unit => not_applicable(owner, role),
            Self::Literal(literal) => validate_literal_role(owner, role, literal),
            Self::EntityReference(reference) => validate_id_ref_role(owner, role, reference),
            Self::LifetimePath(path) => validate_lifetime_path_role(owner, role, path),
            Self::Path(path) => validate_path_role(owner, role, path),
            Self::ShortVariant(_) => {
                admit(owner, role, role == HirExprSourceRole::ShortVariantName)
            }
            Self::Placeholder(_) => {
                admit(owner, role, role == HirExprSourceRole::PlaceholderMarker)
            }
            Self::Tuple(expression) => match role {
                HirExprSourceRole::Element { ordinal } => {
                    validate_ordinal(owner, role, ordinal, expression.elements().len())
                }
                _ => not_applicable(owner, role),
            },
            Self::BracketSequence(expression) => match role {
                HirExprSourceRole::Element { ordinal } => {
                    validate_ordinal(owner, role, ordinal, expression.elements().len())
                }
                _ => not_applicable(owner, role),
            },
            Self::NumericBracketSequence(sequence) => match role {
                HirExprSourceRole::NumericElement { ordinal } => {
                    validate_ordinal(owner, role, ordinal, sequence.source_element_count())
                }
                HirExprSourceRole::NumericCommonSuffix => Ok(()),
                _ => not_applicable(owner, role),
            },
            Self::ArrayRepeat(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::RepeatValue | HirExprSourceRole::RepeatLength
                ),
            ),
            Self::Call(expression) => match role {
                HirExprSourceRole::CallCallee
                    if matches!(expression.callee(), HirCallCallee::Value { .. }) =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallAssociatedReceiver
                | HirExprSourceRole::CallAssociatedSeparator
                | HirExprSourceRole::CallAssociatedMember
                    if matches!(
                        expression.callee(),
                        HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. }
                    ) =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallArgumentListOpen => Ok(()),
                HirExprSourceRole::CallArgumentListClose
                    if matches!(
                        expression.terminator(),
                        crate::expr::HirCallArgumentListTerminator::Closed
                    ) =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallArgumentListRecoveryEnd
                    if matches!(
                        expression.terminator(),
                        crate::expr::HirCallArgumentListTerminator::RecoveredMissing
                    ) =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallArgumentListEmptyInsertion
                    if expression.arguments().is_empty() =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallArgumentSeparator { following }
                    if usize::from(following.get()) > 0
                        && usize::from(following.get()) < expression.arguments().len() =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallArgumentTrailingSeparator
                    if !expression.arguments().is_empty() =>
                {
                    Ok(())
                }
                HirExprSourceRole::CallArgument { argument, part } => validate_call_argument(
                    owner,
                    role,
                    usize::from(argument.get()),
                    part,
                    expression.arguments(),
                ),
                HirExprSourceRole::CallTypeApplication(type_role) => {
                    validate_call_type_application(
                        owner,
                        role,
                        type_role,
                        expression.explicit_type_application(),
                    )
                }
                _ => not_applicable(owner, role),
            },
            Self::Select(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::Target | HirExprSourceRole::SelectedMember
                ),
            ),
            Self::Index(_) => admit(
                owner,
                role,
                matches!(role, HirExprSourceRole::Target | HirExprSourceRole::Index),
            ),
            Self::Pipe(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::LeftOperand | HirExprSourceRole::RightOperand
                ),
            ),
            Self::Try(_) | Self::Await(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::Operand | HirExprSourceRole::Operator
                ),
            ),
            Self::Thread(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::ThreadModifier | HirExprSourceRole::ThreadName
                ),
            ),
            // Choice interior coordinates belong to its specialized attached
            // owner. Generic expression queries retain only `Whole`, handled
            // before this family match.
            Self::Choice(_) => not_applicable(owner, role),
            Self::Range(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::RangeStart
                        | HirExprSourceRole::RangeEnd
                        | HirExprSourceRole::RangeInclusiveMarker
                ),
            ),
            Self::Record(expression) => match role {
                HirExprSourceRole::RecordPath => Ok(()),
                HirExprSourceRole::RecordField { field, part } => {
                    validate_record_field(owner, role, field, part, expression.fields())
                }
                _ => not_applicable(owner, role),
            },
            Self::RecordLiteral(expression) => match role {
                HirExprSourceRole::RecordField { field, part } => {
                    validate_record_field(owner, role, field, part, expression.fields())
                }
                _ => not_applicable(owner, role),
            },
            Self::Binary(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::LeftOperand
                        | HirExprSourceRole::Operator
                        | HirExprSourceRole::RightOperand
                ),
            ),
            Self::Borrow(_) | Self::Dereference(_) | Self::Unary(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::Operator | HirExprSourceRole::Operand
                ),
            ),
            Self::Closure(expression) => match role {
                HirExprSourceRole::ClosureParameter { parameter, part } => {
                    let Some(parameter) = expression.parameters().get(parameter as usize) else {
                        return ordinal_out_of_bounds(owner, role, expression.parameters().len());
                    };
                    validate_closure_parameter_part(owner, role, part, parameter.ty().is_some())
                }
                HirExprSourceRole::ReturnType | HirExprSourceRole::Body => Ok(()),
                _ => not_applicable(owner, role),
            },
            Self::Block(expression) => {
                validate_block_role(owner, role, expression.statements().len(), false)
            }
            Self::ComputationBlock(expression) => {
                validate_block_role(owner, role, expression.statements().len(), false)
            }
            Self::NamedBlock(expression) => {
                validate_block_role(owner, role, expression.statements().len(), true)
            }
            Self::If(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::Condition
                        | HirExprSourceRole::ThenBranch
                        | HirExprSourceRole::ElseBranch
                ),
            ),
            Self::IfLet(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::Pattern
                        | HirExprSourceRole::Scrutinee
                        | HirExprSourceRole::Guard
                        | HirExprSourceRole::ThenBranch
                        | HirExprSourceRole::ElseBranch
                ),
            ),
            Self::Match(expression) => match role {
                HirExprSourceRole::Scrutinee => Ok(()),
                HirExprSourceRole::MatchArm { arm, part } => {
                    let Some(arm_payload) = expression.arms().get(arm as usize) else {
                        return ordinal_out_of_bounds(owner, role, expression.arms().len());
                    };
                    validate_match_arm_part(owner, role, part, arm_payload.guard().is_some())
                }
                _ => not_applicable(owner, role),
            },
            Self::ForSynthetic(_) => not_applicable(owner, role),
            Self::DialogueContentApplication(expression) => {
                validate_dialogue_application_role(owner, role, expression)
            }
            Self::PostfixBracket(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirExprSourceRole::Target
                        | HirExprSourceRole::OpenBracket
                        | HirExprSourceRole::CloseBracket
                        | HirExprSourceRole::Content
                ),
            ),
            Self::Error(_) => admit(owner, role, role == HirExprSourceRole::Recovery),
        }
    }

    /// Validates a role whose applicability can depend on another expression
    /// in the same module arena.
    ///
    /// Dialogue configuration coordinates are sparse: absence from that list
    /// means "not a configuration coordinate", while the authored argument
    /// boundary belongs to the target ordinary call. The module resolves that
    /// target and supplies its exact argument count before the sparse payload
    /// check runs.
    pub(crate) fn validate_source_role_with_context(
        &self,
        owner: ExprId,
        role: HirExprSourceRole,
        target_call_argument_count: Option<usize>,
    ) -> Result<(), HirSourceQueryError> {
        if let (
            Self::DialogueContentApplication(_),
            HirExprSourceRole::ConfigurationArgument { argument, .. },
        ) = (self, role)
        {
            let length = target_call_argument_count.unwrap_or(0);
            if usize::from(argument.get()) >= length {
                return ordinal_out_of_bounds(owner, role, length);
            }
        }
        self.validate_source_role(owner, role)
    }
}

fn validate_call_type_application(
    owner: ExprId,
    role: HirExprSourceRole,
    type_role: HirCallTypeApplicationSourceRole,
    application: &HirCallTypeApplication,
) -> Result<(), HirSourceQueryError> {
    let HirCallTypeApplication::Present {
        spelling,
        arguments,
        terminator,
    } = application
    else {
        return not_applicable(owner, role);
    };
    match type_role {
        HirCallTypeApplicationSourceRole::Whole | HirCallTypeApplicationSourceRole::OpenAngle => {
            Ok(())
        }
        HirCallTypeApplicationSourceRole::TurbofishSeparator
            if *spelling == HirCallTypeApplicationSpelling::Turbofish =>
        {
            Ok(())
        }
        HirCallTypeApplicationSourceRole::CloseAngle
            if matches!(
                terminator,
                HirCallTypeApplicationTerminator::Closed
                    | HirCallTypeApplicationTerminator::InvalidPresent
            ) =>
        {
            Ok(())
        }
        HirCallTypeApplicationSourceRole::RecoveryEnd
            if *terminator == HirCallTypeApplicationTerminator::RecoveredMissing =>
        {
            Ok(())
        }
        HirCallTypeApplicationSourceRole::EmptyInsertion
            if arguments.len() == 1 && matches!(arguments[0], HirCallTypeArgument::Missing) =>
        {
            Ok(())
        }
        HirCallTypeApplicationSourceRole::Argument { argument, part } => {
            let ordinal = usize::from(argument.get());
            let Some(argument) = arguments.get(ordinal) else {
                return ordinal_out_of_bounds(owner, role, arguments.len());
            };
            admit(
                owner,
                role,
                matches!(
                    part,
                    HirCallTypeArgumentSourcePart::Whole | HirCallTypeArgumentSourcePart::Type
                ) && matches!(
                    argument,
                    HirCallTypeArgument::Resolved { .. }
                        | HirCallTypeArgument::InvalidPresent { .. }
                        | HirCallTypeArgument::Missing
                ),
            )
        }
        HirCallTypeApplicationSourceRole::Separator { following }
            if usize::from(following.get()) > 0
                && usize::from(following.get()) < arguments.len() =>
        {
            Ok(())
        }
        HirCallTypeApplicationSourceRole::TrailingSeparator if !arguments.is_empty() => Ok(()),
        _ => not_applicable(owner, role),
    }
}

fn validate_literal_role(
    owner: ExprId,
    role: HirExprSourceRole,
    literal: &HirLiteral,
) -> Result<(), HirSourceQueryError> {
    let applicable = match role {
        HirExprSourceRole::LiteralBody => true,
        HirExprSourceRole::LiteralPrefix => {
            matches!(literal, HirLiteral::String(_) | HirLiteral::Integer(_))
        }
        HirExprSourceRole::LiteralSuffix => {
            matches!(
                literal,
                HirLiteral::Character(_) | HirLiteral::Integer(_) | HirLiteral::Float(_)
            )
        }
        HirExprSourceRole::LiteralUnit => {
            matches!(literal, HirLiteral::UnitNumber(_) | HirLiteral::Duration(_))
        }
        _ => false,
    };
    admit(owner, role, applicable)
}

fn validate_id_ref_role(
    owner: ExprId,
    role: HirExprSourceRole,
    reference: &HirIdRefValue,
) -> Result<(), HirSourceQueryError> {
    let HirExprSourceRole::EntityReference(part) = role else {
        return not_applicable(owner, role);
    };
    match (reference, part) {
        (_, HirIdRefSourcePart::Whole)
        | (HirIdRefValue::Resolved(HirIdRef::Absolute(_)), HirIdRefSourcePart::AbsoluteMarker)
        | (
            HirIdRefValue::Resolved(HirIdRef::FamilyRelative(_)),
            HirIdRefSourcePart::Family | HirIdRefSourcePart::FamilySeparator,
        ) => Ok(()),
        (
            HirIdRefValue::Resolved(HirIdRef::Absolute(reference)),
            HirIdRefSourcePart::SuffixSegment { ordinal },
        ) => validate_ordinal(owner, role, ordinal, reference.segment_count()),
        (
            HirIdRefValue::Resolved(HirIdRef::Relative(relative)),
            HirIdRefSourcePart::ParentMarker { ordinal },
        ) => validate_ordinal(owner, role, ordinal, relative.parent_depth()),
        (
            HirIdRefValue::Resolved(HirIdRef::Relative(relative)),
            HirIdRefSourcePart::SuffixSegment { ordinal },
        ) => validate_ordinal(owner, role, ordinal, relative.suffix().segment_count()),
        (
            HirIdRefValue::Resolved(HirIdRef::FamilyRelative(relative)),
            HirIdRefSourcePart::ParentMarker { ordinal },
        ) => validate_ordinal(owner, role, ordinal, relative.relative().parent_depth()),
        (
            HirIdRefValue::Resolved(HirIdRef::FamilyRelative(relative)),
            HirIdRefSourcePart::SuffixSegment { ordinal },
        ) => validate_ordinal(
            owner,
            role,
            ordinal,
            relative.relative().suffix().segment_count(),
        ),
        (HirIdRefValue::Recovered(recovery), HirIdRefSourcePart::AbsoluteMarker)
            if matches!(recovery.shape(), HirIdRefShape::Absolute { .. }) =>
        {
            Ok(())
        }
        (
            HirIdRefValue::Recovered(recovery),
            HirIdRefSourcePart::Family | HirIdRefSourcePart::FamilySeparator,
        ) if matches!(recovery.shape(), HirIdRefShape::FamilyRelative { .. }) => Ok(()),
        (HirIdRefValue::Recovered(recovery), HirIdRefSourcePart::ParentMarker { ordinal }) => {
            match recovery.shape() {
                HirIdRefShape::Relative { parent_depth, .. }
                | HirIdRefShape::FamilyRelative { parent_depth, .. } => {
                    validate_ordinal(owner, role, ordinal, parent_depth)
                }
                HirIdRefShape::Missing | HirIdRefShape::Absolute { .. } => {
                    not_applicable(owner, role)
                }
            }
        }
        (HirIdRefValue::Recovered(recovery), HirIdRefSourcePart::SuffixSegment { ordinal }) => {
            match recovery.shape() {
                HirIdRefShape::Absolute { segment_count } => validate_ordinal(
                    owner,
                    role,
                    ordinal,
                    usize::try_from(segment_count).expect("u32 ID segment count fits usize"),
                ),
                HirIdRefShape::Relative {
                    suffix_segment_count,
                    ..
                }
                | HirIdRefShape::FamilyRelative {
                    suffix_segment_count,
                    ..
                } => validate_ordinal(
                    owner,
                    role,
                    ordinal,
                    usize::try_from(suffix_segment_count).expect("u32 ID segment count fits usize"),
                ),
                HirIdRefShape::Missing => not_applicable(owner, role),
            }
        }
        _ => not_applicable(owner, role),
    }
}

fn validate_lifetime_path_role(
    owner: ExprId,
    role: HirExprSourceRole,
    path: &HirLifetimePathValue,
) -> Result<(), HirSourceQueryError> {
    match role {
        HirExprSourceRole::RegistryScope | HirExprSourceRole::OptionalMarker => Ok(()),
        HirExprSourceRole::RegistryKeySegment { ordinal } => {
            let length = match path {
                HirLifetimePathValue::Resolved(path) => path.segments().len(),
                HirLifetimePathValue::Recovered(recovery) => {
                    usize::try_from(recovery.segment_count())
                        .expect("u32 registry segment count fits usize")
                }
            };
            validate_ordinal(owner, role, ordinal, length)
        }
        _ => not_applicable(owner, role),
    }
}

fn validate_path_role(
    owner: ExprId,
    role: HirExprSourceRole,
    path: &HirPathValue,
) -> Result<(), HirSourceQueryError> {
    match role {
        HirExprSourceRole::PathRoot => Ok(()),
        HirExprSourceRole::PathSegment { ordinal } => {
            let length = match path {
                HirPathValue::Resolved(path) => path.segments().len(),
                HirPathValue::Recovered(recovery) => usize::try_from(recovery.segment_count())
                    .expect("u32 path segment count fits usize"),
            };
            validate_ordinal(owner, role, ordinal, length)
        }
        _ => not_applicable(owner, role),
    }
}

fn validate_call_argument(
    owner: ExprId,
    role: HirExprSourceRole,
    ordinal: usize,
    part: HirCallArgumentSourcePart,
    arguments: &[HirCallArgument],
) -> Result<(), HirSourceQueryError> {
    let Some(argument) = arguments.get(ordinal) else {
        return ordinal_out_of_bounds(owner, role, arguments.len());
    };
    let applicable = match argument {
        HirCallArgument::Positional { .. } => matches!(
            part,
            HirCallArgumentSourcePart::Whole | HirCallArgumentSourcePart::Value
        ),
        HirCallArgument::Named { .. } => matches!(
            part,
            HirCallArgumentSourcePart::Whole
                | HirCallArgumentSourcePart::Name
                | HirCallArgumentSourcePart::Equals
                | HirCallArgumentSourcePart::Value
        ),
        HirCallArgument::Spread { .. } => matches!(
            part,
            HirCallArgumentSourcePart::Whole
                | HirCallArgumentSourcePart::Value
                | HirCallArgumentSourcePart::Spread
        ),
    };
    admit(owner, role, applicable)
}

fn validate_record_field(
    owner: ExprId,
    role: HirExprSourceRole,
    ordinal: u32,
    part: HirRecordFieldSourcePart,
    fields: &[HirRecordField],
) -> Result<(), HirSourceQueryError> {
    let Some(field) = fields.get(ordinal as usize) else {
        return ordinal_out_of_bounds(owner, role, fields.len());
    };
    let applicable = match field {
        HirRecordField::Explicit { .. } => matches!(
            part,
            HirRecordFieldSourcePart::Whole
                | HirRecordFieldSourcePart::Name
                | HirRecordFieldSourcePart::Colon
                | HirRecordFieldSourcePart::Value
        ),
        HirRecordField::Shorthand { .. } => matches!(
            part,
            HirRecordFieldSourcePart::Whole | HirRecordFieldSourcePart::Name
        ),
        // The typed issue intentionally erases which source subparts were
        // recovered. The attached manifest remains the exact presence owner.
        HirRecordField::Invalid { .. } => true,
    };
    admit(owner, role, applicable)
}

fn validate_closure_parameter_part(
    owner: ExprId,
    role: HirExprSourceRole,
    part: HirClosureParameterSourcePart,
    _has_type: bool,
) -> Result<(), HirSourceQueryError> {
    // Colon and type are optional family components. The semantic payload
    // retains the type when present; the attached manifest retains authored or
    // recovered component presence.
    admit(
        owner,
        role,
        matches!(
            part,
            HirClosureParameterSourcePart::Whole
                | HirClosureParameterSourcePart::Pattern
                | HirClosureParameterSourcePart::Colon
                | HirClosureParameterSourcePart::Type
        ),
    )
}

fn validate_block_role(
    owner: ExprId,
    role: HirExprSourceRole,
    statement_len: usize,
    named: bool,
) -> Result<(), HirSourceQueryError> {
    match role {
        HirExprSourceRole::Statement { ordinal } => {
            validate_ordinal(owner, role, ordinal, statement_len)
        }
        HirExprSourceRole::Tail => Ok(()),
        HirExprSourceRole::Name if named => Ok(()),
        _ => not_applicable(owner, role),
    }
}

fn validate_match_arm_part(
    owner: ExprId,
    role: HirExprSourceRole,
    part: HirMatchArmSourcePart,
    _has_guard: bool,
) -> Result<(), HirSourceQueryError> {
    // Guard is an optional arm-family component. Exact presence remains in the
    // attached source manifest rather than being inferred from its HIR child.
    admit(
        owner,
        role,
        matches!(
            part,
            HirMatchArmSourcePart::Whole
                | HirMatchArmSourcePart::Pattern
                | HirMatchArmSourcePart::Guard
                | HirMatchArmSourcePart::Arrow
                | HirMatchArmSourcePart::Value
        ),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "Dialogue and RichText use one expression-owned closed component matrix"
)]
fn validate_dialogue_application_role(
    owner: ExprId,
    role: HirExprSourceRole,
    expression: &HirDialogueContentApplication,
) -> Result<(), HirSourceQueryError> {
    match role {
        HirExprSourceRole::Target
        | HirExprSourceRole::OpenBracket
        | HirExprSourceRole::CloseBracket
        | HirExprSourceRole::Colon
        | HirExprSourceRole::Content
        | HirExprSourceRole::ContentBody
        | HirExprSourceRole::Plan => Ok(()),
        HirExprSourceRole::ConfigurationArgument { argument, part } => {
            if expression
                .coordinates()
                .iter()
                .any(|coordinate| coordinate.argument() == argument)
            {
                return admit(
                    owner,
                    role,
                    matches!(
                        part,
                        HirCallArgumentSourcePart::Whole
                            | HirCallArgumentSourcePart::Name
                            | HirCallArgumentSourcePart::Value
                    ),
                );
            }
            // Coordinates are a sparse subset of the target Call's authored
            // arguments. Without resolving that target payload here, absence
            // proves only that this is not a configuration coordinate; it does
            // not prove an ordinal boundary.
            not_applicable(owner, role)
        }
        HirExprSourceRole::DialogueNode { ordinal, part } => {
            let Some(node) = expression.content().nodes().get(ordinal as usize) else {
                return ordinal_out_of_bounds(owner, role, expression.content().nodes().len());
            };
            let applicable = match node.kind() {
                HirDialogueNodeKind::Text(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole | HirDialogueNodeSourcePart::Text
                ),
                HirDialogueNodeKind::Raw(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole | HirDialogueNodeSourcePart::Raw
                ),
                HirDialogueNodeKind::Escape(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole | HirDialogueNodeSourcePart::Escape
                ),
                HirDialogueNodeKind::Ruby(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole
                        | HirDialogueNodeSourcePart::RubyBase
                        | HirDialogueNodeSourcePart::RubyText
                ),
                HirDialogueNodeKind::AuthoredStartTag(_)
                | HirDialogueNodeKind::InferredStartTag(_)
                | HirDialogueNodeKind::AuthoredEndTag(_)
                | HirDialogueNodeKind::InferredEndTag(_) => {
                    part == HirDialogueNodeSourcePart::Whole
                }
                HirDialogueNodeKind::Interpolation(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole | HirDialogueNodeSourcePart::Interpolation
                ),
                HirDialogueNodeKind::LineBreak(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole | HirDialogueNodeSourcePart::LineBreak
                ),
                HirDialogueNodeKind::Error(_) => matches!(
                    part,
                    HirDialogueNodeSourcePart::Whole | HirDialogueNodeSourcePart::Error
                ),
            };
            admit(owner, role, applicable)
        }
        HirExprSourceRole::RichTextTag { tag, part } => {
            let Some(tag_payload) = expression.content().tags().get(tag as usize) else {
                return ordinal_out_of_bounds(owner, role, expression.content().tags().len());
            };
            let inferred = expression.content().nodes().iter().any(|node| {
                matches!(
                    node.kind(),
                    HirDialogueNodeKind::InferredStartTag(id) if *id == tag_payload.id()
                )
            });
            let applicable = matches!(
                part,
                HirRichTextTagSourcePart::Whole
                    | HirRichTextTagSourcePart::OpenDelimiter
                    | HirRichTextTagSourcePart::Name
                    | HirRichTextTagSourcePart::Payload
                    | HirRichTextTagSourcePart::CloseDelimiter
                    | HirRichTextTagSourcePart::EndTag
            ) || inferred && part == HirRichTextTagSourcePart::InferenceInsertion;
            admit(owner, role, applicable)
        }
        HirExprSourceRole::RichTextArgument {
            tag,
            argument,
            part,
        } => {
            let Some(tag_payload) = expression.content().tags().get(tag as usize) else {
                return ordinal_out_of_bounds(owner, role, expression.content().tags().len());
            };
            let Some(argument_payload) = tag_payload.arguments().get(usize::from(argument)) else {
                return ordinal_out_of_bounds(owner, role, tag_payload.arguments().len());
            };
            let applicable = match argument_payload {
                HirRichTextArgument::Positional { .. } => matches!(
                    part,
                    HirRichTextArgumentSourcePart::Whole | HirRichTextArgumentSourcePart::Value
                ),
                HirRichTextArgument::Named { .. } | HirRichTextArgument::Invalid { .. } => true,
            };
            admit(owner, role, applicable)
        }
        _ => not_applicable(owner, role),
    }
}

fn validate_ordinal(
    owner: ExprId,
    role: HirExprSourceRole,
    ordinal: u32,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    if ordinal as usize >= length {
        ordinal_out_of_bounds(owner, role, length)
    } else {
        Ok(())
    }
}

fn ordinal_out_of_bounds(
    owner: ExprId,
    role: HirExprSourceRole,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    let length = u32::try_from(length)
        .expect("a failing u32 expression source ordinal proves the semantic length fits u32");
    Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
        owner,
        role,
        length,
    })
}

const fn admit(
    owner: ExprId,
    role: HirExprSourceRole,
    applicable: bool,
) -> Result<(), HirSourceQueryError> {
    if applicable {
        Ok(())
    } else {
        not_applicable(owner, role)
    }
}

const fn not_applicable(owner: ExprId, role: HirExprSourceRole) -> Result<(), HirSourceQueryError> {
    Err(HirSourceQueryError::ExprRoleNotApplicable { owner, role })
}
