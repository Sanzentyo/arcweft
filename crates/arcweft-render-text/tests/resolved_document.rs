use arcweft_core::plan::RuntimeLineId;
use arcweft_render_text::{
    DialogueHostEvent, InlineFailurePolicy, LineDisplaySpec, Milli, ResolvedTextDocument,
    ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle, RichTextControl, RichTextDocument,
    RichTextInlineDirection, RichTextLayout, RichTextNode, RichTextPresentation,
    RichTextPresentationStyle, RichTextRange, RichTextRubyPosition, RichTextStyle,
    RichTextWritingMode, RuntimeLineContext, TextColor, TextDocumentRevision, TextFontFamily,
    TextResolveError, TextStyleCascade, TextWeight,
};
use std::collections::BTreeMap;

fn style() -> ResolvedTextStyle {
    ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 20_000, 27_000)
        .expect("valid test style")
}

fn line(nodes: Vec<RichTextNode>) -> LineDisplaySpec {
    LineDisplaySpec {
        line: RuntimeLineId::canonical("resolved.document.test").expect("canonical test line"),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        view: arcweft_view::ViewId::try_new("view.resolved-document.test").unwrap(),
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: Some(InlineFailurePolicy::FailLine),
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(nodes),
    }
}

#[test]
fn validated_document_exposes_borrowed_text_and_exact_source_mapping() {
    let text = "Arcweft";
    let run = ResolvedTextRun::new(
        RichTextRange::new(0, text.len()),
        RichTextRange::new(11, 11 + text.len()),
        style(),
        RichTextPresentation::default(),
        ResolvedTextRunSource::Plain,
    )
    .expect("run is structurally valid");
    let revision = TextDocumentRevision::new(7);

    let document = ResolvedTextDocument::new(text, 11, vec![run], Vec::new(), revision)
        .expect("document is valid");

    assert_eq!(document.text(), text);
    assert!(std::ptr::eq(document.text().as_ptr(), text.as_ptr()));
    assert_eq!(document.source_origin(), 11);
    assert_eq!(document.source_range(), RichTextRange::new(11, 18));
    assert_eq!(document.runs()[0].source_range(), document.source_range());
    assert_eq!(document.revision(), revision);
}

#[test]
fn document_rejects_a_run_boundary_inside_utf8() {
    let run = ResolvedTextRun::new(
        RichTextRange::new(0, 1),
        RichTextRange::new(0, 1),
        style(),
        RichTextPresentation::default(),
        ResolvedTextRunSource::Plain,
    )
    .expect("range shape is valid before it is checked against text");

    assert!(matches!(
        ResolvedTextDocument::new("夢", 0, vec![run], Vec::new(), TextDocumentRevision::new(0),),
        Err(TextResolveError::InvalidUtf8Range {
            kind: "text run",
            index: 0,
            start: 0,
            end: 1,
            ..
        })
    ));
}

#[test]
fn document_rejects_noncontiguous_and_inexact_source_ranges() {
    let skipped_prefix = ResolvedTextRun::new(
        RichTextRange::new(1, 2),
        RichTextRange::new(5, 6),
        style(),
        RichTextPresentation::default(),
        ResolvedTextRunSource::Plain,
    )
    .expect("run shape is valid");
    assert!(matches!(
        ResolvedTextDocument::new(
            "ab",
            4,
            vec![skipped_prefix],
            Vec::new(),
            TextDocumentRevision::new(0),
        ),
        Err(TextResolveError::RunDiscontinuity {
            expected_start: 0,
            actual_start: 1,
            ..
        })
    ));

    let wrong_source = ResolvedTextRun::new(
        RichTextRange::new(0, 2),
        RichTextRange::new(0, 2),
        style(),
        RichTextPresentation::default(),
        ResolvedTextRunSource::Plain,
    )
    .expect("run shape is valid");
    assert!(matches!(
        ResolvedTextDocument::new(
            "ab",
            4,
            vec![wrong_source],
            Vec::new(),
            TextDocumentRevision::new(0),
        ),
        Err(TextResolveError::SourceRangeMismatch {
            expected_start: 4,
            expected_end: 6,
            actual_start: 0,
            actual_end: 2,
            ..
        })
    ));
}

