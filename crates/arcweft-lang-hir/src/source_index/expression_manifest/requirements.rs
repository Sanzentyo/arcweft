//! Exact source-component requirements for final HIR expressions.

use std::collections::BTreeMap;

use arcweft_lang_syntax::expressions::{
    ExpressionProjection, SyntaxDialogueApplicationForm, SyntaxDialogueContentProjection,
    SyntaxDialogueNodeProjection, SyntaxRichTextArgumentProjection, SyntaxRichTextTagIdentity,
};
use arcweft_lang_syntax::id_ref::SyntaxIdRefPart;

use super::leaf::id_ref_source_shape;
use crate::dialogue_application::HirDialogueContentApplication;
use crate::expr::{
    HirCallArgument, HirCallArgumentListTerminator, HirCallArgumentOrdinal, HirCallCallee,
    HirCallTypeApplication, HirCallTypeApplicationSpelling, HirCallTypeApplicationTerminator,
    HirCallTypeArgument, HirCallTypeArgumentOrdinal, HirExprKind, HirGenericExprIssue,
    HirRecordField,
};
use crate::leaf::{HirIdRefShape, HirLifetimePathValue, HirLiteral, HirPathValue};
use crate::source_index::{
    HirCallArgumentSourcePart, HirCallTypeApplicationSourceRole, HirCallTypeArgumentSourcePart,
    HirClosureParameterSourcePart, HirDialogueNodeSourcePart, HirExprSourceRole,
    HirIdRefSourcePart, HirMatchArmSourcePart, HirRecordFieldSourcePart,
    HirRichTextArgumentSourcePart, HirRichTextTagSourcePart, HirSourceRequirement,
};

