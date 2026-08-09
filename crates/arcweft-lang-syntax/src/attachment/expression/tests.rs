use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::{
    AttachedCandidateDialogueExpression, AttachedCandidateDialogueOwner,
    AttachedCandidateExpressionChild, AttachedExpressionChild, AttachedExpressionNode,
};
use crate::attachment::source_file::{AttachedPathRoot, AttachedPathSegmentKind};
use crate::attachment::{
    AttachmentFailure, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId,
    SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::expressions::{
    ExpressionComponentRole, ExpressionLiteralPart, ExpressionProjection,
    ExpressionRecordFieldPart, PendingExpressionComponent, PendingExpressionProjection,
    SyntaxAssociatedCallSyntax, SyntaxAssociatedSeparator, SyntaxCallArgumentPart,
    SyntaxCallCalleeProjection, SyntaxCallProjection, SyntaxCallTypeArgumentProjection,
    SyntaxCallTypeChildRole, SyntaxClosureSyntax, SyntaxComputationBlockKind, SyntaxExpressionSlot,
    SyntaxNumericSequenceRecovery, SyntaxPlaceholderKind, SyntaxRecordField, SyntaxSelectedMember,
};
use crate::grammar::build::{GrammarBuild, build_grammar};
use crate::grammar::event::SyntaxEvent;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::name::SyntaxNameIssue;
use crate::parser::{ParseOptions, parse_document};

#[path = "tests/candidate_control.rs"]
mod candidate_control;
#[path = "tests/control.rs"]
mod control;
#[path = "tests/match_expression.rs"]
mod match_expression;

fn document(text: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/attached-expression-test").unwrap(),
            SourceName::path("attached-expression-test.arcw"),
            text,
        )
        .unwrap(),
    )
}

fn attach_build(
    document: Arc<SourceDocument>,
    build: &GrammarBuild,
) -> Result<Arc<SyntaxSnapshotData>, AttachmentFailure> {
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(101).unwrap());
    let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
    let snapshot = SyntaxSnapshotId::new(
        lineage,
        SourceSnapshotId::initial(document.display_name().clone()),
    );
    let identities = build
        .index()
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                entry.path().clone(),
                SyntaxNodeId::new(
                    lineage,
                    NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    attach_typed_tree(
        build,
        &GrammarIdentityMap::new(identities),
        snapshot,
        document,
    )
}

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = document(text);
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    attach_build(document, &build).unwrap()
}

fn expression(source: &str, kind: SyntaxKind) -> AttachedExpressionNode {
    let source = format!("predicate leaf() = {source}\n");
    let snapshot = attach(&source);
    AttachedExpressionNode::from_syntax(
        snapshot
            .nodes()
            .find(|node| node.kind() == kind)
            .expect("requested expression family"),
    )
    .unwrap()
}

#[test]
fn attached_associated_calls_retain_the_parser_owned_type_receiver_once() {
    let dot = expression("Vec<I32>.with_capacity(8)", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(dot_projection)) =
        dot.projection()
    else {
        panic!("dot associated Call projection");
    };
    assert!(matches!(
        dot_projection.callee(),
        SyntaxCallCalleeProjection::UnresolvedDot { member: Ok(member), .. }
            if member.as_str() == "with_capacity"
    ));
    assert_eq!(dot.children().len(), 2);
    assert_eq!(dot.children()[0].ordinal(), 0);
    assert_eq!(dot.children()[1].ordinal(), 1);
    assert_eq!(
        dot.children()[0].component_role(),
        ExpressionComponentRole::CallAssociatedReceiver
    );
    assert_eq!(
        dot.children()[1].component_role(),
        ExpressionComponentRole::CallArgument {
            argument: 0,
            part: SyntaxCallArgumentPart::Value,
        }
    );
    let receiver = dot.children()[0]
        .authored_semantic()
        .unwrap()
        .expect("dot value receiver");
    assert!(receiver.path().is_none());
    assert!(matches!(
        receiver
            .nominal_path_type()
            .expect("dot receiver retains its nominal type")
            .value(),
        crate::types::TypeRef::Generic { .. }
    ));
    let [dot_type] = dot.call_type_children() else {
        panic!("dot Call owns one nominal receiver type relation");
    };
    assert_eq!(dot_type.role(), SyntaxCallTypeChildRole::DotNominalReceiver);
    assert_eq!(
        dot_type.node().whole_source_span().range(),
        dot.component(ExpressionComponentRole::CallAssociatedReceiver)
            .expect("dot receiver source")
            .range()
    );

    let explicit = expression("Vec<I32>::with_capacity(8)", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(explicit_projection)) =
        explicit.projection()
    else {
        panic!("explicit associated Call projection");
    };
    assert!(matches!(
        explicit_projection.callee(),
        SyntaxCallCalleeProjection::Associated {
            separator: SyntaxAssociatedSeparator::Present(
                SyntaxAssociatedCallSyntax::ExplicitDoubleColon,
            ),
            member: Ok(member),
            ..
        } if member.as_str() == "with_capacity"
    ));
    assert_eq!(explicit.children().len(), 1);
    assert_eq!(explicit.children()[0].ordinal(), 1);
    assert_eq!(
        explicit.children()[0].component_role(),
        ExpressionComponentRole::CallArgument {
            argument: 0,
            part: SyntaxCallArgumentPart::Value,
        }
    );
    let [explicit_type] = explicit.call_type_children() else {
        panic!("explicit Call owns one associated receiver type relation");
    };
    assert_eq!(
        explicit_type.role(),
        SyntaxCallTypeChildRole::AssociatedReceiver
    );
    assert_eq!(
        explicit_type.node().whole_source_span().range(),
        explicit
            .component(ExpressionComponentRole::CallAssociatedReceiver)
            .expect("explicit receiver source")
            .range()
    );
}

