use super::super::{PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest};
use super::{
    ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryPlatform,
    ViewGeometryRuntimeError,
};
use crate::fonts::DEFAULT_PLAYER_FONT_RESOURCE_BYTES;
use crate::images::BundleImageCatalog;
use crate::input::InputController;
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::view::{ViewObserveClassification, ViewTextSelectionPolicy};
use arcweft_bundle::resource_codec::{
    ViewRuntimeControlVisualStyle, ViewRuntimeSurface, ViewRuntimeSurfaceBounds,
    ViewTextBlockBounds,
};
use arcweft_presentation::appearance::{PresentationEnvironment, SystemPaletteSet};
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePath, BundleViewMountOutput, BundleViewStyleNode, BundleViewStyleNodeId,
    BundleViewStyleNodeKind, BundleViewTextOutput, BundleViewTextTarget, BundleViewTextValue,
};
use arcweft_view::geometry::ViewGeometryConsumer;
use arcweft_view::style::{
    ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewInheritedBoxAxes, ViewStyleNodeKey,
};
use arcweft_view::{ViewElementKind, ViewId, ViewMountId};
use std::sync::Arc;

struct GeometryFixture {
    presentation: BundlePresentationSnapshot,
    fx: FxDefinitions,
    images: BundleImageCatalog,
}

impl GeometryFixture {
    fn text(width_milli: u32) -> Self {
        let mount = ViewMountId::from_raw(1);
        let target = "text.fixture";
        let source = "source.fixture";
        let mut presentation = BundlePresentationSnapshot::default();
        let mut mounted = mount_with_node(
            mount,
            BundleViewStyleNodeKind::Text {
                text_source: source.to_owned(),
            },
            vec![target.to_owned()],
        );
        mounted.text.push(BundleViewTextOutput {
            source_id: source.to_owned(),
            targets: vec![BundleViewTextTarget {
                public_id: target.to_owned(),
                containing_scroll_region: None,
                bounds: ViewTextBlockBounds {
                    x_milli: 75_000,
                    y_milli: 90_000,
                    width_milli,
                    height_milli: 40_000,
                },
                selection_policy: ViewTextSelectionPolicy::Disabled,
                style: ViewRuntimeControlVisualStyle::default(),
            }],
            value: BundleViewTextValue::Plain {
                value: "Geometry fixture".to_owned(),
            },
            classification: ViewObserveClassification::default(),
            replacement: None,
        });
        presentation.view.mounts.push(mounted);
        Self {
            presentation,
            fx: FxDefinitions::default(),
            images: BundleImageCatalog::empty(),
        }
    }

    fn missing_button() -> Self {
        let mount = ViewMountId::from_raw(1);
        let mut presentation = BundlePresentationSnapshot::default();
        presentation.view.mounts.push(mount_with_node(
            mount,
            BundleViewStyleNodeKind::Element {
                element: ViewElementKind::Button,
                target: Some("button.missing".to_owned()),
            },
            vec!["button.missing".to_owned()],
        ));
        Self {
            presentation,
            fx: FxDefinitions::default(),
            images: BundleImageCatalog::empty(),
        }
    }

    fn surface(width_milli: u32) -> Self {
        let mount = ViewMountId::from_raw(1);
        let target = "surface.panel";
        let scoped = format!("view_mount_{}.{}", mount.get(), target);
        let mut presentation = BundlePresentationSnapshot::default();
        presentation.surfaces.push(ViewRuntimeSurface {
            public_id: scoped.clone(),
            target: scoped,
            view: None,
            containing_scroll_region: None,
            element: ViewElementKind::Panel,
            bounds: ViewRuntimeSurfaceBounds {
                x_milli: 20_000,
                y_milli: 30_000,
                width_milli,
                height_milli: 60_000,
            },
            style: ViewRuntimeControlVisualStyle::default(),
        });
        presentation.view.mounts.push(mount_with_node(
            mount,
            BundleViewStyleNodeKind::Element {
                element: ViewElementKind::Panel,
                target: Some(target.to_owned()),
            },
            vec![target.to_owned()],
        ));
        Self {
            presentation,
            fx: FxDefinitions::default(),
            images: BundleImageCatalog::empty(),
        }
    }

