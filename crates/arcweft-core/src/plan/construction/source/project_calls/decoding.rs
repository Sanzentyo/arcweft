//! Restore only the existing seed algebra using the original construction issuer.

use super::super::RuntimePlanSourceConstruction;
use super::super::expression::RuntimeExpressionSourceHandles;
use super::cursor::{Cursor, Ordinary};
use super::{Counts, Error, PreparedRuntimeProjectCallSource, charge};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::{RuntimePattern, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeFlowOpSeed, RuntimeProjectCallAttachedMaterializationSeed,
    RuntimeProjectCallFixedMaterializationSeed, RuntimeProjectCallOperandSeed,
    RuntimeProjectCallOrdinaryMaterializationSeed as Materialization, RuntimeProjectCallPlanSeed,
    RuntimeProjectCallRestMaterializationSeed,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::value::RuntimeExpr;

impl PreparedRuntimeProjectCallSource<'_> {
    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeFlowOpSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let row = cursor.row()?;
            cursor.finish()?;
            let mut counts = Counts::new(self.limits)?;
            let callee = self.expression(row.callee, &mut counts, source, handles, resources)?;
            counts.operands(row.operand_count)?;
            let mut operands = Vec::with_capacity(row.operand_count);
            let mut operand = Cursor::new(row.operands, self.limits);
            for _ in 0..row.operand_count {
                let (mode, bytes) = operand.operand()?;
                operands.push(RuntimeProjectCallOperandSeed {
                    value: self.expression(bytes, &mut counts, source, handles, resources)?,
                    mode,
                });
            }
            operand.finish()?;
            counts.ordinary(row.ordinary_count)?;
            let mut ordinary = Vec::with_capacity(row.ordinary_count);
            let mut parameter = Cursor::new(row.ordinary, self.limits);
            for index in 0..row.ordinary_count {
                ordinary.push(match parameter.ordinary(index, row.operand_count)? {
                    Ordinary::Fixed {
                        parameter,
                        abi,
                        binding,
                        source: operand,
                    } => Materialization::Fixed(RuntimeProjectCallFixedMaterializationSeed {
                        parameter,
                        abi_ty: ty(source, abi),
                        binding_ty: ty(source, binding),
                        source_index: operand,
                    }),
                    Ordinary::Rest {
                        parameter,
                        abi,
                        binding,
                        sources,
                    } => {
                        counts.sources(sources.len() / 4)?;
                        Materialization::Rest(RuntimeProjectCallRestMaterializationSeed {
                            parameter,
                            abi_ty: ty(source, abi),
                            binding_ty: ty(source, binding),
                            source_indices: sources
                                .chunks_exact(4)
                                .map(|bytes| {
                                    u32::from_le_bytes(
                                        bytes.try_into().expect("source index width"),
                                    )
                                })
                                .collect(),
                        })
                    }
                });
            }
            parameter.finish()?;
            let attached =
                row.attached
                    .map(|attached| RuntimeProjectCallAttachedMaterializationSeed {
                        abi_ty: ty(source, attached.abi),
                        binding_ty: ty(source, attached.binding),
                        source_index: attached.source,
                        presence: attached.presence,
                    });
            let pattern = RuntimePattern::preflight_sources(
                row.result,
                counts.pattern(row.result.len()),
                resources,
                &mut source.meter,
            )?;
            counts.child(
                pattern.node_count(),
                pattern.elements(),
                pattern.storage_bytes(),
            )?;
            let result = pattern.materialize(source, resources)?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(RuntimeFlowOpSeed::ProjectCall {
                plan: RuntimeProjectCallPlanSeed {
                    placement: crate::plan::RuntimeProjectCallSitePlacementSeed::Allocate,
                    callee,
                    state: handles.states[row.state as usize - 1].clone(),
                    completed_group: row.group,
                    operands: operands.into_boxed_slice(),
                    ordinary: ordinary.into_boxed_slice(),
                    attached,
                },
                result,
            })
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
    fn expression(
        &self,
        bytes: &[u8],
        counts: &mut Counts,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<crate::plan::RuntimeExprSeed, Error> {
        let child = RuntimeExpr::preflight_sources(
            bytes,
            counts.expression(bytes.len()),
            resources,
            &mut source.meter,
        )?;
        counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        Ok(child.materialize(source, handles, resources)?)
    }
}
fn ty(source: &RuntimePlanSourceConstruction, id: u32) -> RuntimeSemanticTypeId {
    let id = RuntimePlanTypeId::from_accepted_ordinal(
        std::num::NonZeroU32::new(id).expect("checked type"),
    );
    source
        .builder
        .types
        .get(id)
        .expect("authenticated original type namespace")
        .semantic_identity()
}