#[test]
fn attached_associated_call_recovery_requires_an_authored_separator() {
    let invalid_receiver = expression("Bad<>::member(x)", SyntaxKind::CallExpression);
    let [invalid_type] = invalid_receiver.call_type_children() else {
        panic!("invalid receiver retains one poisoned type root");
    };
    assert_eq!(
        invalid_type.role(),
        SyntaxCallTypeChildRole::AssociatedReceiver
    );
    assert!(matches!(
        invalid_type.node().value(),
        crate::types::TypeRef::Recovery(_)
    ));

    for (source, missing) in [("Vec<I32>. (8)", true), ("Vec<I32>.9bad(8)", false)] {
        let attached = expression(source, SyntaxKind::CallExpression);
        let member = attached
            .component(ExpressionComponentRole::CallAssociatedMember)
            .expect("recovered member component")
            .range();
        assert_eq!(member.is_empty(), missing, "{source}");
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed call-type-application and recovery matrix is easier to audit together"
)]
fn attached_call_type_applications_retain_ordered_type_children_and_recovery() {
    let direct = expression("value.collect<Vec<I32>>()", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(direct_call)) =
        direct.projection()
    else {
        panic!("direct-angle Call projection");
    };
    let direct_application = direct_call
        .explicit_type_application()
        .expect("direct-angle application");
    assert_eq!(
        direct_application.spelling(),
        crate::expressions::SyntaxCallTypeApplicationSpelling::DirectAngle
    );
    assert_eq!(
        direct_application.arguments(),
        &[SyntaxCallTypeArgumentProjection::Present]
    );
    let [direct_receiver, direct_type] = direct.call_type_children() else {
        panic!("direct-angle dot Call owns nominal evidence and one type argument");
    };
    assert_eq!(
        direct_receiver.role(),
        SyntaxCallTypeChildRole::DotNominalReceiver
    );
    assert_eq!(
        direct_type.role(),
        SyntaxCallTypeChildRole::ExplicitCallTypeArgument { ordinal: 0 }
    );

    let turbofish = expression("foo::<T>()", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(turbofish_call)) =
        turbofish.projection()
    else {
        panic!("turbofish Call projection");
    };
    assert_eq!(
        turbofish_call
            .explicit_type_application()
            .expect("turbofish application")
            .spelling(),
        crate::expressions::SyntaxCallTypeApplicationSpelling::Turbofish
    );
    assert_eq!(turbofish.call_type_children().len(), 1);

    let associated = expression("Vec<T>::member::<U>(x)", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(associated_call)) =
        associated.projection()
    else {
        panic!("associated member type-application projection");
    };
    assert!(matches!(
        associated_call.callee(),
        SyntaxCallCalleeProjection::Associated {
            separator: SyntaxAssociatedSeparator::Present(
                SyntaxAssociatedCallSyntax::ExplicitDoubleColon,
            ),
            member: Ok(member),
            ..
        } if member.as_str() == "member"
    ));
    let [receiver, member_argument] = associated.call_type_children() else {
        panic!("associated Call keeps receiver and member type argument distinct");
    };
    assert_eq!(receiver.role(), SyntaxCallTypeChildRole::AssociatedReceiver);
    assert_eq!(
        member_argument.role(),
        SyntaxCallTypeChildRole::ExplicitCallTypeArgument { ordinal: 0 }
    );

    let empty = expression("foo::<>()", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(empty_call)) =
        empty.projection()
    else {
        panic!("empty turbofish Call projection");
    };
    assert_eq!(
        empty_call
            .explicit_type_application()
            .expect("empty application")
            .arguments(),
        &[SyntaxCallTypeArgumentProjection::Missing]
    );
    assert!(empty.call_type_children().is_empty());

    let invalid = expression("foo::<9bad>()", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(invalid_call)) =
        invalid.projection()
    else {
        panic!("invalid turbofish Call projection");
    };
    assert_eq!(
        invalid_call
            .explicit_type_application()
            .expect("invalid application")
            .arguments(),
        &[SyntaxCallTypeArgumentProjection::InvalidPresent]
    );
    let [invalid_type] = invalid.call_type_children() else {
        panic!("invalid-present type argument retains one attached type child");
    };
    assert!(matches!(
        invalid_type.node().value(),
        crate::types::TypeRef::Recovery(_)
    ));

    let missing_close = expression("foo::<T()", SyntaxKind::CallExpression);
    let ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(missing_close_call)) =
        missing_close.projection()
    else {
        panic!("missing type-close Call projection");
    };
    assert_eq!(
        missing_close_call
            .explicit_type_application()
            .expect("recovered application")
            .terminator(),
        crate::expressions::SyntaxCallTypeApplicationTerminator::RecoveredMissing
    );
}

#[test]
fn bare_direct_angle_path_is_not_reclassified_as_a_call_type_application() {
    let snapshot = attach("predicate leaf() = foo<T>()\n");
    assert!(
        snapshot
            .nodes()
            .all(|node| node.kind() != SyntaxKind::CallExpression)
    );
}

#[test]
fn attached_leaf_matrix_exposes_typed_semantics_components_and_path_owner() {
    let unit = expression("()", SyntaxKind::TupleExpression);
    assert!(matches!(unit.projection(), ExpressionProjection::Unit));
    assert!(unit.components().is_empty());

    let literal = expression("42ms", SyntaxKind::LiteralExpression);
    assert!(matches!(
        literal.projection(),
        ExpressionProjection::Literal(_)
    ));
    assert!(
        literal
            .component(ExpressionComponentRole::Literal(
                ExpressionLiteralPart::Body,
            ))
            .is_some()
    );
    assert!(
        literal
            .component(ExpressionComponentRole::Literal(
                ExpressionLiteralPart::Unit,
            ))
            .is_some()
    );

    let entity = expression("@scene.entry", SyntaxKind::EntityReferenceExpression);
    assert_eq!(
        entity
            .component(ExpressionComponentRole::EntityReference(
                crate::id_ref::SyntaxIdRefPart::Whole,
            ))
            .unwrap()
            .range(),
        entity.syntax().range()
    );

    let lifetime = expression("'line.focus?", SyntaxKind::LifetimePathExpression);
    assert!(
        lifetime
            .component(ExpressionComponentRole::LifetimeOptionalMarker)
            .is_some()
    );
    assert!(
        lifetime
            .component(ExpressionComponentRole::LifetimeKeySegment { ordinal: 0 })
            .is_some()
    );

    let select = expression("game.actor", SyntaxKind::SelectExpression);
    assert!(matches!(
        select.projection(),
        ExpressionProjection::Select(crate::expressions::SyntaxSelectedMember::Name(member))
            if member.as_str() == "actor"
    ));
    let path = select.children()[0]
        .authored_semantic()
        .expect("Select target child")
        .expect("authored Path target");
    assert!(matches!(path.projection(), ExpressionProjection::Path));
    let path_owner = path.path().expect("Path marker selects attached Path");
    assert!(matches!(path_owner.root(), AttachedPathRoot::ImplicitCrate));
    assert_eq!(path_owner.segments().len(), 1);
    assert_eq!(path_owner.segments()[0].source_text(), "game");

    assert!(matches!(
        expression(".Ready", SyntaxKind::ShortVariantExpression).projection(),
        ExpressionProjection::ShortVariant(Ok(name)) if name.as_str() == "Ready"
    ));
    assert!(matches!(
        expression("^", SyntaxKind::PlaceholderExpression).projection(),
        ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PipeLeft)
    ));
}

#[test]
fn attached_known_leaf_recovery_remains_typed() {
    assert!(matches!(
        expression("@", SyntaxKind::EntityReferenceExpression).projection(),
        ExpressionProjection::EntityReference(entity)
            if matches!(entity.value(), Err(crate::id_ref::SyntaxIdRefIssue::MissingSuffix))
    ));
    assert!(matches!(
        expression("'line..focus", SyntaxKind::LifetimePathExpression).projection(),
        ExpressionProjection::LifetimePath(path) if path.has_recovery()
    ));
    assert!(matches!(
        expression(".", SyntaxKind::ShortVariantExpression).projection(),
        ExpressionProjection::ShortVariant(Err(crate::name::SyntaxNameIssue::Missing))
    ));
}

