//! Typed test-only replacement of one accepted stamp authority.

use std::sync::Arc;

use arcweft_lang_sema::registration::RegisteredSemanticWorld;

use super::{
    AcceptedEnvironmentGeneration, AcceptedProfileEnvironment, AcceptedProfileKey, LspProfileState,
};
use crate::profiles::{accepted_project::AcceptedProjectSnapshot, caches::ProfileSemanticCaches};

/// One independently replaced authority in an otherwise identical environment.
pub(crate) enum AcceptedEnvironmentStampMutation {
    Allocation,
    Generation(AcceptedEnvironmentGeneration),
    Profile(AcceptedProfileKey),
    World(Arc<RegisteredSemanticWorld>),
    Project(Arc<AcceptedProjectSnapshot>),
}

/// Creates a fresh cache namespace around one explicitly selected authority change.
pub(crate) fn mutated_environment(
    current: &Arc<AcceptedProfileEnvironment>,
    mutation: AcceptedEnvironmentStampMutation,
) -> Arc<AcceptedProfileEnvironment> {
    let mut generation = current.generation;
    let mut profile = current.profile.clone();
    let mut world = Arc::clone(&current.world);
    let mut project = Arc::clone(&current.project);
    match mutation {
        AcceptedEnvironmentStampMutation::Allocation => {}
        AcceptedEnvironmentStampMutation::Generation(replacement) => generation = replacement,
        AcceptedEnvironmentStampMutation::Profile(replacement) => profile = replacement,
        AcceptedEnvironmentStampMutation::World(replacement) => world = replacement,
        AcceptedEnvironmentStampMutation::Project(replacement) => project = replacement,
    }
    Arc::new(AcceptedProfileEnvironment {
        generation,
        profile,
        compiled: Arc::clone(&current.compiled),
        world,
        project,
        overlays: current.overlays.clone(),
        caches: ProfileSemanticCaches::default(),
    })
}

impl LspProfileState {
    /// Installs an already-constructed immutable environment for stamp validation tests.
    pub(crate) fn install_stamp_environment_for_test(
        &self,
        environment: Arc<AcceptedProfileEnvironment>,
    ) {
        self.accepted_write().replace(environment);
    }
}
