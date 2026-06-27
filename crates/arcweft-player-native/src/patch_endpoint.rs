//! In-process dev patch endpoint for native player hosts.

use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::patch::{
    PatchBundleError, PatchCompatibility, apply_patch_bundle, decode_patch_bundle,
};
use arcweft_runtime_driver::session::{
    BundleHotSwapError, BundleHotSwapReport, BundlePatchReadiness, BundlePatchReadinessReport,
    BundleSession, BundleSessionError, BundleSessionOptions,
};
use arcweft_runtime_driver::swap::GenerationId;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const PATCH_TRANSPORT_SCHEMA_VERSION: u32 = 1;

/// In-process patch endpoint for an AWFB-backed native player session.
///
/// The endpoint owns the active base AWFB bytes because patch materialization is
/// defined against the previous container bytes, not only the decoded bundle.
#[derive(Debug)]
pub struct NativePatchEndpoint {
    base_awfb_bytes: Vec<u8>,
    options: BundleSessionOptions,
    session: BundleSession,
}

/// Result of applying a patch through the native player endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativePatchOutcome {
    Noop {
        generation: GenerationId,
        content_root: BundleDigest,
    },
    Applied {
        report: BundleHotSwapReport,
        content_root: BundleDigest,
    },
    Restarted {
        generation: GenerationId,
        compatibility: PatchCompatibility,
        content_root: BundleDigest,
    },
}