    fn row() -> Self {
        let mount = ViewMountId::from_raw(1);
        let mut mounted = mount_with_node(
            mount,
            BundleViewStyleNodeKind::Element {
                element: ViewElementKind::Row,
                target: None,
            },
            vec!["text.first".to_owned(), "text.second".to_owned()],
        );
        let parent = BundleViewStyleNodeId {
            path: BundleViewInstancePath::default(),
            instruction: 0,
        };
        mounted.style_nodes.extend([
            BundleViewStyleNode {
                path: BundleViewInstancePath::default(),
                instruction: 1,
                parent: Some(parent.clone()),
                kind: BundleViewStyleNodeKind::Text {
                    text_source: "source.first".to_owned(),
                },
                part: None,
                exported_part: None,
                applications: Vec::new(),
            },
            BundleViewStyleNode {
                path: BundleViewInstancePath::default(),
                instruction: 2,
                parent: Some(parent),
                kind: BundleViewStyleNodeKind::Text {
                    text_source: "source.second".to_owned(),
                },
                part: None,
                exported_part: None,
                applications: Vec::new(),
            },
        ]);
        mounted.text = vec![
            text_output("source.first", "text.first", 50_000, 20_000),
            text_output("source.second", "text.second", 80_000, 30_000),
        ];
        let mut presentation = BundlePresentationSnapshot::default();
        presentation.view.mounts.push(mounted);
        Self {
            presentation,
            fx: FxDefinitions::default(),
            images: BundleImageCatalog::empty(),
        }
    }

    fn transparent_parent() -> Self {
        let mount = ViewMountId::from_raw(1);
        let mut mounted = mount_with_node(
            mount,
            BundleViewStyleNodeKind::Custom {
                element: "Fragment".to_owned(),
            },
            vec!["text.child".to_owned()],
        );
        mounted.style_nodes.push(BundleViewStyleNode {
            path: BundleViewInstancePath::default(),
            instruction: 1,
            parent: Some(BundleViewStyleNodeId {
                path: BundleViewInstancePath::default(),
                instruction: 0,
            }),
            kind: BundleViewStyleNodeKind::Text {
                text_source: "source.child".to_owned(),
            },
            part: None,
            exported_part: None,
            applications: Vec::new(),
        });
        mounted.text = vec![text_output("source.child", "text.child", 90_000, 25_000)];
        let mut presentation = BundlePresentationSnapshot::default();
        presentation.view.mounts.push(mounted);
        Self {
            presentation,
            fx: FxDefinitions::default(),
            images: BundleImageCatalog::empty(),
        }
    }

    fn request(&self) -> PlayerFrameRequest<'_> {
        PlayerFrameRequest {
            presentation: &self.presentation,
            fx_definitions: &self.fx,
            images: &self.images,
            style_program: None,
            style_environment: &PresentationEnvironment::ENGINE_DEFAULT,
            style_palettes: &SystemPaletteSet::ENGINE_DEFAULT,
            viewport: RenderViewport {
                logical_width: 320.0,
                logical_height: 180.0,
                physical_width: 320,
                physical_height: 180,
                scale_factor: 1.0,
            },
            fit: PlayerFrameFit::raw(),
            image_time_millis: 0,
            visual_time_millis: 0,
            dialogue_reveal_complete: false,
            preferences: RenderPreferences::default(),
        }
    }
}

fn text_output(
    source_id: &str,
    public_id: &str,
    width_milli: u32,
    height_milli: u32,
) -> BundleViewTextOutput {
    BundleViewTextOutput {
        source_id: source_id.to_owned(),
        targets: vec![BundleViewTextTarget {
            public_id: public_id.to_owned(),
            containing_scroll_region: None,
            bounds: ViewTextBlockBounds {
                x_milli: 70_000,
                y_milli: 80_000,
                width_milli,
                height_milli,
            },
            selection_policy: ViewTextSelectionPolicy::Disabled,
            style: ViewRuntimeControlVisualStyle::default(),
        }],
        value: BundleViewTextValue::Plain {
            value: public_id.to_owned(),
        },
        classification: ViewObserveClassification::default(),
        replacement: None,
    }
}

fn planner_with_fonts() -> PlayerFramePlannerState {
    let mut planner = PlayerFramePlannerState::new();
    for bytes in DEFAULT_PLAYER_FONT_RESOURCE_BYTES {
        planner
            .register_font_bytes(bytes.to_vec())
            .expect("default font registers");
    }
    planner
}

