use arcweft_bundle::{ArcweftBundle, BundleArtifactIdentity, BundleVirtualFileSpace};
use arcweft_bundle_assets::BundleAssetResolver;
use arcweft_core::entry::RuntimeSchemaLimits;
use arcweft_core::task::{
    BoundTaskSpec, CancelScopeId, HostTaskRequest, RuntimeProgramOwner, SystemInfoKind, TaskEvent,
    TaskEventKind,
};
use arcweft_core::value::{
    RuntimeBundleAssetContext, RuntimePayload, RuntimeValue, runtime_sequence_dense_bytes,
};
use arcweft_runtime_driver::task::HostTaskDispatch;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

/// Browser host task broker for the synchronous embedded-VFS MVP slice.
///
/// Fetch, `IndexedDB`, `WebAudio`, and nested Wasm are explicit post-MVP adapters.
/// Unsupported calls produce deterministic task errors instead of being ignored.
#[derive(Clone, Debug)]
pub struct BrowserTaskBroker {
    allowed_calls: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
    asset_resolver: BundleAssetResolver,
    cancelled_scopes: BTreeSet<CancelScopeId>,
    queued_task_events: Vec<TaskEvent>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BrowserHostTaskError {
    #[error("host call `{0}` is not declared by the active bundle manifests")]
    UndeclaredHostCall(String),
    #[error("browser MVP does not implement host call `{0}`")]
    UnsupportedHostCall(String),
    #[error("virtual path must be relative, normalized, and use asset/save/temp/export")]
    InvalidVirtualPath,
    #[error("virtual file `{0}` was not found")]
    MissingVirtualFile(String),
    #[error("virtual file `{0}` is not valid UTF-8")]
    InvalidUtf8(String),
    #[error("asset virtual files are read-only")]
    ReadOnlyAsset,
    #[error("failed to initialize the bundle asset resolver: {0}")]
    AssetResolver(String),
}

impl BrowserTaskBroker {
    pub fn from_bundle(
        bundle: &ArcweftBundle,
        context: RuntimeBundleAssetContext,
        identity: BundleArtifactIdentity,
    ) -> Result<Self, BrowserHostTaskError> {
        let allowed_calls = bundle
            .manifest
            .required_host_calls
            .iter()
            .cloned()
            .chain(
                bundle
                    .adapter_manifests
                    .iter()
                    .flat_map(|manifest| manifest.host_calls.iter().map(|call| call.id.clone())),
            )
            .collect();
        let files = bundle
            .virtual_files
            .iter()
            .try_fold(BTreeMap::new(), |mut files, file| {
                let key = virtual_file_key(file.space, &file.path);
                validate_virtual_path(&key)?;
                files.insert(key, file.bytes.clone());
                Ok::<_, BrowserHostTaskError>(files)
            })?;
        let asset_resolver =
            BundleAssetResolver::try_new(context, identity, Arc::new(bundle.clone()))
                .map_err(|error| BrowserHostTaskError::AssetResolver(error.to_string()))?;
        Ok(Self {
            allowed_calls,
            files,
            asset_resolver,
            cancelled_scopes: BTreeSet::new(),
            queued_task_events: Vec::new(),
        })
    }

    /// Resolves currently supported work and queues task events for the next VM
    /// step. The return value is a queue count, not a claim that the VM has
    /// already consumed those completions.
    pub fn queue_dispatches(
        &mut self,
        dispatches: Vec<HostTaskDispatch>,
        program_owner: &RuntimeProgramOwner,
    ) -> usize {
        let events = dispatches
            .into_iter()
            .map(|dispatch| {
                if self.cancelled_scopes.contains(&dispatch.task.cancel_scope) {
                    return dispatch.cancelled();
                }
                let kind = self
                    .resolve(&dispatch, program_owner)
                    .unwrap_or_else(|error| TaskEventKind::Failed(error.to_string()));
                dispatch.into_event(kind)
            })
            .collect::<Vec<_>>();
        self.queued_task_events.extend(events);
        self.queued_task_events.sort_by(|left, right| {
            (left.logical_epoch, left.sequence, &left.task_id).cmp(&(
                right.logical_epoch,
                right.sequence,
                &right.task_id,
            ))
        });
        self.queued_task_events.len()
    }

