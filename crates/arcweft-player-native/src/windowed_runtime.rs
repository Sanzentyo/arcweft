//! Windowed runtime ownership and patch event handling.
//!
//! The owner is the single mutation boundary for a windowed scene's runtime
//! session and image catalog. Transport adapters enqueue typed events; the
//! event loop calls `drain_patch_boundary` only after render submission.

use crate::patch_endpoint::{NativePatchEndpoint, NativePatchEndpointError, NativePatchOutcome};
use crate::windowed_patch::{
    FrameBoundary, PatchEventSource, WindowedPatchError, WindowedPatchEvent, WindowedPatchQueue,
    WindowedPatchReport,
};
use arcweft_adapter_desktop::DesktopAdapterSet;
use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::patch::PatchCompatibility;
use arcweft_bundle::{ArcweftBundle, BundleAdapterManifest, BundleFormat, BundleVirtualFile};
use arcweft_core::task::TaskEvent;
use arcweft_desktop_native::NativeDesktopBackend;
use arcweft_host_adapter::{HostAdapterError, HostAdapterRegistryBuilder, HostCallPolicy};
use arcweft_interaction_model::audio::AudioEvent;
use arcweft_player_scene::images::{BundleImageCatalog, BundleImageCatalogError};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{
    BundleSession, BundleSessionOptions, BundleSessionStep, BundleStepInput,
};
use arcweft_runtime_driver::swap::GenerationId;
use arcweft_runtime_driver::task::HostTaskDispatch;
use arcweft_runtime_host::NativeTaskBridge;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Windowed patch processing result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowedRuntimeOutcome {
    Noop {
        generation: GenerationId,
        content_root: BundleDigest,
    },
    Applied {
        generation: GenerationId,
        compatibility: PatchCompatibility,
        content_root: BundleDigest,
    },
    Restarted {
        generation: GenerationId,
        compatibility: PatchCompatibility,
        content_root: BundleDigest,
    },
    Rejected {
        source: PatchEventSource,
        message: String,
    },
}

impl WindowedRuntimeOutcome {
    /// Stable label used by deterministic smoke reports and regeneration
    /// manifests.
    pub const fn kind_label(&self) -> &'static str {
        match self {
            Self::Noop { .. } => "noop",
            Self::Applied { .. } => "applied",
            Self::Restarted { .. } => "restarted",
            Self::Rejected { .. } => "rejected",
        }
    }

    /// Returns the generation touched by the outcome, when one exists.
    pub const fn generation(&self) -> Option<GenerationId> {
        match self {
            Self::Noop { generation, .. }
            | Self::Applied { generation, .. }
            | Self::Restarted { generation, .. } => Some(*generation),
            Self::Rejected { .. } => None,
        }
    }

    /// Returns the patch compatibility class represented by the outcome, when
    /// the outcome reached a valid patch artifact.
    pub const fn compatibility(&self) -> Option<PatchCompatibility> {
        match self {
            Self::Noop { .. } => Some(PatchCompatibility::ContentOnly),
            Self::Applied { compatibility, .. } | Self::Restarted { compatibility, .. } => {
                Some(*compatibility)
            }
            Self::Rejected { .. } => None,
        }
    }

    /// Returns the active content root after a successful outcome.
    pub const fn content_root(&self) -> Option<BundleDigest> {
        match self {
            Self::Noop { content_root, .. }
            | Self::Applied { content_root, .. }
            | Self::Restarted { content_root, .. } => Some(*content_root),
            Self::Rejected { .. } => None,
        }
    }

    /// Returns the event source that produced a rejection.
    pub const fn rejection_source(&self) -> Option<&PatchEventSource> {
        match self {
            Self::Rejected { source, .. } => Some(source),
            Self::Noop { .. } | Self::Applied { .. } | Self::Restarted { .. } => None,
        }
    }

    /// Returns the rejection message.
    pub fn rejection_message(&self) -> Option<&str> {
        match self {
            Self::Rejected { message, .. } => Some(message.as_str()),
            Self::Noop { .. } | Self::Applied { .. } | Self::Restarted { .. } => None,
        }
    }

    /// Returns whether the prepared frame/input hit data from before this outcome
    /// must be discarded before accepting more interaction.
    pub fn invalidates_prepared_frame(&self) -> bool {
        matches!(self, Self::Applied { .. } | Self::Restarted { .. })
    }

    /// Returns whether this outcome replaced bundle-backed scene resources.
    pub fn refreshes_image_catalog(&self) -> bool {
        matches!(self, Self::Applied { .. } | Self::Restarted { .. })
    }
}