#[allow(
    clippy::match_same_arms,
    clippy::too_many_lines,
    reason = "one exhaustive requirements matrix retains every expression family's exact mandatory and optional source roles"
)]
pub(super) fn expression_requirements(
    payload: &HirExprKind,
    projection: &ExpressionProjection,
) -> Option<BTreeMap<HirExprSourceRole, HirSourceRequirement>> {
    use HirSourceRequirement::{Optional, Required};

    let mut requirements = BTreeMap::new();
    match payload {
        HirExprKind::Unit => {}
        HirExprKind::Literal(literal) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::LiteralBody, Required);
            match literal {
                HirLiteral::String(_) => add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::LiteralPrefix,
                    Optional,
                ),
                HirLiteral::Integer(_) => {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::LiteralPrefix,
                        Optional,
                    );
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::LiteralSuffix,
                        Optional,
                    );
                }
                HirLiteral::Float(_) => add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::LiteralSuffix,
                    Optional,
                ),
                HirLiteral::UnitNumber(_) | HirLiteral::Duration(_) => add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::LiteralUnit,
                    Required,
                ),
                HirLiteral::Character(_) => add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::LiteralSuffix,
                    Required,
                ),
                HirLiteral::Boolean(_) => {}
            }
        }
        HirExprKind::EntityReference(reference) => {
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::EntityReference(HirIdRefSourcePart::Whole),
                Required,
            );
            match id_ref_source_shape(reference) {
                HirIdRefShape::Missing => {}
                HirIdRefShape::Absolute { segment_count } => {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::EntityReference(HirIdRefSourcePart::AbsoluteMarker),
                        Required,
                    );
                    add_entity_segments(&mut requirements, segment_count, Required);
                }
                HirIdRefShape::Relative {
                    parent_depth,
                    suffix_segment_count,
                } => {
                    add_entity_parents(&mut requirements, parent_depth, Required);
                    add_entity_segments(&mut requirements, suffix_segment_count, Required);
                }
                HirIdRefShape::FamilyRelative {
                    parent_depth,
                    suffix_segment_count,
                } => {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::EntityReference(HirIdRefSourcePart::Family),
                        Required,
                    );
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::EntityReference(HirIdRefSourcePart::FamilySeparator),
                        Required,
                    );
                    add_entity_parents(&mut requirements, parent_depth, Required);
                    add_entity_segments(&mut requirements, suffix_segment_count, Required);
                }
            }
        }
        HirExprKind::LifetimePath(path) => {
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RegistryScope,
                Required,
            );
            let (segments, optional) = match path {
                HirLifetimePathValue::Resolved(path) => (path.segments().len(), path.optional()),
                HirLifetimePathValue::Recovered(recovery) => (
                    usize::try_from(recovery.segment_count())
                        .expect("u32 registry segment count fits usize"),
                    recovery.optional_marker(),
                ),
            };
            add_indexed_expression_requirements(
                &mut requirements,
                segments,
                |ordinal| HirExprSourceRole::RegistryKeySegment { ordinal },
                Required,
            );
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::OptionalMarker,
                if optional { Required } else { Optional },
            );
        }
        HirExprKind::Path(path) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::PathRoot, Optional);
            let segments = match path {
                HirPathValue::Resolved(path) => path.segments().len(),
                HirPathValue::Recovered(recovery) => usize::try_from(recovery.segment_count())
                    .expect("u32 path segment count fits usize"),
            };
            add_indexed_expression_requirements(
                &mut requirements,
                segments,
                |ordinal| HirExprSourceRole::PathSegment { ordinal },
                Required,
            );
        }
        HirExprKind::ShortVariant(_) => add_expression_requirement(
            &mut requirements,
            HirExprSourceRole::ShortVariantName,
            Required,
        ),
        HirExprKind::Placeholder(_) => add_expression_requirement(
            &mut requirements,
            HirExprSourceRole::PlaceholderMarker,
            Required,
        ),
        HirExprKind::Tuple(expression) => add_indexed_expression_requirements(
            &mut requirements,
            expression.elements().len(),
            |ordinal| HirExprSourceRole::Element { ordinal },
            Required,
        ),
        HirExprKind::BracketSequence(expression) => add_indexed_expression_requirements(
            &mut requirements,
            expression.elements().len(),
            |ordinal| HirExprSourceRole::Element { ordinal },
            Required,
        ),
        HirExprKind::NumericBracketSequence(sequence) => {
            add_indexed_expression_requirements(
                &mut requirements,
                sequence.source_element_count(),
                |ordinal| HirExprSourceRole::NumericElement { ordinal },
                Required,
            );
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::NumericCommonSuffix,
                if sequence.common_suffix().is_some() {
                    Required
                } else {
                    Optional
                },
            );
        }
        HirExprKind::ArrayRepeat(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::RepeatValue, Required);
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RepeatLength,
                Required,
            );
        }
        HirExprKind::Call(expression) => {
            match expression.callee() {
                HirCallCallee::Value { .. } => add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::CallCallee,
                    Required,
                ),
                HirCallCallee::UnresolvedDot { .. } | HirCallCallee::Associated { .. } => {
                    for role in [
                        HirExprSourceRole::CallAssociatedReceiver,
                        HirExprSourceRole::CallAssociatedSeparator,
                        HirExprSourceRole::CallAssociatedMember,
                    ] {
                        add_expression_requirement(&mut requirements, role, Required);
                    }
                }
            }
            if let HirCallTypeApplication::Present {
                spelling,
                arguments,
                terminator,
            } = expression.explicit_type_application()
            {
                for role in [
                    HirCallTypeApplicationSourceRole::Whole,
                    HirCallTypeApplicationSourceRole::OpenAngle,
                ] {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::CallTypeApplication(role),
                        Required,
                    );
                }
                if *spelling == HirCallTypeApplicationSpelling::Turbofish {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::CallTypeApplication(
                            HirCallTypeApplicationSourceRole::TurbofishSeparator,
                        ),
                        Required,
                    );
                }
                add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::CallTypeApplication(match terminator {
                        HirCallTypeApplicationTerminator::Closed
                        | HirCallTypeApplicationTerminator::InvalidPresent => {
                            HirCallTypeApplicationSourceRole::CloseAngle
                        }
                        HirCallTypeApplicationTerminator::RecoveredMissing => {
                            HirCallTypeApplicationSourceRole::RecoveryEnd
                        }
                    }),
                    Required,
                );
                if arguments.len() == 1 && matches!(arguments[0], HirCallTypeArgument::Missing) {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::CallTypeApplication(
                            HirCallTypeApplicationSourceRole::EmptyInsertion,
                        ),
                        Optional,
                    );
                }
                for (position, _) in arguments.iter().enumerate() {
                    let argument = HirCallTypeArgumentOrdinal::try_new(position)
                        .expect("Call constructor preflight retains bounded type ordinals");
                    for part in [
                        HirCallTypeArgumentSourcePart::Whole,
                        HirCallTypeArgumentSourcePart::Type,
                    ] {
                        add_expression_requirement(
                            &mut requirements,
                            HirExprSourceRole::CallTypeApplication(
                                HirCallTypeApplicationSourceRole::Argument { argument, part },
                            ),
                            Required,
                        );
                    }
                    if position > 0 {
                        add_expression_requirement(
                            &mut requirements,
                            HirExprSourceRole::CallTypeApplication(
                                HirCallTypeApplicationSourceRole::Separator {
                                    following: argument,
                                },
                            ),
                            Required,
                        );
                    }
                }
                add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::CallTypeApplication(
                        HirCallTypeApplicationSourceRole::TrailingSeparator,
                    ),
                    Optional,
                );
            }
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::CallArgumentListOpen,
                Required,
            );
            add_expression_requirement(
                &mut requirements,
                match expression.terminator() {
                    HirCallArgumentListTerminator::Closed => {
                        HirExprSourceRole::CallArgumentListClose
                    }
                    HirCallArgumentListTerminator::RecoveredMissing => {
                        HirExprSourceRole::CallArgumentListRecoveryEnd
                    }
                },
                Required,
            );
            if expression.arguments().is_empty() {
                add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::CallArgumentListEmptyInsertion,
                    Required,
                );
            }
            for (position, argument) in expression.arguments().iter().enumerate() {
                let argument_ordinal = HirCallArgumentOrdinal::try_new(position)
                    .expect("Call constructor preflight retains bounded argument ordinals");
                let mut add_part = |part| {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::CallArgument {
                            argument: argument_ordinal,
                            part,
                        },
                        Required,
                    );
                };
                add_part(HirCallArgumentSourcePart::Whole);
                add_part(HirCallArgumentSourcePart::Value);
                match argument {
                    HirCallArgument::Positional { .. } => {}
                    HirCallArgument::Named { .. } => {
                        add_part(HirCallArgumentSourcePart::Name);
                        add_part(HirCallArgumentSourcePart::Equals);
                    }
                    HirCallArgument::Spread { .. } => {
                        add_part(HirCallArgumentSourcePart::Spread);
                    }
                }
                if position > 0 {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::CallArgumentSeparator {
                            following: argument_ordinal,
                        },
                        Required,
                    );
                }
            }
            if !expression.arguments().is_empty() {
                add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::CallArgumentTrailingSeparator,
                    Optional,
                );
            }
        }
        HirExprKind::Select(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Target, Required);
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::SelectedMember,
                Required,
            );
        }
        HirExprKind::Index(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Target, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::Index, Required);
        }
        HirExprKind::DialogueContentApplication(application) => {
            let ExpressionProjection::DialogueContentApplication(projection) = projection else {
                return None;
            };
            add_expression_requirement(&mut requirements, HirExprSourceRole::Target, Required);
            match projection.form() {
                SyntaxDialogueApplicationForm::Bracket { .. } => {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::OpenBracket,
                        Required,
                    );
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::CloseBracket,
                        Required,
                    );
                }
                SyntaxDialogueApplicationForm::Colon => add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::Colon,
                    Required,
                ),
            }
            add_expression_requirement(&mut requirements, HirExprSourceRole::Content, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::ContentBody, Required);
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::Plan,
                if projection.has_plan() {
                    Required
                } else {
                    Optional
                },
            );
            for coordinate in application.coordinates() {
                for part in [
                    HirCallArgumentSourcePart::Whole,
                    HirCallArgumentSourcePart::Name,
                    HirCallArgumentSourcePart::Value,
                ] {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::ConfigurationArgument {
                            argument: coordinate.argument(),
                            part,
                        },
                        Required,
                    );
                }
            }
            add_dialogue_content_requirements(&mut requirements, projection.content());
        }
        HirExprKind::PostfixBracket(_) => {
            if !matches!(projection, ExpressionProjection::PostfixBracket(_)) {
                return None;
            }
            for role in [
                HirExprSourceRole::Target,
                HirExprSourceRole::OpenBracket,
                HirExprSourceRole::CloseBracket,
                HirExprSourceRole::Content,
            ] {
                add_expression_requirement(&mut requirements, role, Required);
            }
        }
        HirExprKind::Pipe(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::LeftOperand, Required);
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RightOperand,
                Required,
            );
        }
        HirExprKind::Try(_)
        | HirExprKind::Await(_)
        | HirExprKind::Borrow(_)
        | HirExprKind::Dereference(_)
        | HirExprKind::Unary(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Operand, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::Operator, Required);
        }
        HirExprKind::Range(expression) => {
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RangeStart,
                if expression.start().is_some() {
                    Required
                } else {
                    Optional
                },
            );
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RangeEnd,
                if expression.end().is_some() {
                    Required
                } else {
                    Optional
                },
            );
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RangeInclusiveMarker,
                if expression.inclusive() {
                    Required
                } else {
                    Optional
                },
            );
        }
        HirExprKind::Record(expression) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::RecordPath, Required);
            add_record_field_requirements(&mut requirements, expression.fields());
        }
        HirExprKind::RecordLiteral(expression) => {
            add_record_field_requirements(&mut requirements, expression.fields());
        }
        HirExprKind::Closure(expression) => {
            for (parameter, value) in expression.parameters().iter().enumerate() {
                let parameter =
                    u32::try_from(parameter).expect("closure parameter ordinal fits u32");
                for part in [
                    HirClosureParameterSourcePart::Whole,
                    HirClosureParameterSourcePart::Pattern,
                ] {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::ClosureParameter { parameter, part },
                        Required,
                    );
                }
                for part in [
                    HirClosureParameterSourcePart::Colon,
                    HirClosureParameterSourcePart::Type,
                ] {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::ClosureParameter { parameter, part },
                        if value.ty().is_some() {
                            Required
                        } else {
                            Optional
                        },
                    );
                }
            }
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::ReturnType,
                if expression.result_type().is_some() {
                    Required
                } else {
                    Optional
                },
            );
            add_expression_requirement(&mut requirements, HirExprSourceRole::Body, Required);
        }
        HirExprKind::Binary(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::LeftOperand, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::Operator, Required);
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::RightOperand,
                Required,
            );
        }
        HirExprKind::If(_) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Condition, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::ThenBranch, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::ElseBranch, Required);
        }
        HirExprKind::IfLet(expression) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Pattern, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::Scrutinee, Required);
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::Guard,
                if expression.guard().is_some() {
                    Required
                } else {
                    Optional
                },
            );
            add_expression_requirement(&mut requirements, HirExprSourceRole::ThenBranch, Required);
            add_expression_requirement(&mut requirements, HirExprSourceRole::ElseBranch, Required);
        }
        HirExprKind::Match(expression) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Scrutinee, Required);
            for (arm, _) in expression.arms().iter().enumerate() {
                let arm = u32::try_from(arm).expect("bounded Match arm ordinal fits u32");
                for part in [
                    HirMatchArmSourcePart::Whole,
                    HirMatchArmSourcePart::Pattern,
                    HirMatchArmSourcePart::Arrow,
                    HirMatchArmSourcePart::Value,
                ] {
                    add_expression_requirement(
                        &mut requirements,
                        HirExprSourceRole::MatchArm { arm, part },
                        Required,
                    );
                }
                add_expression_requirement(
                    &mut requirements,
                    HirExprSourceRole::MatchArm {
                        arm,
                        part: HirMatchArmSourcePart::Guard,
                    },
                    Optional,
                );
            }
        }
        HirExprKind::Block(expression) => {
            add_value_block_requirements(&mut requirements, expression.statements().len());
        }
        HirExprKind::Loop(expression) => {
            add_value_block_requirements(&mut requirements, expression.statements().len());
        }
        HirExprKind::ComputationBlock(expression) => {
            add_value_block_requirements(&mut requirements, expression.statements().len());
        }
        HirExprKind::NamedBlock(expression) => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Name, Required);
            add_value_block_requirements(&mut requirements, expression.statements().len());
        }
        HirExprKind::Thread(_) => {
            let ExpressionProjection::Thread(thread) = projection else {
                return None;
            };
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::ThreadModifier,
                if thread.mode() == arcweft_lang_syntax::expressions::SyntaxThreadMode::Detached {
                    Required
                } else {
                    Optional
                },
            );
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::ThreadName,
                if thread.name().is_some() {
                    Required
                } else {
                    Optional
                },
            );
        }
        // Choice interior source is frozen by its specialized attached
        // relation. Whole remains on slot metadata, so no generic expression
        // component is declared here.
        HirExprKind::Choice(_) => {}
        HirExprKind::Error(error) if error.issue() == HirGenericExprIssue::UnclassifiedSyntax => {
            add_expression_requirement(&mut requirements, HirExprSourceRole::Recovery, Required);
        }
        _ => return None,
    }
    Some(requirements)
}

