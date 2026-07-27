use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::{
    check::{TypeCheckReport, analyze_types},
    diagnostics::TypeCheckErrorKind,
    env::TypeCheckEnv,
    style::StyleDiagnosticCode,
};
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::style::{
    ViewBoxAxisMode, ViewClip, ViewFilter, ViewMask, ViewPropertyKind, ViewSpecifiedValue,
    ViewStyleValueKind,
};
use std::sync::Arc;

fn analyze(source: &str) -> TypeCheckReport {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://sema/style.arcw").expect("source ID"),
            SourceName::Generated,
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    assert_eq!(parsed.errors(), &[], "syntax errors: {:?}", parsed.errors());
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("style source lowers to HIR");
    analyze_types(&hir, &TypeCheckEnv::standard())
}

fn style_codes(report: &TypeCheckReport) -> Vec<StyleDiagnosticCode> {
    report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic } => Some(diagnostic.code()),
            _ => None,
        })
        .collect()
}

#[test]
fn native_style_catalog_contains_typed_tokens_selectors_and_values() {
    let report = analyze(
        r#"pub style control {
    token color.text: Color = system_color(.SurfaceText)
    token metric.radius: Length = 12px
    token font.ui: FontFamilyList = ["Noto Sans JP", system_font(.Ui)]

    Button:hover {
        color = token(color.text)
        border-radius = token(metric.radius)
        font-family = token(font.ui)
        font-weight = 720
    }
}
"#,
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let sheet = &report.style_catalog.sheets()[0];
    assert_eq!(sheet.tokens().len(), 3);
    assert_eq!(
        sheet.rules()[0]
            .selector()
            .specificity()
            .expect("checked selector specificity")
            .predicates(),
        1
    );
    let declarations = sheet.rules()[0].declarations();
    assert_eq!(declarations[0].property(), ViewPropertyKind::Color);
    assert_eq!(declarations[0].value().kind(), ViewStyleValueKind::Color);
    assert_eq!(declarations[3].property(), ViewPropertyKind::FontWeight);
}

#[test]
fn box_axes_and_logical_properties_lower_as_authored_typed_values() {
    let report = analyze(
        r"pub style logical {
    Panel {
        box-axes = .VerticalRl
        inline-size = 320px
        translate-inline = 12px
    }
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let declarations = report.style_catalog.sheets()[0].rules()[0].declarations();
    assert!(matches!(
        declarations[0].value(),
        ViewSpecifiedValue::BoxAxes {
            value: ViewBoxAxisMode::VerticalRl
        }
    ));
    assert_eq!(declarations[1].property(), ViewPropertyKind::InlineSize);
    assert_eq!(
        declarations[2].property(),
        ViewPropertyKind::TranslateInline
    );
}

#[test]
fn box_axis_diagnostics_reject_unknown_modes_and_non_reversible_translation() {
    let report = analyze(
        r"pub style broken_axes {
    Panel {
        box-axes = .VerticalUnknown
        translate-inline = -2147483.648px
    }
}
",
    );
    let diagnostics = report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic } => Some(diagnostic),
            _ => None,
        })
        .collect::<Vec<_>>();
    let unknown = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code() == StyleDiagnosticCode::InvalidValueType)
        .expect("unknown axis mode diagnostic");
    assert_eq!(
        unknown.valid_inventory(),
        &["HorizontalLtr", "HorizontalRtl", "VerticalRl", "VerticalLr"]
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == StyleDiagnosticCode::LogicalTranslationNotSignReversible
    }));
}

#[test]
fn style_semantics_report_unknown_names_value_kinds_and_invalid_append() {
    let report = analyze(
        r"pub style broken {
    Unknown:hovered {
        made-up = 1px
    }
    Button:unknown-state {
        opacity = 12px
        append color = rgba(1, 2, 3, 255)
    }
}
",
    );
    let codes = style_codes(&report);
    assert!(codes.contains(&StyleDiagnosticCode::UnknownElement));
    assert!(codes.contains(&StyleDiagnosticCode::UnknownState));
    assert!(codes.contains(&StyleDiagnosticCode::UnknownProperty));
    assert!(codes.contains(&StyleDiagnosticCode::InvalidUnit));
    assert!(codes.contains(&StyleDiagnosticCode::InvalidAppend));
}