#[test]
fn stage_resolution_borrows_the_frame_slice_and_retains_full_source_ranges() {
    let frame = line(vec![
        RichTextNode::Text {
            text: "夢".to_owned(),
        },
        RichTextNode::Control {
            control: RichTextControl::Page,
        },
        RichTextNode::Text {
            text: "続き".to_owned(),
        },
    ])
    .resolve_frame(&RuntimeLineContext::default())
    .expect("frame resolves");
    let stage = frame.stage(1).expect("second stage");

    let document = frame
        .resolve_stage_document(stage, &TextStyleCascade::default())
        .expect("stage resolves");

    assert_eq!(document.text(), "続き");
    assert!(std::ptr::eq(
        document.text().as_ptr(),
        frame.text["夢".len()..].as_ptr()
    ));
    assert_eq!(document.source_origin(), "夢".len());
    assert_eq!(
        document.runs()[0].range(),
        RichTextRange::new(0, "続き".len())
    );
    assert_eq!(
        document.runs()[0].source_range(),
        RichTextRange::new("夢".len(), "夢続き".len())
    );
    assert_eq!(
        document.runs()[0].source(),
        ResolvedTextRunSource::Dialogue { node_index: 2 }
    );
}

#[test]
fn document_projection_rebases_runs_and_ruby_without_cloning_text() {
    let source = RichTextDocument::new(vec![
        RichTextNode::Text {
            text: "前".to_owned(),
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Strong {
                attrs: String::new(),
            },
        },
        RichTextNode::Ruby {
            base: "漢字".to_owned(),
            ruby: "かんじ".to_owned(),
        },
        RichTextNode::Text {
            text: "後".to_owned(),
        },
    ]);
    let document = source
        .resolve_document(&TextStyleCascade::default())
        .expect("document resolves");
    let prefix = "前".len();

    let projected = document
        .project(RichTextRange::new(prefix, document.text().len()))
        .expect("projection resolves");

    assert_eq!(projected.text(), "漢字後");
    assert!(std::ptr::eq(
        projected.text().as_ptr(),
        document.text()[prefix..].as_ptr()
    ));
    assert_eq!(projected.source_origin(), prefix);
    assert_eq!(projected.runs()[0].range(), RichTextRange::new(0, 6));
    assert_eq!(
        projected.runs()[0].source_range(),
        RichTextRange::new(prefix, prefix + 6)
    );
    assert_eq!(projected.ruby()[0].base_range(), RichTextRange::new(0, 6));
    assert_eq!(
        projected.ruby()[0].source_base_range(),
        RichTextRange::new(prefix, prefix + 6)
    );
}

#[test]
fn direct_rich_text_resolution_preserves_ruby_style_and_presentation() {
    let layout = RichTextLayout {
        writing_mode: RichTextWritingMode::VerticalRl,
        direction: RichTextInlineDirection::Rtl,
        ..RichTextLayout::default()
    };
    let document = RichTextDocument::new(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Strong {
                attrs: String::new(),
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Layout {
                layout: layout.clone(),
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Presentation {
                presentation: RichTextPresentationStyle {
                    opacity: Some(Milli(625)),
                    layer: Some("dialogue".to_owned()),
                    params: BTreeMap::new(),
                    z_index: Some(3),
                },
            },
        },
        RichTextNode::Ruby {
            base: "漢字".to_owned(),
            ruby: "かんじ".to_owned(),
        },
    ]);

    let resolved = document
        .resolve_document(&TextStyleCascade::new(style()))
        .expect("static document resolves");

    assert_eq!(resolved.text(), "漢字");
    assert!(std::ptr::eq(
        resolved.text().as_ptr(),
        document.resolved_text().as_ptr()
    ));
    assert_eq!(resolved.runs().len(), 1);
    assert_eq!(resolved.runs()[0].style().weight(), TextWeight::Bold);
    assert_eq!(
        resolved.runs()[0].style().writing_mode(),
        RichTextWritingMode::VerticalRl
    );
    assert_eq!(
        resolved.runs()[0].style().direction(),
        RichTextInlineDirection::Rtl
    );
    assert_eq!(resolved.runs()[0].presentation().opacity, Some(Milli(625)));
    assert_eq!(
        resolved.runs()[0]
            .presentation()
            .layout
            .as_ref()
            .expect("layout presentation")
            .writing_mode,
        RichTextWritingMode::VerticalRl
    );
    assert_eq!(resolved.ruby().len(), 1);
    assert_eq!(resolved.ruby()[0].text(), "かんじ");
    assert_eq!(resolved.ruby()[0].base_range(), RichTextRange::new(0, 6));
    assert_eq!(resolved.ruby()[0].style(), resolved.runs()[0].style());
    assert_eq!(
        resolved.ruby()[0].presentation(),
        resolved.runs()[0].presentation()
    );
}

