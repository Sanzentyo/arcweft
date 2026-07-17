use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_player_scene::{
    frame::{
        PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
        ViewGeometryGeneration, ViewGeometryRuntimeError,
    },
    images::BundleImageCatalog,
    input::InputController,
};
use arcweft_presentation::appearance::{PresentationEnvironment, SystemPaletteSet};
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use std::cell::Cell;
use std::sync::Arc;

struct FrameFixture {
    presentation: BundlePresentationSnapshot,
    fx: FxDefinitions,
    images: BundleImageCatalog,
}

impl FrameFixture {
    fn new() -> Self {
        Self {
            presentation: BundlePresentationSnapshot::default(),
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

#[test]
fn tx_success_atomically_publishes_candidate_arc_and_invokes_adapter_once() {
    let fixture = FrameFixture::new();
    let mut planner = PlayerFramePlannerState::new();
    let mut input = InputController::default();
    let candidate = planner
        .prepare_candidate(&input, fixture.request())
        .expect("candidate prepares");
    let candidate_geometry = candidate.view_geometry().clone();
    assert_eq!(candidate_geometry.generation().value(), 1);

    let commits = Cell::new(0_u32);
    let (published, receipt) = planner
        .publication_guard()
        .publish_with(candidate, &mut input, |frame| {
            commits.set(commits.get() + 1);
            assert!(Arc::ptr_eq(frame.view_geometry(), &candidate_geometry));
            frame.view_geometry().generation()
        })
        .expect("candidate publishes");

    assert_eq!(commits.get(), 1);
    assert_eq!(receipt.value(), 1);
    assert!(Arc::ptr_eq(published.view_geometry(), &candidate_geometry));

    let next = planner
        .prepare_candidate(&input, fixture.request())
        .expect("next candidate prepares");
    assert_eq!(next.view_geometry().generation().value(), 2);
}

#[test]
fn tx_stale_generation_never_invokes_adapter_or_changes_input() {
    let fixture = FrameFixture::new();
    let mut planner = PlayerFramePlannerState::new();
    let mut input = InputController::default();
    let stale = planner
        .prepare_candidate(&input, fixture.request())
        .expect("stale candidate prepares");
    let winner = planner
        .prepare_candidate(&input, fixture.request())
        .expect("winner candidate prepares");

    let (published, ()) = planner
        .publication_guard()
        .publish_with(winner, &mut input, |_| ())
        .expect("winner publishes");
    let published_geometry = published.view_geometry().clone();
    let input_before_stale = input.clone();
    let commits = Cell::new(0_u32);

    let error = planner
        .publication_guard()
        .publish_with(stale, &mut input, |_| commits.set(commits.get() + 1))
        .expect_err("stale candidate is rejected");

    assert_eq!(
        error,
        PlayerFrameError::ViewGeometry(ViewGeometryRuntimeError::StalePreparedGeneration {
            base: ViewGeometryGeneration::ZERO,
            current: published_geometry.generation(),
        })
    );
    assert_eq!(commits.get(), 0);
    assert_eq!(input, input_before_stale);
    assert_eq!(published_geometry.generation().value(), 1);

    let next = planner
        .prepare_candidate(&input, fixture.request())
        .expect("state remains publishable");
    assert_eq!(next.view_geometry().generation().value(), 2);
}

#[test]
fn tx_headless_preflight_rejects_stale_before_side_effecting_staging() {
    let fixture = FrameFixture::new();
    let mut planner = PlayerFramePlannerState::new();
    let mut input = InputController::default();
    let stale = planner
        .prepare_candidate(&input, fixture.request())
        .expect("stale candidate prepares");
    let winner = planner
        .prepare_candidate(&input, fixture.request())
        .expect("winner candidate prepares");

    planner
        .publication_guard()
        .publish_with(winner, &mut input, |_| ())
        .expect("winner publishes");

    let staging_calls = Cell::new(0_u32);
    let result = planner
        .publication_guard()
        .preflight_candidate(&stale)
        .inspect(|_| {
            staging_calls.set(staging_calls.get() + 1);
        });

    assert!(matches!(
        result,
        Err(PlayerFrameError::ViewGeometry(
            ViewGeometryRuntimeError::StalePreparedGeneration { .. }
        ))
    ));
    assert_eq!(staging_calls.get(), 0);
    drop(result);

    let next = planner
        .prepare_candidate(&input, fixture.request())
        .expect("state remains publishable");
    assert_eq!(next.view_geometry().generation().value(), 2);
}
