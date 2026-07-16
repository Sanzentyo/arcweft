//! Bundle product validation, runtime construction, and initial session assembly.

use super::{
    Arc, ArcweftBundle, ArcweftRuntimeExecutor, AwbcEntryId, AwbcFunctionId,
    AwbcProductStepBuildError, AwbcProgram, BTreeMap, BundleEntryStart, BundleEntryStartError,
    BundleFormat, BundleImageObject, BundleKind, BundlePresentationSnapshot, BundleSession,
    BundleSessionArtifactIdentity, BundleSessionError, BundleSessionOptions, BundleView,
    BundleViewRuntime, FxDefinitions, GenerationBuildError, GenerationId, GenerationRuntimeImage,
    GenerationRuntimeTable, LineDisplayCatalog, PresentationEnvironmentOverrides,
    ProgramGeneration, ReadBudget, RuntimeEntityFamily, RuntimeTaskRegistry,
    SessionEnvironmentState, SwapSession, SystemPaletteSet, ViewProgramResource,
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeScrollRegion, ViewRuntimeSurface, ViewRuntimeTextControl, ViewVirtualizationRuntime,
};

#[derive(Clone, Debug)]
pub(super) struct SessionRuntime {
    pub(super) source_label: String,
    pub(super) program: AwbcProgram,
    pub(super) entry: AwbcEntryId,
    launch_target: SessionLaunchTarget,
    pub(super) executor: ArcweftRuntimeExecutor,
    pub(super) display: LineDisplayCatalog,
    pub(super) image_objects: Vec<BundleImageObject>,
    pub(super) text_inputs: Vec<ViewRuntimeTextControl>,
    pub(super) action_buttons: Vec<ViewRuntimeActionButton>,
    pub(super) scroll_regions: Vec<ViewRuntimeScrollRegion>,
    pub(super) surfaces: Vec<ViewRuntimeSurface>,
    pub(super) focus_groups: Vec<ViewRuntimeFocusGroup>,
    pub(super) focus_navigation: Vec<ViewRuntimeFocusNavigation>,
    pub(super) fx_definitions: FxDefinitions,
    pub(super) view_runtime: BundleViewRuntime,
    pub(super) view_theme_environment: PresentationEnvironmentOverrides,
    pub(super) view_style_palettes: SystemPaletteSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionLaunchTarget {
    Entry(AwbcEntryId),
    Function {
        entry: AwbcEntryId,
        function: AwbcFunctionId,
    },
}

impl SessionLaunchTarget {
    const fn entry(self) -> AwbcEntryId {
        match self {
            Self::Entry(entry) | Self::Function { entry, .. } => entry,
        }
    }
}

#[derive(Clone, Debug)]
struct SessionRuntimeResources {
    display: LineDisplayCatalog,
    image_objects: Vec<BundleImageObject>,
    text_inputs: Vec<ViewRuntimeTextControl>,
    action_buttons: Vec<ViewRuntimeActionButton>,
    scroll_regions: Vec<ViewRuntimeScrollRegion>,
    surfaces: Vec<ViewRuntimeSurface>,
    focus_groups: Vec<ViewRuntimeFocusGroup>,
    focus_navigation: Vec<ViewRuntimeFocusNavigation>,
    fx_definitions: FxDefinitions,
    view_runtime: BundleViewRuntime,
    view_theme_environment: PresentationEnvironmentOverrides,
    view_style_palettes: SystemPaletteSet,
}

impl BundleSession {
    /// Builds a portable bytecode VM session without materializing bundle files.
    pub fn new(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
    ) -> Result<Self, BundleSessionError> {
        let identity = bundle.logical_identity().map_err(|error| {
            BundleSessionError::GenerationFingerprint {
                message: error.to_string(),
            }
        })?;
        Self::new_with_artifact_identity(
            bundle,
            options,
            BundleSessionArtifactIdentity::LogicalBundle { identity },
        )
    }

