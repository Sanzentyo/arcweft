//! Session-facing retained Fx lifecycle and observation projection.

use super::BundleSession;
use crate::fx_runtime::{BundleFxRuntimeError, BundleFxRuntimeSnapshot, bind_runtime_template};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_presentation::fx::{
    FxAuthoredSeed, FxGraphChildPath, FxInstanceId, FxInstanceIdentity, FxRuntimeValue,
};

impl BundleSession {
    /// Returns the canonical bundle definitions used by all renderer adapters.
    pub const fn fx_definitions(&self) -> &FxDefinitions {
        &self.fx_definitions
    }

    /// Returns the deterministic live Fx state carried by presentation/save snapshots.
    pub const fn fx_runtime(&self) -> &BundleFxRuntimeSnapshot {
        &self.presentation.fx
    }

    /// Retains one stable application and refreshes only its reactive parameter slots.
    pub fn retain_fx_instance(
        &mut self,
        identity: FxInstanceIdentity,
        parameters: Vec<FxRuntimeValue>,
        child_path: FxGraphChildPath,
        authored_seed: Option<FxAuthoredSeed>,
    ) -> Result<(), BundleFxRuntimeError> {
        let instance = identity.instance();
        let definition = identity.definition().clone();
        let active = self.fx_definitions.get(&definition).ok_or_else(|| {
            BundleFxRuntimeError::MissingDefinition {
                definition: Box::new(definition.clone()),
                instance,
            }
        })?;
        let template = bind_runtime_template(active, &parameters)?;
        let result = self.presentation.fx.retain_instance(
            &self.fx_definitions,
            identity,
            template,
            parameters,
            child_path,
            authored_seed,
        );
        match result {
            Ok(()) => {
                self.presentation.fx_diagnostics.clear();
                self.presentation.revision = self.presentation.revision.saturating_add(1);
                Ok(())
            }
            Err(error) => {
                self.presentation.record_fx_error(&error);
                Err(error)
            }
        }
    }

    pub fn release_fx_instance(&mut self, instance: FxInstanceId) -> bool {
        let removed = self.presentation.fx.remove_instance(instance).is_some();
        if removed {
            self.presentation.revision = self.presentation.revision.saturating_add(1);
        }
        removed
    }

    pub(super) fn append_fx_diagnostics(&self, diagnostics: &mut Vec<String>) {
        diagnostics.extend(
            self.presentation
                .fx_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone()),
        );
    }
}