/// Re-derives the source-role contract for the Dialogue interpretation of an
/// ambiguous postfix bracket. The candidate keeps the final E33 payload but
/// borrows every source component from its source-backed outer E34 owner.
pub(super) fn candidate_dialogue_requirements(
    application: &HirDialogueContentApplication,
    content: &SyntaxDialogueContentProjection,
) -> BTreeMap<HirExprSourceRole, HirSourceRequirement> {
    use HirSourceRequirement::{Optional, Required};

    let mut requirements = BTreeMap::new();
    for role in [
        HirExprSourceRole::Target,
        HirExprSourceRole::OpenBracket,
        HirExprSourceRole::CloseBracket,
        HirExprSourceRole::Content,
        HirExprSourceRole::ContentBody,
    ] {
        add_expression_requirement(&mut requirements, role, Required);
    }
    add_expression_requirement(&mut requirements, HirExprSourceRole::Plan, Optional);
    for coordinate in application.coordinates() {
        for part in [
            HirCallArgumentSourcePart::Whole,
            HirCallArgumentSourcePart::Name,
            HirCallArgumentSourcePart::Value,
        ] {
            add_expression_requirement(
                &mut requirements,
                HirExprSourceRole::ConfigurationArgument {
                    argument: coordinate.argument(),
                    part,
                },
                Required,
            );
        }
    }
    add_dialogue_content_requirements(&mut requirements, content);
    requirements
}

