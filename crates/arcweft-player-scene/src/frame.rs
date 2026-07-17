use crate::action_buttons::{RuntimeActionButtonLowerer, RuntimeActionButtonLoweringError};
use crate::frame::focus_navigation::{render_focus_groups, render_focus_navigation};
use crate::frame::view_geometry::{
    PlayerViewGeometryState, PresentationIntrinsicGeometryProvider, ViewGeometryFrameInput,
    ViewGeometryPreparedFrame, ViewPaintOutsetSnapshot, ViewScrollStateSnapshot,
    prepare_view_geometry, viewport_input,
};
use crate::frame::view_style::{PlayerViewStyleState, ResolvedViewStyleFrame, StyledViewResources};
use crate::images::{BundleImageCatalog, BundleImageCatalogError};
use crate::input::InputController;
use crate::text_controls::{RuntimeTextControlLowerer, RuntimeTextControlLoweringError};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::ViewRuntimeStyleProjectionError;
use arcweft_layout::{ContentRect, LayoutError, LayoutSize, ScalePolicy};
use arcweft_presentation::appearance::{
    EnvironmentRevision, PresentationEnvironment, PresentationEnvironmentFieldRevisions,
    PresentationEnvironmentFieldSet, SystemPaletteSet,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderChoiceItem, RenderFocusAutoScrollPolicy,
    RenderPreferences, RenderScene, RenderScrollAxis, RenderScrollIndicatorsPolicy,
    RenderScrollOverflow, RenderScrollOverscrollPolicy, RenderScrollRegion, RenderViewport,
    SharedFramePlanContext, SharedFramePlanStats,
};
use arcweft_runtime_driver::display::{BundlePresentationSnapshot, BundleViewportFit};
use arcweft_runtime_driver::session::PresentationEnvironmentUpdate;
use arcweft_view::ViewMountId;
pub use arcweft_view::geometry::ViewGeometryConsumer;
use arcweft_view::style::{ViewPropertyKind, ViewStyleProgram, ViewStyleResolveError};
use thiserror::Error;

mod focus_navigation;
mod surfaces;
mod view_geometry;
mod view_style;
mod view_text;

pub use view_geometry::{
    ViewCommittedGeometryFrame, ViewGeometryConversionError, ViewGeometryConversionField,
    ViewGeometryFailure, ViewGeometryFailureCode, ViewGeometryFailureField,
    ViewGeometryFailureGeneration, ViewGeometryFailureRange, ViewGeometryFailureRect,
    ViewGeometryGeneration, ViewGeometryPlatform, ViewGeometryRuntimeError,
};
pub(crate) use view_geometry::{ViewGeometryProductKind, ViewGeometryTargetKey};

