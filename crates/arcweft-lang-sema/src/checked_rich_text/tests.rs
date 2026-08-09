use std::sync::Arc;

use arcweft_dialogue::rich_text::{DialogueHostProperty, DialogueRichTextControl};
use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_presentation::rich_text::{BuiltinRichTextFx, BuiltinRichTextFxProperty};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDialogueToken, CheckedDirectStyleSpan,
    CheckedFieldOrigin, CheckedLength, CheckedRichTextAction, CheckedRichTextOwner,
    CheckedRichTextProperty, CheckedRichTextReport, CheckedRichTextTag, CheckedRichTextValue,
    LengthUnit, Milli, RichTextAttributeChecker, RichTextDiagnosticCode,
};

fn check(content: &str) -> super::CheckedRichTextReport {
    let package = CallablePackageId::try_new("checked-rich-text-tests").expect("package ID");
    let name = SourceName::path("checked-rich-text.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://sema/checked-rich-text")
                .expect("document ID"),
            name.clone(),
            format!("fn checked_rich_text() {{\n    let line = alice[{content}];\n}}\n"),
        )
        .expect("source document"),
    );
    let parsed = SyntaxDatabase::try_new()
        .expect("syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            Arc::clone(&document),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("parsed source");
    let key = HirModuleKey::new(
        package.clone(),
        CanonicalModulePath::crate_root(),
        document.identity().id().clone(),
    );
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "checked-rich-text-tests",
    )
    .expect("symbol world");
    let revision =
        ProjectSymbolRevision::try_for_documents([document.identity()]).expect("symbol revision");
    let mut database = HirDatabase::try_new().expect("HIR database");
    let transaction = database
        .stage_proof_return_project(
            [LoweringRequest::try_new(key, &parsed).expect("lowering request")],
            world,
            revision,
            [document.identity()],
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("staged HIR project");
    let semantic_facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("fixture has no Proof return facts");
    let module = transaction
        .publish_with_semantic_facts(&mut database, semantic_facts)
        .expect("published HIR project")
        .into_iter()
        .next()
        .expect("one published module")
        .into_module();
    let content = module
        .expressions()
        .find_map(|(_, expression)| match expression.kind() {
            HirExprKind::DialogueContentApplication(application) => Some(application.content()),
            _ => None,
        })
        .expect("fixture publishes one dialogue-content application");
    RichTextAttributeChecker::check(&module, content)
        .expect("final-HIR source manifest is complete")
}

fn open_tags(report: &CheckedRichTextReport) -> Vec<&CheckedRichTextTag> {
    report
        .content()
        .tokens()
        .iter()
        .filter_map(|token| match token {
            CheckedDialogueToken::Open(tag) => Some(tag),
            _ => None,
        })
        .collect()
}

#[test]
fn builtin_fx_defaults_are_materialized_only_for_absent_properties() {
    let report = check("[effect .wave]wave[/effect]");

    assert!(report.is_valid(), "{:?}", report.diagnostics());
    let tags = open_tags(&report);
    let [tag] = tags.as_slice() else {
        panic!("one checked effect tag");
    };
    assert!(matches!(
        tag.owner(),
        CheckedRichTextOwner::BuiltinFx {
            effect: BuiltinRichTextFx::Wave,
            ..
        }
    ));
    let CheckedRichTextAction::BuiltinFx { fields, .. } = tag.action() else {
        panic!("one checked builtin Fx action");
    };
    assert!(!fields.fields().is_empty());
    assert!(
        fields
            .fields()
            .iter()
            .all(|field| matches!(field.origin(), CheckedFieldOrigin::Defaulted { .. }))
    );
}

#[test]
fn authored_fx_value_is_checked_and_not_replaced_by_its_default() {
    let report = check("[effect .wave amp=2px]wave[/effect]");

    assert!(report.is_valid(), "{:?}", report.diagnostics());
    let tags = open_tags(&report);
    let CheckedRichTextAction::BuiltinFx { fields, .. } = tags[0].action() else {
        panic!("one checked builtin Fx action");
    };
    let amp = fields
        .fields()
        .iter()
        .find(|field| {
            field.property() == CheckedRichTextProperty::BuiltinFx(BuiltinRichTextFxProperty::Amp)
        })
        .expect("wave amp field");
    assert!(matches!(amp.origin(), CheckedFieldOrigin::Authored { .. }));
    assert_eq!(
        amp.value(),
        &CheckedRichTextValue::Length(CheckedLength {
            milli: 2_000,
            unit: LengthUnit::Px,
        })
    );
}