fn mount_with_node(
    mount: ViewMountId,
    kind: BundleViewStyleNodeKind,
    active_targets: Vec<String>,
) -> BundleViewMountOutput {
    BundleViewMountOutput {
        handle: PresentationHandleId::try_new(format!("handle.{}", mount.get()))
            .expect("valid handle"),
        mount,
        view: ViewId::try_new("view.GeometryFixture").expect("valid View id"),
        path: BundleViewInstancePath::default(),
        host_axis_seed: Some(ViewInheritedBoxAxes::for_host_seed(
            mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
        )),
        dialogue: None,
        active_targets,
        active_images: Vec::new(),
        paint: Vec::new(),
        text: Vec::new(),
        fx: Vec::new(),
        style_nodes: vec![BundleViewStyleNode {
            path: BundleViewInstancePath::default(),
            instruction: 0,
            parent: None,
            kind,
            part: None,
            exported_part: None,
            applications: Vec::new(),
        }],
    }
}

#[test]
fn cache_candidate_stages_without_mutation_and_exact_hits_publish_one_live_entry() {
    let fixture = GeometryFixture::text(120_000);
    let mut planner = planner_with_fonts();
    let mut input = InputController::default();
    let node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);

    let candidate = planner
        .prepare_candidate(&input, fixture.request())
        .expect("candidate prepares");
    assert_eq!(planner.view_geometry.generation().value(), 0);
    assert_eq!(planner.view_geometry.cache_counts(), (0, 0, 0));
    let geometry = candidate
        .view_geometry()
        .final_geometry(&node)
        .expect("final node");
    assert_eq!(geometry.border_box.size().width_milli, 120_000);
    assert_eq!(geometry.border_box.left_milli, 0);

    planner
        .publication_guard()
        .publish_with(candidate, &mut input, |_| ())
        .expect("candidate publishes");
    assert_eq!(planner.view_geometry.generation().value(), 1);
    assert_eq!(planner.view_geometry.cache_counts(), (1, 1, 1));
    let measured = planner
        .view_geometry
        .measure_entry(&node)
        .expect("measure entry")
        .clone();
    let placed = planner
        .view_geometry
        .place_entry(&node)
        .expect("place entry")
        .clone();
    let final_entry = planner
        .view_geometry
        .final_entry(&node)
        .expect("final entry")
        .clone();

    let all_hit = planner
        .prepare_candidate(&input, fixture.request())
        .expect("all-hit candidate prepares");
    assert_eq!(planner.view_geometry.generation().value(), 1);
    planner
        .publication_guard()
        .publish_with(all_hit, &mut input, |_| ())
        .expect("all-hit candidate publishes");

    assert_eq!(planner.view_geometry.generation().value(), 2);
    assert_eq!(planner.view_geometry.cache_counts(), (1, 1, 1));
    assert_eq!(planner.view_geometry.measure_entry(&node), Some(&measured));
    assert_eq!(planner.view_geometry.place_entry(&node), Some(&placed));
    assert_eq!(planner.view_geometry.final_entry(&node), Some(&final_entry));
}

#[test]
fn supported_surface_product_contributes_container_intrinsic_size_not_authored_position() {
    let fixture = GeometryFixture::surface(140_000);
    let planner = PlayerFramePlannerState::new();
    let input = InputController::default();
    let node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);

    let candidate = planner
        .prepare_candidate(&input, fixture.request())
        .expect("surface candidate prepares");
    let geometry = candidate
        .view_geometry()
        .final_geometry(&node)
        .expect("surface final node");

    assert_eq!(geometry.border_box.left_milli, 0);
    assert_eq!(geometry.border_box.top_milli, 0);
    assert_eq!(geometry.border_box.size().width_milli, 140_000);
    assert_eq!(geometry.border_box.size().height_milli, 60_000);
}

#[test]
fn row_container_measures_and_places_children_in_authored_order() {
    let fixture = GeometryFixture::row();
    let planner = planner_with_fonts();
    let input = InputController::default();
    let root = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);
    let first = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 1);
    let second = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 2);

    let candidate = planner
        .prepare_candidate(&input, fixture.request())
        .expect("row candidate prepares");
    let frame = candidate.view_geometry();
    let root = frame.final_geometry(&root).expect("root geometry");
    let first = frame.final_geometry(&first).expect("first geometry");
    let second = frame.final_geometry(&second).expect("second geometry");

    assert_eq!(root.content_box.size().width_milli, 130_000);
    assert_eq!(root.content_box.size().height_milli, 30_000);
    assert_eq!(first.border_box.left_milli, 0);
    assert_eq!(first.border_box.right_milli, 50_000);
    assert_eq!(second.border_box.left_milli, 50_000);
    assert_eq!(second.border_box.right_milli, 130_000);
}