#[test]
fn attached_composite_slots_preserve_authored_and_missing_ordinals() {
    let tuple = expression("(left,,right)", SyntaxKind::TupleExpression);
    assert!(matches!(
        tuple.projection(),
        ExpressionProjection::Tuple(slots)
            if slots.as_ref()
                == [
                    SyntaxExpressionSlot::Authored,
                    SyntaxExpressionSlot::Missing,
                    SyntaxExpressionSlot::Authored,
                ]
    ));
    assert_eq!(tuple.children().len(), 3);
    assert!(matches!(
        &tuple.children()[1],
        AttachedExpressionChild::Missing {
            ordinal: 1,
            component_role: ExpressionComponentRole::Element { ordinal: 1 },
            recovery,
        }
            if recovery.source_span().range().is_empty()
    ));
    for child in tuple.children() {
        assert_eq!(
            tuple.component(child.component_role()).unwrap(),
            child.source_span()
        );
    }

    let empty = expression("[]", SyntaxKind::BracketSequenceExpression);
    assert!(matches!(
        empty.projection(),
        ExpressionProjection::BracketSequence(slots) if slots.is_empty()
    ));
    assert!(empty.children().is_empty());

    let bracket = expression("[value,,count]", SyntaxKind::BracketSequenceExpression);
    assert!(matches!(
        bracket.projection(),
        ExpressionProjection::BracketSequence(slots)
            if slots.as_ref()
                == [
                    SyntaxExpressionSlot::Authored,
                    SyntaxExpressionSlot::Missing,
                    SyntaxExpressionSlot::Authored,
                ]
    ));
    assert!(matches!(
        &bracket.children()[1],
        AttachedExpressionChild::Missing { ordinal: 1, .. }
    ));
}

#[test]
fn attached_e20_e21_records_keep_path_fields_and_explicit_value_slots() {
    let record = expression(
        "Point { x = value, y: , shorthand }",
        SyntaxKind::RecordExpression,
    );
    let ExpressionProjection::Record(fields) = record.projection() else {
        panic!("path-qualified record owns the E20 projection");
    };
    assert_eq!(fields.len(), 3);
    assert!(matches!(
        &fields[0],
        SyntaxRecordField::Explicit {
            name: Ok(name),
            value: SyntaxExpressionSlot::Authored,
        } if name.as_str() == "x"
    ));
    assert!(matches!(
        &fields[1],
        SyntaxRecordField::Explicit {
            name: Ok(name),
            value: SyntaxExpressionSlot::Missing,
        } if name.as_str() == "y"
    ));
    assert!(matches!(
        &fields[2],
        SyntaxRecordField::Shorthand { name: Ok(name) }
            if name.as_str() == "shorthand"
    ));
    assert_eq!(
        record
            .path()
            .expect("E20 retains the attached path owner")
            .segments()[0]
            .source_text(),
        "Point"
    );
    assert_eq!(record.children().len(), 2);
    assert!(matches!(
        &record.children()[0],
        AttachedExpressionChild::Authored {
            ordinal: 0,
            component_role: ExpressionComponentRole::RecordField {
                field: 0,
                part: ExpressionRecordFieldPart::Value,
            },
            expression,
            ..
        } if expression.source_text() == "value"
    ));
    assert!(matches!(
        &record.children()[1],
        AttachedExpressionChild::Missing {
            ordinal: 1,
            component_role: ExpressionComponentRole::RecordField {
                field: 1,
                part: ExpressionRecordFieldPart::Value,
            },
            recovery,
        }
            if recovery.source_span().range().is_empty()
    ));
    for part in [
        ExpressionRecordFieldPart::Whole,
        ExpressionRecordFieldPart::Name,
        ExpressionRecordFieldPart::Colon,
        ExpressionRecordFieldPart::Value,
    ] {
        assert!(
            record
                .component(ExpressionComponentRole::RecordField { field: 0, part })
                .is_some()
        );
    }
    assert!(
        record
            .component(ExpressionComponentRole::RecordField {
                field: 2,
                part: ExpressionRecordFieldPart::Colon,
            })
            .is_none()
    );

    let literal = expression(
        "{ first = value, second: }",
        SyntaxKind::RecordLiteralExpression,
    );
    assert!(literal.path().is_none());
    assert!(matches!(
        literal.projection(),
        ExpressionProjection::RecordLiteral(fields)
            if fields.len() == 2
                && matches!(
                    fields[1],
                    SyntaxRecordField::Explicit {
                        value: SyntaxExpressionSlot::Missing,
                        ..
                    }
                )
    ));
    assert_eq!(literal.children().len(), 2);
}

#[test]
fn grouped_composite_slots_keep_outer_source_and_inner_expression_identity() {
    let tuple = expression("(left,((right)),)", SyntaxKind::TupleExpression);
    let grouped = &tuple.children()[1];
    assert_eq!(grouped.ordinal(), 1);
    let inner = grouped
        .authored()
        .expect("grouped authored slot keeps its semantic identity");
    assert_eq!(inner.role(), SyntaxRole::Element(1));
    assert_eq!(inner.source_text(), "right");
    assert_eq!(
        grouped.source_span().range(),
        SourceRange::new(inner.range().start() - 2, inner.range().end() + 2)
    );
    assert_eq!(
        grouped
            .authored_semantic()
            .unwrap()
            .expect("authored semantic expression")
            .whole_source_span()
            .range(),
        inner.range()
    );

    let bracket = expression("[(left), right]", SyntaxKind::BracketSequenceExpression);
    let bracket_inner = bracket.children()[0]
        .authored()
        .expect("grouped bracket element");
    assert_eq!(bracket_inner.source_text(), "left");
    assert_eq!(
        bracket.children()[0].source_span().range(),
        SourceRange::new(
            bracket_inner.range().start() - 1,
            bracket_inner.range().end() + 1,
        )
    );

    let repeat = expression("[(value); (count)]", SyntaxKind::ArrayRepeatExpression);
    let value = repeat.children()[0]
        .authored()
        .expect("repeat value identity");
    let count = repeat.children()[1]
        .authored()
        .expect("repeat length identity");
    assert_eq!(value.source_text(), "value");
    assert_eq!(count.source_text(), "count");
    assert_eq!(
        repeat.children()[0].source_span().range(),
        SourceRange::new(value.range().start() - 1, value.range().end() + 1)
    );
    assert_eq!(
        repeat.children()[1].source_span().range(),
        SourceRange::new(count.range().start() - 1, count.range().end() + 1)
    );
}