#[test]
fn style_tokens_report_duplicate_unresolved_and_cycles() {
    let report = analyze(
        r"pub style broken {
    token color.a: Color = token(color.b)
    token color.b: Color = token(color.a)
    token color.a: Color = rgba(1, 2, 3, 255)
    Button { color = token(color.missing) }
}
",
    );
    let codes = style_codes(&report);
    assert!(codes.contains(&StyleDiagnosticCode::DuplicateToken));
    assert!(codes.contains(&StyleDiagnosticCode::TokenCycle));
    assert!(codes.contains(&StyleDiagnosticCode::UnresolvedToken));

    let style_diagnostics = report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic } => Some(diagnostic),
            _ => None,
        });
    let unresolved = style_diagnostics
        .clone()
        .find(|diagnostic| diagnostic.code() == StyleDiagnosticCode::UnresolvedToken)
        .expect("unresolved token diagnostic");
    assert!(unresolved.owner_sheet().is_some());
    let cycle = style_diagnostics
        .clone()
        .find(|diagnostic| diagnostic.code() == StyleDiagnosticCode::TokenCycle)
        .expect("token cycle diagnostic");
    assert!(cycle.ordered_subjects().len() >= 3);
    assert_eq!(
        cycle.ordered_subjects().first(),
        cycle.ordered_subjects().last()
    );
    assert!(cycle.related_ranges().len() >= 3);
}

#[test]
fn numeric_style_diagnostics_distinguish_units_from_fixed_point_overflow() {
    let report = analyze(
        r"pub style broken_numbers {
    Button {
        opacity = 12px
        width = 999999999999999999999px
        scale = 1e999
    }
}
",
    );
    let diagnostics = report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic } => Some(diagnostic),
            _ => None,
        });
    let invalid_unit = diagnostics
        .clone()
        .find(|diagnostic| diagnostic.code() == StyleDiagnosticCode::InvalidUnit)
        .expect("invalid unit diagnostic");
    assert_eq!(invalid_unit.subject(), Some("px"));
    assert_eq!(invalid_unit.accepted_units(), &["milli", "%"]);
    assert_eq!(
        diagnostics
            .filter(|diagnostic| diagnostic.code() == StyleDiagnosticCode::NonFiniteValue)
            .count(),
        2
    );
}

#[test]
fn unknown_style_names_carry_canonical_repair_evidence() {
    let report = analyze(
        r"pub style broken {
    Buton { backgrond-color = rgba(1, 2, 3, 255) }
}
",
    );
    let diagnostics = report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic } => Some(diagnostic),
            _ => None,
        });
    let unknown_element = diagnostics
        .clone()
        .find(|diagnostic| diagnostic.code() == StyleDiagnosticCode::UnknownElement)
        .expect("unknown element diagnostic");
    assert!(
        unknown_element
            .valid_inventory()
            .iter()
            .any(|name| name == "Button")
    );
    let unknown_property = diagnostics
        .clone()
        .find(|diagnostic| diagnostic.code() == StyleDiagnosticCode::UnknownProperty)
        .expect("unknown property diagnostic");
    assert!(
        unknown_property
            .nearest_names()
            .iter()
            .any(|name| name == "background-color")
    );
}

#[test]
fn inline_token_lookup_rejects_ambiguous_names_deterministically() {
    let report = analyze(
        r#"pub style first {
    token color.shared: Color = rgba(1, 2, 3, 255)
}
pub style second {
    token color.shared: Color = rgba(4, 5, 6, 255)
}
pub style third {
    token color.shared: Color = rgba(7, 8, 9, 255)
}
pub view Example() {
    Button("OK").style { color = token(color.shared) }
}
"#,
    );
    assert!(style_codes(&report).contains(&StyleDiagnosticCode::UnresolvedToken));
    assert!(
        report.style_catalog.inline_patches()[0]
            .declarations()
            .is_empty()
    );
}

#[test]
fn top_level_and_inline_native_styles_use_the_same_value_checker() {
    let report = analyze(
        r#"pub style named {
    Button { opacity = 12px }
}
pub view Example() {
    Button("OK").style { opacity = 12px }
}
"#,
    );
    let invalid_units = style_codes(&report)
        .into_iter()
        .filter(|code| *code == StyleDiagnosticCode::InvalidUnit)
        .count();
    assert_eq!(invalid_units, 2);
    assert_eq!(report.style_catalog.inline_patches().len(), 1);
}