#[test]
fn malformed_and_duplicate_values_reject_the_tag_without_guessed_output() {
    let malformed = check("[effect .wave amp=oops]wave[/effect]");
    assert!(open_tags(&malformed).is_empty());
    assert_eq!(
        malformed
            .diagnostics()
            .iter()
            .map(super::diagnostic::RichTextAttributeDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![RichTextDiagnosticCode::InvalidUnit]
    );

    let duplicate = check("[effect .wave amp=2px amp=3px]wave[/effect]");
    assert!(open_tags(&duplicate).is_empty());
    let [diagnostic] = duplicate.diagnostics() else {
        panic!("one duplicate diagnostic");
    };
    assert_eq!(diagnostic.code(), RichTextDiagnosticCode::Duplicate);
    assert_eq!(diagnostic.related().len(), 1);
}

#[test]
fn recovered_argument_and_crossing_spans_remain_typed_failures() {
    let recovered = check("[effect .wave amp=]wave[/effect]");
    assert!(open_tags(&recovered).is_empty());
    assert!(
        recovered
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == RichTextDiagnosticCode::InvalidArgument)
    );

    let crossing = check("[strong][em]text[/strong][/em]");
    let codes = crossing
        .diagnostics()
        .iter()
        .map(super::diagnostic::RichTextAttributeDiagnostic::code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&RichTextDiagnosticCode::CrossingSpan));
    assert!(codes.contains(&RichTextDiagnosticCode::UnclosedSpan));
}

#[test]
fn point_controls_and_explicit_markers_publish_typed_open_actions() {
    let point = check("before[p]after");
    assert!(point.is_valid(), "{:?}", point.diagnostics());
    let point_tags = open_tags(&point);
    let [page] = point_tags.as_slice() else {
        panic!("one checked page control");
    };
    assert_eq!(
        page.owner(),
        &CheckedRichTextOwner::Control(DialogueRichTextControl::Page)
    );
    assert!(matches!(
        page.action(),
        CheckedRichTextAction::Control {
            action: CheckedDialogueControl::Page,
            ..
        }
    ));
    assert!(
        !point
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == RichTextDiagnosticCode::UnclosedSpan)
    );

    let marker = check("before[mark .release]after");
    assert!(marker.is_valid(), "{:?}", marker.diagnostics());
    let marker_tags = open_tags(&marker);
    let [marker] = marker_tags.as_slice() else {
        panic!("one checked explicit marker");
    };
    let CheckedRichTextAction::Marker(marker) = marker.action() else {
        panic!("explicit marker action");
    };
    assert_eq!(marker.as_str(), "release");
}

#[test]
fn family_close_uses_the_exact_paired_start_identity() {
    let report = check("[effect .wave]wave[/effect]");
    assert!(report.is_valid(), "{:?}", report.diagnostics());
    let mut open = None;
    let mut close = None;
    for token in report.content().tokens() {
        match token {
            CheckedDialogueToken::Open(tag) => open = Some(tag.id()),
            CheckedDialogueToken::Close(tag) => close = Some(tag.open()),
            _ => {}
        }
    }
    assert_eq!(open, close);
    assert!(open.is_some());
}

#[test]
fn invalid_selector_keeps_child_text_without_open_semantics() {
    let report = check("[.typo]visible[/]");
    assert!(!report.is_valid());
    assert!(open_tags(&report).is_empty());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == RichTextDiagnosticCode::UnknownSelector })
    );
    assert!(
        report
            .content()
            .tokens()
            .iter()
            .any(|token| { matches!(token, CheckedDialogueToken::InvalidTag { .. }) })
    );
    assert!(report.content().tokens().iter().any(|token| {
        matches!(token, CheckedDialogueToken::Text(text) if text.as_ref() == "visible")
    }));
}

#[test]
fn direct_style_and_host_defaults_remain_family_typed() {
    let size = check("[size 12pt]text[/size]");
    assert!(size.is_valid(), "{:?}", size.diagnostics());
    let size_tags = open_tags(&size);
    assert!(matches!(
        size_tags[0].action(),
        CheckedRichTextAction::DirectStyle {
            action: CheckedDirectStyleSpan::Size {
                value: CheckedLength {
                    milli: 12_000,
                    unit: LengthUnit::Pt,
                }
            },
            ..
        }
    ));

    let scale = check("before[scale x=2]after");
    assert!(scale.is_valid(), "{:?}", scale.diagnostics());
    let scale_tags = open_tags(&scale);
    let CheckedRichTextAction::Host { action, fields, .. } = scale_tags[0].action() else {
        panic!("typed scale host action");
    };
    assert_eq!(
        action,
        &CheckedDialogueHostEvent::Scale {
            x: Milli(2_000),
            y: Milli(2_000),
        }
    );
    let y = fields
        .fields()
        .iter()
        .find(|field| field.property() == CheckedRichTextProperty::Host(DialogueHostProperty::Y))
        .expect("scale y default copied from x");
    assert!(matches!(y.origin(), CheckedFieldOrigin::Defaulted { .. }));

    let missing_move = check("before[move]after");
    assert!(!missing_move.is_valid());
    assert!(
        missing_move
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == RichTextDiagnosticCode::Conflict)
    );
}
