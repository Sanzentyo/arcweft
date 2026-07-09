use arcweft_takumi_adapter::{
    CSS_COVERAGE_MATRIX, CssCascadeLayer, CssCascadePriority, CssCoverageFeature,
    CssCoverageReport, CssCoverageStatus, CssInvalidationClass, CssMatchedDeclaration,
    CssSelectorCoverage, CssSpecificity, TakumiDiagnosticCode, winning_declaration,
};

#[test]
fn css_layout_cascade_supported_selector_subset_reports_specificity() {
    let selector = CssSelectorCoverage::analyze("div.aw-card > #title[data-aw-part=\"3\"]:hover");

    assert!(selector.is_supported());
    assert_eq!(selector.specificity(), CssSpecificity::new(1, 3, 1));
}

#[test]
fn css_layout_cascade_pseudo_elements_and_structural_selectors_are_diagnostic_driven() {
    let pseudo_element = CssSelectorCoverage::analyze(".card::before");
    let structural = CssSelectorCoverage::analyze(".card:nth-child(2)");

    assert_eq!(
        pseudo_element.diagnostics()[0].code(),
        TakumiDiagnosticCode::UnsupportedCssSelector
    );
    assert_eq!(
        structural.diagnostics()[0].code(),
        TakumiDiagnosticCode::UnsupportedCssSelector
    );
}

#[test]
fn css_layout_cascade_specificity_then_source_order_chooses_winner() {
    let earlier = CssMatchedDeclaration::new(
        ".card.title",
        "color",
        "red",
        CssCascadePriority::new(CssCascadeLayer::CssView, CssSpecificity::new(0, 2, 0), 1),
    );
    let later = CssMatchedDeclaration::new(
        ".panel .title",
        "color",
        "blue",
        CssCascadePriority::new(CssCascadeLayer::CssView, CssSpecificity::new(0, 2, 0), 2),
    );

    assert_eq!(
        winning_declaration(&[earlier, later]).unwrap().value(),
        "blue"
    );
}

#[test]
fn css_layout_cascade_arcweft_and_css_layers_resolve_deterministically() {
    let arcweft = CssCascadePriority::new(
        CssCascadeLayer::ArcweftView,
        CssSpecificity::new(1, 0, 0),
        99,
    );
    let css = CssCascadePriority::new(CssCascadeLayer::CssBase, CssSpecificity::new(0, 1, 0), 1);

    assert!(css.is_stronger_than(arcweft));
}

#[test]
fn css_layout_cascade_important_beats_later_non_important_source_order() {
    let important =
        CssCascadePriority::new(CssCascadeLayer::CssBase, CssSpecificity::new(0, 1, 0), 1)
            .important();
    let later =
        CssCascadePriority::new(CssCascadeLayer::CssBase, CssSpecificity::new(1, 0, 0), 999);

    assert!(important.is_stronger_than(later));
}

#[test]
fn css_layout_cascade_custom_property_resolution_is_either_represented_or_diagnostic() {
    let ok = CssCoverageReport::analyze_css(
        ".theme { --accent: #fff; } .card { color: var(--accent); }",
    );
    let missing = CssCoverageReport::analyze_css(".card { color: var(--missing); }");
    let fallback = CssCoverageReport::analyze_css(".card { color: var(--optional, #fff); }");

    assert!(ok.is_direct_wgpu_ready());
    assert!(fallback.is_direct_wgpu_ready());
    assert_eq!(
        missing.diagnostics()[0].code(),
        TakumiDiagnosticCode::UnresolvedCssVariable
    );
    assert!(ok.declarations().iter().any(|declaration| {
        declaration.property() == "--accent"
            && declaration.status() == CssCoverageStatus::ProductDataOnly
    }));
}

#[test]
fn css_layout_cascade_unsupported_grid_container_and_media_features_are_diagnosed() {
    let report = CssCoverageReport::analyze_css(
        "@media (min-width: 800px) { .card { display: flex; } }
         @container card (width > 10px) { .card { color: red; } }
         .cards { display: grid; grid-template-columns: 1fr 1fr; }",
    );

    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == TakumiDiagnosticCode::CssCoverageGap)
    );
    assert!(report.at_rules().iter().any(|rule| {
        rule.rule() == "container" && rule.status() == CssCoverageStatus::IntentionallyRejected
    }));
    assert!(report.declarations().iter().any(|declaration| {
        declaration.property() == "grid-template-columns"
            && declaration.status() == CssCoverageStatus::StructuredDiagnostic
    }));
}

#[test]
fn css_layout_cascade_layout_and_paint_invalidation_classes_are_exposed() {
    let report = CssCoverageReport::analyze_css(".card { opacity: 0.8; gap: 8px; }");
    let opacity = report
        .declarations()
        .iter()
        .find(|declaration| declaration.property() == "opacity")
        .expect("opacity declaration");
    let gap = report
        .declarations()
        .iter()
        .find(|declaration| declaration.property() == "gap")
        .expect("gap declaration");

    assert_eq!(opacity.invalidation(), CssInvalidationClass::PaintOnly);
    assert_eq!(gap.invalidation(), CssInvalidationClass::LayoutScene);
}

#[test]
fn css_layout_cascade_flex_gap_padding_fixture_is_supported_but_grid_is_not() {
    let report = CssCoverageReport::analyze_css(include_str!(
        "../../../fixtures/css-layout-cascade-coverage/coverage.css"
    ));

    assert!(report.declarations().iter().any(|declaration| {
        declaration.property() == "display"
            && declaration.value() == "flex"
            && declaration.status() == CssCoverageStatus::SupportedNow
    }));
    assert!(report.declarations().iter().any(|declaration| {
        declaration.property() == "gap"
            && declaration.invalidation() == CssInvalidationClass::LayoutScene
    }));
    assert!(report.declarations().iter().any(|declaration| {
        declaration.property() == "padding"
            && declaration.invalidation() == CssInvalidationClass::LayoutScene
    }));
    assert!(report.declarations().iter().any(|declaration| {
        declaration.property() == "grid-template-columns"
            && declaration.status() == CssCoverageStatus::StructuredDiagnostic
    }));
}

#[test]
fn css_layout_cascade_matrix_keeps_future_work_explicit() {
    assert!(CSS_COVERAGE_MATRIX.iter().any(|row| {
        row.feature() == CssCoverageFeature::GridLayout
            && row.status() == CssCoverageStatus::StructuredDiagnostic
    }));
    assert!(CSS_COVERAGE_MATRIX.iter().any(|row| {
        row.feature() == CssCoverageFeature::ContainerQuery
            && row.status() == CssCoverageStatus::IntentionallyRejected
    }));
}

#[test]
fn css_layout_cascade_visual_smoke_manifest_names_two_sizes_and_hidpi() {
    let manifest =
        include_str!("../../../fixtures/css-layout-cascade-coverage/visual-smoke-manifest.json");

    assert!(manifest.contains("\"default\""));
    assert!(manifest.contains("\"compact\""));
    assert!(manifest.contains("\"hidpi\""));
    assert!(manifest.contains("\"scale\": 2.0"));
}
