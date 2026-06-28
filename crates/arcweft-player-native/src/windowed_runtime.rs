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
use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::patch::{
    PatchBundleError, PatchCompatibility, apply_patch_bundle, decode_patch_bundle,
};
use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_player_scene::images::{BundleImageCatalog, BundleImageCatalogError};
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions};
use arcweft_runtime_driver::swap::GenerationId;
use std::path::Path;
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
    #[error("failed to decode windowed patch bundle before commit: {0}")]
    DecodePatch(#[source] PatchBundleError),
    #[error("failed to materialize windowed patch target before commit: {0}")]
    MaterializePatch(#[source] PatchBundleError),
}

/// Shared owner for windowed runtime session, active AWFB bytes, image catalog,
/// and queued patch reports.
#[derive(Debug)]
pub struct WindowedRuntimeOwner {
    endpoint: NativePatchEndpoint,
    images: BundleImageCatalog,
    patch_queue: WindowedPatchQueue,
}

impl WindowedRuntimeOwner {
    /// Creates an owner from a decoded bundle.
    pub fn from_bundle(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
    ) -> Result<Self, WindowedRuntimeOwnerError> {
        let images = BundleImageCatalog::from_bundle(bundle)?;
        let awfb_bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .map_err(|error| WindowedRuntimeOwnerError::EncodeBundle {
                message: error.to_string(),
            })?;
        let endpoint = NativePatchEndpoint::from_awfb_bytes(awfb_bytes, options)?;
        Ok(Self {
            endpoint,
            images,
            patch_queue: WindowedPatchQueue::default(),
        })
    }

    /// Creates an owner from AWFB bytes.
    pub fn from_awfb_bytes(
        awfb_bytes: Vec<u8>,
        options: BundleSessionOptions,
    ) -> Result<Self, WindowedRuntimeOwnerError> {
        let images = images_from_awfb_bytes(&awfb_bytes)?;
        let endpoint = NativePatchEndpoint::from_awfb_bytes(awfb_bytes, options)?;
        Ok(Self {
            endpoint,
            images,
            patch_queue: WindowedPatchQueue::default(),
        })
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
            WindowedPatchEvent::ApplyTransportSidecar { bytes, .. } => {
                self.apply_transport_sidecar(source.clone(), &bytes)
            }
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
        let target_images = self.images_from_patch_target(bytes)?;
        let outcome = self.endpoint.apply_patch_bytes(bytes)?;
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
    ) -> Result<WindowedRuntimeOutcome, WindowedRuntimeOwnerError> {
        let patch_bytes =
            NativePatchEndpoint::patch_bytes_from_transport_json_bytes(bytes, Path::new("."))?;
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

    fn images_from_patch_target(
        &self,
        patch_awfb_bytes: &[u8],
    ) -> Result<BundleImageCatalog, WindowedRuntimeOwnerError> {
        let artifact = decode_patch_bundle(patch_awfb_bytes)
            .map_err(WindowedRuntimeOwnerError::DecodePatch)?;
        let materialized = apply_patch_bundle(self.endpoint.active_awfb_bytes(), &artifact)
            .map_err(WindowedRuntimeOwnerError::MaterializePatch)?;
        images_from_awfb_bytes(&materialized.bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windowed_patch::{WindowedPatchEvent, WindowedPatchState};
    use arcweft_bundle::container::{BundleView, ReadBudget};
    use arcweft_bundle::patch::{BundlePatchArtifact, encode_patch_bundle};
    use arcweft_bundle::{
        BundleImageAnimation, BundleImageAsset, BundleImageDimensions, BundleImageFormat,
        BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds,
        BundleImageObjectFit, BundleImageObjectPlayback, BundleImageObjectTransform,
        BundleManifest, BundleRuntimeSummary, BundleSource, BundleVirtualFile,
        BundleVirtualFileRef, BundleVirtualFileSpace,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_core::line_task::LineTaskGroup;
    use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan};
    use arcweft_render_text::{
        LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode,
    };
    use arcweft_runtime_driver::clock::RuntimeClockStep;
    use arcweft_runtime_driver::session::BundleStepInput;
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

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
                .as_ref()
                .map(|frame| frame.text.as_str()),
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
            .render_images(&[fixture_image_object()], 0)
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
        let line = RuntimeLineId("line.opening".to_owned());
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
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
        .expect("runtime plan is valid");
        let display = LineDisplayCatalog::new(vec![LineDisplaySpec {
            line,
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![RichTextNode::Text {
                text: display_text.to_owned(),
            }]),
        }]);
        let product_awbc = AwbcLowerer::new(&plan, &display, "windowed-runtime-owner.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bytecode = BytecodeProgram::from_runtime_plan(plan);
        let stats = bytecode.stats();
        ArcweftBundle::new(
            BundleManifest {
                source_label: "windowed-runtime-owner.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: None,
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
            BundleSource {
                label: "windowed-runtime-owner.arcw".to_owned(),
                text: "flow @flow.main main { ... }".to_owned(),
            },
            bytecode,
            display,
        )
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

    fn fixture_image_object() -> BundleImageObject {
        BundleImageObject {
            id: "sprite.object".to_owned(),
            asset: "sprite".to_owned(),
            bounds: BundleImageObjectBounds {
                x_milli: 0,
                y_milli: 0,
                width_milli: 1000,
                height_milli: 1000,
            },
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
            visible: true,
        }
    }
}