/// Player-owned frame inputs shared by native, web, and Agent observation.
#[derive(Clone, Copy, Debug)]
pub struct PlayerFrameRequest<'a> {
    pub presentation: &'a BundlePresentationSnapshot,
    pub fx_definitions: &'a FxDefinitions,
    pub images: &'a BundleImageCatalog,
    pub style_program: Option<&'a ViewStyleProgram>,
    pub style_environment: &'a PresentationEnvironment,
    pub style_palettes: &'a SystemPaletteSet,
    pub viewport: RenderViewport,
    pub fit: PlayerFrameFit,
    pub image_time_millis: u64,
    pub visual_time_millis: u64,
    pub dialogue_reveal_complete: bool,
    pub preferences: RenderPreferences,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerFrameFit {
    pub design_width: u32,
    pub design_height: u32,
    pub scale_policy: ScalePolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerPreparedFrame {
    pub scene: RenderScene,
    pub frame: PreparedFrame,
    geometry: std::sync::Arc<ViewCommittedGeometryFrame>,
}

impl PlayerPreparedFrame {
    pub fn view_geometry(&self) -> &std::sync::Arc<ViewCommittedGeometryFrame> {
        &self.geometry
    }
}

/// Fully prepared player-owned work that is not observable until guarded
/// publication succeeds.
#[derive(Debug)]
pub struct PlayerPreparedFrameCandidate {
    base_generation: ViewGeometryGeneration,
    prepared: PreparedFrame,
    geometry: ViewGeometryPreparedFrame,
    staged_player_state: PlayerFrameStagedState,
}

impl PlayerPreparedFrameCandidate {
    pub fn prepared(&self) -> &PreparedFrame {
        &self.prepared
    }

    pub fn view_geometry(&self) -> &std::sync::Arc<ViewCommittedGeometryFrame> {
        self.geometry.committed()
    }
}

#[derive(Debug)]
struct PlayerFrameStagedState {
    scene: RenderScene,
    shared: SharedFramePlanContext,
    view_style: PlayerViewStyleState,
    prepared_environment: PreparedEnvironmentStamp,
    input: InputController,
}

/// Exclusive publication boundary for one player and adapter transaction.
pub struct PlayerFramePublicationGuard<'a> {
    planner: &'a mut PlayerFramePlannerState,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PlayerFrameError {
    #[error(transparent)]
    TextControlLowering(#[from] RuntimeTextControlLoweringError),
    #[error(transparent)]
    ActionButtonLowering(#[from] RuntimeActionButtonLoweringError),
    #[error(transparent)]
    Images(#[from] BundleImageCatalogError),
    #[error(transparent)]
    TextEditor(#[from] TextEditorError),
    #[error(transparent)]
    Layout(#[from] LayoutError),
    #[error(transparent)]
    StyleResolve(#[from] ViewStyleResolveError),
    #[error(transparent)]
    StyleProjection(ViewRuntimeStyleProjectionError),
    #[error(transparent)]
    ViewGeometry(#[from] ViewGeometryRuntimeError),
    #[error("executed View Style applications have no canonical Style program")]
    MissingStyleProgram,
    #[error("top-level View mount {mount:?} instruction {instruction} has no host axis seed")]
    MissingHostAxisSeed {
        mount: ViewMountId,
        instruction: u32,
    },
    #[error("nested View mount {mount:?} unexpectedly contains a host axis seed")]
    UnexpectedHostAxisSeed { mount: ViewMountId },
    #[error("View Style node target `{target}` is produced more than once in one frame")]
    DuplicateStyleTarget { target: String },
    #[error("View Style node identity repeats in mount {mount} at instruction {instruction}")]
    DuplicateStyleNode { mount: u64, instruction: u32 },
    #[error("View Style parent is missing for mount {mount} instruction {instruction}")]
    MissingStyleParent { mount: u64, instruction: u32 },
    #[error("View Style parent is ambiguous for mount {mount} instruction {instruction}")]
    AmbiguousStyleParent { mount: u64, instruction: u32 },
    #[error(
        "View Style property {property:?} has no {target} consumer for mount {mount} instruction {instruction}"
    )]
    UnsupportedStyleProperty {
        mount: u64,
        instruction: u32,
        target: &'static str,
        property: ViewPropertyKind,
    },
    #[error("invalid focus navigation public id `{value}`")]
    InvalidId { value: String },
    #[error(transparent)]
    FramePlan(#[from] FramePlanError),
}

impl PlayerFrameError {
    pub fn geometry_failure(&self) -> Option<ViewGeometryFailure> {
        match self {
            Self::ViewGeometry(error) => Some(error.geometry_failure()),
            _ => None,
        }
    }
}

/// Shared player frame construction.
///
/// All interactive hosts should use this path so runtime View controls, semantic
/// focus, and render geometry cannot drift between native, web, and Agent
/// observation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerFramePlanner;

/// Stateful player frame planner for long-lived native/web player windows.
///
/// The stateless `PlayerFramePlanner` facade remains for tests and one-shot
/// observation. Hosts that register project-owned font bytes should keep this
/// state and register the same bytes here and in the renderer.
#[derive(Debug, Default)]
pub struct PlayerFramePlannerState {
    shared: SharedFramePlanContext,
    view_style: PlayerViewStyleState,
    view_geometry: PlayerViewGeometryState,
    prepared_environment: Option<PreparedEnvironmentStamp>,
    published_frame: Option<PlayerPreparedFrame>,
}

/// Exact environment fields and revisions used by the latest prepared work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedEnvironmentStamp {
    generation: EnvironmentRevision,
    fields: PresentationEnvironmentFieldSet,
    field_revisions: PresentationEnvironmentFieldRevisions,
}

/// Field-local effect of applying one committed session environment update.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerEnvironmentInvalidation {
    selection_nodes: usize,
    projection_nodes: usize,
    unchanged_nodes: usize,
    prepared_work_discarded: bool,
    redraw_requested: bool,
}

struct ResolvedPlayerScene {
    scene: RenderScene,
    styles: ResolvedViewStyleFrame,
    resources: StyledViewResources,
    geometry: std::sync::Arc<ViewCommittedGeometryFrame>,
    geometry_prepared: ViewGeometryPreparedFrame,
}

impl PlayerFrameFit {
    pub const fn raw() -> Self {
        Self {
            design_width: 0,
            design_height: 0,
            scale_policy: ScalePolicy::Raw,
        }
    }

    pub const fn design_1280x720(scale_policy: ScalePolicy) -> Self {
        Self::design(1280, 720, scale_policy)
    }

    pub const fn design(design_width: u32, design_height: u32, scale_policy: ScalePolicy) -> Self {
        Self {
            design_width: if design_width == 0 { 1 } else { design_width },
            design_height: if design_height == 0 { 1 } else { design_height },
            scale_policy,
        }
    }

    #[must_use]
    pub fn with_presentation_override(self, presentation: &BundlePresentationSnapshot) -> Self {
        presentation.viewport_fit.map_or(self, Self::from)
    }

    fn planning_viewport(self, output: RenderViewport) -> Result<RenderViewport, PlayerFrameError> {
        if self.scale_policy == ScalePolicy::Raw {
            return Ok(output);
        }
        Ok(RenderViewport {
            logical_width: design_dimension(self.design_width, ViewGeometryConversionField::Width)?,
            logical_height: design_dimension(
                self.design_height,
                ViewGeometryConversionField::Height,
            )?,
            physical_width: output.physical_width,
            physical_height: output.physical_height,
            scale_factor: output.scale_factor,
        })
    }

    fn content_rect(self, output: RenderViewport) -> Result<Option<ContentRect>, PlayerFrameError> {
        if self.scale_policy == ScalePolicy::Raw {
            return Ok(None);
        }
        Ok(ContentRect::calculate(
            LayoutSize::new(
                design_dimension(self.design_width, ViewGeometryConversionField::Width)?,
                design_dimension(self.design_height, ViewGeometryConversionField::Height)?,
            ),
            LayoutSize::new(output.logical_width, output.logical_height),
            self.scale_policy,
        )
        .map(Some)?)
    }
}

impl PreparedEnvironmentStamp {
    fn new(environment: PresentationEnvironment, fields: PresentationEnvironmentFieldSet) -> Self {
        Self {
            generation: environment.revision(),
            fields,
            field_revisions: environment.field_revisions(),
        }
    }

    pub const fn generation(self) -> EnvironmentRevision {
        self.generation
    }

    pub const fn fields(self) -> PresentationEnvironmentFieldSet {
        self.fields
    }

    pub const fn field_revisions(self) -> PresentationEnvironmentFieldRevisions {
        self.field_revisions
    }

    pub fn is_current(self, environment: PresentationEnvironment) -> bool {
        self.fields.iter().all(|field| {
            self.field_revisions.field_revision(field) == environment.field_revision(field)
        })
    }
}

impl PlayerEnvironmentInvalidation {
    pub const fn selection_nodes(self) -> usize {
        self.selection_nodes
    }

    pub const fn projection_nodes(self) -> usize {
        self.projection_nodes
    }

    pub const fn unchanged_nodes(self) -> usize {
        self.unchanged_nodes
    }

    pub const fn prepared_work_discarded(self) -> bool {
        self.prepared_work_discarded
    }

    pub const fn redraw_requested(self) -> bool {
        self.redraw_requested
    }
}

impl From<BundleViewportFit> for PlayerFrameFit {
    fn from(value: BundleViewportFit) -> Self {
        if value.scale_policy == ScalePolicy::Raw {
            Self::raw()
        } else {
            Self::design(value.design_width, value.design_height, value.scale_policy)
        }
    }
}

impl PlayerFramePlanner {
    pub fn render_scene(
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<RenderScene, PlayerFrameError> {
        Self::prepare(input, request).map(|prepared| prepared.scene)
    }

    pub fn prepare(
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<PlayerPreparedFrame, PlayerFrameError> {
        let mut planner = PlayerFramePlannerState::new();
        for bytes in crate::fonts::DEFAULT_PLAYER_FONT_RESOURCE_BYTES {
            planner.register_font_bytes(bytes.to_vec())?;
        }
        let candidate = planner.prepare_candidate(input, request)?;
        planner
            .publication_guard()
            .publish_with(candidate, input, |_| ())
            .map(|(frame, ())| frame)
    }
}

fn resolve_player_scene(
    style_state: &mut PlayerViewStyleState,
    geometry_state: &PlayerViewGeometryState,
    input: &mut InputController,
    request: PlayerFrameRequest<'_>,
) -> Result<ResolvedPlayerScene, PlayerFrameError> {
    let styles = style_state.resolve(
        input,
        request.presentation,
        request.style_program,
        request.style_environment,
        request.style_palettes,
    )?;
    let scroll = ViewScrollStateSnapshot::from_frame(&request.presentation.view, input)?;
    let paint_outsets = ViewPaintOutsetSnapshot::from_styles(&styles)?;
    let viewport = viewport_input(
        f64::from(request.viewport.logical_width),
        f64::from(request.viewport.logical_height),
    )
    .map_err(|source| ViewGeometryRuntimeError::Conversion {
        node: None,
        consumer: arcweft_view::geometry::ViewGeometryConsumer::Layout,
        source,
    })?;
    let mut intrinsic = PresentationIntrinsicGeometryProvider;
    let geometry_prepared = prepare_view_geometry(
        geometry_state,
        ViewGeometryFrameInput {
            frame: &request.presentation.view,
            styles: &styles,
            presentation: request.presentation,
            viewport,
            scroll: &scroll,
            paint_outsets: &paint_outsets,
        },
        &mut intrinsic,
    )?;
    let geometry = geometry_prepared.committed().clone();
    let resources = styles.apply_to_presentation(request.presentation, geometry.clone());
    let text_inputs = RuntimeTextControlLowerer::lower_for_geometry(
        input,
        &resources.text_inputs,
        &resources.geometry,
    )?;
    let action_buttons = RuntimeActionButtonLowerer::lower_for_geometry(
        &resources.action_buttons,
        &text_inputs,
        &resources.geometry,
    )?;
    let scene = RenderScene {
        content_avoidance_regions: dialogue_content_avoidance_regions(
            request.presentation,
            &resources.surfaces,
            &resources.geometry,
        )?,
        choices: request
            .presentation
            .choices
            .iter()
            .map(|choice| RenderChoiceItem {
                id: choice.id.clone(),
                label: choice.label.clone(),
            })
            .collect(),
        text_inputs,
        action_buttons,
        focus_groups: render_focus_groups(&request.presentation.focus_groups)?,
        focus_navigation: render_focus_navigation(&request.presentation.focus_navigation)?,
        images: request.images.render_images(
            &resources.images,
            request.image_time_millis,
            request.viewport,
        )?,
        viewport: request.viewport,
        visual_time_millis: request.visual_time_millis,
        preferences: request.preferences,
        interaction: input.visual_state(),
        choice_scroll: input.choice_scroll(),
        scroll_regions: resources
            .scroll_regions
            .iter()
            .filter_map(|region| {
                render_scroll_region(
                    input,
                    region,
                    &resources.geometry,
                    request.visual_time_millis,
                    request.preferences.reduce_motion,
                )
                .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(ResolvedPlayerScene {
        scene,
        styles,
        resources,
        geometry,
        geometry_prepared,
    })
}

fn dialogue_content_avoidance_regions(
    presentation: &BundlePresentationSnapshot,
    surfaces: &[arcweft_bundle::resource_codec::ViewRuntimeSurface],
    geometry: &ViewCommittedGeometryFrame,
) -> Result<Vec<HitRect>, PlayerFrameError> {
    let mut regions = Vec::new();
    for mount in presentation
        .view
        .mounts
        .iter()
        .filter(|mount| mount.dialogue.is_some() && mount.path.segments().is_empty())
    {
        let owner = mount.scoped_id(mount.view.as_str());
        let mut bounds = None;
        for surface in surfaces
            .iter()
            .filter(|surface| surface.view.as_deref() == Some(owner.as_str()))
        {
            let target = ViewGeometryTargetKey::new(
                ViewGeometryProductKind::Surface,
                surface.target.clone(),
            );
            if let Some(surface_bounds) =
                geometry.target_consumer_hit_rect(&target, ViewGeometryConsumer::Avoidance)?
            {
                bounds = Some(bounds.map_or(surface_bounds, |current| {
                    union_hit_rects(current, surface_bounds)
                }));
            }
        }
        if let Some(bounds) = bounds {
            regions.push(bounds);
        }
    }
    Ok(regions)
}

fn union_hit_rects(left: HitRect, right: HitRect) -> HitRect {
    let min_x = left.x.min(right.x);
    let min_y = left.y.min(right.y);
    let max_x = (left.x + left.width).max(right.x + right.width);
    let max_y = (left.y + left.height).max(right.y + right.height);
    HitRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn render_scroll_region(
    input: &mut InputController,
    region: &arcweft_bundle::resource_codec::ViewRuntimeScrollRegion,
    geometry: &ViewCommittedGeometryFrame,
    visual_time_millis: u64,
    reduce_motion: bool,
) -> Result<Option<RenderScrollRegion>, PlayerFrameError> {
    let target =
        ViewGeometryTargetKey::new(ViewGeometryProductKind::ScrollRegion, region.target.clone());
    let Some((node, final_geometry)) = geometry.target_geometry(&target) else {
        return Ok(None);
    };
    let Some(bounds) = geometry.target_consumer_hit_rect(&target, ViewGeometryConsumer::Scroll)?
    else {
        return Ok(None);
    };
    let content_width = view_geometry::exact_f32(
        node,
        ViewGeometryConsumer::Scroll,
        ViewGeometryConversionField::Width,
        i64::from(final_geometry.scroll.x.content.extent_milli()),
    )
    .map_err(|source| ViewGeometryRuntimeError::Conversion {
        node: Some(node.clone()),
        consumer: ViewGeometryConsumer::Scroll,
        source,
    })?;
    let content_height = view_geometry::exact_f32(
        node,
        ViewGeometryConsumer::Scroll,
        ViewGeometryConversionField::Height,
        i64::from(final_geometry.scroll.y.content.extent_milli()),
    )
    .map_err(|source| ViewGeometryRuntimeError::Conversion {
        node: Some(node.clone()),
        consumer: ViewGeometryConsumer::Scroll,
        source,
    })?;
    let min_offset_x = scroll_offset_f32(
        node,
        final_geometry.scroll.x.min_offset_milli,
        ViewGeometryConversionField::Left,
    )?;
    let max_offset_x = scroll_offset_f32(
        node,
        final_geometry.scroll.x.max_offset_milli,
        ViewGeometryConversionField::Right,
    )?;
    let min_offset_y = scroll_offset_f32(
        node,
        final_geometry.scroll.y.min_offset_milli,
        ViewGeometryConversionField::Top,
    )?;
    let max_offset_y = scroll_offset_f32(
        node,
        final_geometry.scroll.y.max_offset_milli,
        ViewGeometryConversionField::Bottom,
    )?;
    let offset_x = scroll_offset_f32(
        node,
        final_geometry.scroll.x.current_offset_milli,
        ViewGeometryConversionField::Left,
    )?;
    let offset_y = scroll_offset_f32(
        node,
        final_geometry.scroll.y.current_offset_milli,
        ViewGeometryConversionField::Top,
    )?;
    let mut render_region = RenderScrollRegion {
        id: region.public_id.clone(),
        bounds,
        content_width,
        content_height,
        min_offset_x,
        max_offset_x,
        min_offset_y,
        max_offset_y,
        offset_x,
        offset_y,
        overscroll_x: 0.0,
        overscroll_y: 0.0,
        axis: render_scroll_axis(region.axis),
        overflow: render_scroll_overflow(region.overflow),
        indicators: render_scroll_indicators_policy(region.indicators),
        overscroll: render_scroll_overscroll_policy(region.overscroll),
        auto_scroll_focus: render_focus_auto_scroll_policy(region.auto_scroll_focus),
        indicator_activity_millis: None,
    };
    input.resolve_scroll_region(&mut render_region, visual_time_millis, reduce_motion);
    Ok(Some(render_region))
}

fn scroll_offset_f32(
    node: &arcweft_view::style::ViewStyleNodeKey,
    value_milli: i32,
    field: ViewGeometryConversionField,
) -> Result<f32, PlayerFrameError> {
    view_geometry::exact_f32(
        node,
        ViewGeometryConsumer::Scroll,
        field,
        i64::from(value_milli),
    )
    .map_err(|source| {
        ViewGeometryRuntimeError::Conversion {
            node: Some(node.clone()),
            consumer: ViewGeometryConsumer::Scroll,
            source,
        }
        .into()
    })
}

const fn render_scroll_axis(
    axis: arcweft_bundle::resource_codec::ViewScrollAxis,
) -> RenderScrollAxis {
    match axis {
        arcweft_bundle::resource_codec::ViewScrollAxis::Vertical => RenderScrollAxis::Vertical,
        arcweft_bundle::resource_codec::ViewScrollAxis::Horizontal => RenderScrollAxis::Horizontal,
    }
}

fn render_scroll_overflow(
    overflow: arcweft_bundle::resource_codec::ViewScrollOverflowPolicy,
) -> RenderScrollOverflow {
    match overflow {
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Auto => {
            RenderScrollOverflow::Auto
        }
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Scroll => {
            RenderScrollOverflow::Scroll
        }
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Hidden => {
            RenderScrollOverflow::Hidden
        }
    }
}

const fn render_scroll_indicators_policy(
    policy: arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy,
) -> RenderScrollIndicatorsPolicy {
    match policy {
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Auto => {
            RenderScrollIndicatorsPolicy::Auto
        }
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Visible => {
            RenderScrollIndicatorsPolicy::Visible
        }
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Hidden => {
            RenderScrollIndicatorsPolicy::Hidden
        }
    }
}

const fn render_scroll_overscroll_policy(
    policy: arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy,
) -> RenderScrollOverscrollPolicy {
    match policy {
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Clamp => {
            RenderScrollOverscrollPolicy::Clamp
        }
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Contain => {
            RenderScrollOverscrollPolicy::Contain
        }
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Elastic => {
            RenderScrollOverscrollPolicy::Elastic
        }
    }
}

const fn render_focus_auto_scroll_policy(
    policy: arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy,
) -> RenderFocusAutoScrollPolicy {
    match policy {
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Nearest => {
            RenderFocusAutoScrollPolicy::Nearest
        }
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Start => {
            RenderFocusAutoScrollPolicy::Start
        }
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::End => {
            RenderFocusAutoScrollPolicy::End
        }
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Disabled => {
            RenderFocusAutoScrollPolicy::Disabled
        }
    }
}

impl PlayerFramePlannerState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), PlayerFrameError> {
        self.shared.register_font_bytes(bytes)?;
        Ok(())
    }

    #[must_use]
    pub fn stats(&self) -> SharedFramePlanStats {
        self.shared.stats()
    }

    pub const fn prepared_environment_stamp(&self) -> Option<PreparedEnvironmentStamp> {
        self.prepared_environment
    }

    pub fn apply_environment_update(
        &mut self,
        update: PresentationEnvironmentUpdate,
    ) -> Result<PlayerEnvironmentInvalidation, PlayerFrameError> {
        let style = self.view_style.apply_environment_update(update);
        let prepared_work_discarded = self
            .prepared_environment
            .is_some_and(|stamp| !stamp.is_current(update.current()));
        if prepared_work_discarded {
            self.prepared_environment = None;
        }
        let redraw_requested =
            update.effective_changed() && (style.selected > 0 || style.projected > 0);
        Ok(PlayerEnvironmentInvalidation {
            selection_nodes: style.selected,
            projection_nodes: style.projected,
            unchanged_nodes: style.unchanged,
            prepared_work_discarded,
            redraw_requested,
        })
    }

    pub fn prepare_candidate(
        &self,
        input: &InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<PlayerPreparedFrameCandidate, PlayerFrameError> {
        let mut shared = self.shared.fork_for_candidate()?;
        let mut view_style = self.view_style.clone();
        let mut staged_input = input.clone();
        let fit = request.fit.with_presentation_override(request.presentation);
        let design_request = PlayerFrameRequest {
            viewport: fit.planning_viewport(request.viewport)?,
            fit,
            ..request
        };
        let content_rect = fit.content_rect(request.viewport)?;
        let mut resolved = resolve_player_scene(
            &mut view_style,
            &self.view_geometry,
            &mut staged_input,
            design_request,
        )?;
        let mut frame =
            prepare_mapped_frame(&mut shared, &resolved, &staged_input, request, content_rect)?;
        if staged_input.ensure_choice_focus(&frame) {
            resolved = resolve_player_scene(
                &mut view_style,
                &self.view_geometry,
                &mut staged_input,
                design_request,
            )?;
            frame =
                prepare_mapped_frame(&mut shared, &resolved, &staged_input, request, content_rect)?;
        }
        if staged_input.apply_pending_text_pointer_selection(&frame)? {
            resolved = resolve_player_scene(
                &mut view_style,
                &self.view_geometry,
                &mut staged_input,
                design_request,
            )?;
            frame =
                prepare_mapped_frame(&mut shared, &resolved, &staged_input, request, content_rect)?;
        }
        let base_generation = resolved.geometry_prepared.base_generation();
        Ok(PlayerPreparedFrameCandidate {
            base_generation,
            prepared: frame,
            geometry: resolved.geometry_prepared,
            staged_player_state: PlayerFrameStagedState {
                scene: resolved.scene,
                shared,
                prepared_environment: PreparedEnvironmentStamp::new(
                    *request.style_environment,
                    view_style.environment_fields(),
                ),
                view_style,
                input: staged_input,
            },
        })
    }

    pub fn publication_guard(&mut self) -> PlayerFramePublicationGuard<'_> {
        PlayerFramePublicationGuard { planner: self }
    }
}

impl PlayerFramePublicationGuard<'_> {
    /// Checks that `candidate` can still be published while retaining the
    /// planner's exclusive publication borrow for side-effecting adapter work.
    pub fn preflight_candidate(
        self,
        candidate: &PlayerPreparedFrameCandidate,
    ) -> Result<Self, PlayerFrameError> {
        self.ensure_current(candidate)?;
        Ok(self)
    }

    pub fn publish_with<T>(
        self,
        candidate: PlayerPreparedFrameCandidate,
        input: &mut InputController,
        commit_adapter: impl FnOnce(&PlayerPreparedFrame) -> T,
    ) -> Result<(PlayerPreparedFrame, T), PlayerFrameError> {
        self.ensure_current(&candidate)?;

        let PlayerPreparedFrameCandidate {
            prepared,
            geometry,
            staged_player_state,
            ..
        } = candidate;
        let PlayerFrameStagedState {
            scene,
            shared,
            view_style,
            prepared_environment,
            input: staged_input,
        } = staged_player_state;
        debug_assert_eq!(
            geometry.next_generation(),
            geometry.committed().generation()
        );
        let committed_geometry = geometry.committed().clone();
        let frame = PlayerPreparedFrame {
            scene,
            frame: prepared,
            geometry: committed_geometry.clone(),
        };

        self.planner.view_geometry.commit(geometry);
        debug_assert!(std::sync::Arc::ptr_eq(
            &committed_geometry,
            frame.view_geometry()
        ));
        self.planner.shared = shared;
        self.planner.view_style = view_style;
        self.planner.prepared_environment = Some(prepared_environment);
        *input = staged_input;

        let receipt = commit_adapter(&frame);
        self.planner.published_frame = Some(frame.clone());
        Ok((frame, receipt))
    }

    fn ensure_current(
        &self,
        candidate: &PlayerPreparedFrameCandidate,
    ) -> Result<(), PlayerFrameError> {
        if candidate.base_generation != self.planner.view_geometry.generation() {
            return Err(ViewGeometryRuntimeError::StalePreparedGeneration {
                base: candidate.base_generation,
                current: self.planner.view_geometry.generation(),
            }
            .into());
        }
        Ok(())
    }
}

fn prepare_mapped_frame(
    shared: &mut SharedFramePlanContext,
    resolved: &ResolvedPlayerScene,
    input: &InputController,
    request: PlayerFrameRequest<'_>,
    content_rect: Option<ContentRect>,
) -> Result<PreparedFrame, PlayerFrameError> {
    let mut frame = match content_rect {
        Some(content_rect) => {
            shared.prepare_mapped(&resolved.scene, request.viewport, content_rect)?
        }
        None => shared.prepare(&resolved.scene)?,
    };
    let prepared_view_text = view_text::prepare_runtime_view_text(
        shared,
        &mut frame,
        view_text::RuntimeViewTextRequest {
            input,
            scene: &resolved.scene,
            presentation: request.presentation,
            fx_definitions: request.fx_definitions,
            visual_time_millis: request.visual_time_millis,
            latest_reveal_complete: request.dialogue_reveal_complete,
            styles: &resolved.styles,
            geometry: &resolved.geometry,
            content: content_rect,
        },
    )?;
    surfaces::push_runtime_view_scene(
        &mut frame,
        &resolved.resources.surfaces,
        &request.presentation.view,
        &prepared_view_text,
        &resolved.styles,
        &resolved.geometry,
        content_rect,
    )?;
    Ok(frame)
}

fn design_dimension(
    value: u32,
    field: ViewGeometryConversionField,
) -> Result<f32, PlayerFrameError> {
    ViewGeometryConversionError::exact_f32(
        None,
        ViewGeometryPlatform::Headless,
        ViewGeometryConsumer::Layout,
        field,
        i64::from(value) * 1_000,
    )
    .map_err(|source| {
        ViewGeometryRuntimeError::Conversion {
            node: None,
            consumer: ViewGeometryConsumer::Layout,
            source,
        }
        .into()
    })
}