    pub fn from_awfb_bytes(
        bytes: &[u8],
        options: BundleSessionOptions,
    ) -> Result<Self, BundleSessionError> {
        let view = BundleView::parse(bytes, ReadBudget::default()).map_err(|error| {
            BundleSessionError::DecodeBundle {
                message: error.to_string(),
            }
        })?;
        let container_identity = view.artifact_identity();
        let bundle =
            ArcweftBundle::from_format_slice(BundleFormat::Awfb, bytes).map_err(|error| {
                BundleSessionError::DecodeBundle {
                    message: error.to_string(),
                }
            })?;
        Self::new_with_artifact_identity(
            &bundle,
            options,
            BundleSessionArtifactIdentity::AwfbContainer {
                identity: container_identity,
            },
        )
    }

    fn new_with_artifact_identity(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
        active_artifact_identity: BundleSessionArtifactIdentity,
    ) -> Result<Self, BundleSessionError> {
        let generation = Arc::new(initial_generation(bundle)?);
        let runtime = build_session_runtime(bundle, &options)?;
        let executor = runtime.executor.clone();
        let display = runtime.display.clone();
        let image_objects = runtime.image_objects.clone();
        let text_inputs = runtime.text_inputs.clone();
        let action_buttons = runtime.action_buttons.clone();
        let scroll_regions = runtime.scroll_regions.clone();
        let surfaces = runtime.surfaces.clone();
        let focus_groups = runtime.focus_groups.clone();
        let focus_navigation = runtime.focus_navigation.clone();
        let fx_definitions = runtime.fx_definitions.clone();
        let view_runtime = runtime.view_runtime.clone();
        let environment = SessionEnvironmentState::new(
            options.presentation_environment,
            runtime.view_theme_environment,
        );
        let view_style_palettes = runtime.view_style_palettes;
        let source_label = runtime.source_label.clone();

        Ok(Self {
            source_label,
            executor,
            runtime_images: GenerationRuntimeTable::new(GenerationRuntimeImage::new(
                generation.clone(),
                runtime,
            )),
            display,
            image_objects,
            text_inputs,
            action_buttons,
            scroll_regions,
            surfaces,
            focus_groups,
            focus_navigation,
            fx_definitions,
            view_runtime,
            environment,
            view_style_palettes,
            options,
            pending_input_events: Vec::new(),
            pending_presentation_inputs: Vec::new(),
            pending_text_control_write_backs: Vec::new(),
            pending_host_call_results: Vec::new(),
            waiting_action_receive_calls: Vec::new(),
            presentation: BundlePresentationSnapshot::default(),
            view_virtualization: ViewVirtualizationRuntime::default(),
            next_step_index: 0,
            next_task_sequence: 0,
            swap: SwapSession::new(generation.clone()),
            runtime_generation_pin: Some(generation),
            task_generation_pins: BTreeMap::new(),
            tasks: RuntimeTaskRegistry::default(),
            next_generation_id: 1,
            active_artifact_identity,
        })
    }
}

fn initial_generation(bundle: &ArcweftBundle) -> Result<ProgramGeneration, BundleSessionError> {
    ProgramGeneration::from_bundle(GenerationId(0), bundle).map_err(|error| match error {
        GenerationBuildError::UnsupportedBundleKind(kind) => {
            BundleSessionError::UnsupportedBundleKind(kind)
        }
        GenerationBuildError::VerifyBytecode(error) => BundleSessionError::VerifyBytecode(error),
        GenerationBuildError::ProductAwbcVerification { message } => {
            BundleSessionError::ProductAwbcVerification { message }
        }
        GenerationBuildError::EncodeFingerprint(error) => {
            BundleSessionError::GenerationFingerprint {
                message: error.to_string(),
            }
        }
        GenerationBuildError::AdapterRequirementFingerprint { message } => {
            BundleSessionError::GenerationFingerprint { message }
        }
    })
}

impl SessionRuntime {
    fn new(
        source_label: String,
        program: AwbcProgram,
        launch_target: SessionLaunchTarget,
        resources: SessionRuntimeResources,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let entry = launch_target.entry();
        let executor = match launch_target {
            SessionLaunchTarget::Entry(entry) => {
                ArcweftRuntimeExecutor::from_awbc_product(program.clone(), entry)?
            }
            SessionLaunchTarget::Function { entry, function } => {
                ArcweftRuntimeExecutor::from_awbc_product_function(
                    program.clone(),
                    entry,
                    function,
                )?
            }
        };
        Ok(Self {
            source_label,
            program,
            entry,
            launch_target,
            executor,
            display: resources.display,
            image_objects: resources.image_objects,
            text_inputs: resources.text_inputs,
            action_buttons: resources.action_buttons,
            scroll_regions: resources.scroll_regions,
            surfaces: resources.surfaces,
            focus_groups: resources.focus_groups,
            focus_navigation: resources.focus_navigation,
            fx_definitions: resources.fx_definitions,
            view_runtime: resources.view_runtime,
            view_theme_environment: resources.view_theme_environment,
            view_style_palettes: resources.view_style_palettes,
        })
    }