#[test]
fn native_inline_patch_catalog_uses_source_order_as_its_only_identity() {
    let source = r#"pub style controls {
Button:hover { color = rgba(1, 2, 3, 255) }
}
pub view Example() {
    Button("OK")
        .style { opacity = 900milli }
        .style { outline-width = 2px }
}
"#;
    let report = analyze(source);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let sheet = &report.style_catalog.sheets()[0];
    assert_eq!(sheet.rules().len(), 1);
    let patches = report.style_catalog.inline_patches();
    assert_eq!(patches.len(), 2);
    assert_eq!(patches[0].id().value(), 0);
    assert_eq!(patches[1].id().value(), 1);
    assert_eq!(
        patches[0].declarations()[0].property(),
        ViewPropertyKind::Opacity
    );
    assert_eq!(
        patches[1].declarations()[0].property(),
        ViewPropertyKind::OutlineWidth
    );
}

#[test]
fn interactive_overflow_requires_scroll_and_text_input_properties_are_applicable() {
    let report = analyze(
        r"pub style controls {
    Button { overflow = .Auto }
    TextField { placeholder-color = rgba(1, 2, 3, 255) }
}
",
    );
    let codes = style_codes(&report);
    assert!(codes.contains(&StyleDiagnosticCode::InteractiveOverflowRequiresScroll));
    assert!(!codes.contains(&StyleDiagnosticCode::PropertyNotApplicable));
}

#[test]
fn alignment_keywords_are_checked_against_the_owning_property() {
    let report = analyze(
        r"pub style alignment {
    Button { text-align = .SpaceBetween }
    Panel { align-content = .SpaceBetween }
}
",
    );
    let invalid = report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic }
                if diagnostic.code() == StyleDiagnosticCode::InvalidValueType =>
            {
                Some(diagnostic)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].subject(), Some("text-align"));
    assert_eq!(
        report.style_catalog.sheets()[0].rules()[1]
            .declarations()
            .len(),
        1
    );
}

#[test]
fn scalar_filters_clip_mask_and_transitions_lower_to_owned_values() {
    let report = analyze(
        r#"pub style effects {
    token motion.emphasis: Scalar = 1250milli

    Panel {
        scale = token(motion.emphasis)
        filter = [brightness(180%), contrast(1.25), opacity(80%)]
        clip = rounded_rect(12px)
        mask = resource(asset.mask.soft)
        transition = [
            transition(property = "opacity", duration = 200ms, delay = 25ms),
        ]
    }
}
"#,
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    let sheet = &report.style_catalog.sheets()[0];
    assert!(matches!(
        sheet.tokens()[0].value(),
        ViewSpecifiedValue::Scalar { value } if value.value() == 1_250
    ));
    let declarations = sheet.rules()[0].declarations();
    assert!(matches!(
        declarations[0].value(),
        ViewSpecifiedValue::Token {
            value_kind: ViewStyleValueKind::Scalar,
            ..
        }
    ));
    let ViewSpecifiedValue::FilterList { value: filters } = declarations[1].value() else {
        panic!("filter declaration should be typed");
    };
    assert!(matches!(
        filters.as_slice(),
        [
            ViewFilter::Brightness { amount: brightness },
            ViewFilter::Contrast { amount: contrast },
            ViewFilter::Opacity { amount: opacity },
        ] if brightness.value() == 1_800
            && contrast.value() == 1_250
            && opacity.value() == 800
    ));
    assert!(matches!(
        declarations[2].value(),
        ViewSpecifiedValue::Clip {
            value: ViewClip::RoundedRect(radii),
        } if radii.top_left.value() == 12_000
            && radii.top_right.value() == 12_000
            && radii.bottom_right.value() == 12_000
            && radii.bottom_left.value() == 12_000
    ));
    assert!(matches!(
        declarations[3].value(),
        ViewSpecifiedValue::Mask {
            value: ViewMask::Resource(resource),
        } if resource.as_str() == "asset.mask.soft"
    ));
    let ViewSpecifiedValue::Transition { value: transitions } = declarations[4].value() else {
        panic!("transition declaration should be typed");
    };
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].property(), ViewPropertyKind::Opacity);
    assert_eq!(transitions[0].duration_millis(), 200);
    assert_eq!(transitions[0].delay_millis(), 25);
}