#[test]
fn transparent_groups_forward_final_navigation_roles_only_to_semantic_expressions() {
    let let_snapshot = attach("fn main() { let local = (seed); }\n");
    let let_statement = let_snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::LetStatement)
        .expect("typed let statement");
    let initializer = let_statement.children_with_role(SyntaxRole::Initializer);
    assert_eq!(initializer.len(), 1);
    assert_eq!(initializer[0].kind(), SyntaxKind::PathExpression);
    assert_eq!(initializer[0].source_text(), "seed");
    assert!(let_statement.children().iter().all(|child| {
        !matches!(
            child.kind(),
            SyntaxKind::OpenParenNode | SyntaxKind::CloseParenNode
        ) || matches!(
            child.role(),
            SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter
        )
    }));

    for (source, parent_kind, role, expected) in [
        (
            "predicate leaf() = (f)(x)\n",
            SyntaxKind::CallExpression,
            SyntaxRole::Callee,
            "f",
        ),
        (
            "predicate leaf() = (value).member\n",
            SyntaxKind::SelectExpression,
            SyntaxRole::Target,
            "value",
        ),
        (
            "predicate leaf() = (left) + right\n",
            SyntaxKind::BinaryExpression,
            SyntaxRole::LeftOperand,
            "left",
        ),
    ] {
        let snapshot = attach(source);
        let parent = snapshot
            .nodes()
            .find(|node| node.kind() == parent_kind)
            .expect("Pratt parent expression");
        let children = parent.children_with_role(role);
        assert_eq!(children.len(), 1, "{source}");
        assert_eq!(children[0].kind(), SyntaxKind::PathExpression, "{source}");
        assert_eq!(children[0].source_text(), expected, "{source}");
        assert_ne!(children[0].role(), SyntaxRole::Operand, "{source}");
    }
}

#[test]
fn attached_select_owns_target_and_exact_missing_member_insertion() {
    let expression_base = "predicate leaf() = ".len();
    let selected = expression("target.member", SyntaxKind::SelectExpression);
    assert!(matches!(
        selected.projection(),
        ExpressionProjection::Select(crate::expressions::SyntaxSelectedMember::Name(member))
            if member.as_str() == "member"
    ));
    assert_eq!(selected.children().len(), 1);
    assert_eq!(selected.children()[0].ordinal(), 0);
    assert_eq!(
        selected
            .component(ExpressionComponentRole::SelectedMember)
            .expect("selected member component")
            .range(),
        SourceRange::new(expression_base + 7, expression_base + 13)
    );

    let missing = expression("target.   ", SyntaxKind::SelectExpression);
    assert!(matches!(
        missing.projection(),
        ExpressionProjection::Select(crate::expressions::SyntaxSelectedMember::Missing)
    ));
    assert_eq!(
        missing
            .component(ExpressionComponentRole::SelectedMember)
            .expect("missing member insertion")
            .range(),
        SourceRange::new(expression_base + 10, expression_base + 10)
    );

    let optional = expression("target?.member", SyntaxKind::SelectExpression);
    let target = optional.children()[0]
        .authored_semantic()
        .expect("attached Try target")
        .expect("authored Try target");
    assert!(matches!(
        target.projection(),
        ExpressionProjection::Try {
            form: crate::expressions::SyntaxTryForm::PostfixQuestion,
            ..
        }
    ));
}

#[test]
fn expression_paths_require_qualified_syntax_for_explicit_module_roots() {
    let selected = expression("self.value", SyntaxKind::SelectExpression);
    let value_target = selected.children()[0]
        .authored_semantic()
        .expect("Select target child")
        .expect("authored value Path");
    let value_path = value_target.path().expect("value Path owner");
    assert!(matches!(value_path.root(), AttachedPathRoot::ImplicitCrate));
    assert_eq!(value_path.segments().len(), 1);
    assert_eq!(value_path.segments()[0].source_text(), "self");

    let qualified = expression("self::value", SyntaxKind::PathExpression);
    let qualified_path = qualified.path().expect("qualified Path owner");
    assert!(matches!(
        qualified_path.root(),
        AttachedPathRoot::SelfModule { .. }
    ));
    assert_eq!(qualified_path.segments().len(), 1);
    assert_eq!(qualified_path.segments()[0].source_text(), "value");
}

#[test]
fn attached_numeric_sequence_owns_idless_elements_suffix_and_typed_recovery() {
    let complete = expression(
        "[0xff_u8, 2, 0b11_u8]",
        SyntaxKind::NumericBracketSequenceExpression,
    );
    let ExpressionProjection::NumericBracketSequence(sequence) = complete.projection() else {
        panic!("numeric kind must own its numeric projection");
    };
    assert_eq!(sequence.elements().len(), 3);
    assert_eq!(
        sequence.common_suffix(),
        Some(crate::literal::IntSuffix::U8)
    );
    assert!(matches!(
        sequence.recovery(),
        SyntaxNumericSequenceRecovery::Complete
    ));
    assert!(complete.children().is_empty());
    assert!(
        complete
            .component(ExpressionComponentRole::NumericCommonSuffix)
            .is_some()
    );
    assert_eq!(
        complete
            .components()
            .iter()
            .filter(|component| matches!(
                component.role(),
                ExpressionComponentRole::NumericElement { .. }
            ))
            .count(),
        3
    );

    let conflicting = expression("[1u8, 2u16]", SyntaxKind::NumericBracketSequenceExpression);
    assert!(matches!(
        conflicting.projection(),
        ExpressionProjection::NumericBracketSequence(sequence)
            if matches!(
                sequence.recovery(),
                SyntaxNumericSequenceRecovery::ConflictingSuffix {
                    ordinal: 1,
                    first: crate::literal::IntSuffix::U8,
                    conflicting: crate::literal::IntSuffix::U16,
                }
            )
    ));

    let missing = expression("[1u8,]", SyntaxKind::NumericBracketSequenceExpression);
    assert!(matches!(
        missing.projection(),
        ExpressionProjection::NumericBracketSequence(sequence)
            if matches!(
                sequence.recovery(),
                SyntaxNumericSequenceRecovery::MissingFinalElement { ordinal: 1 }
            )
    ));
    assert!(
        missing
            .component(ExpressionComponentRole::NumericElement { ordinal: 1 })
            .is_some_and(|source| source.range().is_empty())
    );

    assert!(matches!(
        expression("[1.5, 2.5]", SyntaxKind::BracketSequenceExpression).projection(),
        ExpressionProjection::BracketSequence(_)
    ));
}

