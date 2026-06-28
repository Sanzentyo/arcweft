//! Windowed runtime ownership and patch event handling.
//!
//! The owner is the single mutation boundary for a windowed scene's runtime
//! session and image catalog. Transport adapters enqueue typed events; the
//! event loop calls `process_patch_boundary` only after render submission.

use crate::patch_endpoint::{NativePatchEndpoint, NativePatchEndpointError, NativePatchOutcome};
use crate::windowed_patch::{
    FrameBoundary, PatchEventSource, WindowedPatchError, WindowedPatchEvent, WindowedPatchQueue,
};
use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::patch::PatchCompatibility;
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

/// Windowed runtime owner error.
#[derive(Debug, Error)]
pub enum WindowedRuntimeOwnerError {
    #[error(transparent)]
    PatchEndpoint(#[from] NativePatchEndpointError),
    #[error(transparent)]
    ImageCatalog(#[from] BundleImageCatalogError),
    #[error(transparent)]
    PatchQueue(#[from] WindowedPatchError),
    #[error("failed to decode windowed runtime bundle: {message}")]
    DecodeBundle { message: String },
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

    /// Enqueues one typed patch event for safe-boundary processing.
    pub fn push_patch_event(&mut self, event: WindowedPatchEvent) {
        self.patch_queue.push(event);
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
            WindowedPatchEvent::ApplyBundle { bytes, .. }
            | WindowedPatchEvent::RestartWithBundle { bytes, .. } => {
                self.restart_from_bundle_bytes(source.clone(), bytes)
            }
            WindowedPatchEvent::ApplyTransportSidecar { bytes, .. } => {
                self.apply_transport_sidecar(source.clone(), &bytes)
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

    fn apply_transport_sidecar(
        &mut self,
        source: PatchEventSource,
        bytes: &[u8],
    ) -> Result<WindowedRuntimeOutcome, WindowedRuntimeOwnerError> {
        let outcome = self
            .endpoint
            .apply_patch_transport_json_bytes(bytes, Path::new("."))?;
        self.refresh_images_from_active_bytes()?;
        Ok(windowed_outcome_from_native(outcome, source))
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

    fn refresh_images_from_active_bytes(&mut self) -> Result<(), WindowedRuntimeOwnerError> {
        self.images = images_from_awfb_bytes(self.endpoint.active_awfb_bytes())?;
        Ok(())
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
