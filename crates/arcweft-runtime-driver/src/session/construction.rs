//! Bundle product validation, runtime construction, and initial session assembly.

use super::{
    Arc, ArcweftBundle, ArcweftRuntimeExecutor, AwbcEntryId, AwbcProductStepBuildError,
    AwbcProgram, BTreeMap, BundleEntryStart, BundleEntryStartError, BundleFormat,
    BundleImageObject, BundleKind, BundlePresentationSnapshot, BundleSession,
    BundleSessionArtifactIdentity, BundleSessionError, BundleSessionOptions, BundleView,
    BundleViewRuntime, BundleViewRuntimeError, EntryRuntimeId, FxDefinitions, GenerationBuildError,
    GenerationId, GenerationRuntimeImage, GenerationRuntimeTable, LineDisplayCatalog,
    PresentationEnvironmentOverrides, ProgramGeneration, ReadBudget, RootCommandHostCallCatalog,
    RuntimeTaskRegistry, SessionEnvironmentState, SwapSession, SystemPaletteSet,
    ViewProgramResource, ViewRuntimeActionButton, ViewRuntimeFocusGroup,
    ViewRuntimeFocusNavigation, ViewRuntimeScrollRegion, ViewRuntimeSurface,
    ViewRuntimeTextControl, ViewVirtualizationRuntime,
};

#[derive(Clone, Debug)]
pub(super) struct SessionRuntime {
    pub(super) source_label: String,
    pub(super) program: AwbcProgram,
    pub(super) entry: AwbcEntryId,
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
            pending_deferred_root_events: Vec::new(),
            pending_root_command_results: BTreeMap::new(),
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
        GenerationBuildError::ProductAwbcIdentity { message }
        | GenerationBuildError::AdapterRequirementFingerprint { message } => {
            BundleSessionError::GenerationFingerprint { message }
        }
        GenerationBuildError::InvalidEntryKind { entry } => {
            BundleSessionError::ProductAwbcVerification {
                message: format!("failed to decode executable entry kind for `{entry}`"),
            }
        }
    })
}

impl SessionRuntime {
    fn new(
        source_label: String,
        program: AwbcProgram,
        entry: AwbcEntryId,
        resources: SessionRuntimeResources,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let executor = ArcweftRuntimeExecutor::from_awbc_product(program.clone(), entry)?;
        Ok(Self::with_executor(
            source_label,
            program,
            entry,
            resources,
            executor,
        ))
    }

    fn with_executor(
        source_label: String,
        program: AwbcProgram,
        entry: AwbcEntryId,
        resources: SessionRuntimeResources,
        executor: ArcweftRuntimeExecutor,
    ) -> Self {
        Self {
            source_label,
            program,
            entry,
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
        }
    }

    pub(super) fn start_entry(
        &self,
        start: BundleEntryStart,
        root_command_host_calls: &RootCommandHostCallCatalog,
    ) -> Result<Self, BundleEntryStartError> {
        let entry = match start {
            BundleEntryStart::SessionDefault => self.entry,
            BundleEntryStart::Entry(entry) => {
                ensure_start_awbc_entry_selects_flow(&self.program, entry)?;
                entry
            }
        };
        validate_root_command_host_call_catalog(&self.program, entry, root_command_host_calls)?;
        Self::new(
            self.source_label.clone(),
            self.program.clone(),
            entry,
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
    build_session_runtime_with_executor(bundle, options, None)
}

pub(super) fn build_session_runtime_preserving_executor(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
    executor: &ArcweftRuntimeExecutor,
) -> Result<SessionRuntime, BundleSessionError> {
    build_session_runtime_with_executor(bundle, options, Some(executor))
}

fn build_session_runtime_with_executor(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
    preserved_executor: Option<&ArcweftRuntimeExecutor>,
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
    let entry = selected_awbc_entry(&program, bundle, options)?;
    ensure_session_awbc_entry_selects_flow(&program, entry)?;
    validate_root_command_host_call_catalog(&program, entry, &options.root_command_host_calls)?;
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
    let view_product = arcweft_bundle::resource_codec::ValidatedViewProduct::try_new(
        Some(bundle.source_map.clone()),
        bundle.view_program.clone(),
        bundle.view_style.clone(),
        arcweft_bundle::resource_codec::ViewProductValidationLimits::default(),
    )
    .map_err(BundleViewRuntimeError::from)?;
    let mut view_runtime = BundleViewRuntime::try_new(view_product, bundle.view_text.clone())?;
    view_runtime.accept_dialogue_view_definitions(&bundle.display)?;
    let view_theme = bundle.view_theme.clone().unwrap_or_default();
    let view_theme_environment = view_theme.environment_overrides();
    let view_style_palettes = view_theme.system_palette_set();
    let resources = SessionRuntimeResources {
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
    };
    match preserved_executor {
        Some(executor) => {
            let mut executor = executor.clone();
            executor.replace_product_awbc_program(program.clone())?;
            Ok(SessionRuntime::with_executor(
                bundle.source_display_name().to_owned(),
                program,
                entry,
                resources,
                executor,
            ))
        }
        None => SessionRuntime::new(
            bundle.source_display_name().to_owned(),
            program,
            entry,
            resources,
        )
        .map_err(BundleSessionError::from),
    }
}

fn validate_root_command_host_call_catalog(
    program: &AwbcProgram,
    entry: AwbcEntryId,
    catalog: &RootCommandHostCallCatalog,
) -> Result<(), crate::session::RootCommandHostCallCatalogError> {
    let contracts = program
        .entries
        .get(entry.index())
        .and_then(|entry| match &entry.roles {
            arcweft_core::plan::RuntimeEntryRoles::Stateful(roles) => {
                Some(roles.command_policy.admitted.as_slice())
            }
            arcweft_core::plan::RuntimeEntryRoles::None
            | arcweft_core::plan::RuntimeEntryRoles::Agent(_) => None,
        })
        .unwrap_or_default();
    catalog.validate_policy(contracts)
}

pub(super) fn selected_awbc_entry(
    program: &AwbcProgram,
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<AwbcEntryId, BundleSessionError> {
    let Some(entry) = selected_entry(bundle, options)? else {
        return Err(BundleSessionError::MissingEntrySelection);
    };
    program
        .entries
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| {
            (candidate.runtime_id == entry).then(|| {
                AwbcEntryId(
                    u32::try_from(index)
                        .expect("verified AWBC entry table indices fit the u32 wire contract"),
                )
            })
        })
        .ok_or(BundleSessionError::ProductAwbcEntry {
            entry: entry.public_label().into_string(),
        })
}

fn selected_entry(
    bundle: &ArcweftBundle,
    options: &BundleSessionOptions,
) -> Result<Option<EntryRuntimeId>, BundleSessionError> {
    if let Some(entry) = &options.entry {
        return Ok(Some(entry.clone()));
    }
    bundle
        .manifest
        .entry
        .as_deref()
        .map(|entry| {
            EntryRuntimeId::from_source_entity_body(entry).map_err(|error| {
                BundleSessionError::InvalidEntrySelection {
                    entry: entry.to_owned(),
                    message: error.to_string(),
                }
            })
        })
        .transpose()
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
    if program.entries.get(entry.index()).is_none() {
        return Err(BundleEntryStartError::UnknownEntry { entry });
    }
    if awbc_entry_selects_flow(program, entry) {
        Ok(())
    } else {
        Err(BundleEntryStartError::NonFlowEntry { entry })
    }
}

fn awbc_entry_selects_flow(program: &AwbcProgram, entry: AwbcEntryId) -> bool {
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