#[test]
fn attached_array_repeat_keeps_fixed_value_and_length_slots() {
    for (source, expected) in [
        (
            "[value; count]",
            [
                SyntaxExpressionSlot::Authored,
                SyntaxExpressionSlot::Authored,
            ],
        ),
        (
            "[; count]",
            [
                SyntaxExpressionSlot::Missing,
                SyntaxExpressionSlot::Authored,
            ],
        ),
        (
            "[value;]",
            [
                SyntaxExpressionSlot::Authored,
                SyntaxExpressionSlot::Missing,
            ],
        ),
        (
            "[;]",
            [SyntaxExpressionSlot::Missing, SyntaxExpressionSlot::Missing],
        ),
    ] {
        let repeat = expression(source, SyntaxKind::ArrayRepeatExpression);
        assert!(matches!(
            repeat.projection(),
            ExpressionProjection::ArrayRepeat(slots) if *slots == expected
        ));
        assert_eq!(repeat.children().len(), 2);
        assert_eq!(
            repeat
                .component(ExpressionComponentRole::RepeatValue)
                .unwrap(),
            repeat.children()[0].source_span()
        );
        assert_eq!(
            repeat
                .component(ExpressionComponentRole::RepeatLength)
                .unwrap(),
            repeat.children()[1].source_span()
        );
    }
}

