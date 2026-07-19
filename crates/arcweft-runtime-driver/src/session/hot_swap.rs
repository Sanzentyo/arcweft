//! Atomic bundle and patch hot-swap transactions.

use super::{
    Arc, ArcweftBundle, BundleFormat, BundleHotSwapError, BundleHotSwapReport, BundlePatchArtifact,
    BundlePatchReadiness, BundlePatchReadinessReport, BundlePresentationSnapshot, BundleSession,
    BundleSessionArtifactIdentity, BundleSessionError, BundleView, GenerationId,
    GenerationRuntimeImage, PatchMaterializedTarget, ProgramGeneration, ReadBudget,
    SwapCompatibility, ViewRuntimeTextControl, apply_patch_bundle, build_session_runtime,
    build_session_runtime_preserving_executor, classify_swap_for_entry, decode_patch_bundle,
    reconciled_root_handles_for_restore, validate_virtual_list_scroll_owner,
};

impl BundleSession {
    pub fn hot_swap_bundle(
        &mut self,
        bundle: &ArcweftBundle,
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let identity = bundle.logical_identity().map_err(|error| {
            BundleSessionError::GenerationFingerprint {
                message: error.to_string(),
            }
        })?;
        self.hot_swap_bundle_with_compatibility_floor(
            bundle,
            BundleSessionArtifactIdentity::LogicalBundle { identity },
            SwapCompatibility::ContentOnly,
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "hot-swap validation and commit ordering form one atomic generation transaction"
    )]
    fn hot_swap_bundle_with_compatibility_floor(
        &mut self,
        bundle: &ArcweftBundle,
        next_artifact_identity: BundleSessionArtifactIdentity,
        compatibility_floor: SwapCompatibility,
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let next_id = GenerationId(self.next_generation_id);
        let next_generation = Arc::new(ProgramGeneration::from_bundle(next_id, bundle)?);
        let active_entry = self
            .executor
            .product_active_entry_snapshot_identity()
            .map_err(|error| BundleHotSwapError::ActiveEntry {
                message: error.to_string(),
            })?
            .ok_or_else(|| BundleHotSwapError::ActiveEntry {
                message: "active executor has no Product AWBC entry identity".to_owned(),
            })?;
        let compatibility =
            classify_swap_for_entry(self.swap.active(), &next_generation, &active_entry.id)
                .max(compatibility_floor);
        if compatibility == SwapCompatibility::ContentOnly
            && next_artifact_identity == self.active_artifact_identity
        {
            return Ok(BundleHotSwapReport {
                generation: self.active_generation().id,
                compatibility,
            });
        }
        if compatibility == SwapCompatibility::RestartRequired {
            return Err(BundleHotSwapError::RestartRequired { compatibility });
        }
        let root_blockers = self.executor.product_root_save_blockers();
        let reducer_active = root_blockers
            .as_ref()
            .is_some_and(|blockers| blockers.reducer_active);
        let pending_events = root_blockers
            .as_ref()
            .map_or(0, |blockers| blockers.pending_events as usize)
            .checked_add(self.pending_deferred_root_events.len())
            .expect("live root-event queues cannot exceed addressable memory");
        let pending_commands = root_blockers
            .as_ref()
            .map_or(0, |blockers| blockers.pending_commands);
        let pending_command_results = self.pending_root_command_results.len();
        if reducer_active
            || pending_events > 0
            || pending_commands > 0
            || pending_command_results > 0
        {
            return Err(BundleHotSwapError::PendingRootWork {
                reducer_active,
                pending_events,
                pending_commands,
                pending_command_results,
            });
        }

        let mut next_runtime = if matches!(
            compatibility,
            SwapCompatibility::ContentOnly | SwapCompatibility::CodeCompatible
        ) {
            build_session_runtime_preserving_executor(bundle, &self.options, &self.executor)?
        } else {
            build_session_runtime(bundle, &self.options)?
        };
        let mut next_environment = self.environment.clone();
        let _environment_update =
            next_environment.replace_theme(next_runtime.view_theme_environment)?;
        preserve_runtime_text_control_values(&self.text_inputs, &mut next_runtime.text_inputs);
        if matches!(
            compatibility,
            SwapCompatibility::ContentOnly | SwapCompatibility::CodeCompatible
        ) {
            for list in self.view_virtualization.mounts() {
                validate_virtual_list_scroll_owner(
                    &next_runtime.scroll_regions,
                    list.scroll_target(),
                    list.axis(),
                )
                .map_err(|error| BundleHotSwapError::ViewVirtualization {
                    message: error.to_string(),
                })?;
            }
        }
        if compatibility == SwapCompatibility::ContentOnly {
            let dialogue = self.presentation.dialogue.view_inputs();
            next_runtime
                .view_runtime
                .validate_dialogue_inputs(&dialogue)
                .map_err(|error| BundleHotSwapError::ViewRuntime {
                    message: error.to_string(),
                })?;
            let reconciled_root_handles = reconciled_root_handles_for_restore(
                &self.presentation.presentation_handles,
                &dialogue,
            )
            .map_err(|error| BundleHotSwapError::ViewRuntime {
                message: error.to_string(),
            })?;
            let view_snapshot =
                self.view_runtime
                    .snapshot()
                    .map_err(|error| BundleHotSwapError::ViewRuntime {
                        message: error.to_string(),
                    })?;
            next_runtime
                .view_runtime
                .restore(&view_snapshot, &reconciled_root_handles)
                .map_err(|error| BundleHotSwapError::ViewRuntime {
                    message: error.to_string(),
                })?;
        }
        if compatibility == SwapCompatibility::ContentOnly
            && let Err(error) = self
                .presentation
                .fx
                .validate_for_definitions(&next_runtime.fx_definitions)
        {
            self.presentation.record_fx_error(&error);
            return Err(error.into());
        }

        self.swap
            .prepare_with_compatibility(next_generation, compatibility)
            .map_err(BundleHotSwapError::Prepare)?;
        self.swap
            .begin_quiescence()
            .map_err(BundleHotSwapError::Prepare)?;

        match compatibility {
            SwapCompatibility::ContentOnly => {
                bundle
                    .source_display_name()
                    .clone_into(&mut self.source_label);
                self.display = bundle.display.clone();
                self.image_objects.clone_from(&bundle.image_objects);
                self.text_inputs.clone_from(&next_runtime.text_inputs);
                self.action_buttons.clone_from(&next_runtime.action_buttons);
                self.scroll_regions.clone_from(&next_runtime.scroll_regions);
                self.surfaces.clone_from(&next_runtime.surfaces);
                self.focus_groups.clone_from(&next_runtime.focus_groups);
                self.focus_navigation
                    .clone_from(&next_runtime.focus_navigation);
                self.fx_definitions.clone_from(&next_runtime.fx_definitions);
                self.view_runtime = next_runtime.view_runtime.clone();
                self.view_style_palettes = next_runtime.view_style_palettes;
            }
            SwapCompatibility::CodeCompatible => {
                self.activate_runtime(next_runtime.clone());
                self.pending_input_events.clear();
                self.pending_presentation_inputs.clear();
                self.pending_host_call_results.clear();
                self.waiting_action_receive_calls.clear();
                self.presentation = BundlePresentationSnapshot::default();
            }
            SwapCompatibility::CodeGenerational => {
                // The current fiber keeps running on its existing executor. The
                // new runtime image becomes the binding target for new entries.
            }
            SwapCompatibility::RestartRequired => {
                unreachable!("restart-required compatibility returned before prepare")
            }
        }

        let committed = self.swap.commit().map_err(BundleHotSwapError::Commit)?;
        self.runtime_images.insert(GenerationRuntimeImage::new(
            self.swap.active().clone(),
            next_runtime,
        ))?;
        self.environment = next_environment;
        if committed == SwapCompatibility::CodeCompatible {
            self.runtime_generation_pin = Some(self.swap.pin_active_generation());
        }
        self.retire_unused_generations();
        self.next_generation_id = self.next_generation_id.saturating_add(1);
        self.active_artifact_identity = next_artifact_identity;
        Ok(BundleHotSwapReport {
            generation: next_id,
            compatibility: committed,
        })
    }