fn add_dialogue_content_requirements(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    projection: &SyntaxDialogueContentProjection,
) {
    let SyntaxDialogueContentProjection::Present(content) = projection else {
        return;
    };
    for (ordinal, node) in content.nodes().iter().enumerate() {
        let ordinal = u32::try_from(ordinal).expect("bounded Dialogue node ordinal fits u32");
        for part in dialogue_node_source_parts(node) {
            add_expression_requirement(
                requirements,
                HirExprSourceRole::DialogueNode {
                    ordinal,
                    part: *part,
                },
                HirSourceRequirement::Required,
            );
        }
    }
    for (tag, projection) in content.tags().iter().enumerate() {
        let tag = u32::try_from(tag).expect("bounded RichText tag ordinal fits u32");
        for part in [
            HirRichTextTagSourcePart::Whole,
            HirRichTextTagSourcePart::OpenDelimiter,
            HirRichTextTagSourcePart::Name,
            HirRichTextTagSourcePart::Payload,
            HirRichTextTagSourcePart::CloseDelimiter,
        ] {
            add_expression_requirement(
                requirements,
                HirExprSourceRole::RichTextTag { tag, part },
                HirSourceRequirement::Required,
            );
        }
        if content.nodes().iter().any(|node| {
            matches!(
                node,
                SyntaxDialogueNodeProjection::InferredStartTag { tag: node_tag }
                    if *node_tag == tag
            )
        }) {
            add_expression_requirement(
                requirements,
                HirExprSourceRole::RichTextTag {
                    tag,
                    part: HirRichTextTagSourcePart::InferenceInsertion,
                },
                HirSourceRequirement::Required,
            );
        }
        add_expression_requirement(
            requirements,
            HirExprSourceRole::RichTextTag {
                tag,
                part: HirRichTextTagSourcePart::EndTag,
            },
            if projection.paired_end_node().is_some() {
                HirSourceRequirement::Required
            } else {
                HirSourceRequirement::Optional
            },
        );
        if let SyntaxRichTextTagIdentity::Marker(selector) = projection.identity() {
            for part in selector
                .components()
                .iter()
                .map(|component| component.part())
            {
                add_expression_requirement(
                    requirements,
                    HirExprSourceRole::RichTextTag {
                        tag,
                        part: HirRichTextTagSourcePart::Marker(hir_id_ref_source_part(part)),
                    },
                    HirSourceRequirement::Required,
                );
            }
        }
        for (argument, projection) in projection.arguments().iter().enumerate() {
            let argument =
                u16::try_from(argument).expect("bounded RichText argument ordinal fits u16");
            for part in rich_text_argument_source_parts(projection) {
                add_expression_requirement(
                    requirements,
                    HirExprSourceRole::RichTextArgument {
                        tag,
                        argument,
                        part,
                    },
                    HirSourceRequirement::Required,
                );
            }
        }
    }
}