#[test]
fn attached_e14_through_e17_keep_exact_semantic_children_and_recovery_slots() {
    let index = expression("items[0]", SyntaxKind::PostfixBracketExpression);
    assert!(matches!(
        index.projection(),
        ExpressionProjection::Index(index)
            if index.target() == SyntaxExpressionSlot::Authored
                && index.index() == SyntaxExpressionSlot::Authored
    ));
    assert_eq!(index.children().len(), 2);
    assert_eq!(
        index.children()[0].authored().unwrap().source_text(),
        "items"
    );
    assert_eq!(index.children()[1].authored().unwrap().source_text(), "0");
    assert_eq!(
        index.component(ExpressionComponentRole::Target).unwrap(),
        index.children()[0].source_span()
    );
    assert_eq!(
        index.component(ExpressionComponentRole::Index).unwrap(),
        index.children()[1].source_span()
    );

    let missing_content = expression("items[]", SyntaxKind::DialogueContentApplicationExpression);
    assert!(matches!(
        missing_content.projection(),
        ExpressionProjection::DialogueContentApplication(application)
            if matches!(
                application.content(),
                crate::expressions::SyntaxDialogueContentProjection::Missing { .. }
            )
    ));
    assert_eq!(missing_content.children().len(), 1);

    let ambiguous = expression("items[key]", SyntaxKind::PostfixBracketExpression);
    assert!(matches!(
        ambiguous.projection(),
        ExpressionProjection::PostfixBracket(
            crate::expressions::SyntaxPostfixBracketProjection::Ambiguous { .. }
        )
    ));
    assert_eq!(ambiguous.children().len(), 1);

    let pipe = expression("left |> right", SyntaxKind::PipeExpression);
    assert_eq!(pipe.children().len(), 2);
    assert_eq!(
        pipe.children()[0].authored().unwrap().role(),
        SyntaxRole::LeftOperand
    );
    assert_eq!(
        pipe.children()[1].authored().unwrap().role(),
        SyntaxRole::RightOperand
    );
    assert!(
        pipe.component(ExpressionComponentRole::Operator)
            .is_some_and(|source| source.range().as_range().len() == 2)
    );

    let prefix_try = expression("try value", SyntaxKind::TryExpression);
    assert!(matches!(
        prefix_try.projection(),
        ExpressionProjection::Try {
            operand: SyntaxExpressionSlot::Authored,
            form: crate::expressions::SyntaxTryForm::PrefixTry,
        }
    ));
    assert_eq!(prefix_try.children().len(), 1);

    let missing_await = expression("await", SyntaxKind::AwaitExpression);
    assert!(matches!(
        missing_await.projection(),
        ExpressionProjection::Await {
            operand: SyntaxExpressionSlot::Missing,
            propagation: crate::expressions::SyntaxAwaitPropagation::PreserveResult,
        }
    ));
    assert!(matches!(
        &missing_await.children()[0],
        AttachedExpressionChild::Missing { ordinal: 0, .. }
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed postfix ambiguity and revision-binding matrix is easier to audit together"
)]
fn ambiguous_postfix_candidates_expose_borrowed_revision_bound_graphs_only() {
    let ambiguous = expression("items[key]", SyntaxKind::PostfixBracketExpression);
    let cloned_lease = ambiguous.clone();
    assert!(std::ptr::eq(
        ambiguous.projection(),
        cloned_lease.projection()
    ));
    let target_end = ambiguous.children()[0].source_span().range().end();

    let index = ambiguous
        .ambiguous_index_candidate()
        .expect("ordinary-index candidate");
    let cloned_index = cloned_lease
        .ambiguous_index_candidate()
        .expect("cloned lease borrows the same candidate graph");
    assert!(std::ptr::eq(index.graph, cloned_index.graph));
    assert!(index.dialogue_content().is_none());
    let primary = index.primary().expect("index expression root");
    assert_eq!(primary.kind(), SyntaxKind::PathExpression);
    assert!(matches!(
        primary.expression_projection(),
        Some(ExpressionProjection::Path)
    ));
    assert!(primary.source_span().range().start() > target_end);
    assert!(primary.assertion_projection().is_none());
    assert!(primary.type_projection().is_none());
    assert!(primary.pattern_projection().is_none());

    let path_node = primary
        .children()
        .find(|node| node.path_projection().is_some())
        .expect("candidate path projection");
    assert_eq!(path_node.kind(), SyntaxKind::Path);
    assert_eq!(path_node.role(), SyntaxRole::Target);
    let path = path_node.path_projection().expect("typed path payload");
    assert!(matches!(path.root(), AttachedPathRoot::ImplicitCrate));
    let segments = path.segments().collect::<Vec<_>>();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].kind(), AttachedPathSegmentKind::Identifier);
    assert_eq!(segments[0].source_span(), path_node.source_span());
    assert_eq!(segments[0].source_text(), "key");
    assert!(!path.has_recovery());
    assert!(path.missing_name().is_none());

    let dialogue = ambiguous
        .ambiguous_dialogue_candidate()
        .expect("dialogue-content candidate");
    assert!(dialogue.primary().is_none());
    assert!(dialogue.dialogue_content().is_some());

    for source in ["items[0]", "items[,]"] {
        let expression = expression(source, SyntaxKind::PostfixBracketExpression);
        assert!(expression.ambiguous_index_candidate().is_none());
        assert!(expression.ambiguous_dialogue_candidate().is_none());
    }
    let dialogue = expression(
        "alice[こんにちは。]",
        SyntaxKind::DialogueContentApplicationExpression,
    );
    assert!(dialogue.ambiguous_index_candidate().is_none());
    assert!(dialogue.ambiguous_dialogue_candidate().is_none());

    let composite = expression("items[left + right]", SyntaxKind::PostfixBracketExpression);
    let primary = composite
        .ambiguous_index_candidate()
        .expect("composite ordinary-index candidate")
        .primary()
        .expect("binary index root");
    let children = primary.semantic_expression_children().collect::<Vec<_>>();
    assert_eq!(
        children
            .iter()
            .map(super::AttachedCandidateExpressionChild::ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(children[0].node().role(), SyntaxRole::LeftOperand);
    assert_eq!(children[1].node().role(), SyntaxRole::RightOperand);
    assert_eq!(primary.expression_components().unwrap().count(), 3);

    let associated = expression(
        "items[Vec<I32>::with_capacity(8)]",
        SyntaxKind::PostfixBracketExpression,
    );
    let associated_children = associated
        .ambiguous_index_candidate()
        .expect("associated Call remains ambiguous with dialogue text")
        .primary()
        .expect("associated Call root")
        .semantic_expression_children()
        .collect::<Vec<_>>();
    assert_eq!(associated_children.len(), 1);
    assert_eq!(associated_children[0].ordinal(), 1);
    assert_eq!(
        associated_children[0].slot(),
        SyntaxExpressionSlot::Authored
    );
    assert_eq!(
        associated_children[0].component_role(),
        ExpressionComponentRole::CallArgument {
            argument: 0,
            part: SyntaxCallArgumentPart::Value,
        }
    );

    let recovered = expression("items[left +]", SyntaxKind::PostfixBracketExpression);
    let recovered_index = recovered
        .ambiguous_index_candidate()
        .expect("recovered index candidate remains viable");
    let recovered_children = recovered_index
        .primary()
        .expect("recovered Binary root")
        .semantic_expression_children()
        .collect::<Vec<_>>();
    assert_eq!(
        recovered_children
            .iter()
            .map(AttachedCandidateExpressionChild::ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(matches!(
        recovered_children.as_slice(),
        [
            AttachedCandidateExpressionChild::Authored { .. },
            AttachedCandidateExpressionChild::Missing { .. }
        ]
    ));
    assert_eq!(recovered_children[1].slot(), SyntaxExpressionSlot::Missing);
}

#[test]
fn candidate_associated_call_exposes_direct_typed_receiver_children() {
    let expression = expression(
        "items[Vec<I32>::with_capacity(8)]",
        SyntaxKind::PostfixBracketExpression,
    );
    let index = expression
        .ambiguous_index_candidate()
        .expect("associated Call index candidate");
    let call = index.primary().expect("associated Call root");
    assert!(matches!(
        call.expression_projection(),
        Some(ExpressionProjection::Call(SyntaxCallProjection::Parenthesized(call)))
            if matches!(call.callee(), SyntaxCallCalleeProjection::Associated { .. })
    ));

    let roots = call.direct_semantic_type_roots().collect::<Vec<_>>();
    let [receiver] = roots.as_slice() else {
        panic!("associated Call owns exactly one receiver type root");
    };
    assert_eq!(receiver.role(), SyntaxCallTypeChildRole::AssociatedReceiver);
    assert_eq!(
        receiver.node().source_span(),
        receiver.source_span().clone()
    );
    assert!(matches!(
        receiver
            .node()
            .type_projection()
            .expect("typed receiver projection")
            .value(),
        crate::types::TypeRef::Generic { base, .. }
            if base.segments().last().is_some_and(|segment| segment.as_str() == "Vec")
    ));

    let children = receiver
        .node()
        .direct_semantic_type_children()
        .collect::<Vec<_>>();
    let [argument] = children.as_slice() else {
        panic!("Generic receiver owns exactly one type argument");
    };
    assert_eq!(
        argument.step(),
        crate::types::TypeRefNodeStep::GenericArgument(0)
    );
    assert_eq!(
        argument.node().source_span(),
        argument.source_span().clone()
    );
    assert!(matches!(
        argument
            .node()
            .type_projection()
            .expect("typed generic argument projection")
            .value(),
        crate::types::TypeRef::Path(path)
            if path.segments().last().is_some_and(|segment| segment.as_str() == "I32")
    ));

    assert!(
        receiver.source_span().range().start() <= argument.source_span().range().start()
            && argument.source_span().range().end() <= receiver.source_span().range().end()
    );
}

#[test]
fn ambiguous_dialogue_candidate_retains_interpolation_and_tag_payload_expressions() {
    fn assert_candidate_attachment(slot: &AttachedCandidateDialogueExpression<'_>) {
        let root = slot.node();
        root.children()
            .find(|node| node.path_projection().is_some())
            .expect("Dialogue expression root retains its typed Path child");

        assert_eq!(root.source_span(), slot.source_span().clone());
    }

    let interpolation = expression("x[#[y]]", SyntaxKind::PostfixBracketExpression);
    assert!(interpolation.ambiguous_index_candidate().is_some());
    let dialogue = interpolation
        .ambiguous_dialogue_candidate()
        .expect("interpolation is also viable Dialogue content");
    let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
        dialogue.dialogue_content().expect("dialogue content")
    else {
        panic!("interpolation retains present content");
    };
    assert!(content.nodes().iter().any(|node| matches!(
        node,
        crate::expressions::SyntaxDialogueNodeProjection::Interpolation(
            SyntaxExpressionSlot::Authored
        )
    )));
    let slots = dialogue
        .dialogue_expression_slots()
        .expect("Dialogue expression slots")
        .collect::<Vec<_>>();
    let [slot] = slots.as_slice() else {
        panic!("interpolation owns exactly one expression slot");
    };
    assert_eq!(
        slot.owner(),
        AttachedCandidateDialogueOwner::Node { ordinal: 0 }
    );
    assert_eq!(slot.slot(), SyntaxExpressionSlot::Authored);
    assert_candidate_attachment(slot);

    let conditional = expression("x[[if y]]", SyntaxKind::PostfixBracketExpression);
    assert!(conditional.ambiguous_index_candidate().is_some());
    let dialogue = conditional
        .ambiguous_dialogue_candidate()
        .expect("conditional tag is also viable Dialogue content");
    let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
        dialogue.dialogue_content().expect("dialogue content")
    else {
        panic!("conditional retains present content");
    };
    assert!(content.tags().iter().any(|tag| matches!(
        tag.payload(),
        crate::expressions::SyntaxRichTextTagPayloadProjection::Condition(
            SyntaxExpressionSlot::Authored
        )
    )));
    let slots = dialogue
        .dialogue_expression_slots()
        .expect("Dialogue expression slots")
        .collect::<Vec<_>>();
    let [slot] = slots.as_slice() else {
        panic!("conditional tag owns exactly one expression slot");
    };
    assert_eq!(
        slot.owner(),
        AttachedCandidateDialogueOwner::Tag { ordinal: 0 }
    );
    assert_eq!(slot.slot(), SyntaxExpressionSlot::Authored);
    assert_candidate_attachment(slot);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed Dialogue content attachment matrix is easier to audit together"
)]
fn attached_dialogue_content_keeps_typed_nodes_and_nested_expression_identity() {
    let text = expression(
        "alice[こんにちは。]",
        SyntaxKind::DialogueContentApplicationExpression,
    );
    let ExpressionProjection::DialogueContentApplication(application) = text.projection() else {
        panic!("dialogue application projection");
    };
    let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
        application.content()
    else {
        panic!("present dialogue content");
    };
    assert!(matches!(
        content.nodes(),
        [crate::expressions::SyntaxDialogueNodeProjection::Text(value)]
            if value.as_ref() == "こんにちは。"
    ));
    assert_eq!(text.children().len(), 1);
    assert!(
        text.component(ExpressionComponentRole::DialogueNode {
            ordinal: 0,
            part: crate::expressions::SyntaxDialogueNodeSourcePart::Text,
        })
        .is_some()
    );

    let interpolation = expression(
        "alice[こんにちは #[actor.name]]",
        SyntaxKind::DialogueContentApplicationExpression,
    );
    assert_eq!(interpolation.children().len(), 2);
    assert_eq!(
        interpolation.children()[1].component_role(),
        ExpressionComponentRole::DialogueNode {
            ordinal: 1,
            part: crate::expressions::SyntaxDialogueNodeSourcePart::Interpolation,
        }
    );
    assert_eq!(
        interpolation
            .component(interpolation.children()[1].component_role())
            .expect("interpolation source component"),
        interpolation.children()[1].source_span()
    );
    assert_eq!(
        interpolation.children()[1]
            .authored()
            .expect("attached interpolation expression")
            .source_text(),
        "actor.name"
    );

    let rich_text = expression(
        "alice[前[strong]強調[/strong]後]",
        SyntaxKind::DialogueContentApplicationExpression,
    );
    let ExpressionProjection::DialogueContentApplication(application) = rich_text.projection()
    else {
        panic!("RichText dialogue application");
    };
    let crate::expressions::SyntaxDialogueContentProjection::Present(content) =
        application.content()
    else {
        panic!("present RichText content");
    };
    assert_eq!(content.tags().len(), 1);
    assert!(content.tags()[0].paired_end_node().is_some());
    assert!(
        rich_text
            .component(ExpressionComponentRole::RichTextTag {
                tag: 0,
                part: crate::expressions::SyntaxRichTextTagSourcePart::EndTag,
            })
            .is_some()
    );

    let call = expression(
        "alice[本文。[call notify()]]",
        SyntaxKind::DialogueContentApplicationExpression,
    );
    assert_eq!(call.children().len(), 2);
    assert_eq!(
        call.children()[1].component_role(),
        ExpressionComponentRole::RichTextTag {
            tag: 0,
            part: crate::expressions::SyntaxRichTextTagSourcePart::Payload,
        }
    );
    assert_eq!(
        call.children()[1]
            .authored()
            .expect("dialogue-safe Call payload expression")
            .source_text(),
        "notify()"
    );

    let interleaved = expression(
        "alice[#[first][if condition]yes[endif]#[last]]",
        SyntaxKind::DialogueContentApplicationExpression,
    );
    let nested = &interleaved.children()[1..];
    assert_eq!(
        nested
            .iter()
            .map(|child| child
                .authored()
                .expect("authored Dialogue child")
                .source_text())
            .collect::<Vec<_>>(),
        ["first", "condition", "last"]
    );
    assert!(matches!(
        nested[0].component_role(),
        ExpressionComponentRole::DialogueNode { .. }
    ));
    assert!(matches!(
        nested[1].component_role(),
        ExpressionComponentRole::RichTextTag { .. }
    ));
    assert!(matches!(
        nested[2].component_role(),
        ExpressionComponentRole::DialogueNode { .. }
    ));
}

#[test]
fn attached_e19_range_keeps_fixed_optional_endpoint_ordinals() {
    let bounded = expression("start..=end", SyntaxKind::RangeExpression);
    assert!(matches!(
        bounded.projection(),
        ExpressionProjection::Range {
            start: Some(SyntaxExpressionSlot::Authored),
            end: Some(SyntaxExpressionSlot::Authored),
            inclusive: true,
        }
    ));
    assert_eq!(bounded.children().len(), 2);
    assert_eq!(bounded.children()[0].ordinal(), 0);
    assert_eq!(bounded.children()[1].ordinal(), 1);
    assert_eq!(
        bounded.children()[0].authored().unwrap().source_text(),
        "start"
    );
    assert_eq!(
        bounded.children()[1].authored().unwrap().source_text(),
        "end"
    );
    assert_eq!(
        bounded
            .component(ExpressionComponentRole::RangeStart)
            .unwrap(),
        bounded.children()[0].source_span()
    );
    assert_eq!(
        bounded
            .component(ExpressionComponentRole::RangeEnd)
            .unwrap(),
        bounded.children()[1].source_span()
    );
    assert_eq!(
        bounded
            .component(ExpressionComponentRole::RangeInclusiveMarker)
            .unwrap()
            .range()
            .as_range()
            .len(),
        3
    );

    let prefix = expression("..end", SyntaxKind::RangeExpression);
    assert_eq!(prefix.children().len(), 1);
    assert_eq!(prefix.children()[0].ordinal(), 1);
    assert_eq!(
        prefix.children()[0].authored().unwrap().source_text(),
        "end"
    );
    assert!(
        prefix
            .component(ExpressionComponentRole::RangeStart)
            .is_none()
    );

    let suffix = expression("start..", SyntaxKind::RangeExpression);
    assert_eq!(suffix.children().len(), 1);
    assert_eq!(suffix.children()[0].ordinal(), 0);
    assert!(
        suffix
            .component(ExpressionComponentRole::RangeEnd)
            .is_none()
    );

    let unbounded = expression("..", SyntaxKind::RangeExpression);
    assert!(unbounded.children().is_empty());
    assert!(matches!(
        unbounded.projection(),
        ExpressionProjection::Range {
            start: None,
            end: None,
            inclusive: false,
        }
    ));

    let inclusive_unbounded = expression("..=", SyntaxKind::RangeExpression);
    assert!(inclusive_unbounded.children().is_empty());
    assert!(matches!(
        inclusive_unbounded.projection(),
        ExpressionProjection::Range {
            start: None,
            end: None,
            inclusive: true,
        }
    ));
    assert!(
        inclusive_unbounded
            .component(ExpressionComponentRole::RangeInclusiveMarker)
            .is_some()
    );
}

#[test]
fn attached_e22_binary_keeps_operator_and_fixed_operand_ordinals() {
    let binary = expression("left >= right", SyntaxKind::BinaryExpression);
    assert!(matches!(
        binary.projection(),
        ExpressionProjection::Binary {
            left: SyntaxExpressionSlot::Authored,
            operator: crate::expressions::SyntaxBinaryOperator::GreaterOrEqual,
            right: SyntaxExpressionSlot::Authored,
        }
    ));
    assert_eq!(binary.children().len(), 2);
    assert_eq!(binary.children()[0].ordinal(), 0);
    assert_eq!(binary.children()[1].ordinal(), 1);
    assert_eq!(
        binary.children()[0].authored().unwrap().source_text(),
        "left"
    );
    assert_eq!(
        binary.children()[1].authored().unwrap().source_text(),
        "right"
    );
    assert_eq!(
        binary
            .component(ExpressionComponentRole::LeftOperand)
            .unwrap(),
        binary.children()[0].source_span()
    );
    assert_eq!(
        binary
            .component(ExpressionComponentRole::RightOperand)
            .unwrap(),
        binary.children()[1].source_span()
    );
    assert_eq!(
        binary
            .component(ExpressionComponentRole::Operator)
            .unwrap()
            .range()
            .as_range()
            .len(),
        2
    );

    let recovered = expression("left +", SyntaxKind::BinaryExpression);
    assert!(matches!(
        &recovered.children()[1],
        AttachedExpressionChild::Missing {
            ordinal: 1,
            component_role: ExpressionComponentRole::RightOperand,
            recovery,
        }
            if recovery.source_span().range().is_empty()
    ));

    let missing_left = expression("+ right", SyntaxKind::BinaryExpression);
    assert!(matches!(
        &missing_left.children()[0],
        AttachedExpressionChild::Missing {
            ordinal: 0,
            component_role: ExpressionComponentRole::LeftOperand,
            recovery,
        }
            if recovery.source_span().range().is_empty()
    ));
    assert_eq!(
        missing_left.children()[1].authored().unwrap().source_text(),
        "right"
    );
}

#[test]
fn attached_e35_error_retains_only_an_authored_recovery_prefix() {
    let standalone = expression(":", SyntaxKind::ErrorExpression);
    assert!(matches!(
        standalone.projection(),
        ExpressionProjection::Error
    ));
    assert!(standalone.children().is_empty());
    assert_eq!(
        standalone
            .component(ExpressionComponentRole::Recovery)
            .expect("E35 recovery component"),
        standalone.whole_source_span()
    );

    let wrapped = expression("value : bad", SyntaxKind::ErrorExpression);
    assert!(matches!(wrapped.projection(), ExpressionProjection::Error));
    let [prefix] = wrapped.children() else {
        panic!("wrapped E35 recovery must retain one parsed prefix");
    };
    assert_eq!(prefix.ordinal(), 0);
    assert_eq!(prefix.component_role(), ExpressionComponentRole::Recovery);
    assert_eq!(
        wrapped.component(prefix.component_role()).unwrap(),
        prefix.source_span()
    );
    let prefix = prefix
        .authored_semantic()
        .expect("wrapped E35 prefix access")
        .expect("wrapped E35 authored prefix");
    assert_eq!(
        prefix.whole_source_span().range().start(),
        wrapped.whole_source_span().range().start()
    );
    assert_eq!(
        prefix.whole_source_span().range().end() - prefix.whole_source_span().range().start(),
        5
    );
    let recovery = wrapped
        .component(ExpressionComponentRole::Recovery)
        .expect("wrapped E35 recovery component");
    assert!(recovery.range().start() > wrapped.whole_source_span().range().start());
    assert_eq!(
        recovery.range().end(),
        wrapped.whole_source_span().range().end()
    );

    for (source, insertion) in [("target.42", 7), ("target. 42", 8)] {
        let wrapped = expression(source, SyntaxKind::ErrorExpression);
        let [prefix] = wrapped.children() else {
            panic!("numeric member recovery must retain one parsed prefix: {source}");
        };
        let prefix = prefix
            .authored_semantic()
            .expect("numeric member prefix access")
            .expect("numeric member authored prefix");
        assert!(matches!(
            prefix.projection(),
            ExpressionProjection::Select(SyntaxSelectedMember::Missing)
        ));
        let expression_start = wrapped.whole_source_span().range().start();
        assert_eq!(
            prefix
                .component(ExpressionComponentRole::SelectedMember)
                .expect("missing selected-member component")
                .range(),
            SourceRange::new(expression_start + insertion, expression_start + insertion)
        );
    }
}

#[test]
fn snapshot_rejects_invalid_component_and_unit_child_invariants() {
    let invalid_component = [
        SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
        SyntaxEvent::expression_start(
            SyntaxKind::PlaceholderExpression,
            SyntaxRole::Element(0),
            PendingExpressionProjection::new(
                ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PartialApplication),
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::PlaceholderMarker,
                    SourceRange::new(1, 1),
                )],
            ),
        ),
        SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
    ];
    let source_document = document("x");
    let build = build_grammar(&source_document, &invalid_component).unwrap();
    assert_eq!(
        attach_build(source_document, &build).unwrap_err(),
        AttachmentFailure::SnapshotInvariant
    );

    let invalid_unit = [
        SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
        SyntaxEvent::expression_start(
            SyntaxKind::TupleExpression,
            SyntaxRole::Element(0),
            PendingExpressionProjection::new(ExpressionProjection::Unit, Vec::new()),
        ),
        SyntaxEvent::expression_start(
            SyntaxKind::PlaceholderExpression,
            SyntaxRole::Element(0),
            PendingExpressionProjection::new(
                ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PartialApplication),
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::PlaceholderMarker,
                    SourceRange::new(0, 1),
                )],
            ),
        ),
        SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 1)),
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
    ];
    let source_document = document("x");
    let build = build_grammar(&source_document, &invalid_unit).unwrap();
    assert_eq!(
        attach_build(source_document, &build).unwrap_err(),
        AttachmentFailure::SnapshotInvariant
    );

    let forged_group_component = vec![
        SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
        SyntaxEvent::expression_start(
            SyntaxKind::TupleExpression,
            SyntaxRole::Element(0),
            PendingExpressionProjection::new(
                ExpressionProjection::Tuple(Box::new([SyntaxExpressionSlot::Authored])),
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::Element { ordinal: 0 },
                    SourceRange::new(2, 3),
                )],
            ),
        ),
        SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(0, 1)),
        SyntaxEvent::start(SyntaxKind::ExpressionList, SyntaxRole::Element(0)),
        SyntaxEvent::transparent_expression_group(SyntaxRole::Element(0)),
        SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(1, 2)),
        SyntaxEvent::expression_start(
            SyntaxKind::PlaceholderExpression,
            SyntaxRole::Operand,
            PendingExpressionProjection::new(
                ExpressionProjection::Placeholder(SyntaxPlaceholderKind::PipeLeft),
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::PlaceholderMarker,
                    SourceRange::new(2, 3),
                )],
            ),
        ),
        SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(2, 3)),
        SyntaxEvent::FinishNode,
        SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(3, 4)),
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
        SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(4, 5)),
        SyntaxEvent::token(SyntaxKind::PunctuationToken, SourceRange::new(5, 6)),
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
    ];
    let source_document = document("((^),)");
    let build = build_grammar(&source_document, &forged_group_component).unwrap();
    assert_eq!(
        attach_build(source_document, &build).unwrap_err(),
        AttachmentFailure::SnapshotInvariant,
        "a grouped parent component cannot shrink to the inner expression span"
    );
}
