use super::*;
use arcweft_bundle::resource_codec::view::{ViewElementKind, ViewProgramInstruction};
use arcweft_view::style::{
    ViewOverflow, ViewPropertyKind, ViewSpecifiedValue, ViewStyleApplicationTarget,
    ViewStyleSheetId, ViewStyleValueKind,
};

fn assert_scroll_regions_match_unstyled_baseline(
    source: &str,
    style_modifier: &str,
    styled_program: &ViewProgramResource,
) {
    let baseline_source = source.replace(style_modifier, "");
    assert_ne!(baseline_source, source, "Style modifier fixture must exist");
    let parsed = arcweft_lang_syntax::parser::parse_source(&baseline_source);
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("baseline HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("baseline sidecars lower");
    let baseline_program = sidecars.program.expect("baseline program sidecar");
    assert_eq!(
        styled_program.scroll_regions,
        baseline_program.scroll_regions
    );
}

fn scroll_instruction_styles(program: &ViewProgramResource) -> &[ViewStyleApplicationTarget] {
    program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Scroll,
                styles,
                ..
            } => Some(styles.as_slice()),
            _ => None,
        })
        .expect("Scroll instruction")
}

#[test]
fn view_scroll_retains_typed_style_defaults_without_baking_them_into_layout() {
    let source = r#"
style scroll_defaults {
  token layout.scroll_width: Length = 512px

  Scroll {
    width = token(layout.scroll_width)
    height = 96px
    overflow = .Hidden
  }
}

view StyledScroll() {
  Scroll {
    Text("One")
    Text("Two")
  }
    .style(@style:.scroll_defaults)
}

flow test {
  view(@view:.StyledScroll)
}
"#;
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.as_ref().expect("program sidecar");
    assert_eq!(
        scroll_instruction_styles(program),
        &[ViewStyleApplicationTarget::named(
            ViewStyleSheetId::try_new("style.scroll_defaults").unwrap()
        )]
    );

    let style = sidecars.style.as_ref().expect("style sidecar");
    let sheet = style
        .program
        .sheets()
        .iter()
        .find(|sheet| sheet.id().public_id().as_str() == "style.scroll_defaults")
        .expect("typed Scroll sheet");
    let width_token = sheet
        .tokens()
        .iter()
        .find(|token| token.id().public_id().as_str() == "layout.scroll_width")
        .expect("sheet-owned width token");
    assert_eq!(width_token.value_kind(), ViewStyleValueKind::Length);
    assert!(matches!(
        width_token.value(),
        ViewSpecifiedValue::Length { value } if value.value() == 512_000
    ));
    let rule = sheet
        .rules()
        .iter()
        .find(|rule| rule.selector().target_element() == Some(ViewElementKind::Scroll))
        .expect("typed Scroll rule");
    assert!(rule.declarations().iter().any(|declaration| {
        declaration.property() == ViewPropertyKind::Width
            && matches!(
                declaration.value(),
                ViewSpecifiedValue::Token { token, value_kind }
                    if token.public_id().as_str() == "layout.scroll_width"
                        && *value_kind == ViewStyleValueKind::Length
            )
    }));
    assert!(rule.declarations().iter().any(|declaration| {
        declaration.property() == ViewPropertyKind::Height
            && matches!(
                declaration.value(),
                ViewSpecifiedValue::Length { value } if value.value() == 96_000
            )
    }));
    assert!(rule.declarations().iter().any(|declaration| {
        declaration.property() == ViewPropertyKind::Overflow
            && matches!(
                declaration.value(),
                ViewSpecifiedValue::Overflow {
                    value: ViewOverflow::Hidden
                }
            )
    }));

    assert_eq!(program.scroll_regions.len(), 1);
    assert_eq!(
        program.scroll_regions[0].public_id,
        "scroll.view.StyledScroll.0"
    );
    assert_scroll_regions_match_unstyled_baseline(
        source,
        "\n    .style(@style:.scroll_defaults)",
        program,
    );
}

#[test]
fn view_scroll_retains_typed_overflow_x_without_baking_horizontal_layout() {
    let source = r#"
style horizontal_scroll {
  Scroll {
    width = 128px
    height = 72px
    overflow-x = .Scroll
  }
}

view Gallery() {
  Scroll {
    Row {
      Button(@button:.one, label = "One")
      Button(@button:.two, label = "Two")
    }
  }
    .style(@style:.horizontal_scroll)
}

flow test {
  view(@view:.Gallery)
}
"#;
    let parsed = arcweft_lang_syntax::parser::parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.as_ref().expect("program sidecar");
    assert_eq!(
        scroll_instruction_styles(program),
        &[ViewStyleApplicationTarget::named(
            ViewStyleSheetId::try_new("style.horizontal_scroll").unwrap()
        )]
    );

    let style = sidecars.style.as_ref().expect("style sidecar");
    let sheet = style
        .program
        .sheets()
        .iter()
        .find(|sheet| sheet.id().public_id().as_str() == "style.horizontal_scroll")
        .expect("typed horizontal Scroll sheet");
    let rule = sheet
        .rules()
        .iter()
        .find(|rule| rule.selector().target_element() == Some(ViewElementKind::Scroll))
        .expect("typed Scroll rule");
    assert!(rule.declarations().iter().any(|declaration| {
        declaration.property() == ViewPropertyKind::Width
            && matches!(
                declaration.value(),
                ViewSpecifiedValue::Length { value } if value.value() == 128_000
            )
    }));
    assert!(rule.declarations().iter().any(|declaration| {
        declaration.property() == ViewPropertyKind::Height
            && matches!(
                declaration.value(),
                ViewSpecifiedValue::Length { value } if value.value() == 72_000
            )
    }));
    assert!(rule.declarations().iter().any(|declaration| {
        declaration.property() == ViewPropertyKind::OverflowX
            && matches!(
                declaration.value(),
                ViewSpecifiedValue::Overflow {
                    value: ViewOverflow::Scroll
                }
            )
    }));

    assert_eq!(program.scroll_regions.len(), 1);
    assert_scroll_regions_match_unstyled_baseline(
        source,
        "\n    .style(@style:.horizontal_scroll)",
        program,
    );
}