fn hir_id_ref_source_part(part: SyntaxIdRefPart) -> HirIdRefSourcePart {
    match part {
        SyntaxIdRefPart::Whole => HirIdRefSourcePart::Whole,
        SyntaxIdRefPart::AbsoluteMarker => HirIdRefSourcePart::AbsoluteMarker,
        SyntaxIdRefPart::Family => HirIdRefSourcePart::Family,
        SyntaxIdRefPart::FamilySeparator => HirIdRefSourcePart::FamilySeparator,
        SyntaxIdRefPart::ParentMarker { ordinal } => HirIdRefSourcePart::ParentMarker { ordinal },
        SyntaxIdRefPart::SuffixSegment { ordinal } => HirIdRefSourcePart::SuffixSegment { ordinal },
    }
}

fn dialogue_node_source_parts(
    node: &SyntaxDialogueNodeProjection,
) -> &'static [HirDialogueNodeSourcePart] {
    use HirDialogueNodeSourcePart::{
        Error, Escape, Interpolation, LineBreak, Raw, RubyBase, RubyText, Text, Whole,
    };

    match node {
        SyntaxDialogueNodeProjection::Text(_) => &[Whole, Text],
        SyntaxDialogueNodeProjection::Raw(_) => &[Whole, Raw],
        SyntaxDialogueNodeProjection::Escape(_) => &[Whole, Escape],
        SyntaxDialogueNodeProjection::Ruby { .. } => &[Whole, RubyBase, RubyText],
        SyntaxDialogueNodeProjection::AuthoredStartTag { .. }
        | SyntaxDialogueNodeProjection::InferredStartTag { .. }
        | SyntaxDialogueNodeProjection::AuthoredEndTag(_)
        | SyntaxDialogueNodeProjection::InferredEndTag(_) => &[Whole],
        SyntaxDialogueNodeProjection::Interpolation(_) => &[Whole, Interpolation],
        SyntaxDialogueNodeProjection::LineBreak(_) => &[Whole, LineBreak],
        SyntaxDialogueNodeProjection::Error(_) => &[Whole, Error],
    }
}

