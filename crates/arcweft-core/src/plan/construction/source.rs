//! Source decoding uses the actual construction issuer and owning tables.
//! Handles bind admitted rows, while bodies and local placements remain absent.

use super::task_plans::RuntimeTaskPlanDefinition;
use super::{RuntimeControlEffectContractSeedId, RuntimeLocalSeedId, RuntimePlanBuilder};
use crate::plan::body_semantic::task_image::{RuntimeTaskPlanSealError, UnsealedRuntimePlanImage};
use crate::plan::declaration_sources::RuntimePlanSourceDeclarations;
use crate::plan::{RuntimeControlEffectContract, RuntimeControlEffectContractId};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::semantic::TaskSemanticMeter;
use std::sync::Mutex;

pub(in crate::plan) mod callable_specializations;
pub(in crate::plan) mod callable_states;
pub(in crate::plan) mod content_declarations;
pub(in crate::plan) mod expression;
pub(in crate::plan) mod functions;
pub(in crate::plan) mod pattern;
pub(in crate::plan) mod pure_callables;

pub(in crate::plan) struct RuntimePlanSourceConstruction {
    pub(in crate::plan) builder: RuntimePlanBuilder,
    locals: Box<[RuntimeLocalSeedId]>,
    controls: Box<[RuntimeControlEffectContractSeedId]>,
    meter: TaskSemanticMeter,
}

impl RuntimePlanSourceConstruction {
    /// Extra logical storage while the admitted tables enter construction.
    /// Shared Arc payloads include their two reference-count slots once.
    pub(in crate::plan) fn storage_bytes(
        types: usize,
        locals: usize,
        controls: usize,
    ) -> Option<usize> {
        let types = super::RuntimePlanTypeTableBuilder::source_storage_bytes(types)?;
        let locals = RuntimeLocalSeedId::source_storage_bytes(locals)?;
        let controls = controls.checked_mul(
            size_of::<Option<RuntimeControlEffectContract>>()
                + size_of::<RuntimeControlEffectContractSeedId>(),
        )?;
        let shared = size_of::<Self>()
            + size_of::<super::RuntimePlanConstructionIssuer>()
            + size_of::<std::sync::atomic::AtomicBool>()
            + size_of::<Mutex<super::RuntimeLocalDeclarationTableBuilder>>()
            + size_of::<Mutex<Option<Vec<RuntimeTaskPlanDefinition>>>>()
            + 8 * size_of::<usize>();
        types
            .checked_add(locals)?
            .checked_add(controls)?
            .checked_add(shared)
    }

    pub(in crate::plan) fn from_declarations(
        declarations: RuntimePlanSourceDeclarations,
        meter: TaskSemanticMeter,
    ) -> Self {
        let RuntimePlanSourceDeclarations {
            types,
            records,
            variants,
            locals,
            controls,
        } = declarations;
        let local_count = locals.len();
        let mut builder = RuntimePlanBuilder::new();
        builder.types = types.into_source_builder();
        builder.nominal_record_domains = records.into_source_builder();
        builder.variant_domains = variants.into_source_builder();
        *builder
            .locals
            .lock()
            .expect("local request table only runs checked, callback-free transitions") = locals;
        builder.control_effect_contracts = controls.into_iter().map(Some).collect();
        let locals = (0..local_count)
            .map(|index| {
                let ordinal = u32::try_from(index + 1).expect("preflighted local coordinate");
                let local = RuntimeLocalDeclarationId::from_accepted_ordinal(
                    std::num::NonZeroU32::new(ordinal).expect("source coordinates are positive"),
                );
                RuntimeLocalSeedId::issued_source(&builder.issuer, &builder.locals, local)
                    .expect("the source handle binds its actual pending row")
            })
            .collect();
        let controls = (0..builder.control_effect_contracts.len())
            .map(|index| {
                let id = RuntimeControlEffectContractId::for_index(index)
                    .expect("preflighted control coordinate");
                RuntimeControlEffectContractSeedId::issued_source(&builder, id)
                    .expect("the source handle binds its actual original definition")
            })
            .collect();
        Self {
            builder,
            locals,
            controls,
            meter,
        }
    }

    pub(in crate::plan) fn prepare_image(
        self,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<UnsealedRuntimePlanImage, RuntimeTaskPlanSealError> {
        self.builder
            .prepare_task_image_with_meter(limits, self.meter)
    }

    pub(in crate::plan) fn local(
        &self,
        id: RuntimeLocalDeclarationId,
    ) -> Option<&RuntimeLocalSeedId> {
        let index = usize::try_from(id.get().get() - 1).ok()?;
        self.locals.get(index)
    }

    pub(in crate::plan) fn control(
        &self,
        id: RuntimeControlEffectContractId,
    ) -> Option<&RuntimeControlEffectContractSeedId> {
        self.controls.get(id.index())
    }
}