/// Windowed runtime owner error.
#[derive(Debug, Error)]
pub enum WindowedRuntimeOwnerError {
    #[error(transparent)]
    PatchEndpoint(#[from] NativePatchEndpointError),
    #[error(transparent)]
    ImageCatalog(#[from] BundleImageCatalogError),
    #[error(transparent)]
    PatchQueue(#[from] WindowedPatchError),
    #[error("failed to encode windowed runtime bundle: {message}")]
    EncodeBundle { message: String },
    #[error("failed to decode windowed runtime bundle: {message}")]
    DecodeBundle { message: String },
    #[error("native adapter registration failed: {0}")]
    NativeAdapter(#[from] HostAdapterError),
    #[error("failed to create windowed runtime workspace: {0}")]
    CreateWorkspace(std::io::Error),
    #[error("failed to create windowed runtime source directory: {0}")]
    CreateSourceDirectory(std::io::Error),
    #[error("failed to materialize windowed runtime source: {0}")]
    MaterializeSource(std::io::Error),
    #[error("failed to create windowed runtime virtual file directory: {0}")]
    CreateVirtualFileDirectory(std::io::Error),
    #[error("failed to materialize windowed runtime virtual file: {0}")]
    MaterializeVirtualFile(std::io::Error),
    #[error("bundle virtual file path must be relative and normalized")]
    InvalidVirtualFilePath,
}

/// Shared owner for windowed runtime session, active AWFB bytes, image catalog,
/// and queued patch reports.
#[derive(Debug)]
pub struct WindowedRuntimeOwner {
    endpoint: NativePatchEndpoint,
    images: BundleImageCatalog,
    patch_queue: WindowedPatchQueue,
    _workspace: WindowedRuntimeWorkspace,
    host: NativeTaskBridge,
    pending_task_events: Vec<TaskEvent>,
    pending_audio_events: Vec<AudioEvent>,
    pending_host_dispatches: Vec<HostTaskDispatch>,
}

impl WindowedRuntimeOwner {
    /// Creates an owner from a decoded bundle.
    pub fn from_bundle(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
    ) -> Result<Self, WindowedRuntimeOwnerError> {
        Self::from_bundle_with_adapter_installer(bundle, options, |_source_path, builder| {
            Ok(builder)
        })
    }

    /// Creates an owner from a decoded bundle and installs a native desktop backend
    /// before the runtime session is started.
    pub fn from_bundle_with_desktop_backend(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
        backend: NativeDesktopBackend,
    ) -> Result<Self, WindowedRuntimeOwnerError> {
        Self::from_bundle_with_adapter_installer(bundle, options, move |_source_path, builder| {
            let (builder, _coordinator) =
                DesktopAdapterSet::bind_current_thread(backend).register(builder)?;
            Ok(builder)
        })
    }

    /// Creates an owner from a decoded bundle and lets the embedding native host
    /// install event-loop-owned adapters before the AWFB-backed session starts.
    pub fn from_bundle_with_adapter_installer<F>(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
        install: F,
    ) -> Result<Self, WindowedRuntimeOwnerError>
    where
        F: FnOnce(
            &Path,
            HostAdapterRegistryBuilder,
        ) -> Result<HostAdapterRegistryBuilder, HostAdapterError>,
    {
        let images = BundleImageCatalog::from_bundle(bundle)?;
        let awfb_bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .map_err(|error| WindowedRuntimeOwnerError::EncodeBundle {
                message: error.to_string(),
            })?;
        Self::from_decoded_bundle_and_awfb_bytes(bundle, awfb_bytes, images, options, install)
    }

    fn from_decoded_bundle_and_awfb_bytes<F>(
        bundle: &ArcweftBundle,
        awfb_bytes: Vec<u8>,
        images: BundleImageCatalog,
        options: BundleSessionOptions,
        install: F,
    ) -> Result<Self, WindowedRuntimeOwnerError>
    where
        F: FnOnce(
            &Path,
            HostAdapterRegistryBuilder,
        ) -> Result<HostAdapterRegistryBuilder, HostAdapterError>,
    {
        let workspace = WindowedRuntimeWorkspace::create(bundle)?;
        let host = windowed_native_task_bridge(bundle, workspace.source_path(), install)?;
        let endpoint = NativePatchEndpoint::from_awfb_bytes(awfb_bytes, options)?;
        Ok(Self {
            endpoint,
            images,
            patch_queue: WindowedPatchQueue::default(),
            _workspace: workspace,
            host,
            pending_task_events: Vec::new(),
            pending_audio_events: Vec::new(),
            pending_host_dispatches: Vec::new(),
        })
    }

    /// Creates an owner from AWFB bytes.
    pub fn from_awfb_bytes(
        awfb_bytes: Vec<u8>,
        options: BundleSessionOptions,
    ) -> Result<Self, WindowedRuntimeOwnerError> {
        let bundle =
            ArcweftBundle::from_format_slice(BundleFormat::Awfb, &awfb_bytes).map_err(|error| {
                WindowedRuntimeOwnerError::DecodeBundle {
                    message: error.to_string(),
                }
            })?;
        let images = BundleImageCatalog::from_bundle(&bundle)?;
        Self::from_decoded_bundle_and_awfb_bytes(
            &bundle,
            awfb_bytes,
            images,
            options,
            |_source_path, builder| Ok(builder),
        )
    }

    /// Returns the active runtime session.
    pub const fn session(&self) -> &BundleSession {
        self.endpoint.session()
    }

    /// Returns the mutable active runtime session. The windowed event loop owns
    /// this owner, so callers must only borrow mutably inside frame boundaries.
    pub fn session_mut(&mut self) -> &mut BundleSession {
        self.endpoint.session_mut()
    }

    /// Runs queued host-main-thread adapter work and stores deterministic task
    /// events for the next runtime step.
    pub fn pump_main_thread(&mut self) -> Result<usize, WindowedRuntimeOwnerError> {
        self.host.pump_main_thread()?;
        let completions = self.host.poll_completions();
        let completion_count = completions.len();
        let events = self.normalize_host_events(completions);
        self.pending_task_events.extend(events);
        Ok(completion_count)
    }

    /// Pushes native audio events into the scene-safe owner boundary.
    pub fn push_audio_events(&mut self, events: impl IntoIterator<Item = AudioEvent>) {
        self.pending_audio_events.extend(events);
    }

    /// Steps the runtime with queued host-task and audio events, then dispatches
    /// new host task requests through the owner-held native task bridge.
    pub fn step_with_clock(
        &mut self,
        clock: RuntimeClockStep,
        mut input: BundleStepInput,
    ) -> BundleSessionStep {
        input.task_events.append(&mut self.pending_task_events);
        input.audio_events.append(&mut self.pending_audio_events);
        let step = self.endpoint.session_mut().step_with_clock(clock, input);
        self.complete_requested_tasks(step.requested_tasks.clone());
        step
    }

    fn complete_requested_tasks(&mut self, dispatches: Vec<HostTaskDispatch>) {
        if dispatches.is_empty() {
            return;
        }
        self.pending_host_dispatches
            .extend(dispatches.iter().cloned());
        let tasks = dispatches
            .into_iter()
            .map(|dispatch| dispatch.task)
            .collect::<Vec<_>>();
        let events = self.host.complete_tasks(tasks);
        let events = self.normalize_host_events(events);
        self.pending_task_events.extend(events);
    }

    fn normalize_host_events(&mut self, events: Vec<TaskEvent>) -> Vec<TaskEvent> {
        events
            .into_iter()
            .map(|event| self.normalize_host_event(event))
            .collect()
    }

    fn normalize_host_event(&mut self, event: TaskEvent) -> TaskEvent {
        let Some(index) = self
            .pending_host_dispatches
            .iter()
            .position(|dispatch| dispatch.task.id == event.task_id)
        else {
            return event;
        };
        let dispatch = self.pending_host_dispatches.remove(index);
        dispatch.into_event(event.kind)
    }

    /// Returns the active image catalog used by the renderer.
    pub const fn images(&self) -> &BundleImageCatalog {
        &self.images
    }

    /// Returns the active patch queue/report model.
    pub const fn patch_queue(&self) -> &WindowedPatchQueue {
        &self.patch_queue
    }

    /// Returns the retained patch report for future debug overlays and logging.
    pub const fn last_patch_report(&self) -> &WindowedPatchReport {
        self.patch_queue.last_report()
    }

    /// Returns the number of queued patch events awaiting a safe frame boundary.
    pub fn queued_patch_count(&self) -> usize {
        self.patch_queue.len()
    }

    /// Enqueues one typed patch event for safe-boundary processing.
    pub fn push_patch_event(&mut self, event: WindowedPatchEvent) {
        self.patch_queue.push(event);
    }

    /// Retains an adapter-side ingress rejection without mutating the active session/catalog.
    pub fn retain_patch_ingress_rejection(
        &mut self,
        source: PatchEventSource,
        message: impl Into<String>,
    ) {
        self.patch_queue.reject(source, message);
    }

    /// Processes all queued patch events at `boundary`.
    ///
    /// No session or catalog mutation occurs unless `boundary` is
    /// `AfterRenderSubmitted`. Invalid patches update the retained patch report
    /// and leave the old session and catalog intact.
    pub fn drain_patch_boundary(
        &mut self,
        boundary: FrameBoundary,
    ) -> Result<Vec<WindowedRuntimeOutcome>, WindowedRuntimeOwnerError> {
        let mut outcomes = Vec::new();
        while let Some(outcome) = self.process_patch_boundary(boundary)? {
            outcomes.push(outcome);
        }
        Ok(outcomes)
    }

    /// Processes at most one queued patch event at `boundary`.
    ///
    /// No session or catalog mutation occurs unless `boundary` is
    /// `AfterRenderSubmitted`. Invalid patches update the retained patch report
    /// and leave the old session and catalog intact.
    pub fn process_patch_boundary(
        &mut self,
        boundary: FrameBoundary,
    ) -> Result<Option<WindowedRuntimeOutcome>, WindowedRuntimeOwnerError> {
        let event = match self.patch_queue.pop_ready(boundary) {
            Ok(event) => event,
            Err(WindowedPatchError::NoQueuedPatch) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let source = event.source();
        self.patch_queue
            .preparing(source.clone(), "preparing windowed patch");
        let outcome = match event {
            WindowedPatchEvent::ApplyBundle { bytes, .. } => {
                self.apply_patch_bundle_bytes(source.clone(), &bytes)
            }
            WindowedPatchEvent::ApplyTransportSidecar {
                bytes, base_dir, ..
            } => self.apply_transport_sidecar(source.clone(), &bytes, base_dir.as_path()),
            WindowedPatchEvent::RestartWithBundle { bytes, .. } => {
                self.restart_from_bundle_bytes(source.clone(), bytes)
            }
        };
        match outcome {
            Ok(outcome) => {
                self.record_success(source, &outcome);
                Ok(Some(outcome))
            }
            Err(error) => {
                let message = error.to_string();
                self.patch_queue.reject(source.clone(), message.clone());
                Ok(Some(WindowedRuntimeOutcome::Rejected { source, message }))
            }
        }
    }

    fn apply_patch_bundle_bytes(
        &mut self,
        source: PatchEventSource,
        bytes: &[u8],
    ) -> Result<WindowedRuntimeOutcome, WindowedRuntimeOwnerError> {
        let prepared = self.endpoint.prepare_patch_bytes(bytes)?;
        let target_images = images_from_awfb_bytes(prepared.target_awfb_bytes())?;
        let outcome = self.endpoint.apply_prepared_patch(prepared)?;
        let outcome = windowed_outcome_from_native(outcome, source);
        if outcome.refreshes_image_catalog() {
            self.images = target_images;
        }
        Ok(outcome)
    }

    fn apply_transport_sidecar(
        &mut self,
        source: PatchEventSource,
        bytes: &[u8],
        base_dir: &Path,
    ) -> Result<WindowedRuntimeOutcome, WindowedRuntimeOwnerError> {
        let patch_bytes =
            NativePatchEndpoint::patch_bytes_from_transport_json_bytes(bytes, base_dir)?;
        self.apply_patch_bundle_bytes(source, &patch_bytes)
    }

    fn restart_from_bundle_bytes(
        &mut self,
        _source: PatchEventSource,
        bytes: Vec<u8>,
    ) -> Result<WindowedRuntimeOutcome, WindowedRuntimeOwnerError> {
        let images = images_from_awfb_bytes(&bytes)?;
        let endpoint =
            NativePatchEndpoint::from_awfb_bytes(bytes, self.endpoint.options().clone())?;
        let generation = endpoint.session().active_generation().id;
        let content_root = endpoint
            .active_content_root()
            .unwrap_or_else(|| endpoint.session().active_generation().content_root);
        self.endpoint = endpoint;
        self.images = images;
        Ok(WindowedRuntimeOutcome::Restarted {
            generation,
            compatibility: PatchCompatibility::RestartRequired,
            content_root,
        })
    }

    fn record_success(&mut self, source: PatchEventSource, outcome: &WindowedRuntimeOutcome) {
        match outcome {
            WindowedRuntimeOutcome::Noop { .. } => {
                self.patch_queue.applied(
                    source,
                    PatchCompatibility::ContentOnly,
                    "patch was a noop",
                );
            }
            WindowedRuntimeOutcome::Applied { compatibility, .. } => {
                self.patch_queue
                    .applied(source, *compatibility, "patch applied at frame boundary");
            }
            WindowedRuntimeOutcome::Restarted { compatibility, .. } => {
                self.patch_queue.restarting(
                    source,
                    *compatibility,
                    "session restarted at frame boundary",
                );
            }
            WindowedRuntimeOutcome::Rejected { .. } => {}
        }
    }
}

fn windowed_outcome_from_native(
    outcome: NativePatchOutcome,
    _source: PatchEventSource,
) -> WindowedRuntimeOutcome {
    match outcome {
        NativePatchOutcome::Noop {
            generation,
            content_root,
        } => WindowedRuntimeOutcome::Noop {
            generation,
            content_root,
        },
        NativePatchOutcome::Applied {
            report,
            content_root,
        } => WindowedRuntimeOutcome::Applied {
            generation: report.generation,
            compatibility: patch_compatibility_from_swap(report.compatibility),
            content_root,
        },
        NativePatchOutcome::Restarted {
            generation,
            compatibility,
            content_root,
        } => WindowedRuntimeOutcome::Restarted {
            generation,
            compatibility,
            content_root,
        },
    }
}

const fn patch_compatibility_from_swap(
    compatibility: arcweft_runtime_driver::swap::SwapCompatibility,
) -> PatchCompatibility {
    match compatibility {
        arcweft_runtime_driver::swap::SwapCompatibility::ContentOnly => {
            PatchCompatibility::ContentOnly
        }
        arcweft_runtime_driver::swap::SwapCompatibility::CodeCompatible => {
            PatchCompatibility::CodeCompatible
        }
        arcweft_runtime_driver::swap::SwapCompatibility::CodeGenerational => {
            PatchCompatibility::CodeGenerational
        }
        arcweft_runtime_driver::swap::SwapCompatibility::RestartRequired => {
            PatchCompatibility::RestartRequired
        }
    }
}

fn images_from_awfb_bytes(bytes: &[u8]) -> Result<BundleImageCatalog, WindowedRuntimeOwnerError> {
    let bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, bytes).map_err(|error| {
        WindowedRuntimeOwnerError::DecodeBundle {
            message: error.to_string(),
        }
    })?;
    BundleImageCatalog::from_bundle(&bundle).map_err(WindowedRuntimeOwnerError::ImageCatalog)
}

#[derive(Debug)]
struct WindowedRuntimeWorkspace {
    root: PathBuf,
    source_path: PathBuf,
}

impl WindowedRuntimeWorkspace {
    fn create(bundle: &ArcweftBundle) -> Result<Self, WindowedRuntimeOwnerError> {
        let root = std::env::temp_dir().join(format!(
            "arcweft-windowed-runtime-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        fs::create_dir_all(&root).map_err(WindowedRuntimeOwnerError::CreateWorkspace)?;
        let source = bundle.primary_source_document();
        let source_name = bundle_source_file_name(
            source.map_or("bundle.arcw", |source| source.display_name().display_name()),
        );
        let source_path = root.join(source_name);
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent).map_err(WindowedRuntimeOwnerError::CreateSourceDirectory)?;
        }
        fs::write(&source_path, source.map_or("", |source| source.text()))
            .map_err(WindowedRuntimeOwnerError::MaterializeSource)?;
        materialize_bundle_virtual_files(&root, &bundle.virtual_files)?;
        Ok(Self { root, source_path })
    }

    fn source_path(&self) -> &Path {
        &self.source_path
    }
}

impl Drop for WindowedRuntimeWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn windowed_native_task_bridge<F>(
    bundle: &ArcweftBundle,
    source_path: &Path,
    install: F,
) -> Result<NativeTaskBridge, WindowedRuntimeOwnerError>
where
    F: FnOnce(
        &Path,
        HostAdapterRegistryBuilder,
    ) -> Result<HostAdapterRegistryBuilder, HostAdapterError>,
{
    let builder = arcweft_runtime_host::native_task::standard_cli_registry_builder(
        arcweft_runtime_host::NativeFileRoots::for_bundle_workspace(source_path),
    )?;
    let registry = install(source_path, builder)?.build();
    NativeTaskBridge::try_with_registry(windowed_host_policy(bundle), registry)
        .map_err(WindowedRuntimeOwnerError::NativeAdapter)
}

fn windowed_host_policy(bundle: &ArcweftBundle) -> HostCallPolicy {
    HostCallPolicy::from_host_call_ids(
        bundle
            .adapter_manifests
            .iter()
            .flat_map(BundleAdapterManifest::host_call_ids),
    )
}

fn bundle_source_file_name(label: &str) -> String {
    let path = Path::new(label);
    path.file_name()
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
        })
        .map_or_else(
            || "bundle.arcw".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
}

fn materialize_bundle_virtual_files(
    root: &Path,
    files: &[BundleVirtualFile],
) -> Result<(), WindowedRuntimeOwnerError> {
    for file in files {
        let relative = Path::new(&file.path);
        validate_relative_virtual_path(relative)?;
        let path = root
            .join(".arcweft")
            .join(file.space.as_str())
            .join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(WindowedRuntimeOwnerError::CreateVirtualFileDirectory)?;
        }
        fs::write(&path, &file.bytes).map_err(WindowedRuntimeOwnerError::MaterializeVirtualFile)?;
    }
    Ok(())
}

fn validate_relative_virtual_path(path: &Path) -> Result<(), WindowedRuntimeOwnerError> {
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
        .then_some(())
        .ok_or(WindowedRuntimeOwnerError::InvalidVirtualFilePath)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windowed_patch::{WindowedPatchEvent, WindowedPatchState};
    use arcweft_bundle::container::{BundleView, ReadBudget};
    use arcweft_bundle::patch::{BundlePatchArtifact, encode_patch_bundle};
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{
        BundleImageAnimation, BundleImageAsset, BundleImageDimensions, BundleImageFormat,
        BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds,
        BundleImageObjectFit, BundleImageObjectPlayback, BundleImageObjectTransform,
        BundleManifest, BundleRuntimeSummary, BundleVirtualFile, BundleVirtualFileRef,
        BundleVirtualFileSpace,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_core::line_task::LineTaskGroup;
    use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan};
    use arcweft_dialogue::{DialogueProfileRevision, InlineFailurePolicy};
    use arcweft_render_text::{
        LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode,
    };
    use arcweft_render_wgpu::geometry::RenderViewport;
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_runtime_driver::clock::RuntimeClockStep;
    use arcweft_runtime_driver::session::BundleStepInput;
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

    fn test_dialogue_revision() -> DialogueProfileRevision {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("player-native-windowed-runtime-test").expect("document ID"),
            SourceName::Memory,
            "test manifest",
        )
        .expect("test document");
        let sources = SourceSetRevision::try_for_identities([manifest.identity()])
            .expect("test source revision");
        DialogueProfileRevision::from_admitted_parts(
            manifest.identity().clone(),
            sources,
            sources,
            ViewProgramId::try_new("view_program.player-native-windowed-runtime-test")
                .expect("View program ID"),
            AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
            ResourceTypeRegistry::empty().digest(),
        )
    }

    const RED_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    const BLUE_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0x60, 0xf8, 0xff, 0x1f, 0x00, 0x03, 0x02, 0x01, 0xff, 0xe6, 0x77, 0x0b, 0xae, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn unsafe_boundaries_do_not_pop_queued_owner_events() {
        let bundle = fixture_bundle_with("Old text", RED_PNG);
        let mut owner = WindowedRuntimeOwner::from_bundle(&bundle, BundleSessionOptions::default())
            .expect("owner starts");
        owner.push_patch_event(WindowedPatchEvent::ApplyBundle {
            bytes: b"not a patch".to_vec(),
            source: PatchEventSource::EmbeddingApi,
        });

        let error = owner
            .drain_patch_boundary(FrameBoundary::BeforeRender)
            .expect_err("unsafe boundary rejects processing");

        assert!(matches!(
            error,
            WindowedRuntimeOwnerError::PatchQueue(WindowedPatchError::UnsafeBoundary(
                FrameBoundary::BeforeRender
            ))
        ));
        assert_eq!(owner.queued_patch_count(), 1);
        assert_eq!(owner.last_patch_report().state, WindowedPatchState::Queued);
    }

    #[test]
    fn invalid_patch_report_leaves_previous_session_and_image_catalog_observable() {
        let bundle = fixture_bundle_with("Old text", RED_PNG);
        let old_root = awfb_root(&awfb_bytes(&bundle));
        let mut owner = WindowedRuntimeOwner::from_bundle(&bundle, BundleSessionOptions::default())
            .expect("owner starts");
        let old_rgba = rendered_rgba(&owner);

        owner.push_patch_event(WindowedPatchEvent::ApplyBundle {
            bytes: b"not a patch".to_vec(),
            source: PatchEventSource::EmbeddingApi,
        });
        let outcomes = owner
            .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)
            .expect("invalid patch is retained as rejected outcome");

        assert!(matches!(
            outcomes.as_slice(),
            [WindowedRuntimeOutcome::Rejected { .. }]
        ));
        assert_eq!(
            owner.last_patch_report().state,
            WindowedPatchState::Rejected
        );
        assert_eq!(
            owner.session().active_container_content_root(),
            Some(old_root)
        );
        let step = owner.session_mut().step_with_clock(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput::default(),
        );
        assert_eq!(
            step.presentation
                .dialogue
                .latest_active()
                .and_then(|(_, entry)| entry.current_stage())
                .map(arcweft_render_text::LineDisplayStage::text),
            Some("Old text")
        );
        assert_eq!(rendered_rgba(&owner), old_rgba);
    }

