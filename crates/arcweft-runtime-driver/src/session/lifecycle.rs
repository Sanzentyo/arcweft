//! Session identity, generation ownership, task access, and environment entrypoints.

use super::{
    Arc, ArtifactIdentity, BundleDigest, BundlePresentationSnapshot, BundleSession, GenerationId,
    PresentationEnvironment, PresentationEnvironmentField, PresentationEnvironmentUpdate,
    PresentationEnvironmentUpdateError, PresentationEnvironmentValue,
    PresentationEnvironmentValues, ProgramGeneration, RuntimeTaskCancelOutcome,
    RuntimeTaskCancelTarget, RuntimeTaskListOptions, RuntimeTaskOwner, RuntimeTaskRecord,
    SystemPaletteSet, TaskSequence, ViewStyleProgram,
};

impl BundleSession {
    pub fn source_label(&self) -> &str {
        &self.source_label
    }

    pub fn active_generation(&self) -> &ProgramGeneration {
        self.swap.active()
    }

    /// Returns the generation currently bound to the active runtime fiber.
    pub fn current_fiber_generation(&self) -> Option<GenerationId> {
        self.runtime_generation_pin
            .as_ref()
            .map(|generation| generation.id)
    }

    /// Returns the generation that emitted an outstanding host task.
    pub fn task_generation(&self, sequence: TaskSequence) -> Option<GenerationId> {
        self.task_generation_pins
            .get(&sequence)
            .map(|generation| generation.id)
    }

    pub fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        self.tasks.list(options)
    }

    pub fn cancel_runtime_tasks(
        &mut self,
        target: &RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        self.tasks.cancel(target)
    }

    pub fn runtime_image_count(&self) -> usize {
        self.runtime_images.len()
    }

    pub fn has_runtime_image(&self, generation: GenerationId) -> bool {
        self.runtime_images.contains_generation(generation)
    }

    pub fn pin_active_generation(&self) -> Arc<ProgramGeneration> {
        self.swap.pin_active_generation()
    }

    pub fn retired_generation_count(&self) -> usize {
        self.swap.retired().len()
    }

    pub fn retire_unused_generations(&mut self) {
        self.release_table_only_retired_runtime_images();
        self.swap.retire_unused();
        self.prune_runtime_images();
    }

    pub const fn active_container_content_root(&self) -> Option<BundleDigest> {
        match self.active_artifact_identity.awfb_container() {
            Some(identity) => Some(identity.content_root),
            None => None,
        }
    }

    /// Returns the logical identity of the active AWFB container, when present.
    pub const fn active_container_artifact_identity(&self) -> Option<ArtifactIdentity> {
        self.active_artifact_identity.awfb_container()
    }

    pub const fn presentation(&self) -> &BundlePresentationSnapshot {
        &self.presentation
    }

    /// Canonical native Style program for the currently active View runtime.
    #[must_use]
    pub const fn view_style_program(&self) -> Option<&ViewStyleProgram> {
        self.view_runtime.style_program()
    }

    /// Current revisioned Style-visible presentation environment.
    #[must_use]
    pub const fn presentation_environment(&self) -> PresentationEnvironment {
        self.environment.effective()
    }

    pub fn update_presentation_environment_provider(
        &mut self,
        values: PresentationEnvironmentValues,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        self.environment.replace_provider(values)
    }

    pub fn clear_presentation_environment_provider(
        &mut self,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        self.environment.clear_provider()
    }

    pub fn set_presentation_environment_override(
        &mut self,
        value: PresentationEnvironmentValue,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        self.environment.set_session_override(value)
    }

    pub fn remove_presentation_environment_override(
        &mut self,
        field: PresentationEnvironmentField,
    ) -> Result<PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError> {
        self.environment.remove_session_override(field)
    }

    /// Engine palette plus typed `ViewTheme` role overrides.
    #[must_use]
    pub const fn view_style_palettes(&self) -> &SystemPaletteSet {
        &self.view_style_palettes
    }
}

impl RuntimeTaskOwner for BundleSession {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        BundleSession::runtime_tasks(self, options)
    }

    fn cancel_runtime_tasks(
        &mut self,
        target: RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        BundleSession::cancel_runtime_tasks(self, &target)
    }
}
