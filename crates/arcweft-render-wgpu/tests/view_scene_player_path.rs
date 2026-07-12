use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedViewScene, RenderPreferences, RenderScene,
    RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::view_compositor::ViewCompositorPlan;
use arcweft_render_wgpu::view_scene::{
    PreparedTextId, ViewAffine2D, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewFilter, ViewFilterList, ViewPaintNode, ViewPrimitive, ViewPrimitiveRange, ViewScene,
    ViewSceneContext, ViewSolidRect, ViewTextPrimitive,
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
        content_avoidance_regions: Vec::new(),
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
        scroll_regions: Vec::new(),
    }
}

fn white() -> ViewColorRgba8 {
    ViewColorRgba8 {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    }
}

fn direct_scene() -> ViewScene {
    let mut scene = ViewScene::new(320.0, 180.0);
    scene.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
        bounds: HitRect::new(12.0, 16.0, 80.0, 40.0),
        color: white(),
    }));
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::default(),
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
    }));
    scene
}

#[test]
fn view_scene_attaches_to_prepared_frame_without_replacing_base_fields() {
    let prepared = SharedFramePlanner::prepare(&empty_scene())
        .expect("base prepared frame")
        .with_view_scenes([PreparedViewScene::new(direct_scene())]);

    assert_eq!(prepared.view_scenes().len(), 1);
    assert_eq!(prepared.view_scenes()[0].scene.primitives().len(), 1);
    assert!(!prepared.rectangles.is_empty(), "base background remains");
}

#[test]
fn direct_only_view_scene_produces_compositor_direct_plan() {
    let scene = direct_scene();
    let plan = ViewCompositorPlan::from_scene(&scene, 2.0);

    assert_eq!(plan.nodes().len(), 1);
    assert_eq!(plan.backdrop_copy_count(), 0);
    assert!(plan.offscreen_target_count() >= 1);
}

#[test]
fn filter_and_backdrop_scene_plans_offscreen_and_one_backdrop_copy() {
    let effects = ViewCompositingEffects {
        filters: ViewFilterList::new([ViewFilter::DropShadow {
            offset_x_px: 4.0,
            offset_y_px: 8.0,
            blur_radius_px: 6.0,
            color: white(),
        }]),
        backdrop_filters: ViewFilterList::new([ViewFilter::Blur { radius_px: 4.0 }]),
        ..ViewCompositingEffects::default()
    };
    let group = ViewCompositingGroup::new(HitRect::new(0.0, 0.0, 160.0, 90.0), effects)
        .with_children(vec![ViewPaintNode::Direct(ViewSceneContext {
            transform: ViewAffine2D::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        })]);
    let mut scene = direct_scene();
    scene.replace_paint_nodes(vec![ViewPaintNode::Group(group)]);

    let plan = ViewCompositorPlan::from_scene(&scene, 1.0);

    assert!(plan.offscreen_target_count() >= 2);
    assert_eq!(plan.backdrop_copy_count(), 1);
}

#[test]
fn text_primitive_references_canonical_prepared_item_directly() {
    let mut scene = ViewScene::new(320.0, 180.0);
    scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive {
        text: PreparedTextId::from_index(0),
    }));
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::default(),
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
    }));

    let prepared_view = PreparedViewScene::new(scene);

    assert_eq!(
        prepared_view.scene.primitives(),
        [ViewPrimitive::Text(ViewTextPrimitive {
            text: PreparedTextId::from_index(0),
        })]
    );
}
