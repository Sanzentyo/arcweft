use arcweft_takumi_adapter::{
    ComputedDirectPaintExtractor, ComputedDirectPaintInput, DirectPaintResourceTable,
    TakumiAdapter, TakumiAdapterInput, TakumiCssBundle,
};
use arcweft_ui::{ContainerKind, FragmentKind, NodeKey, StyleId, ViewFragmentBuilder};
use takumi::prelude::{Fonts, Viewport};
use takumi::unstable::base::{
    context::RenderContext,
    layout::{
        style::{ComputedStyle, SizingContext},
        tree::RenderNode,
    },
};

fn render_tree_for(css: &str) -> (RenderNode, arcweft_takumi_adapter::TakumiMetadataMap) {
    let mut builder = ViewFragmentBuilder::default();
    let root = builder
        .push_node(
            NodeKey(1),
            FragmentKind::Container(ContainerKind::Block),
            StyleId(1),
            &[],
            &[],
            None,
        )
        .expect("root node builds");
    let fragment = builder.finish();
    let text = arcweft_takumi_adapter::ArcweftTextLayoutBridge::default();
    let adapted = TakumiAdapter::adapt(&TakumiAdapterInput {
        fragment: &fragment,
        root,
        stylesheets: TakumiCssBundle::new([css]),
        text: &text,
        view: None,
        program: None,
        node_parts: &[],
        agent: None,
    })
    .expect("adapter output");

    let render_context = RenderContext::builder()
        .fonts(Fonts::default().snapshot_with_fallbacks(None))
        .sizing(
            SizingContext::builder()
                .viewport(Viewport::default())
                .build(),
        )
        .stylesheet(std::rc::Rc::new(adapted.stylesheet))
        .time_ms(0)
        .draw_debug_border(false)
        .style(Box::<ComputedStyle>::default())
        .build();

    (
        RenderNode::from_node(&render_context, adapted.node),
        adapted.metadata,
    )
}

#[test]
fn computed_direct_paint_background_color_extracts_solid_rect_layer() {
    let (root, metadata) =
        render_tree_for(".aw-block { width: 64px; height: 32px; background-color: #336699; }");

    let output = ComputedDirectPaintExtractor::extract(ComputedDirectPaintInput {
        root: &root,
        metadata: &metadata,
        resources: &DirectPaintResourceTable::default(),
    });

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.catalog.entries().len(), 1);
    assert_eq!(output.catalog.entries()[0].1.backgrounds.len(), 1);
    assert_eq!(output.evidence.records()[0].layers().len(), 1);
}

#[test]
fn computed_direct_paint_border_radius_adds_rounded_clip_metadata() {
    let (root, metadata) = render_tree_for(
        ".aw-block { width: 64px; height: 32px; background-color: red; border-radius: 8px; }",
    );

    let output = ComputedDirectPaintExtractor::extract(ComputedDirectPaintInput {
        root: &root,
        metadata: &metadata,
        resources: &DirectPaintResourceTable::default(),
    });

    let paint = &output.catalog.entries()[0].1;
    assert!(paint.clip.is_some());
    assert!(
        output.evidence.records()[0]
            .layers()
            .iter()
            .any(|layer| matches!(
                layer.kind(),
                arcweft_takumi_adapter::DirectPaintLayerKind::RoundedClip
            ))
    );
}

#[test]
fn computed_direct_paint_linear_gradient_extracts_gradient_layer() {
    let css = ".aw-block { width: 64px; height: 32px; background-image: linear-gradient(90deg, red 0%, blue 100%); }";
    let (root, metadata) = render_tree_for(css);

    let output = ComputedDirectPaintExtractor::extract(ComputedDirectPaintInput {
        root: &root,
        metadata: &metadata,
        resources: &DirectPaintResourceTable::default(),
    });

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.catalog.entries()[0].1.backgrounds.len(), 1);
}

#[test]
fn computed_direct_paint_missing_image_records_resource_requirement_without_io() {
    let css = ".aw-block { width: 64px; height: 32px; background-image: url(\"arcweft://image/missing\"); }";
    let (root, metadata) = render_tree_for(css);

    let output = ComputedDirectPaintExtractor::extract(ComputedDirectPaintInput {
        root: &root,
        metadata: &metadata,
        resources: &DirectPaintResourceTable::default(),
    });

    assert_eq!(output.resource_requirements.len(), 1);
    assert!(!output.diagnostics.is_empty());
    assert!(output.catalog.entries().is_empty());
}

#[test]
fn computed_direct_paint_supported_layer_survives_unsupported_layer() {
    let css = ".aw-block { width: 64px; height: 32px; background-color: #111827; background-image: radial-gradient(circle, red 0%, transparent 100%); }";
    let (root, metadata) = render_tree_for(css);

    let output = ComputedDirectPaintExtractor::extract(ComputedDirectPaintInput {
        root: &root,
        metadata: &metadata,
        resources: &DirectPaintResourceTable::default(),
    });

    assert!(!output.diagnostics.is_empty());
    assert_eq!(output.catalog.entries()[0].1.backgrounds.len(), 1);
}
