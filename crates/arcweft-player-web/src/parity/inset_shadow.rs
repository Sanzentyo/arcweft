//! Deterministic retained player frame shared by the native and Web exact PNG probes.

use arcweft_bundle::container::ArtifactIdentity;
use arcweft_core::plan::EntryRuntimeId;
use arcweft_core::step::{RuntimeStepMode, RuntimeStepStopReason};
use arcweft_player_scene::{
    frame::{
        PlayerFrameError, PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest, PlayerFrameTime,
    },
    images::BundleImageCatalog,
    input::InputController,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::{
    geometry::{PreparedFrame, RenderPreferences, RenderViewport},
    view_scene::{ViewBoxShadowKind, ViewPaintNode},
};
use arcweft_runtime_driver::{
    clock::{RuntimeClockError, RuntimeClockStep},
    session::{BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput},
};
use serde::Serialize;
use thiserror::Error;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 180;
const ENTRY: &str = "entry.main";
const STEPS: u64 = 4;
const STEP_MILLIS: u32 = 16;

pub struct InsetShadowExactFrame {
    pub frame: PreparedFrame,
    pub bundle_identity: ArtifactIdentity,
    pub stats: InsetShadowExactStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct InsetShadowExactStats {
    pub runtime_surfaces: usize,
    pub player_view_scenes: usize,
    pub primitives: usize,
    pub paint_nodes: usize,
    pub outer_shadows: usize,
    pub inset_shadows: usize,
}

#[derive(Debug, Error)]
pub enum InsetShadowExactError {
    #[error(transparent)]
    Session(#[from] BundleSessionError),
    #[error(transparent)]
    Clock(#[from] RuntimeClockError),
    #[error(transparent)]
    Frame(#[from] PlayerFrameError),
    #[error("exact capture does not have an AWFB artifact identity")]
    MissingArtifactIdentity,
    #[error("exact capture runtime failed on tick {tick}: {diagnostics:?}")]
    Runtime { tick: u64, diagnostics: Vec<String> },
    #[error("exact capture got an unexpected step outcome on tick {tick}: {stop_reason:?}")]
    UnexpectedStep {
        tick: u64,
        stop_reason: RuntimeStepStopReason,
    },
    #[error("exact capture requires the two mounted surfaces and no image assets")]
    InvalidPresentation,
    #[error("exact capture requires two filled cards with one outer and two inset shadows")]
    InvalidScene,
}

/// Evaluates the compiled fixture and plans its fixed fourth-tick viewport.
pub fn prepare_inset_shadow_exact_frame(
    bundle_bytes: &[u8],
) -> Result<InsetShadowExactFrame, InsetShadowExactError> {
    let entry = EntryRuntimeId::from_source_entity_body(ENTRY).map_err(|error| {
        BundleSessionError::InvalidEntrySelection {
            entry: ENTRY.to_owned(),
            message: error.to_string(),
        }
    })?;
    let mut session = BundleSession::from_awfb_bytes(
        bundle_bytes,
        BundleSessionOptions {
            entry: Some(entry),
            mode: RuntimeStepMode::Drain,
            max_ops: 128,
            ..BundleSessionOptions::default()
        },
    )?;
    let bundle_identity = session
        .active_container_artifact_identity()
        .ok_or(InsetShadowExactError::MissingArtifactIdentity)?;
    for tick in 1..=STEPS {
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, STEP_MILLIS)?,
            BundleStepInput::default(),
        );
        if !step.diagnostics.is_empty() || !step.presentation.view.diagnostics.is_empty() {
            let mut diagnostics = step.diagnostics;
            diagnostics.extend(
                step.presentation
                    .view
                    .diagnostics
                    .iter()
                    .map(|row| format!("{row:?}")),
            );
            return Err(InsetShadowExactError::Runtime { tick, diagnostics });
        }
        let expected_stop = if tick == 1 {
            RuntimeStepStopReason::Output
        } else {
            RuntimeStepStopReason::Blocked
        };
        if step.stop_reason != expected_stop
            || !step.assertion_failures.is_empty()
            || !step.requested_host_calls.is_empty()
            || !step.requested_tasks.is_empty()
        {
            return Err(InsetShadowExactError::UnexpectedStep {
                tick,
                stop_reason: step.stop_reason,
            });
        }
    }
    let presentation = session.presentation();
    let runtime_surfaces = presentation
        .view
        .mounts
        .iter()
        .map(|mount| mount.surface_nodes().count())
        .sum::<usize>();
    if runtime_surfaces != 2 || !presentation.images.is_empty() {
        return Err(InsetShadowExactError::InvalidPresentation);
    }
    let prepared = PlayerFramePlanner::prepare(
        &mut InputController::default(),
        PlayerFrameRequest {
            presentation,
            fx_definitions: session.fx_definitions(),
            images: &BundleImageCatalog::empty(),
            style_program: session.view_style_program(),
            style_environment: &session.presentation_environment(),
            style_palettes: session.view_style_palettes(),
            viewport: RenderViewport {
                logical_width: 320.0,
                logical_height: 180.0,
                physical_width: WIDTH,
                physical_height: HEIGHT,
                scale_factor: 1.0,
            },
            fit: PlayerFrameFit::raw(),
            time: PlayerFrameTime::runtime(STEPS * u64::from(STEP_MILLIS)),
            preferences: RenderPreferences::default(),
        },
    )?;
    let [scene] = prepared.frame.view_scenes() else {
        return Err(InsetShadowExactError::InvalidScene);
    };
    let [ViewPaintNode::Group(first), ViewPaintNode::Group(second)] = scene.scene.paint_nodes()
    else {
        return Err(InsetShadowExactError::InvalidScene);
    };
    let first_shadows = first.effects.box_shadows.shadows();
    let second_shadows = second.effects.box_shadows.shadows();
    if first.bounds != HitRect::new(24.0, 24.0, 112.0, 72.0)
        || second.bounds != HitRect::new(176.0, 40.0, 112.0, 72.0)
        || scene.scene.primitives().len() != 2
        || !matches!(first_shadows, [shadow] if shadow.kind == ViewBoxShadowKind::Inset)
        || !matches!(second_shadows, [outer, inset] if outer.kind == ViewBoxShadowKind::Outer
            && inset.kind == ViewBoxShadowKind::Inset)
    {
        return Err(InsetShadowExactError::InvalidScene);
    }
    let stats = InsetShadowExactStats {
        runtime_surfaces,
        player_view_scenes: prepared.frame.view_scenes().len(),
        primitives: scene.scene.primitives().len(),
        paint_nodes: scene.scene.paint_nodes().len(),
        outer_shadows: second_shadows
            .iter()
            .filter(|s| s.kind == ViewBoxShadowKind::Outer)
            .count(),
        inset_shadows: first_shadows.len()
            + second_shadows
                .iter()
                .filter(|s| s.kind == ViewBoxShadowKind::Inset)
                .count(),
    };
    Ok(InsetShadowExactFrame {
        frame: prepared.frame,
        bundle_identity,
        stats,
    })
}

/// A fully opaque clear must still fail a capture with either missing card.
pub fn has_exact_card_pixels(rgba: &[u8]) -> bool {
    const ROW: usize = WIDTH as usize * 4;
    if rgba.len() != ROW * HEIGHT as usize {
        return false;
    }
    let pixel = |x: usize, y: usize| &rgba[y * ROW + x * 4..y * ROW + x * 4 + 4];
    let background = pixel(0, 0);
    let first = pixel(80, 60);
    let second = pixel(232, 76);
    first != background && second != background && first != second
}
