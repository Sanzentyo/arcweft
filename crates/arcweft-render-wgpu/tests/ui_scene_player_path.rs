use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedUiGlyphRunHandoff, PreparedUiScene,
    PreparedUiSceneResources, RenderPreferences, RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::ui_compositor::UiCompositorPlan;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2D, UiColorRgba8, UiCompositingEffects, UiCompositingGroup, UiFilter, UiFilterList,
    UiGlyphRun, UiPaintNode, UiPrimitive, UiPrimitiveRange, UiScene, UiSceneContext, UiSolidRect,
};

fn viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 320.0,
        logical_height: 180.0,
        physical_width: 640,
        physical_height: 360,
        scale_factor: 2.0,
    }
}

fn empty_scene() -> RenderScene {
    RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: viewport(),
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    }
}

fn white() -> UiColorRgba8 {
    UiColorRgba8 {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    }
}

fn direct_scene() -> UiScene {
    let mut scene = UiScene::new(320.0, 180.0);
    scene.push_primitive(UiPrimitive::SolidRect(UiSolidRect {
        bounds: HitRect::new(12.0, 16.0, 80.0, 40.0),
        color: white(),
    }));
    scene.push_paint_node(UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2D::default(),
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start: 0, end: 1 },
    }));
    scene
}

#[test]
fn ui_scene_attaches_to_prepared_frame_without_replacing_base_fields() {
    let prepared = SharedFramePlanner::prepare(&empty_scene())
        .expect("base prepared frame")
        .with_ui_scenes([PreparedUiScene::new(direct_scene())]);

    assert_eq!(prepared.ui_scenes().len(), 1);
    assert_eq!(prepared.ui_scenes()[0].scene.primitives().len(), 1);
    assert!(!prepared.rectangles.is_empty(), "base background remains");
}

#[test]
fn direct_only_ui_scene_produces_compositor_direct_plan() {
    let scene = direct_scene();
    let plan = UiCompositorPlan::from_scene(&scene, 2.0);

    assert_eq!(plan.nodes().len(), 1);
    assert_eq!(plan.backdrop_copy_count(), 0);
    assert!(plan.offscreen_target_count() >= 1);
}

#[test]
fn filter_and_backdrop_scene_plans_offscreen_and_one_backdrop_copy() {
    let effects = UiCompositingEffects {
        filters: UiFilterList::new([UiFilter::DropShadow {
            offset_x_px: 4.0,
            offset_y_px: 8.0,
            blur_radius_px: 6.0,
            color: white(),
        }]),
        backdrop_filters: UiFilterList::new([UiFilter::Blur { radius_px: 4.0 }]),
        ..UiCompositingEffects::default()
    };
    let group = UiCompositingGroup::new(HitRect::new(0.0, 0.0, 160.0, 90.0), effects)
        .with_children(vec![UiPaintNode::Direct(UiSceneContext {
            transform: UiAffine2D::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 0, end: 1 },
        })]);
    let mut scene = direct_scene();
    scene.replace_paint_nodes(vec![UiPaintNode::Group(group)]);

    let plan = UiCompositorPlan::from_scene(&scene, 1.0);

    assert!(plan.offscreen_target_count() >= 2);
    assert_eq!(plan.backdrop_copy_count(), 1);
}

#[test]
fn glyph_run_requires_explicit_text_handoff() {
    let mut resources = PreparedUiSceneResources::default();
    resources.push_glyph_handoff(PreparedUiGlyphRunHandoff {
        run_index: 7,
        prepared_text_index: 0,
    });
    let mut scene = UiScene::new(320.0, 180.0);
    scene.push_primitive(UiPrimitive::GlyphRun(UiGlyphRun {
        run_index: 7,
        bounds: HitRect::new(0.0, 0.0, 40.0, 20.0),
        color: white(),
    }));
    scene.push_paint_node(UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2D::default(),
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start: 0, end: 1 },
    }));

    let prepared_ui = PreparedUiScene::new(scene).with_resources(resources);

    assert_eq!(prepared_ui.resources.glyph_handoffs()[0].run_index, 7);
}
