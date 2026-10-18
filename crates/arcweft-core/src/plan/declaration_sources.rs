//! Joint admission of original declaration frames, before any table is retained.
//! These sources grant neither an executable proof nor a completed plan lease.

use super::control_effect::source_codec::{
    PreparedRuntimeControlSources, RuntimeControlSourceError, RuntimeControlSourceLimits,
};
use super::local_declarations::RuntimeLocalDeclarationTableBuilder;
use super::local_declarations::source_codec::{
    PreparedRuntimeLocalSources, RuntimeLocalSourceError, RuntimeLocalSourceLimits,
};
use super::type_table::source_codec::domains::PreparedRuntimeNominalSources;
use super::type_table::source_codec::{
    PreparedRuntimeTypeTableSource, RuntimeTypeSourceError, RuntimeTypeSourceLimits,
};
use super::{
    RuntimeControlEffectContract, RuntimeControlEffectContractTable,
    RuntimeNominalRecordDomainTable, RuntimePlanInventory, RuntimePlanTypeTable,
    RuntimeVariantDomainTable,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

#[derive(Clone, Copy)]
pub(super) struct RuntimePlanDeclarationSourceLimits {
    pub(super) types: u32,
    pub(super) nominals: u32,
    pub(super) locals: u32,
    pub(super) controls: u32,
    pub(super) elements: u32,
    pub(super) encoded_bytes: usize,
    pub(super) decoded_bytes: usize,
    pub(super) policy_depth: u32,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(super) enum RuntimePlanDeclarationSourceError {
    #[error("declaration source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("declaration source count arithmetic overflow")]
    Arithmetic,
    #[error("declaration source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("invalid type source: {0}")]
    Types(#[from] RuntimeTypeSourceError),
    #[error("invalid nominal source: {0}")]
    Nominals(RuntimeTypeSourceError),
    #[error("invalid local source: {0}")]
    Locals(#[from] RuntimeLocalSourceError),
    #[error("invalid control source: {0}")]
    Controls(#[from] RuntimeControlSourceError),
}

impl From<TaskSemanticEncodingError> for RuntimePlanDeclarationSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

/// Borrowed, complete extents of the four owning source grammars.
pub(super) struct RuntimePlanDeclarationSourceBytes<'a> {
    pub(super) types: &'a [u8],
    pub(super) nominals: &'a [u8],
    pub(super) locals: &'a [u8],
    pub(super) controls: &'a [u8],
}

impl<'a> RuntimePlanDeclarationSourceBytes<'a> {
    pub(super) fn preflight(
        self,
        limits: RuntimePlanDeclarationSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimePlanDeclarationSources<'a>, RuntimePlanDeclarationSourceError> {
        meter.status()?;
        let result = (|| {
            let encoded = [self.types, self.nominals, self.locals, self.controls]
                .into_iter()
                .try_fold(0usize, |sum, bytes| sum.checked_add(bytes.len()))
                .ok_or(RuntimePlanDeclarationSourceError::Arithmetic)?;
            limit("encoded bytes", encoded, limits.encoded_bytes)?;
            let type_limits = RuntimeTypeSourceLimits {
                rows: limits.types,
                elements: limits.elements,
                encoded_bytes: limits.encoded_bytes,
                decoded_bytes: limits.decoded_bytes,
                policy_depth: limits.policy_depth,
            };
            let types = RuntimePlanTypeTable::preflight_sources(self.types, type_limits, meter)?;
            let type_count = types.row_count();
            let nominals = RuntimePlanInventory::preflight_nominal_sources(
                self.nominals,
                type_count,
                RuntimeTypeSourceLimits {
                    rows: limits.nominals,
                    ..type_limits
                },
                meter,
            )
            .map_err(RuntimePlanDeclarationSourceError::Nominals)?;
            let locals = RuntimeLocalDeclarationTableBuilder::preflight_sources(
                self.locals,
                RuntimeLocalSourceLimits {
                    rows: limits.locals,
                    encoded_bytes: limits.encoded_bytes,
                    decoded_bytes: limits.decoded_bytes,
                    type_count,
                },
                meter,
            )?;
            let controls = RuntimeControlEffectContractTable::preflight_sources(
                self.controls,
                RuntimeControlSourceLimits {
                    rows: limits.controls,
                    elements: limits.elements as usize,
                    encoded_bytes: limits.encoded_bytes,
                    decoded_bytes: limits.decoded_bytes,
                    type_count,
                },
                meter,
            )?;
            let storage_bytes = sum([
                types.storage_bytes(),
                nominals.storage_bytes(),
                locals.storage_bytes(),
                controls.storage_bytes(),
            ])?;
            limit("decoded bytes", storage_bytes, limits.decoded_bytes)?;
            let elements = sum([types.elements(), nominals.elements(), controls.elements()])?;
            limit("declaration elements", elements, limits.elements as usize)?;
            Ok(PreparedRuntimePlanDeclarationSources {
                types,
                nominals,
                locals,
                controls,
                storage_bytes,
                limits,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

/// All four child grammars and their combined footprint have passed preflight.
pub(super) struct PreparedRuntimePlanDeclarationSources<'a> {
    pub(super) types: PreparedRuntimeTypeTableSource<'a>,
    pub(super) nominals: PreparedRuntimeNominalSources<'a>,
    pub(super) locals: PreparedRuntimeLocalSources<'a>,
    pub(super) controls: PreparedRuntimeControlSources<'a>,
    storage_bytes: usize,
    limits: RuntimePlanDeclarationSourceLimits,
}

impl PreparedRuntimePlanDeclarationSources<'_> {
    pub(super) fn counts(&self) -> [usize; 5] {
        [
            self.types.row_count(),
            self.nominals.record_count(),
            self.nominals.variant_count(),
            self.locals.row_count(),
            self.controls.row_count(),
        ]
    }

    pub(super) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    /// Includes the construction index and actual issuer-bound handles. This
    /// check precedes materialization, including when a containing body decoder
    /// needs to sum this footprint with its other admitted children.
    pub(super) fn construction_storage_bytes(
        &self,
    ) -> Result<usize, RuntimePlanDeclarationSourceError> {
        let [types, _, _, locals, controls] = self.counts();
        let extra = super::construction::source::RuntimePlanSourceConstruction::storage_bytes(
            types, locals, controls,
        )
        .ok_or(RuntimePlanDeclarationSourceError::Arithmetic)?;
        let total = self
            .storage_bytes
            .checked_add(extra)
            .ok_or(RuntimePlanDeclarationSourceError::Arithmetic)?;
        limit("decoded bytes", total, self.limits.decoded_bytes)?;
        Ok(total)
    }

    pub(super) fn into_construction(
        self,
        mut meter: TaskSemanticMeter,
    ) -> Result<
        super::construction::source::RuntimePlanSourceConstruction,
        RuntimePlanDeclarationSourceError,
    > {
        meter.status()?;
        let result = (|| {
            self.construction_storage_bytes()?;
            let [types, _, _, locals, controls] = self.counts();
            let work = sum([types, locals, controls, 1])?;
            meter.charge_work(
                u64::try_from(work).map_err(|_| RuntimePlanDeclarationSourceError::Arithmetic)?,
            )?;
            self.materialize(&mut meter)
        })();
        match result {
            Ok(declarations) => Ok(
                super::construction::source::RuntimePlanSourceConstruction::from_declarations(
                    declarations,
                    meter,
                ),
            ),
            Err(error) => {
                meter.reject_owner();
                Err(error)
            }
        }
    }

    pub(super) fn materialize(
        self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimePlanSourceDeclarations, RuntimePlanDeclarationSourceError> {
        meter.status()?;
        let result = (|| {
            let types = self.types.materialize(meter)?;
            let (records, variants) = self
                .nominals
                .materialize(&types, meter)
                .map_err(RuntimePlanDeclarationSourceError::Nominals)?;
            let locals = self.locals.materialize(&types, meter)?;
            let controls = self.controls.materialize_definitions(&types, meter)?;
            Ok(RuntimePlanSourceDeclarations {
                types,
                records,
                variants,
                locals,
                controls,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

/// Real owning tables and pending local rows; no placement or proof is supplied.
pub(super) struct RuntimePlanSourceDeclarations {
    pub(super) types: RuntimePlanTypeTable,
    pub(super) records: RuntimeNominalRecordDomainTable,
    pub(super) variants: RuntimeVariantDomainTable,
    pub(super) locals: RuntimeLocalDeclarationTableBuilder,
    pub(super) controls: Vec<RuntimeControlEffectContract>,
}

fn sum<const N: usize>(values: [usize; N]) -> Result<usize, RuntimePlanDeclarationSourceError> {
    values
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(RuntimePlanDeclarationSourceError::Arithmetic)
}

fn limit(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimePlanDeclarationSourceError> {
    if actual > maximum {
        return Err(RuntimePlanDeclarationSourceError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