    #[test]
    fn content_only_patch_refreshes_image_catalog_at_safe_boundary() {
        let old = fixture_bundle_with("Old text", RED_PNG);
        let new = fixture_bundle_with("New text", BLUE_PNG);
        let patch_bytes = patch_bytes(&awfb_bytes(&old), &awfb_bytes(&new));
        let mut owner = WindowedRuntimeOwner::from_bundle(&old, BundleSessionOptions::default())
            .expect("owner starts");
        assert_eq!(rendered_rgba(&owner), vec![255, 0, 0, 255]);

        owner.push_patch_event(WindowedPatchEvent::ApplyBundle {
            bytes: patch_bytes,
            source: PatchEventSource::EmbeddingApi,
        });
        let outcomes = owner
            .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)
            .expect("content patch applies");

        assert!(matches!(
            outcomes.as_slice(),
            [WindowedRuntimeOutcome::Applied {
                compatibility: PatchCompatibility::ContentOnly,
                ..
            }]
        ));
        assert_eq!(rendered_rgba(&owner), vec![0, 0, 255, 255]);
        assert_eq!(owner.last_patch_report().state, WindowedPatchState::Applied);
    }

    fn rendered_rgba(owner: &WindowedRuntimeOwner) -> Vec<u8> {
        let rendered = owner
            .images()
            .render_images(&[fixture_image_object()], 0, fixture_image_probe_viewport())
            .expect("fixture image renders");
        let [image] = rendered.as_slice() else {
            panic!("one fixture image renders");
        };
        assert_eq!(image.frame.width, 1);
        assert_eq!(image.frame.height, 1);
        image.frame.rgba.clone()
    }

    fn awfb_bytes(bundle: &ArcweftBundle) -> Vec<u8> {
        bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("fixture encodes")
    }

    fn awfb_root(bytes: &[u8]) -> BundleDigest {
        BundleView::parse(bytes, ReadBudget::default())
            .expect("fixture parses")
            .content_root()
    }

    fn patch_bytes(old: &[u8], new: &[u8]) -> Vec<u8> {
        let old_view = BundleView::parse(old, ReadBudget::default()).expect("old parses");
        let new_view = BundleView::parse(new, ReadBudget::default()).expect("new parses");
        let artifact = BundlePatchArtifact::from_views(&old_view, &new_view).expect("patch builds");
        encode_patch_bundle(&artifact).expect("patch encodes")
    }

    fn fixture_bundle_with(display_text: &str, image_bytes: &[u8]) -> ArcweftBundle {
        let line = RuntimeLineId::from_runtime_line_value("line.opening").expect("runtime line id");
        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
                ops: vec![
                    FlowOp::Dialogue {
                        line: line.clone(),
                        task_group: 0,
                    },
                    FlowOp::Return("done".to_owned()),
                ],
            }],
            vec![LineTaskGroup::default()],
        )
        .expect("runtime plan is valid")
        .with_entries(vec![arcweft_core::plan::RuntimeEntrySpec {
            id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.main")
                .expect("test entry ID is valid"),
            kind: arcweft_core::plan::RuntimeEntryKind::Cli,
            binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
            target: arcweft_core::plan::RuntimeEntryTarget::Flow(
                FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
            ),
            roles: arcweft_core::entry::RuntimeEntryRoles::None,
        }]);
        let display = LineDisplayCatalog::try_from_lines(
            test_dialogue_revision(),
            vec![LineDisplaySpec {
                line,
                callee: "alice".to_owned(),
                speaker_label: None,
                text_key: None,
                view: arcweft_bundle::standard_view::dialogue_view_id(),
                profile_style: None,
                dialogue_revision: test_dialogue_revision(),
                voice: None,
                look: None,
                style: None,
                base_styles: Vec::new(),
                inline_failure: InlineFailurePolicy::FailLine,
                style_contributions: Vec::new(),
                args: Vec::new(),
                content: RichTextDocument::new(vec![RichTextNode::Text {
                    text: display_text.to_owned(),
                }]),
            }],
        )
        .expect("test display catalog is revision-consistent");
        let product_awbc = AwbcLowerer::new(&plan, &display, "windowed-runtime-owner.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bytecode = BytecodeProgram::from_runtime_plan(plan);
        let stats = bytecode.stats();
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: stats.flows,
                    bytecode_instructions: stats.instructions,
                    line_task_groups: stats.line_task_groups,
                    stream_plans: stats.stream_plans,
                    source_plans: stats.source_plans,
                },
            },
            source_map("windowed-runtime-owner.arcw", "flow main { ... }"),
            bytecode,
            display,
        )
        .expect("standard dialogue source joins source map")
        .with_product_awbc(product_awbc)
        .with_virtual_files([BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "sprite.png".to_owned(),
            bytes: image_bytes.to_vec(),
        }])
        .with_image_assets([BundleImageAsset {
            id: "sprite".to_owned(),
            file: BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "sprite.png".to_owned(),
            },
            format: BundleImageFormat::Png,
            animation: BundleImageAnimation::Static,
            dimensions: Some(BundleImageDimensions {
                width: 1,
                height: 1,
            }),
        }])
        .with_image_objects([fixture_image_object()])
    }

    fn source_map(label: &str, text: &str) -> SourceMapSection {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(label).expect("source ID"),
            SourceName::path(label),
            text,
        )
        .expect("source document");
        SourceMapSection::try_from_documents(&[&document]).expect("source map")
    }

    fn fixture_image_object() -> BundleImageObject {
        BundleImageObject {
            id: "sprite.object".to_owned(),
            asset: "sprite".to_owned(),
            target: None,
            layer: None,
            view: None,
            containing_scroll_region: None,
            bounds: BundleImageObjectBounds {
                x_milli: 0,
                y_milli: 0,
                width_milli: 1000,
                height_milli: 1000,
            },
            placement: None,
            fit: BundleImageObjectFit::Stretch,
            alignment: BundleImageObjectAlignment {
                x_milli: 500,
                y_milli: 500,
            },
            playback: BundleImageObjectPlayback {
                start_time_millis: 0,
                rate_milli: 1000,
                paused_at_millis: None,
                pinned_local_time_millis: None,
            },
            transform: BundleImageObjectTransform {
                m11_milli: 1000,
                m12_milli: 0,
                m21_milli: 0,
                m22_milli: 1000,
                tx_milli: 0,
                ty_milli: 0,
            },
            depth_milli: 0,
            opacity_milli: 1000,
            actions: Vec::new(),
            params: std::collections::BTreeMap::default(),
            proxies: Vec::new(),
            visible: true,
        }
    }

    fn fixture_image_probe_viewport() -> RenderViewport {
        RenderViewport {
            logical_width: 1.0,
            logical_height: 1.0,
            physical_width: 1,
            physical_height: 1,
            scale_factor: 1.0,
        }
    }
}