    pub(super) fn start_entry(
        &self,
        start: BundleEntryStart,
    ) -> Result<Self, BundleEntryStartError> {
        let launch_target = match start {
            BundleEntryStart::SessionDefault => self.launch_target,
            BundleEntryStart::Entry(entry) => {
                ensure_start_awbc_entry_selects_flow(&self.program, entry)?;
                SessionLaunchTarget::Entry(entry)
            }
        };
        Self::new(
            self.source_label.clone(),
            self.program.clone(),
            launch_target,
            SessionRuntimeResources {
                display: self.display.clone(),
                image_objects: self.image_objects.clone(),
                text_inputs: self.text_inputs.clone(),
                action_buttons: self.action_buttons.clone(),
                scroll_regions: self.scroll_regions.clone(),
                surfaces: self.surfaces.clone(),
                focus_groups: self.focus_groups.clone(),
                focus_navigation: self.focus_navigation.clone(),
                fx_definitions: self.fx_definitions.clone(),
                view_runtime: self.view_runtime.clone(),
                view_theme_environment: self.view_theme_environment,
                view_style_palettes: self.view_style_palettes,
            },
        )
        .map_err(BundleEntryStartError::from)
    }
}

pub(super) fn build_session_runtime(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<SessionRuntime, BundleSessionError> {
    if bundle.bundle_kind != BundleKind::Game {
        return Err(BundleSessionError::UnsupportedBundleKind(
            bundle.bundle_kind,
        ));
    }

    let program = bundle
        .product_awbc_program()
        .map_err(|_| BundleSessionError::MissingProductAwbc)?
        .clone();
    let launch_target = selected_awbc_launch_target(&program, bundle, options)?;
    if let SessionLaunchTarget::Entry(entry) = launch_target {
        ensure_session_awbc_entry_selects_flow(&program, entry)?;
    }
    let text_inputs = bundle.view_input.as_ref().map_or_else(Vec::new, |input| {
        input.runtime_text_controls(bundle.view_text.as_ref(), bundle.view_program.as_ref())
    });
    let action_buttons = bundle
        .view_program
        .as_ref()
        .map_or_else(Vec::new, |program| {
            program.runtime_action_buttons(bundle.view_text.as_ref())
        });
    let scroll_regions = bundle
        .view_program
        .as_ref()
        .map_or_else(Vec::new, ViewProgramResource::runtime_scroll_regions);
    let surfaces = bundle
        .view_program
        .as_ref()
        .map_or_else(Vec::new, ViewProgramResource::runtime_surfaces);
    let focus_groups = bundle
        .view_program
        .as_ref()
        .map_or_else(Vec::new, ViewProgramResource::runtime_focus_groups);
    let focus_navigation = bundle
        .view_program
        .as_ref()
        .map_or_else(Vec::new, ViewProgramResource::runtime_focus_navigation);
    let view_runtime = BundleViewRuntime::try_new(
        bundle.view_program.clone(),
        bundle.view_text.clone(),
        bundle.view_style.as_ref(),
    )?;
    let view_theme = bundle.view_theme.clone().unwrap_or_default();
    let view_theme_environment = view_theme.environment_overrides();
    let view_style_palettes = view_theme.system_palette_set();
    SessionRuntime::new(
        bundle.manifest.source_label.clone(),
        program,
        launch_target,
        SessionRuntimeResources {
            display: bundle.display.clone(),
            image_objects: bundle.image_objects.clone(),
            text_inputs,
            action_buttons,
            scroll_regions,
            surfaces,
            focus_groups,
            focus_navigation,
            fx_definitions: bundle.fx_definitions.clone(),
            view_runtime,
            view_theme_environment,
            view_style_palettes,
        },
    )
    .map_err(BundleSessionError::from)
}

fn selected_awbc_launch_target(
    program: &AwbcProgram,
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<SessionLaunchTarget, BundleSessionError> {
    if options.entry.is_some() && options.flow.is_some() {
        return Err(BundleSessionError::ConflictingEntrySelection);
    }
    if let Some(flow) = options.flow.as_deref() {
        let selected = RuntimeEntityFamily::Flow.selector(flow);
        return program
            .functions
            .iter()
            .enumerate()
            .find_map(|(index, function)| {
                if !function.kind.is_flow() {
                    return None;
                }
                let public_id = function
                    .public_id
                    .and_then(|public_id| program.strings.get(public_id.index()))?;
                (public_id == &selected).then(|| SessionLaunchTarget::Function {
                    entry: AwbcEntryId(0),
                    function: AwbcFunctionId(u32::try_from(index).unwrap_or(u32::MAX)),
                })
            })
            .ok_or(BundleSessionError::UnknownFlow { flow: selected });
    }
    let Some(entry) = selected_entry(bundle, options) else {
        return Ok(SessionLaunchTarget::Entry(AwbcEntryId(0)));
    };
    let selected = RuntimeEntityFamily::Entry.selector(entry);
    program
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            let public_id = program.strings.get(candidate.public_id.index())?;
            (public_id == entry || public_id == &selected).then(|| {
                SessionLaunchTarget::Entry(AwbcEntryId(u32::try_from(index).unwrap_or(u32::MAX)))
            })
        })
        .ok_or(BundleSessionError::ProductAwbcEntry { entry: selected })
}

fn selected_entry<'a>(
    bundle: &'a ArcweftBundle,
    options: &'a BundleSessionOptions,
) -> Option<&'a str> {
    options.entry.as_deref().or_else(|| {
        options
            .flow
            .is_none()
            .then_some(bundle.manifest.entry.as_deref())
            .flatten()
    })
}