/// Error raised by the native in-process patch endpoint.
#[derive(Debug, Error)]
pub enum NativePatchEndpointError {
    #[error("failed to start native patch endpoint session: {0}")]
    Session(#[from] BundleSessionError),
    #[error("failed to decode AWFB patch bundle: {0}")]
    DecodePatch(#[source] PatchBundleError),
    #[error("patch is not ready for the active native session: {0}")]
    InspectPatch(#[source] BundleHotSwapError),
    #[error("failed to materialize patch target: {0}")]
    MaterializePatch(#[source] PatchBundleError),
    #[error("failed to apply live patch: {0}")]
    LiveApply(#[source] BundleHotSwapError),
    #[error("failed to restart native session from patch target: {0}")]
    Restart(#[source] BundleSessionError),
    #[error("failed to read patch transport file {path}: {message}")]
    ReadTransport { path: PathBuf, message: String },
    #[error("failed to read patch bundle file {path}: {message}")]
    ReadPatch { path: PathBuf, message: String },
    #[error("failed to decode patch transport JSON: {0}")]
    DecodeTransport(#[source] serde_json::Error),
    #[error("unsupported patch transport schema version {actual}; expected {expected}")]
    UnsupportedTransportSchema { actual: u32, expected: u32 },
    #[error("patch transport runner `{0}` is not supported by the native player endpoint")]
    UnsupportedTransportRunner(String),
    #[error("patch transport field `{field}` must not be empty")]
    EmptyTransportField { field: &'static str },
    #[error(
        "patch transport action `{actual:?}` does not match compatibility `{compatibility:?}`; expected `{expected:?}`"
    )]
    TransportActionMismatch {
        actual: NativePatchTransportAction,
        expected: NativePatchTransportAction,
        compatibility: PatchCompatibility,
    },
    #[error("patch transport {field} root mismatch: envelope {envelope}, artifact {artifact}")]
    TransportRootMismatch {
        field: &'static str,
        envelope: BundleDigest,
        artifact: BundleDigest,
    },
    #[error(
        "patch transport compatibility mismatch: envelope `{envelope}`, artifact `{artifact:?}`"
    )]
    TransportCompatibilityMismatch {
        envelope: String,
        artifact: PatchCompatibility,
    },
    #[error("patch transport operation count mismatch: envelope {envelope}, artifact {artifact}")]
    TransportOperationCountMismatch { envelope: usize, artifact: usize },
    #[error("invalid patch transport digest `{value}`")]
    InvalidTransportDigest { value: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct NativePatchTransportEnvelope {
    schema_version: u32,
    runner: String,
    source: String,
    target_bundle: String,
    patch_bundle: String,
    base_content_root: String,
    target_content_root: String,
    compatibility: String,
    operation_count: usize,
    action: NativePatchTransportAction,
}

/// Apply/restart action recorded in a local dev patch transport sidecar.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NativePatchTransportAction {
    ApplyPatch,
    RestartPlayer,
}

impl NativePatchEndpoint {
    /// Starts an AWFB-backed player session that can receive patch bundles.
    pub fn from_awfb_bytes(
        base_awfb_bytes: Vec<u8>,
        options: BundleSessionOptions,
    ) -> Result<Self, NativePatchEndpointError> {
        let session = BundleSession::from_awfb_bytes(&base_awfb_bytes, options.clone())?;
        Ok(Self {
            base_awfb_bytes,
            options,
            session,
        })
    }

    pub const fn session(&self) -> &BundleSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut BundleSession {
        &mut self.session
    }

    pub const fn active_content_root(&self) -> Option<BundleDigest> {
        self.session.active_container_content_root()
    }

    pub fn active_awfb_bytes(&self) -> &[u8] {
        &self.base_awfb_bytes
    }

    pub fn inspect_patch_bytes(
        &self,
        patch_awfb_bytes: &[u8],
    ) -> Result<BundlePatchReadinessReport, NativePatchEndpointError> {
        self.session
            .inspect_hot_swap_patch_bytes(patch_awfb_bytes)
            .map_err(NativePatchEndpointError::InspectPatch)
    }

    /// Applies a patch live when possible, otherwise restarts the owned session.
    pub fn apply_patch_bytes(
        &mut self,
        patch_awfb_bytes: &[u8],
    ) -> Result<NativePatchOutcome, NativePatchEndpointError> {
        let artifact =
            decode_patch_bundle(patch_awfb_bytes).map_err(NativePatchEndpointError::DecodePatch)?;
        let readiness = self
            .session
            .inspect_hot_swap_patch_artifact(&artifact)
            .map_err(NativePatchEndpointError::InspectPatch)?;
        if readiness.readiness == BundlePatchReadiness::Noop {
            return Ok(NativePatchOutcome::Noop {
                generation: readiness.base_generation,
                content_root: readiness.target_content_root,
            });
        }

        let materialized = apply_patch_bundle(&self.base_awfb_bytes, &artifact)
            .map_err(NativePatchEndpointError::MaterializePatch)?;
        let target_awfb_bytes = materialized.bytes;
        match self
            .session
            .hot_swap_patch_bytes(&self.base_awfb_bytes, patch_awfb_bytes)
        {
            Ok(report) => {
                self.base_awfb_bytes = target_awfb_bytes;
                Ok(NativePatchOutcome::Applied {
                    report,
                    content_root: readiness.target_content_root,
                })
            }
            Err(BundleHotSwapError::RestartRequired { .. }) => self.restart_from_patch_target(
                target_awfb_bytes,
                readiness.compatibility,
                readiness.target_content_root,
            ),
            Err(error) => Err(NativePatchEndpointError::LiveApply(error)),
        }
    }

    /// Applies a watch-transport sidecar emitted by `arcw run --watch`.
    pub fn apply_patch_transport_path(
        &mut self,
        transport_path: &Path,
    ) -> Result<NativePatchOutcome, NativePatchEndpointError> {
        let bytes =
            fs::read(transport_path).map_err(|error| NativePatchEndpointError::ReadTransport {
                path: transport_path.to_path_buf(),
                message: error.to_string(),
            })?;
        let base_dir = transport_path.parent().unwrap_or_else(|| Path::new("."));
        self.apply_patch_transport_json_bytes(&bytes, base_dir)
    }

    /// Applies a decoded watch-transport JSON payload using `base_dir` for
    /// relative sidecar paths.
    pub fn apply_patch_transport_json_bytes(
        &mut self,
        bytes: &[u8],
        base_dir: &Path,
    ) -> Result<NativePatchOutcome, NativePatchEndpointError> {
        let envelope: NativePatchTransportEnvelope =
            serde_json::from_slice(bytes).map_err(NativePatchEndpointError::DecodeTransport)?;
        self.apply_patch_transport_envelope(&envelope, base_dir)
    }

    fn apply_patch_transport_envelope(
        &mut self,
        envelope: &NativePatchTransportEnvelope,
        base_dir: &Path,
    ) -> Result<NativePatchOutcome, NativePatchEndpointError> {
        validate_transport_envelope_header(envelope)?;
        let patch_path = resolve_transport_path(base_dir, &envelope.patch_bundle);
        let patch_bytes =
            fs::read(&patch_path).map_err(|error| NativePatchEndpointError::ReadPatch {
                path: patch_path,
                message: error.to_string(),
            })?;
        validate_transport_patch_metadata(envelope, &patch_bytes)?;
        self.apply_patch_bytes(&patch_bytes)
    }

    fn restart_from_patch_target(
        &mut self,
        target_awfb_bytes: Vec<u8>,
        compatibility: PatchCompatibility,
        content_root: BundleDigest,
    ) -> Result<NativePatchOutcome, NativePatchEndpointError> {
        let session = BundleSession::from_awfb_bytes(&target_awfb_bytes, self.options.clone())
            .map_err(NativePatchEndpointError::Restart)?;
        let generation = session.active_generation().id;
        self.base_awfb_bytes = target_awfb_bytes;
        self.session = session;
        Ok(NativePatchOutcome::Restarted {
            generation,
            compatibility,
            content_root,
        })
    }
}

fn validate_transport_envelope_header(
    envelope: &NativePatchTransportEnvelope,
) -> Result<(), NativePatchEndpointError> {
    if envelope.schema_version != PATCH_TRANSPORT_SCHEMA_VERSION {
        return Err(NativePatchEndpointError::UnsupportedTransportSchema {
            actual: envelope.schema_version,
            expected: PATCH_TRANSPORT_SCHEMA_VERSION,
        });
    }
    if !matches!(envelope.runner.as_str(), "native" | "headless" | "auto") {
        return Err(NativePatchEndpointError::UnsupportedTransportRunner(
            envelope.runner.clone(),
        ));
    }
    for (field, value) in [
        ("source", envelope.source.as_str()),
        ("target_bundle", envelope.target_bundle.as_str()),
        ("patch_bundle", envelope.patch_bundle.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(NativePatchEndpointError::EmptyTransportField { field });
        }
    }
    Ok(())
}

fn validate_transport_patch_metadata(
    envelope: &NativePatchTransportEnvelope,
    patch_bytes: &[u8],
) -> Result<(), NativePatchEndpointError> {
    let artifact =
        decode_patch_bundle(patch_bytes).map_err(NativePatchEndpointError::DecodePatch)?;
    let base = parse_transport_digest(&envelope.base_content_root)?;
    let target = parse_transport_digest(&envelope.target_content_root)?;
    if base != artifact.manifest.base_content_root {
        return Err(NativePatchEndpointError::TransportRootMismatch {
            field: "base_content_root",
            envelope: base,
            artifact: artifact.manifest.base_content_root,
        });
    }
    if target != artifact.manifest.target_content_root {
        return Err(NativePatchEndpointError::TransportRootMismatch {
            field: "target_content_root",
            envelope: target,
            artifact: artifact.manifest.target_content_root,
        });
    }
    if envelope.compatibility != artifact.manifest.compatibility.label() {
        return Err(NativePatchEndpointError::TransportCompatibilityMismatch {
            envelope: envelope.compatibility.clone(),
            artifact: artifact.manifest.compatibility,
        });
    }
    if envelope.operation_count != artifact.plan.operations.len() {
        return Err(NativePatchEndpointError::TransportOperationCountMismatch {
            envelope: envelope.operation_count,
            artifact: artifact.plan.operations.len(),
        });
    }
    let expected_action = transport_action_for_compatibility(artifact.manifest.compatibility);
    if envelope.action != expected_action {
        return Err(NativePatchEndpointError::TransportActionMismatch {
            actual: envelope.action,
            expected: expected_action,
            compatibility: artifact.manifest.compatibility,
        });
    }
    Ok(())
}

const fn transport_action_for_compatibility(
    compatibility: PatchCompatibility,
) -> NativePatchTransportAction {
    match compatibility {
        PatchCompatibility::ContentOnly | PatchCompatibility::CodeCompatible => {
            NativePatchTransportAction::ApplyPatch
        }
        PatchCompatibility::CodeGenerational | PatchCompatibility::RestartRequired => {
            NativePatchTransportAction::RestartPlayer
        }
    }
}

fn resolve_transport_path(base_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() || path.exists() {
        path
    } else {
        base_dir.join(path)
    }
}

fn parse_transport_digest(value: &str) -> Result<BundleDigest, NativePatchEndpointError> {
    let mut bytes = [0; 32];
    if value.len() != 64 {
        return Err(NativePatchEndpointError::InvalidTransportDigest {
            value: value.to_owned(),
        });
    }
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index.saturating_mul(2);
        let hex = &value[start..start + 2];
        *byte = u8::from_str_radix(hex, 16).map_err(|_| {
            NativePatchEndpointError::InvalidTransportDigest {
                value: value.to_owned(),
            }
        })?;
    }
    Ok(BundleDigest::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::container::{BundleView, ReadBudget};
    use arcweft_bundle::patch::{BundlePatchArtifact, encode_patch_bundle};
    use arcweft_bundle::{
        ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary, BundleSource,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_core::line_task::LineTaskGroup;
    use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan};
    use arcweft_render_text::{
        LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode,
    };
    use arcweft_runtime_driver::clock::RuntimeClockStep;
    use arcweft_runtime_driver::session::BundleStepInput;
    use arcweft_runtime_driver::swap::SwapCompatibility;
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn native_patch_endpoint_applies_content_patch_live() {
        let old = fixture_bundle_with("Old text", false);
        let new = fixture_bundle_with("New text", false);
        let old_bytes = awfb_bytes(&old);
        let new_bytes = awfb_bytes(&new);
        let patch_bytes = patch_bytes(&old_bytes, &new_bytes);
        let new_root = awfb_root(&new_bytes);
        let mut endpoint =
            NativePatchEndpoint::from_awfb_bytes(old_bytes, BundleSessionOptions::default())
                .expect("endpoint starts");

        let outcome = endpoint
            .apply_patch_bytes(&patch_bytes)
            .expect("content patch applies");

        assert!(matches!(
            outcome,
            NativePatchOutcome::Applied {
                report: BundleHotSwapReport {
                    compatibility: SwapCompatibility::ContentOnly,
                    ..
                },
                content_root,
            } if content_root == new_root
        ));
        assert_eq!(endpoint.active_content_root(), Some(new_root));
        let step = endpoint.session_mut().step_with_clock(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput::default(),
        );
        assert_eq!(
            step.presentation
                .dialogue
                .as_ref()
                .map(|frame| frame.text.as_str()),
            Some("New text")
        );
    }

    #[test]
    fn native_patch_endpoint_restarts_for_generational_patch_target() {
        let old = fixture_bundle_with("Dialogue text", false);
        let new = fixture_bundle_with("Dialogue text", true);
        let old_bytes = awfb_bytes(&old);
        let new_bytes = awfb_bytes(&new);
        let patch_bytes = patch_bytes_with_compatibility(
            &old_bytes,
            &new_bytes,
            PatchCompatibility::CodeGenerational,
        );
        let new_root = awfb_root(&new_bytes);
        let mut endpoint =
            NativePatchEndpoint::from_awfb_bytes(old_bytes, BundleSessionOptions::default())
                .expect("endpoint starts");

        let outcome = endpoint
            .apply_patch_bytes(&patch_bytes)
            .expect("generational patch restarts");

        assert_eq!(
            outcome,
            NativePatchOutcome::Restarted {
                generation: GenerationId(0),
                compatibility: PatchCompatibility::CodeGenerational,
                content_root: new_root,
            }
        );
        assert_eq!(endpoint.active_content_root(), Some(new_root));
        let step = endpoint.session_mut().step_with_clock(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput::default(),
        );
        assert!(step.finished);
        assert!(step.status_label.contains("changed"));
    }

    #[test]
    fn native_patch_endpoint_applies_watch_transport_sidecar() {
        let old = fixture_bundle_with("Old text", false);
        let new = fixture_bundle_with("New text", false);
        let old_bytes = awfb_bytes(&old);
        let new_bytes = awfb_bytes(&new);
        let patch_bytes = patch_bytes(&old_bytes, &new_bytes);
        let operation_count = patch_operation_count(&patch_bytes);
        let old_root = awfb_root(&old_bytes);
        let new_root = awfb_root(&new_bytes);
        let temp_dir = temp_dir("watch-transport");
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let patch_path = temp_dir.join("update.awfb");
        fs::write(&patch_path, patch_bytes).expect("patch writes");
        let transport = serde_json::json!({
            "schema_version": 1,
            "runner": "native",
            "source": "src/main.arcw",
            "target_bundle": "target.awfb",
            "patch_bundle": "update.awfb",
            "base_content_root": old_root.to_string(),
            "target_content_root": new_root.to_string(),
            "compatibility": "content-only",
            "operation_count": operation_count,
            "action": "apply_patch"
        });
        let mut endpoint =
            NativePatchEndpoint::from_awfb_bytes(old_bytes, BundleSessionOptions::default())
                .expect("endpoint starts");

        let outcome = endpoint
            .apply_patch_transport_json_bytes(
                &serde_json::to_vec(&transport).expect("transport encodes"),
                &temp_dir,
            )
            .expect("transport applies");
        let _ = fs::remove_dir_all(&temp_dir);

        assert!(matches!(
            outcome,
            NativePatchOutcome::Applied {
                content_root,
                ..
            } if content_root == new_root
        ));
    }

    #[test]
    fn native_patch_endpoint_rejects_mismatched_transport_action() {
        let old = fixture_bundle_with("Old text", false);
        let new = fixture_bundle_with("New text", false);
        let old_bytes = awfb_bytes(&old);
        let new_bytes = awfb_bytes(&new);
        let patch_bytes = patch_bytes(&old_bytes, &new_bytes);
        let operation_count = patch_operation_count(&patch_bytes);
        let old_root = awfb_root(&old_bytes);
        let new_root = awfb_root(&new_bytes);
        let temp_dir = temp_dir("watch-transport-action");
        fs::create_dir_all(&temp_dir).expect("temp dir");
        fs::write(temp_dir.join("update.awfb"), patch_bytes).expect("patch writes");
        let transport = serde_json::json!({
            "schema_version": 1,
            "runner": "native",
            "source": "src/main.arcw",
            "target_bundle": "target.awfb",
            "patch_bundle": "update.awfb",
            "base_content_root": old_root.to_string(),
            "target_content_root": new_root.to_string(),
            "compatibility": "content-only",
            "operation_count": operation_count,
            "action": "restart_player"
        });
        let mut endpoint =
            NativePatchEndpoint::from_awfb_bytes(old_bytes, BundleSessionOptions::default())
                .expect("endpoint starts");

        let error = endpoint
            .apply_patch_transport_json_bytes(
                &serde_json::to_vec(&transport).expect("transport encodes"),
                &temp_dir,
            )
            .expect_err("wrong action rejects");
        let _ = fs::remove_dir_all(&temp_dir);

        assert!(matches!(
            error,
            NativePatchEndpointError::TransportActionMismatch {
                actual: NativePatchTransportAction::RestartPlayer,
                expected: NativePatchTransportAction::ApplyPatch,
                compatibility: PatchCompatibility::ContentOnly,
            }
        ));
    }

    #[test]
    fn native_patch_endpoint_accepts_cli_style_cwd_relative_patch_path() {
        let old = fixture_bundle_with("Old text", false);
        let new = fixture_bundle_with("New text", false);
        let old_bytes = awfb_bytes(&old);
        let new_bytes = awfb_bytes(&new);
        let patch_bytes = patch_bytes(&old_bytes, &new_bytes);
        let operation_count = patch_operation_count(&patch_bytes);
        let old_root = awfb_root(&old_bytes);
        let new_root = awfb_root(&new_bytes);
        let root = current_dir_temp_root("watch-transport-cwd");
        let patch_dir = root.join("patches");
        fs::create_dir_all(&patch_dir).expect("patch dir");
        let patch_path = patch_dir.join("update.awfb");
        fs::write(&patch_path, patch_bytes).expect("patch writes");
        let patch_value = patch_path
            .strip_prefix(std::env::current_dir().expect("cwd"))
            .expect("patch is under cwd")
            .to_string_lossy()
            .replace('\\', "/");
        let transport = serde_json::json!({
            "schema_version": 1,
            "runner": "native",
            "source": "src/main.arcw",
            "target_bundle": "target.awfb",
            "patch_bundle": patch_value,
            "base_content_root": old_root.to_string(),
            "target_content_root": new_root.to_string(),
            "compatibility": "content-only",
            "operation_count": operation_count,
            "action": "apply_patch"
        });
        let mut endpoint =
            NativePatchEndpoint::from_awfb_bytes(old_bytes, BundleSessionOptions::default())
                .expect("endpoint starts");

        let outcome = endpoint
            .apply_patch_transport_json_bytes(
                &serde_json::to_vec(&transport).expect("transport encodes"),
                &patch_dir,
            )
            .expect("cwd-relative transport applies");
        let _ = fs::remove_dir_all(&root);

        assert!(matches!(
            outcome,
            NativePatchOutcome::Applied {
                content_root,
                ..
            } if content_root == new_root
        ));
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

    fn patch_bytes_with_compatibility(
        old: &[u8],
        new: &[u8],
        compatibility: PatchCompatibility,
    ) -> Vec<u8> {
        let old_view = BundleView::parse(old, ReadBudget::default()).expect("old parses");
        let new_view = BundleView::parse(new, ReadBudget::default()).expect("new parses");
        let mut artifact =
            BundlePatchArtifact::from_views(&old_view, &new_view).expect("patch builds");
        for fingerprint in &mut artifact.manifest.compatibility_fingerprints {
            fingerprint.compatibility = compatibility;
        }
        artifact.manifest.compatibility = compatibility;
        encode_patch_bundle(&artifact).expect("patch encodes")
    }

    fn patch_operation_count(patch_bytes: &[u8]) -> usize {
        decode_patch_bundle(patch_bytes)
            .expect("patch decodes")
            .plan
            .operations
            .len()
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "arcweft-native-patch-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn current_dir_temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after epoch")
            .as_nanos();
        std::env::current_dir()
            .expect("cwd")
            .join("target")
            .join("codex")
            .join(format!(
                "arcweft-native-patch-{label}-{}-{nanos}",
                std::process::id()
            ))
    }

    fn fixture_bundle_with(display_text: &str, changed_main_code: bool) -> ArcweftBundle {
        let line = RuntimeLineId("line.opening".to_owned());
        let main_ops = if changed_main_code {
            vec![FlowOp::Return("changed".to_owned())]
        } else {
            vec![
                FlowOp::Dialogue {
                    line: line.clone(),
                    task_group: 0,
                },
                FlowOp::Return("done".to_owned()),
            ]
        };
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: main_ops,
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
        let product_awbc = AwbcLowerer::new(&plan, &display, "native-patch.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bytecode = BytecodeProgram::from_runtime_plan(plan);
        let stats = bytecode.stats();
        ArcweftBundle::new(
            BundleManifest {
                source_label: "native-patch.arcw".to_owned(),
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
                label: "native-patch.arcw".to_owned(),
                text: "flow @flow.main main { ... }".to_owned(),
            },
            bytecode,
            display,
        )
        .with_product_awbc(product_awbc)
    }
}