fn rich_text_argument_source_parts(
    argument: &SyntaxRichTextArgumentProjection,
) -> Vec<HirRichTextArgumentSourcePart> {
    use HirRichTextArgumentSourcePart::{Equals, Name, Value, Whole};

    match argument {
        SyntaxRichTextArgumentProjection::Positional { .. } => vec![Whole, Value],
        SyntaxRichTextArgumentProjection::Named { .. } => vec![Whole, Name, Equals, Value],
        SyntaxRichTextArgumentProjection::Invalid { authored_parts, .. } => {
            let mut parts = vec![Whole];
            if authored_parts.has_name() {
                parts.push(Name);
            }
            if authored_parts.has_equals() {
                parts.push(Equals);
            }
            if authored_parts.has_value() {
                parts.push(Value);
            }
            parts
        }
    }
}

fn add_value_block_requirements(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    statement_count: usize,
) {
    add_indexed_expression_requirements(
        requirements,
        statement_count,
        |ordinal| HirExprSourceRole::Statement { ordinal },
        HirSourceRequirement::Required,
    );
    add_expression_requirement(
        requirements,
        HirExprSourceRole::Tail,
        HirSourceRequirement::Required,
    );
}

fn add_record_field_requirements(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    fields: &[HirRecordField],
) {
    for (field, value) in fields.iter().enumerate() {
        let field = u32::try_from(field).expect("record field ordinal fits u32");
        let mut add = |part| {
            add_expression_requirement(
                requirements,
                HirExprSourceRole::RecordField { field, part },
                HirSourceRequirement::Required,
            );
        };
        add(HirRecordFieldSourcePart::Whole);
        add(HirRecordFieldSourcePart::Name);
        match value {
            HirRecordField::Explicit { .. } | HirRecordField::Invalid { .. } => {
                add(HirRecordFieldSourcePart::Colon);
                add(HirRecordFieldSourcePart::Value);
            }
            HirRecordField::Shorthand { .. } => {}
        }
    }
}

fn add_entity_parents(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    count: usize,
    requirement: HirSourceRequirement,
) {
    add_indexed_expression_requirements(
        requirements,
        count,
        |ordinal| HirExprSourceRole::EntityReference(HirIdRefSourcePart::ParentMarker { ordinal }),
        requirement,
    );
}

fn add_entity_segments(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    count: u32,
    requirement: HirSourceRequirement,
) {
    add_indexed_expression_requirements(
        requirements,
        usize::try_from(count).expect("u32 entity segment count fits usize"),
        |ordinal| HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal }),
        requirement,
    );
}

fn add_indexed_expression_requirements(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    count: usize,
    role: impl Fn(u32) -> HirExprSourceRole,
    requirement: HirSourceRequirement,
) {
    for ordinal in 0..count {
        add_expression_requirement(
            requirements,
            role(u32::try_from(ordinal).expect("expression source ordinal fits u32")),
            requirement,
        );
    }
}

fn add_expression_requirement(
    requirements: &mut BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    role: HirExprSourceRole,
    requirement: HirSourceRequirement,
) {
    debug_assert!(requirements.insert(role, requirement).is_none());
}