    /// Applies a materialized target whose patch identities were verified.
    pub fn hot_swap_materialized_patch(
        &mut self,
        materialized: &PatchMaterializedTarget,
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let active_identity = self
            .active_artifact_identity
            .awfb_container()
            .ok_or(BundleHotSwapError::MissingActiveContainerIdentity)?;
        let base_identity = materialized.report().base_artifact;
        if base_identity != active_identity {
            return Err(BundleHotSwapError::WrongPatchBaseArtifact {
                active: Box::new(active_identity),
                expected: Box::new(base_identity),
            });
        }

        let target_view =
            BundleView::parse(materialized.bytes(), ReadBudget::default()).map_err(|error| {
                BundleHotSwapError::DecodePatchTarget {
                    message: error.to_string(),
                }
            })?;
        let actual_identity = target_view.artifact_identity();
        if actual_identity != materialized.report().target_artifact {
            return Err(BundleHotSwapError::DecodePatchTarget {
                message: format!(
                    "materialized target identity {actual_identity:?} does not match verified identity {:?}",
                    materialized.report().target_artifact
                ),
            });
        }
        let target_bundle =
            ArcweftBundle::from_format_slice(BundleFormat::Awfb, materialized.bytes()).map_err(
                |error| BundleHotSwapError::DecodePatchTarget {
                    message: error.to_string(),
                },
            )?;
        let compatibility_floor =
            SwapCompatibility::from_patch_compatibility(materialized.compatibility());
        self.hot_swap_bundle_with_compatibility_floor(
            &target_bundle,
            BundleSessionArtifactIdentity::AwfbContainer {
                identity: actual_identity,
            },
            compatibility_floor,
        )
    }

    pub fn inspect_hot_swap_patch_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<BundlePatchReadinessReport, BundleHotSwapError> {
        let artifact = decode_patch_bundle(bytes).map_err(BundleHotSwapError::DecodePatch)?;
        self.inspect_hot_swap_patch_artifact(&artifact)
    }