    pub fn drain_queued_task_events(&mut self) -> Vec<TaskEvent> {
        std::mem::take(&mut self.queued_task_events)
    }

    pub fn queued_task_event_count(&self) -> usize {
        self.queued_task_events.len()
    }

    pub fn cancel_scopes(&mut self, scopes: impl IntoIterator<Item = CancelScopeId>) {
        self.cancelled_scopes.extend(scopes);
    }

    fn resolve(
        &mut self,
        dispatch: &HostTaskDispatch,
        program_owner: &RuntimeProgramOwner,
    ) -> Result<TaskEventKind, BrowserHostTaskError> {
        let request = &dispatch.task.request;
        let call = request.host_call_id();
        if !self.allowed_calls.contains(&call) && !is_internal_scheduler_marker(request) {
            return Err(BrowserHostTaskError::UndeclaredHostCall(call));
        }
        match request {
            HostTaskRequest::FileReadText(request) => self.read_text(&request.path),
            HostTaskRequest::FileReadBytes(request) => self.read_bytes(&request.path),
            HostTaskRequest::FileWriteText(request) => {
                self.write_bytes(&request.path, request.text.as_bytes())
            }
            HostTaskRequest::FileWriteBytes(request) => {
                self.write_bytes(&request.path, &request.bytes)
            }
            HostTaskRequest::AssetLoad(request) => {
                let context = dispatch.bundle_asset_context().ok_or_else(|| {
                    BrowserHostTaskError::AssetResolver(
                        "asset task has no exact generation context".to_owned(),
                    )
                })?;
                if context.generation() != dispatch.generation {
                    return Err(BrowserHostTaskError::AssetResolver(
                        "asset task context does not match its dispatch generation".to_owned(),
                    ));
                }
                let value = match request.kind.as_str() {
                    "image" => self.asset_resolver.load_image(context, &request.id),
                    "voice" => self.asset_resolver.load_voice(context, &request.id),
                    kind => {
                        return Err(BrowserHostTaskError::UnsupportedHostCall(format!(
                            "asset.{kind}"
                        )));
                    }
                }
                .map_err(|error| BrowserHostTaskError::AssetResolver(error.to_string()))?;
                let bound = BoundTaskSpec::bind(
                    dispatch.task.clone(),
                    Some(program_owner.clone()),
                    RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|error| BrowserHostTaskError::AssetResolver(error.to_string()))?;
                let payload = bound
                    .outcome()
                    .try_payload(value)
                    .map_err(|error| BrowserHostTaskError::AssetResolver(error.to_string()))?;
                Ok(TaskEventKind::Ready(payload))
            }
            HostTaskRequest::SystemInfo(request) => {
                let value = match request.kind {
                    SystemInfoKind::CoreCount
                    | SystemInfoKind::ThreadCount
                    | SystemInfoKind::AvailableParallelism => 1_u64,
                };
                Ok(TaskEventKind::Ready(RuntimePayload::new(
                    RuntimeValue::usize(value),
                )))
            }
            request if is_internal_scheduler_marker(request) => Ok(TaskEventKind::Ready(
                RuntimePayload::new(RuntimeValue::Unit),
            )),
            request => Err(BrowserHostTaskError::UnsupportedHostCall(
                request.host_call_id(),
            )),
        }
    }

    fn read_text(&self, path: &str) -> Result<TaskEventKind, BrowserHostTaskError> {
        validate_virtual_path(path)?;
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| BrowserHostTaskError::MissingVirtualFile(path.to_owned()))?;
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| BrowserHostTaskError::InvalidUtf8(path.to_owned()))?;
        Ok(TaskEventKind::Ready(RuntimePayload::from(text)))
    }

    fn read_bytes(&self, path: &str) -> Result<TaskEventKind, BrowserHostTaskError> {
        validate_virtual_path(path)?;
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| BrowserHostTaskError::MissingVirtualFile(path.to_owned()))?;
        Ok(TaskEventKind::Ready(RuntimePayload::new(
            runtime_sequence_dense_bytes(bytes.clone()),
        )))
    }

    fn write_bytes(
        &mut self,
        path: &str,
        bytes: &[u8],
    ) -> Result<TaskEventKind, BrowserHostTaskError> {
        match validate_virtual_path(path)? {
            BundleVirtualFileSpace::Asset => Err(BrowserHostTaskError::ReadOnlyAsset),
            BundleVirtualFileSpace::Save
            | BundleVirtualFileSpace::Temp
            | BundleVirtualFileSpace::Export => {
                self.files.insert(path.to_owned(), bytes.to_vec());
                Ok(TaskEventKind::Ready(RuntimePayload::new(
                    RuntimeValue::Unit,
                )))
            }
        }
    }
}