#[test]
fn transparent_parent_collapses_without_creating_a_second_geometry_owner() {
    let fixture = GeometryFixture::transparent_parent();
    let planner = planner_with_fonts();
    let input = InputController::default();
    let transparent = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);
    let child = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 1);

    let candidate = planner
        .prepare_candidate(&input, fixture.request())
        .expect("transparent tree prepares");
    let frame = candidate.view_geometry();

    assert!(frame.is_transparent(&transparent));
    assert!(frame.final_geometry(&transparent).is_none());
    assert_eq!(frame.final_nodes().len(), 1);
    assert_eq!(
        frame
            .final_geometry(&child)
            .expect("child owns geometry")
            .border_box
            .size()
            .width_milli,
        90_000
    );
}

#[test]
fn intrinsic_change_misses_exact_measure_key_without_mutating_old_cache_before_publish() {
    let initial = GeometryFixture::text(120_000);
    let changed = GeometryFixture::text(200_000);
    let mut planner = planner_with_fonts();
    let mut input = InputController::default();
    let node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);
    let initial_candidate = planner
        .prepare_candidate(&input, initial.request())
        .expect("initial frame prepares");
    planner
        .publication_guard()
        .publish_with(initial_candidate, &mut input, |_| ())
        .expect("initial frame publishes");
    let initial_measure = planner
        .view_geometry
        .measure_entry(&node)
        .expect("initial measure")
        .clone();

    let candidate = planner
        .prepare_candidate(&input, changed.request())
        .expect("changed candidate prepares");
    assert_eq!(
        planner.view_geometry.measure_entry(&node),
        Some(&initial_measure)
    );
    assert_eq!(
        candidate
            .view_geometry()
            .final_geometry(&node)
            .expect("changed final node")
            .border_box
            .size()
            .width_milli,
        200_000
    );

    planner
        .publication_guard()
        .publish_with(candidate, &mut input, |_| ())
        .expect("changed candidate publishes");
    assert_ne!(
        planner.view_geometry.measure_entry(&node),
        Some(&initial_measure)
    );
}

#[test]
fn product_failure_preserves_published_frame_cache_generation_and_input() {
    let valid = GeometryFixture::text(120_000);
    let invalid = GeometryFixture::missing_button();
    let mut planner = planner_with_fonts();
    let mut input = InputController::default();
    let initial_candidate = planner
        .prepare_candidate(&input, valid.request())
        .expect("initial frame prepares");
    let published = planner
        .publication_guard()
        .publish_with(initial_candidate, &mut input, |_| ())
        .expect("initial frame publishes")
        .0;
    let published_geometry = published.view_geometry().clone();
    let input_before = input.clone();
    let counts_before = planner.view_geometry.cache_counts();

    let error = planner
        .prepare_candidate(&input, invalid.request())
        .expect_err("missing product rejects candidate");
    assert!(matches!(
        error,
        PlayerFrameError::ViewGeometry(ViewGeometryRuntimeError::Product(_))
    ));
    assert_eq!(planner.view_geometry.generation().value(), 1);
    assert_eq!(planner.view_geometry.cache_counts(), counts_before);
    assert_eq!(input, input_before);
    assert!(Arc::ptr_eq(
        planner
            .published_frame
            .as_ref()
            .expect("published frame retained")
            .view_geometry(),
        &published_geometry
    ));
}

#[test]
fn web_zero_viewport_remains_zero_without_adapter_clamping() {
    let viewport = ViewGeometryConversionError::viewport_input(ViewGeometryPlatform::Web, 0.0, 0.0)
        .expect("zero viewport is valid");

    assert_eq!(viewport.rect.left_milli, 0);
    assert_eq!(viewport.rect.top_milli, 0);
    assert_eq!(viewport.rect.right_milli, 0);
    assert_eq!(viewport.rect.bottom_milli, 0);
}

#[test]
fn native_pointer_conversion_floors_to_milli_without_zero_fallback() {
    assert_eq!(
        ViewGeometryConversionError::logical_pointer(
            ViewGeometryPlatform::Native,
            ViewGeometryConversionField::Left,
            -0.0001,
        )
        .expect("finite pointer converts")
        .to_bits(),
        (-0.001_f32).to_bits()
    );
    assert_eq!(
        ViewGeometryConversionError::logical_pointer(
            ViewGeometryPlatform::Native,
            ViewGeometryConversionField::Top,
            f64::NAN,
        ),
        Err(ViewGeometryConversionError::NonFiniteInput {
            node: None,
            platform: ViewGeometryPlatform::Native,
            consumer: ViewGeometryConsumer::HitTest,
            field: ViewGeometryConversionField::Top,
            value_bits: f64::NAN.to_bits(),
        })
    );
}