    pub fn hot_swap_patch_bytes(
        &mut self,
        base_awfb_bytes: &[u8],
        patch_awfb_bytes: &[u8],
    ) -> Result<BundleHotSwapReport, BundleHotSwapError> {
        let artifact =
            decode_patch_bundle(patch_awfb_bytes).map_err(BundleHotSwapError::DecodePatch)?;
        self.inspect_hot_swap_patch_artifact(&artifact)?;
        let materialized = apply_patch_bundle(base_awfb_bytes, &artifact)
            .map_err(BundleHotSwapError::MaterializePatch)?;
        self.hot_swap_materialized_patch(&materialized)
    }

    pub fn inspect_hot_swap_patch_artifact(
        &self,
        artifact: &BundlePatchArtifact,
    ) -> Result<BundlePatchReadinessReport, BundleHotSwapError> {
        artifact
            .validate()
            .map_err(BundleHotSwapError::InvalidPatch)?;
        let active_container_identity = self
            .active_artifact_identity
            .awfb_container()
            .ok_or(BundleHotSwapError::MissingActiveContainerIdentity)?;
        if artifact.manifest.base_artifact != active_container_identity {
            return Err(BundleHotSwapError::WrongPatchBaseArtifact {
                active: Box::new(active_container_identity),
                expected: Box::new(artifact.manifest.base_artifact),
            });
        }
        artifact
            .plan
            .validate_base(active_container_identity.content_root)
            .map_err(BundleHotSwapError::WrongPatchBase)?;
        let readiness = if artifact.plan.is_empty()
            && artifact.manifest.target_artifact == active_container_identity
        {
            BundlePatchReadiness::Noop
        } else {
            BundlePatchReadiness::TargetBundleRequired {
                operations: artifact.plan.operations.len(),
            }
        };
        Ok(BundlePatchReadinessReport {
            base_generation: self.active_generation().id,
            base_content_root: artifact.plan.base_content_root,
            target_content_root: artifact.plan.target_content_root,
            declared_compatibility: artifact.manifest.compatibility,
            readiness,
        })
    }
}

fn preserve_runtime_text_control_values(
    current: &[ViewRuntimeTextControl],
    next: &mut [ViewRuntimeTextControl],
) {
    for next_control in next.iter_mut() {
        if let Some(current_control) = current.iter().find(|current_control| {
            same_runtime_text_control_identity(current_control, next_control)
        }) {
            next_control.value.clone_from(&current_control.value);
            next_control.selection = current_control.selection;
        }
    }
}

fn same_runtime_text_control_identity(
    left: &ViewRuntimeTextControl,
    right: &ViewRuntimeTextControl,
) -> bool {
    left.public_id == right.public_id
        && left.target == right.target
        && left.session == right.session
}

#[cfg(test)]
mod tests {
    use super::preserve_runtime_text_control_values;
    use crate::session::ViewRuntimeTextControl;
    use arcweft_bundle::resource_codec::view::{
        CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
        ViewInputPurpose, ViewRuntimeControlVisualStyle, ViewRuntimeTextControlBounds,
        ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions, ViewSecureInputPolicy,
        ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextTabPolicy,
        ViewTextVerticalNavigationPolicy,
    };

    fn runtime_control(target: &str, session: u64, value: &str) -> ViewRuntimeTextControl {
        ViewRuntimeTextControl {
            public_id: target.to_owned(),
            target: target.to_owned(),
            view: None,
            containing_scroll_region: None,
            session,
            value: value.to_owned(),
            selection: super::super::ViewRuntimeTextSelection::collapsed_at_end(value),
            options: ViewRuntimeTextControlOptions {
                purpose: ViewInputPurpose::Text,
                autocorrect: TextAssistPolicy::PlatformDefault,
                spellcheck: TextAssistPolicy::PlatformDefault,
                capitalization: TextCapitalization::None,
                enter_key: EnterKeyHint::Default,
                multiline: false,
                selection_policy: ViewTextSelectionPolicy::Enabled,
                shortcut_policy: ViewTextShortcutPolicy::Enabled,
                tab_policy: ViewTextTabPolicy::FocusNavigation,
                vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
                secure_policy: ViewSecureInputPolicy::Plain,
                composition_on_blur: CompositionOnBlurPolicy::Commit,
            },
            kind: ViewInputKind::TextField,
            bounds: ViewRuntimeTextControlBounds::from_px(0, 0, 100, 24),
            label: None,
            handlers: ViewRuntimeTextControlHandlers::default(),
            style: ViewRuntimeControlVisualStyle::default(),
        }
    }

    #[test]
    fn hot_swap_preserves_matching_runtime_text_value_and_drops_removed_controls() {
        let current = vec![runtime_control("field.name", 7, "edited")];
        let mut next = vec![runtime_control("field.name", 7, "default")];
        preserve_runtime_text_control_values(&current, &mut next);
        assert_eq!(next[0].value, "edited");

        let mut incompatible = vec![runtime_control("field.other", 8, "default")];
        preserve_runtime_text_control_values(&current, &mut incompatible);
        assert_eq!(incompatible[0].value, "default");
    }
}