fn is_internal_scheduler_marker(request: &HostTaskRequest) -> bool {
    matches!(
        request,
        HostTaskRequest::Custom { capability, operation, .. }
            if matches!(capability.0.as_str(), "line_task" | "flow_thread")
                && operation == "run_child"
    )
}

fn virtual_file_key(space: BundleVirtualFileSpace, path: &str) -> String {
    format!("{}:{path}", space.as_str())
}

fn validate_virtual_path(path: &str) -> Result<BundleVirtualFileSpace, BrowserHostTaskError> {
    let (space, relative) = path
        .split_once(':')
        .ok_or(BrowserHostTaskError::InvalidVirtualPath)?;
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains('\\')
        || relative.contains('\0')
        || relative
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(BrowserHostTaskError::InvalidVirtualPath);
    }
    match space {
        "asset" => Ok(BundleVirtualFileSpace::Asset),
        "save" => Ok(BundleVirtualFileSpace::Save),
        "temp" => Ok(BundleVirtualFileSpace::Temp),
        "export" => Ok(BundleVirtualFileSpace::Export),
        _ => Err(BrowserHostTaskError::InvalidVirtualPath),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_audio_core::graph::{
        AudioAsset, AudioBusDef, AudioDecodeStrategy, AudioFormat, AudioGraph,
    };
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{
        ArcweftBundle, BundleImageAnimation, BundleImageAsset, BundleImageFormat, BundleManifest,
        BundleRuntimeSummary, BundleVirtualFile, BundleVirtualFileRef,
    };
    use arcweft_core::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFlowBinding, AwbcFlowExecutable, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction,
        AwbcFunctionFlag, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcSafePointKind,
        AwbcSignature, AwbcSignatureId, AwbcStringId, AwbcTableRange, AwbcTerminator,
    };
    use arcweft_core::effect::RuntimeArtifactFingerprint;
    use arcweft_core::entry::{EntryBindingIdentity, FlowContractHash, RuntimeFlowExecutable};
    use arcweft_core::pattern::{
        RuntimeBuiltinVariantCaseIdentity, RuntimeSemanticTypeId, runtime_standard_opaque_type,
    };
    use arcweft_core::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
    use arcweft_core::task::{
        AssetRequest, GenerationId, TaskClass, TaskId, TaskKey, TaskOutcomeContract, TaskPolicy,
        TaskPriority, TaskSpec,
    };
    use arcweft_core::value::{
        RuntimeAssetErrorValue, RuntimeAudioHandleValue, RuntimeBundleAssetArtifactDigest,
        RuntimeImageHandleValue, RuntimeVoiceErrorValue,
    };
    use arcweft_interaction_model::audio::{
        AudioBusId, AudioLoopMode, AudioResourceId, GainDbMilli,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use arcweft_text_model::DialogueContentCatalog;

    const IMAGE_BYTES: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn asset_loads_publish_typed_image_and_voice_results() {
        let bundle = asset_test_bundle();
        let identity = BundleArtifactIdentity::LogicalBundle {
            identity: bundle.logical_identity().expect("logical bundle identity"),
        };
        let context = test_asset_context(identity, 0);
        let mut broker = BrowserTaskBroker::from_bundle(&bundle, context, identity)
            .expect("browser broker binds the complete bundle catalog");
        let (program, image_result, voice_result) = asset_result_program();
        let owner = RuntimeProgramOwner::Plan(Arc::clone(&program));

        broker.queue_dispatches(
            vec![
                asset_dispatch(context, 0, "image", "asset.image.good", image_result),
                asset_dispatch(context, 1, "voice", "asset.voice.good", voice_result),
            ],
            &owner,
        );
        let events = broker.drain_queued_task_events();
        assert_eq!(events.len(), 2);

        let image = ready_result_payload(
            events[0].kind.clone(),
            RuntimeBuiltinVariantCaseIdentity::ResultOk,
        )
        .expect("image load succeeds");
        let image = RuntimeImageHandleValue::try_from_runtime_value(&image)
            .expect("image success carries the standard ImageHandle");
        assert_eq!(
            broker
                .asset_resolver
                .resolve_image_handle(&image)
                .expect("image bytes remain bound to this generation")
                .as_ref(),
            IMAGE_BYTES
        );

        let voice = ready_result_payload(
            events[1].kind.clone(),
            RuntimeBuiltinVariantCaseIdentity::ResultOk,
        )
        .expect("voice load succeeds");
        let voice = RuntimeAudioHandleValue::try_from_runtime_value(&voice)
            .expect("voice success carries the standard AudioHandle");
        assert_eq!(
            broker
                .asset_resolver
                .resolve_voice_handle(&voice)
                .expect("voice bytes remain bound to this generation")
                .as_ref(),
            pcm_wav().as_slice()
        );
    }

    #[test]
    fn missing_and_malformed_assets_publish_typed_domain_errors() {
        let bundle = asset_test_bundle();
        let identity = BundleArtifactIdentity::LogicalBundle {
            identity: bundle.logical_identity().expect("logical bundle identity"),
        };
        let context = test_asset_context(identity, 0);
        let mut broker = BrowserTaskBroker::from_bundle(&bundle, context, identity)
            .expect("browser broker binds the complete bundle catalog");
        let (program, image_result, voice_result) = asset_result_program();
        let owner = RuntimeProgramOwner::Plan(program);

        broker.queue_dispatches(
            vec![
                asset_dispatch(context, 0, "image", "asset.image.missing", image_result),
                asset_dispatch(context, 1, "voice", "asset.voice.missing", voice_result),
                asset_dispatch(context, 2, "image", "asset.image.malformed", image_result),
                asset_dispatch(context, 3, "voice", "asset.voice.malformed", voice_result),
            ],
            &owner,
        );
        let events = broker.drain_queued_task_events();
        assert_eq!(events.len(), 4);

        let image_missing = ready_result_payload(
            events[0].kind.clone(),
            RuntimeBuiltinVariantCaseIdentity::ResultErr,
        )
        .expect("missing image is a completed domain result");
        assert_eq!(
            RuntimeAssetErrorValue::try_from_runtime_value(&image_missing)
                .expect("image failure uses AssetError")
                .failure()
                .reason(),
            arcweft_core::value::RuntimeBundleAssetFailureReason::Missing
        );

        let voice_missing = ready_result_payload(
            events[1].kind.clone(),
            RuntimeBuiltinVariantCaseIdentity::ResultErr,
        )
        .expect("missing voice is a completed domain result");
        assert_eq!(
            RuntimeVoiceErrorValue::try_from_runtime_value(&voice_missing)
                .expect("voice failure uses VoiceError")
                .failure()
                .reason(),
            arcweft_core::value::RuntimeBundleAssetFailureReason::Missing
        );

        assert_eq!(
            RuntimeAssetErrorValue::try_from_runtime_value(
                &ready_result_payload(
                    events[2].kind.clone(),
                    RuntimeBuiltinVariantCaseIdentity::ResultErr,
                )
                .expect("malformed image has Result::Err")
            )
            .expect("malformed image uses AssetError")
            .failure()
            .reason(),
            arcweft_core::value::RuntimeBundleAssetFailureReason::Decode
        );
        assert_eq!(
            RuntimeVoiceErrorValue::try_from_runtime_value(
                &ready_result_payload(
                    events[3].kind.clone(),
                    RuntimeBuiltinVariantCaseIdentity::ResultErr,
                )
                .expect("malformed voice has Result::Err")
            )
            .expect("malformed voice uses VoiceError")
            .failure()
            .reason(),
            arcweft_core::value::RuntimeBundleAssetFailureReason::Decode
        );
    }

    #[test]
    fn asset_dispatch_context_mismatch_is_a_failed_task() {
        let bundle = asset_test_bundle();
        let identity = BundleArtifactIdentity::LogicalBundle {
            identity: bundle.logical_identity().expect("logical bundle identity"),
        };
        let context = test_asset_context(identity, 0);
        let wrong_context = test_asset_context(identity, 1);
        let mut broker = BrowserTaskBroker::from_bundle(&bundle, context, identity)
            .expect("browser broker binds the complete bundle catalog");
        let (program, image_result, _) = asset_result_program();
        let owner = RuntimeProgramOwner::Plan(program);

        broker.queue_dispatches(
            vec![asset_dispatch(
                wrong_context,
                0,
                "image",
                "asset.image.good",
                image_result,
            )],
            &owner,
        );

        assert!(matches!(
            broker.drain_queued_task_events()[0].kind,
            TaskEventKind::Failed(_)
        ));
    }

    fn asset_test_bundle() -> ArcweftBundle {
        let mut bundle = empty_asset_test_bundle();
        bundle
            .manifest
            .required_host_calls
            .extend(["asset.image".to_owned(), "asset.voice".to_owned()]);
        bundle.image_assets.extend([
            BundleImageAsset {
                id: "asset.image.good".to_owned(),
                file: BundleVirtualFileRef {
                    space: BundleVirtualFileSpace::Asset,
                    path: "images/good.png".to_owned(),
                },
                format: BundleImageFormat::Png,
                animation: BundleImageAnimation::Static,
                dimensions: None,
            },
            BundleImageAsset {
                id: "asset.image.malformed".to_owned(),
                file: BundleVirtualFileRef {
                    space: BundleVirtualFileSpace::Asset,
                    path: "images/malformed.png".to_owned(),
                },
                format: BundleImageFormat::Png,
                animation: BundleImageAnimation::Static,
                dimensions: None,
            },
        ]);
        bundle.virtual_files.extend([
            BundleVirtualFile {
                space: BundleVirtualFileSpace::Asset,
                path: "images/good.png".to_owned(),
                bytes: IMAGE_BYTES.to_vec(),
            },
            BundleVirtualFile {
                space: BundleVirtualFileSpace::Asset,
                path: "images/malformed.png".to_owned(),
                bytes: b"not a png".to_vec(),
            },
            BundleVirtualFile {
                space: BundleVirtualFileSpace::Asset,
                path: "audio/good.wav".to_owned(),
                bytes: pcm_wav(),
            },
            BundleVirtualFile {
                space: BundleVirtualFileSpace::Asset,
                path: "audio/malformed.wav".to_owned(),
                bytes: b"not a wav".to_vec(),
            },
        ]);
        let master_bus = AudioBusId::new("bus.master").expect("bus identity");
        bundle.audio = Some(AudioGraph {
            master_bus: master_bus.clone(),
            assets: vec![
                AudioAsset {
                    id: AudioResourceId::new("asset.voice.good").expect("voice identity"),
                    path: "audio/good.wav".to_owned(),
                    format: AudioFormat::Wav,
                    strategy: AudioDecodeStrategy::Preload,
                    default_loop: AudioLoopMode::None,
                },
                AudioAsset {
                    id: AudioResourceId::new("asset.voice.malformed").expect("voice identity"),
                    path: "audio/malformed.wav".to_owned(),
                    format: AudioFormat::Wav,
                    strategy: AudioDecodeStrategy::Preload,
                    default_loop: AudioLoopMode::None,
                },
            ],
            buses: vec![AudioBusDef {
                id: master_bus,
                parent: None,
                gain: GainDbMilli::UNITY,
                muted: false,
                effects: Vec::new(),
            }],
            snapshots: Vec::new(),
        });
        bundle
    }

    fn empty_asset_test_bundle() -> ArcweftBundle {
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                locale: arcweft_manifest_model::ProjectLocaleSpec::default(),
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
                        .expect("test artifact fingerprint is non-zero"),
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                },
            },
            asset_test_source_map(),
            minimal_asset_test_program(),
            DialogueContentCatalog::new(),
        )
        .expect("minimal test bundle joins its required standard documents")
    }

    fn asset_test_source_map() -> SourceMapSection {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("browser_asset_load.arcw").expect("source id"),
            SourceName::path("browser_asset_load.arcw"),
            "flow main { return \"ok\" }",
        )
        .expect("test source document");
        SourceMapSection::try_from_documents(&[&document]).expect("test source map")
    }

    fn minimal_asset_test_program() -> arcweft_core::awbc::schema::AwbcProgram {
        let flow = arcweft_core::plan::FlowRuntimeId::from_checked_declaration_digest(
            [0xa4; 32],
            "flow.main",
        )
        .expect("test flow identity");
        arcweft_core::awbc::schema::AwbcProgram {
            strings: vec!["entry.main".to_owned()],
            signatures: vec![AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            }],
            frame_layouts: vec![AwbcFrameLayout {
                scopes: Vec::new(),
                slots: Vec::new(),
                max_scope_depth: 0,
            }],
            functions: vec![AwbcFunction {
                public_id: Some(AwbcStringId(0)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                type_context: None,
                input_ownership: Vec::new(),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
            }],
            flow_bindings: vec![AwbcFlowBinding {
                flow: flow.clone(),
                function: AwbcFunctionId(0),
            }],
            flow_executables: vec![AwbcFlowExecutable {
                metadata: RuntimeFlowExecutable {
                    flow,
                    contract: FlowContractHash::from_bytes([0xb4; 32]),
                    controller: None,
                },
                function: AwbcFunctionId(0),
            }],
            blocks: vec![AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            }],
            entries: vec![AwbcEntry {
                runtime_id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body(
                    "entry.main",
                )
                .expect("test entry identity"),
                binding: EntryBindingIdentity::from_bytes([1; 32]),
                public_id: AwbcStringId(0),
                kind: AwbcEntryKind::Cli,
                target: AwbcEntryTarget::Function {
                    function: AwbcFunctionId(0),
                },
                roles: arcweft_core::entry::RuntimeEntryRoles::None,
            }],
            ..arcweft_core::awbc::schema::AwbcProgram::default()
        }
    }

    fn test_asset_context(
        identity: BundleArtifactIdentity,
        generation: u64,
    ) -> RuntimeBundleAssetContext {
        RuntimeBundleAssetContext::new(
            GenerationId::new(generation),
            RuntimeBundleAssetArtifactDigest::try_from_bytes(
                identity.binding_digest().as_bytes().to_owned(),
            )
            .expect("artifact digest"),
        )
    }

    fn asset_result_program() -> (
        Arc<arcweft_core::plan::RuntimePlan>,
        RuntimeSemanticTypeId,
        RuntimeSemanticTypeId,
    ) {
        let (image_handle, image_seed) = standard_opaque_seed("ImageHandle");
        let (asset_error, asset_error_seed) = standard_opaque_seed("AssetError");
        let (audio_handle, audio_seed) = standard_opaque_seed("AudioHandle");
        let (voice_error, voice_error_seed) = standard_opaque_seed("VoiceError");
        let image_ok_payload = semantic_type(0x91);
        let image_error_payload = semantic_type(0x92);
        let voice_ok_payload = semantic_type(0x93);
        let voice_error_payload = semantic_type(0x94);
        let image_result = semantic_type(0x95);
        let voice_result = semantic_type(0x96);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    image_seed,
                    asset_error_seed,
                    audio_seed,
                    voice_error_seed,
                    RuntimePlanTypeSeed::new(
                        image_ok_payload,
                        RuntimePlanTypeProjection::Tuple(Box::new([image_handle])),
                    ),
                    RuntimePlanTypeSeed::new(
                        image_error_payload,
                        RuntimePlanTypeProjection::Tuple(Box::new([asset_error])),
                    ),
                    RuntimePlanTypeSeed::new(
                        voice_ok_payload,
                        RuntimePlanTypeProjection::Tuple(Box::new([audio_handle])),
                    ),
                    RuntimePlanTypeSeed::new(
                        voice_error_payload,
                        RuntimePlanTypeProjection::Tuple(Box::new([voice_error])),
                    ),
                    RuntimePlanTypeSeed::new(
                        image_result,
                        RuntimePlanTypeProjection::Result {
                            value: image_handle,
                            error: asset_error,
                            value_payload: image_ok_payload,
                            error_payload: image_error_payload,
                        },
                    ),
                    RuntimePlanTypeSeed::new(
                        voice_result,
                        RuntimePlanTypeProjection::Result {
                            value: audio_handle,
                            error: voice_error,
                            value_payload: voice_ok_payload,
                            error_payload: voice_error_payload,
                        },
                    ),
                ],
                [],
            )
            .expect("standard asset result type rows admit");
        (
            Arc::new(builder.finish().expect("asset type plan seals")),
            image_result,
            voice_result,
        )
    }

    fn standard_opaque_seed(name: &str) -> (RuntimeSemanticTypeId, RuntimePlanTypeSeed) {
        let owner = runtime_standard_opaque_type(&[name])
            .and_then(|spec| spec.monomorphic_owner())
            .expect("asset carrier is a standard monomorphic opaque type");
        let semantic = owner.semantic_identity();
        (
            semantic,
            RuntimePlanTypeSeed::new(
                semantic,
                RuntimePlanTypeProjection::Opaque {
                    producer: owner.producer().clone(),
                    admission: owner.admission(),
                    value_class: owner.value_class(),
                    persistence: owner.persistence(),
                    arguments: Box::new([]),
                },
            ),
        )
    }

    fn semantic_type(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn asset_dispatch(
        context: RuntimeBundleAssetContext,
        sequence: u64,
        kind: &str,
        id: &str,
        result_type: RuntimeSemanticTypeId,
    ) -> HostTaskDispatch {
        let task = TaskSpec::new(
            TaskId(format!("task.{sequence}")),
            TaskKey(format!("asset.{kind}.{id}")),
            TaskClass::AssetDecode,
            TaskPriority(0),
            CancelScopeId("asset-tests".to_owned()),
            TaskPolicy::JoinSameKey,
            HostTaskRequest::AssetLoad(AssetRequest {
                id: id.to_owned(),
                kind: kind.to_owned(),
            }),
        )
        .with_outcome(TaskOutcomeContract::program(result_type));
        let value = serde_json::json!({
            "generation": context.generation(),
            "logical_epoch": 0,
            "sequence": sequence,
            "task": serde_json::to_value(task).expect("task encodes"),
            "last_publication_revision": null,
            "bundle_asset_context": serde_json::to_value(context).expect("context encodes"),
        });
        serde_json::from_value(value).expect("host task dispatch decodes")
    }

    fn ready_result_payload(
        kind: TaskEventKind,
        expected_case: RuntimeBuiltinVariantCaseIdentity,
    ) -> Result<RuntimeValue, String> {
        let TaskEventKind::Ready(payload) = kind else {
            return Err(format!("asset task outcome was not Ready: {kind:?}"));
        };
        let (actual_case, payload) = payload
            .into_value()
            .try_into_builtin_variant_case()
            .map_err(|_| "asset result is not a canonical builtin Result".to_owned())?;
        if actual_case != expected_case {
            return Err(format!(
                "asset result case was {actual_case:?}; expected {expected_case:?}"
            ));
        }
        payload.ok_or_else(|| "Result case has no payload".to_owned())
    }

    fn pcm_wav() -> Vec<u8> {
        let samples = [0_i16; 4];
        let data_length = u32::try_from(samples.len() * 2).expect("test samples fit");
        let mut bytes = Vec::with_capacity(44 + data_length as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        bytes.extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));
        bytes
    }

    #[test]
    fn virtual_paths_reject_traversal_and_host_paths() {
        for path in [
            "save:../slot.json",
            "save:/slot.json",
            "save:slot//data.json",
            "save:slot\\data.json",
            "native:slot.json",
        ] {
            assert!(
                validate_virtual_path(path).is_err(),
                "path should fail: {path}"
            );
        }
    }
}
