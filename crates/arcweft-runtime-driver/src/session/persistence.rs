//! Foreground runtime activation and atomic session snapshot persistence.

use super::{
    Arc, ArcweftRuntimeExecutorSnapshot, BUNDLE_SESSION_SAVE_SCHEMA_ID,
    BUNDLE_SESSION_SAVE_SCHEMA_VERSION, BundleEntryStart, BundleEntryStartError,
    BundlePresentationSnapshot, BundleSession, BundleSessionExecutorSnapshot,
    BundleSessionGenerationSnapshot, BundleSessionPendingBlocker, BundleSessionRuntimeSnapshot,
    BundleSessionSaveError, BundleSessionSnapshot, BundleViewRuntime, RuntimeExecutor,
    RuntimeTaskListOptions, RuntimeTaskRegistry, SessionRuntime, StartedForegroundEntry,
    ViewVirtualizationRuntime, digest_label, reconciled_root_handles_for_restore,
    validate_presentation_runtime_status, validate_presentation_snapshot,
    validate_product_awbc_snapshot, validate_virtual_list_scroll_owner,
};

impl BundleSession {
    /// Starts a fresh foreground entry on the currently committed active generation.
    pub fn start_foreground_entry_on_current_generation(
        &mut self,
        start: BundleEntryStart,
    ) -> Result<StartedForegroundEntry, BundleEntryStartError> {
        let generation = self.swap.active_generation_id();
        let runtime = self
            .runtime_images
            .get(generation)?
            .runtime()
            .start_entry(start)?;
        let entry = runtime.entry;
        self.activate_runtime(runtime);
        self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        self.pending_input_events.clear();
        self.pending_presentation_inputs.clear();
        self.presentation = BundlePresentationSnapshot::default();
        self.view_virtualization = ViewVirtualizationRuntime::default();
        self.retire_unused_generations();
        Ok(StartedForegroundEntry { generation, entry })
    }

    pub fn snapshot_session(&self) -> Result<BundleSessionSnapshot, BundleSessionSaveError> {
        let blockers = self.session_save_blockers();
        if !blockers.is_empty() {
            return Err(BundleSessionSaveError::NonQuiescent { blockers });
        }
        validate_presentation_snapshot(&self.presentation, &self.fx_definitions)?;
        validate_dialogue_view_save_point(&self.view_runtime, &self.presentation)?;
        validate_presentation_runtime_status(&self.presentation, &self.executor.fiber().status)?;
        let active = self.active_generation();
        let product_program = self.executor.product_awbc_program().ok_or_else(|| {
            BundleSessionSaveError::UnsupportedExecutorTier {
                tier: self.executor.tier().as_str().to_owned(),
            }
        })?;
        let executor = match self.executor.snapshot()? {
            ArcweftRuntimeExecutorSnapshot::AwbcProduct(state) => {
                validate_product_awbc_snapshot(&state, product_program)?;
                BundleSessionExecutorSnapshot {
                    generation: active.id,
                    state,
                }
            }
        };
        Ok(BundleSessionSnapshot {
            generation: BundleSessionGenerationSnapshot {
                active_generation: active.id,
                artifact: self.active_artifact_identity,
                bytecode_abi: active.bytecode_abi,
                adapter_requirements: active.adapter_requirements,
            },
            runtime: BundleSessionRuntimeSnapshot {
                source_label: self.source_label.clone(),
                next_step_index: u64::try_from(self.next_step_index).unwrap_or(u64::MAX),
                next_task_sequence: self.next_task_sequence,
                next_generation_id: self.next_generation_id,
                runtime_generation_pin: self.runtime_generation_pin.as_ref().map(|pin| pin.id),
            },
            executor,
            presentation: self.presentation.clone(),
            view_virtualization: self.view_virtualization.snapshot(),
            view_runtime: self.view_runtime.snapshot(),
        })
    }

    pub fn export_session_save_bytes(&self) -> Result<Vec<u8>, BundleSessionSaveError> {
        let snapshot = self.snapshot_session()?;
        arcweft_save::encode_typed_json_save(
            &snapshot,
            arcweft_save::SaveSchemaId::new(BUNDLE_SESSION_SAVE_SCHEMA_ID),
            BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
        )
        .map_err(|error| BundleSessionSaveError::Encode {
            message: error.to_string(),
        })
    }