#[test]
fn cascade_applies_closed_color_and_font_values_without_losing_presentation() {
    let document = RichTextDocument::new(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::from_tag("font", "monospace"),
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::from_tag("color", "#123456"),
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::from_tag("opacity", "0.5"),
        },
        RichTextNode::Text {
            text: "styled".to_owned(),
        },
    ]);

    let resolved = document
        .resolve_document(&TextStyleCascade::default())
        .expect("styled document resolves");
    let run = &resolved.runs()[0];

    assert_eq!(run.style().font_families(), &[TextFontFamily::Monospace]);
    assert_eq!(run.style().color(), TextColor::rgba(0x12, 0x34, 0x56, 255));
    assert_eq!(run.presentation().opacity, Some(Milli(500)));
}

#[test]
fn nested_ruby_layout_does_not_reset_the_inherited_vertical_flow() {
    let document = RichTextDocument::new(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Layout {
                layout: RichTextLayout {
                    writing_mode: RichTextWritingMode::VerticalRl,
                    ..RichTextLayout::default()
                },
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Layout {
                layout: RichTextLayout {
                    ruby_position: RichTextRubyPosition::Under,
                    ..RichTextLayout::default()
                },
            },
        },
        RichTextNode::Text {
            text: "夢".to_owned(),
        },
    ]);

    let resolved = document
        .resolve_document(&TextStyleCascade::default())
        .expect("nested layout resolves");
    let run = &resolved.runs()[0];

    assert_eq!(run.style().writing_mode(), RichTextWritingMode::VerticalRl);
    let layout = run
        .presentation()
        .layout
        .as_ref()
        .expect("layout presentation is retained");
    assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);
    assert_eq!(layout.ruby_position, RichTextRubyPosition::Under);
}

#[test]
fn direct_document_rejects_dynamic_nodes() {
    let document = RichTextDocument::new(vec![RichTextNode::HostEvent {
        event: DialogueHostEvent::Conditional {
            name: "if".to_owned(),
            attrs: "flag".to_owned(),
        },
    }]);

    assert!(matches!(
        document.resolve_document(&TextStyleCascade::default()),
        Err(TextResolveError::DynamicNode { node_index: 0 })
    ));
}

#[test]
fn direct_document_rejects_public_node_mutation_that_would_stale_the_borrowed_text() {
    let mut document = RichTextDocument::new(vec![RichTextNode::Text {
        text: "old".to_owned(),
    }]);
    document.nodes[0] = RichTextNode::Text {
        text: "new".to_owned(),
    };

    assert!(matches!(
        document.resolve_document(&TextStyleCascade::default()),
        Err(TextResolveError::SourceTextMismatch {
            node_index: 0,
            start: 0,
            end: 3,
        })
    ));
}

#[test]
fn style_resolution_rejects_zero_sized_authored_text() {
    let document = RichTextDocument::new(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Size {
                points: Some(0),
                raw: "0".to_owned(),
            },
        },
        RichTextNode::Text {
            text: "invalid".to_owned(),
        },
    ]);

    assert!(matches!(
        document.resolve_document(&TextStyleCascade::default()),
        Err(TextResolveError::ZeroFontSize)
    ));
}

#[test]
fn source_revision_changes_when_ruby_text_changes_without_base_text_changes() {
    let first = RichTextDocument::new(vec![RichTextNode::Ruby {
        base: "漢字".to_owned(),
        ruby: "かんじ".to_owned(),
    }]);
    let second = RichTextDocument::new(vec![RichTextNode::Ruby {
        base: "漢字".to_owned(),
        ruby: "kanji".to_owned(),
    }]);

    let first_revision = first
        .resolve_document(&TextStyleCascade::default())
        .expect("first document resolves")
        .revision();
    let second_revision = second
        .resolve_document(&TextStyleCascade::default())
        .expect("second document resolves")
        .revision();

    assert_ne!(first_revision, second_revision);
}