fn ensure_session_awbc_entry_selects_flow(
    program: &AwbcProgram,
    entry: AwbcEntryId,
) -> Result<(), BundleSessionError> {
    if awbc_entry_selects_flow(program, entry) {
        Ok(())
    } else {
        Err(BundleSessionError::NonFlowEntry {
            entry: awbc_entry_label(program, entry),
        })
    }
}

fn ensure_start_awbc_entry_selects_flow(
    program: &AwbcProgram,
    entry: AwbcEntryId,
) -> Result<(), BundleEntryStartError> {
    if !awbc_entry_exists_or_empty_program_default(program, entry) {
        return Err(BundleEntryStartError::UnknownEntry { entry });
    }
    if awbc_entry_selects_flow(program, entry) {
        Ok(())
    } else {
        Err(BundleEntryStartError::NonFlowEntry { entry })
    }
}

fn awbc_entry_exists_or_empty_program_default(program: &AwbcProgram, entry: AwbcEntryId) -> bool {
    program.entries.get(entry.index()).is_some()
        || (program.entries.is_empty() && entry == AwbcEntryId(0))
}

fn awbc_entry_selects_flow(program: &AwbcProgram, entry: AwbcEntryId) -> bool {
    if program.entries.is_empty() && entry == AwbcEntryId(0) {
        return true;
    }
    let Some(entry) = program.entries.get(entry.index()) else {
        return false;
    };
    let Some(function) = entry.target.function() else {
        return false;
    };
    program
        .functions
        .get(function.index())
        .is_some_and(|function| function.kind.is_flow())
}

fn awbc_entry_label(program: &AwbcProgram, entry: AwbcEntryId) -> String {
    program
        .entries
        .get(entry.index())
        .and_then(|entry| program.strings.get(entry.public_id.index()).cloned())
        .unwrap_or_else(|| format!("entry#{}", entry.0))
}