    pub fn import_session_save_bytes(
        &mut self,
        bytes: &[u8],
        options: &arcweft_save::SaveDecodeOptions,
    ) -> Result<(), BundleSessionSaveError> {
        let snapshot = arcweft_save::decode_typed_json_save::<BundleSessionSnapshot>(
            bytes,
            &arcweft_save::SaveSchemaId::new(BUNDLE_SESSION_SAVE_SCHEMA_ID),
            BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
            options,
        )
        .map_err(|error| BundleSessionSaveError::Decode {
            message: error.to_string(),
        })?;
        self.restore_session_snapshot(snapshot)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "session restore validates every candidate subsystem before committing the facade state atomically"
    )]
    pub fn restore_session_snapshot(
        &mut self,
        snapshot: BundleSessionSnapshot,
    ) -> Result<(), BundleSessionSaveError> {
        self.validate_session_save_generation(&snapshot.generation)?;
        validate_presentation_snapshot(&snapshot.presentation, &self.fx_definitions)?;
        let active_generation = self.active_generation().id;
        let BundleSessionExecutorSnapshot { generation, state } = snapshot.executor;
        if generation != active_generation {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "executor_generation",
                saved: format!("{generation:?}"),
                actual: format!("{active_generation:?}"),
            });
        }
        let BundleSessionRuntimeSnapshot {
            source_label,
            next_step_index,
            next_task_sequence,
            next_generation_id,
            runtime_generation_pin,
        } = snapshot.runtime;
        let next_step_index = usize::try_from(next_step_index).map_err(|_| {
            BundleSessionSaveError::CounterOutOfRange {
                field: "next_step_index",
                value: next_step_index,
            }
        })?;
        let restore_runtime_generation_pin = match runtime_generation_pin {
            Some(id) if id == active_generation => true,
            Some(id) => {
                return Err(BundleSessionSaveError::GenerationMismatch {
                    field: "runtime_generation_pin",
                    saved: format!("{id:?}"),
                    actual: format!("{active_generation:?}"),
                });
            }
            None => false,
        };
        let product_program = self.executor.product_awbc_program().ok_or_else(|| {
            BundleSessionSaveError::UnsupportedExecutorTier {
                tier: self.executor.tier().as_str().to_owned(),
            }
        })?;
        validate_product_awbc_snapshot(&state, product_program)?;
        let executor_snapshot = ArcweftRuntimeExecutorSnapshot::AwbcProduct(state);
        let mut restored_executor = self.executor.clone();
        restored_executor.restore_snapshot(executor_snapshot)?;
        let restored_view_virtualization = ViewVirtualizationRuntime::from_snapshot(
            &snapshot.view_virtualization,
        )
        .map_err(|error| BundleSessionSaveError::ViewVirtualization {
            message: error.to_string(),
        })?;
        let mut restored_view_runtime = self.view_runtime.clone();
        let dialogue = snapshot.presentation.dialogue.view_inputs();
        let reconciled_root_handles = reconciled_root_handles_for_restore(
            &snapshot.presentation.presentation_handles,
            &dialogue,
        )
        .map_err(|error| BundleSessionSaveError::ViewRuntime {
            message: error.to_string(),
        })?;
        restored_view_runtime
            .restore(&snapshot.view_runtime, &reconciled_root_handles)
            .map_err(|error| BundleSessionSaveError::ViewRuntime {
                message: error.to_string(),
            })?;
        validate_dialogue_view_save_point(&restored_view_runtime, &snapshot.presentation)?;
        restored_view_runtime
            .validate_frame(&snapshot.presentation.view)
            .map_err(|error| BundleSessionSaveError::ViewRuntime {
                message: error.to_string(),
            })?;
        for list in restored_view_virtualization.mounts() {
            validate_virtual_list_scroll_owner(
                &self.scroll_regions,
                list.scroll_target(),
                list.axis(),
            )
            .map_err(|error| BundleSessionSaveError::ViewVirtualization {
                message: error.to_string(),
            })?;
        }
        validate_presentation_runtime_status(
            &snapshot.presentation,
            &restored_executor.fiber().status,
        )?;
        self.source_label = source_label;
        self.next_step_index = next_step_index;
        self.next_task_sequence = next_task_sequence;
        self.next_generation_id = next_generation_id;
        self.executor = restored_executor;
        self.runtime_generation_pin =
            restore_runtime_generation_pin.then(|| self.swap.pin_active_generation());
        self.pending_input_events.clear();
        self.pending_presentation_inputs.clear();
        self.pending_text_control_write_backs.clear();
        self.pending_host_call_results.clear();
        self.waiting_action_receive_calls.clear();
        self.task_generation_pins.clear();
        self.tasks = RuntimeTaskRegistry::default();
        self.presentation = snapshot.presentation;
        self.view_virtualization = restored_view_virtualization;
        self.view_runtime = restored_view_runtime;
        self.retire_unused_generations();
        Ok(())
    }

    fn session_save_blockers(&self) -> Vec<BundleSessionPendingBlocker> {
        let mut blockers = Vec::new();
        if !self.pending_presentation_inputs.is_empty() {
            blockers.push(BundleSessionPendingBlocker::PendingPresentationInputs {
                count: self.pending_presentation_inputs.len(),
            });
        }
        if !self.pending_input_events.is_empty() {
            blockers.push(BundleSessionPendingBlocker::PendingInputEvents {
                count: self.pending_input_events.len(),
            });
        }
        if !self.pending_text_control_write_backs.is_empty() {
            blockers.push(BundleSessionPendingBlocker::PendingTextControlWriteBacks {
                count: self.pending_text_control_write_backs.len(),
            });
        }
        if !self.pending_host_call_results.is_empty() {
            blockers.push(BundleSessionPendingBlocker::PendingHostCallResults {
                count: self.pending_host_call_results.len(),
            });
        }
        if !self.waiting_action_receive_calls.is_empty() {
            blockers.push(BundleSessionPendingBlocker::WaitingActionReceiveCalls {
                count: self.waiting_action_receive_calls.len(),
            });
        }
        let active_tasks = self.tasks.list(RuntimeTaskListOptions::default()).len();
        let queued_task_events = self.tasks.queued_task_event_count();
        if active_tasks > 0 || queued_task_events > 0 {
            blockers.push(BundleSessionPendingBlocker::HostTasks {
                active: active_tasks,
                queued_events: queued_task_events,
            });
        }
        if !self.task_generation_pins.is_empty() {
            blockers.push(BundleSessionPendingBlocker::TaskGenerationPins {
                count: self.task_generation_pins.len(),
            });
        }
        blockers
    }

    fn validate_session_save_generation(
        &self,
        snapshot: &BundleSessionGenerationSnapshot,
    ) -> Result<(), BundleSessionSaveError> {
        let active = self.active_generation();
        if snapshot.active_generation != active.id {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "active_generation",
                saved: format!("{:?}", snapshot.active_generation),
                actual: format!("{:?}", active.id),
            });
        }
        let actual_artifact = self.active_artifact_identity;
        if snapshot.artifact != actual_artifact {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "artifact",
                saved: format!("{:?}", snapshot.artifact),
                actual: format!("{actual_artifact:?}"),
            });
        }
        if snapshot.bytecode_abi != active.bytecode_abi {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "bytecode_abi",
                saved: snapshot.bytecode_abi.to_string(),
                actual: active.bytecode_abi.to_string(),
            });
        }
        if snapshot.adapter_requirements != active.adapter_requirements {
            return Err(BundleSessionSaveError::GenerationMismatch {
                field: "adapter_requirements",
                saved: digest_label(&snapshot.adapter_requirements),
                actual: digest_label(&active.adapter_requirements),
            });
        }
        Ok(())
    }

    pub(super) fn activate_runtime(&mut self, runtime: SessionRuntime) {
        self.source_label = runtime.source_label;
        self.executor = runtime.executor;
        self.display = runtime.display;
        self.image_objects = runtime.image_objects;
        self.text_inputs = runtime.text_inputs;
        self.action_buttons = runtime.action_buttons;
        self.scroll_regions = runtime.scroll_regions;
        self.surfaces = runtime.surfaces;
        self.focus_groups = runtime.focus_groups;
        self.focus_navigation = runtime.focus_navigation;
        self.fx_definitions = runtime.fx_definitions;
        self.view_runtime = runtime.view_runtime;
        self.view_style_palettes = runtime.view_style_palettes;
    }

    pub(super) fn prune_runtime_images(&mut self) {
        let live = self.swap.live_generation_ids();
        self.runtime_images.retain_generations(&live);
    }

    pub(super) fn release_table_only_retired_runtime_images(&mut self) {
        let table_only_generations = self
            .swap
            .retired()
            .iter()
            .filter(|generation| {
                self.runtime_images.contains_generation(generation.id)
                    && Arc::strong_count(generation) <= 2
            })
            .map(|generation| generation.id)
            .collect::<Vec<_>>();
        for generation in table_only_generations {
            self.runtime_images.remove(generation);
        }
    }
}

fn validate_dialogue_view_save_point(
    runtime: &BundleViewRuntime,
    presentation: &BundlePresentationSnapshot,
) -> Result<(), BundleSessionSaveError> {
    runtime
        .validate_dialogue_snapshot(
            &presentation.view,
            &presentation.dialogue.view_inputs(),
            &presentation.presentation_handles,
        )
        .map_err(|error| BundleSessionSaveError::ViewRuntime {
            message: error.to_string(),
        })
}
