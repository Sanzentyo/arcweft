use super::*;
use crate::effect_row::EffectFormula;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeArrayLength, RuntimeBoundEffectReference, RuntimeCallableSpecializationState,
};

pub(in crate::plan) struct PreparedRuntimeSpecializationEncoding<'a> {
    plan: &'a RuntimePlan,
    limits: RuntimeSpecializationSourceLimits,
    encoded_bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeSpecializationEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(Error::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.plan, self.limits)?;
        writer.table(self.plan.callable_specializations())?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.footprint, self.footprint);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn prepare_specialization_sources_encoding<'a>(
        &'a self,
        limits: RuntimeSpecializationSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeSpecializationEncoding<'a>, Error> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer::new(None, Some(&mut *meter), self, limits)?;
            writer.table(self.callable_specializations())?;
            let encoded_bytes = writer.length;
            let footprint = writer.counts.footprint;
            meter.charge_work(
                u64::try_from(encoded_bytes)
                    .ok()
                    .and_then(|n| n.checked_mul(2))
                    .ok_or(Error::Arithmetic)?,
            )?;
            Ok(PreparedRuntimeSpecializationEncoding {
                plan: self,
                limits,
                encoded_bytes,
                footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a, 'types> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    types: Types<'types>,
    length: usize,
    counts: Counts,
}
impl<'a, 'types> Writer<'a, 'types> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        plan: &'types RuntimePlan,
        limits: RuntimeSpecializationSourceLimits,
    ) -> Result<Self, Error> {
        if plan.type_table().len() != limits.type_count
            || plan.callable_states().len() != limits.state_count
        {
            return Err(Error::Namespace { kind: "program" });
        }
        Ok(Self {
            output,
            meter,
            types: Types::Program(plan.type_table()),
            length: 0,
            counts: Counts::new(limits, plan.callable_specializations().len())?,
        })
    }
    fn account(&mut self, count: usize) -> Result<(), Error> {
        self.length = self.length.checked_add(count).ok_or(Error::Arithmetic)?;
        limit(
            "encoded bytes",
            self.length,
            self.counts.limits.encoded_bytes,
        )?;
        if let Some(meter) = &mut self.meter {
            meter.charge_work(u64::try_from(count).map_err(|_| Error::Arithmetic)?)?;
        }
        Ok(())
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.account(bytes.len())?;
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.u32(u32::try_from(value).map_err(|_| Error::Arithmetic)?)
    }
    fn ty(&mut self, ty: RuntimePlanTypeId) -> Result<(), Error> {
        reference(
            "type",
            ty.get().get() as usize,
            self.counts.limits.type_count,
            true,
        )?;
        self.u32(ty.get().get())
    }
    fn state(&mut self, state: RuntimeCallableStateId) -> Result<(), Error> {
        reference(
            "state",
            state.index(),
            self.counts.limits.state_count,
            false,
        )?;
        self.count(state.index())
    }
    fn row(&mut self, row: &Definition) -> Result<(), Error> {
        self.ty(row.source_type)?;
        self.ty(row.target_type)?;
        let (target, scope) = self.types.target(row.target_type)?;
        self.counts.array(
            row.arguments.types.len(),
            size_of::<RuntimeSemanticTypeId>() + size_of::<RuntimePlanTypeId>(),
        )?;
        self.count(row.arguments.types.len())?;
        for &ty in &row.arguments.types {
            self.ty(ty)?;
        }
        self.counts.array(
            row.arguments.const_lengths.len(),
            size_of::<RuntimeArrayLength>(),
        )?;
        self.count(row.arguments.const_lengths.len())?;
        for &length in &row.arguments.const_lengths {
            match length {
                RuntimeArrayLength::Constant(value) => {
                    self.bytes(&[0])?;
                    self.bytes(&value.to_le_bytes())?;
                }
                RuntimeArrayLength::Bound(reference) => {
                    self.bytes(&[1])?;
                    self.u32(reference.depth())?;
                    self.bytes(&reference.slot().to_le_bytes())?;
                }
            }
        }
        self.counts.array(
            row.arguments.effects.len(),
            size_of::<EffectFormula<RuntimeBoundEffectReference>>(),
        )?;
        self.count(row.arguments.effects.len())?;
        for argument in &row.arguments.effects {
            let available = self
                .counts
                .limits
                .encoded_bytes
                .checked_sub(self.length)
                .and_then(|n| n.checked_sub(4))
                .ok_or(Error::Arithmetic)?;
            let child_limits = self.counts.effect_limits(available)?;
            let prepared = match self.meter.as_deref_mut() {
                Some(meter) => {
                    target.prepare_argument_source_encoding(argument, scope, child_limits, meter)?
                }
                None => target.prepare_argument_source_in_frame(argument, scope, child_limits)?,
            };
            self.counts
                .child(prepared.elements(), prepared.storage_bytes())?;
            self.count(prepared.encoded_bytes())?;
            self.account(prepared.encoded_bytes())?;
            if let Some(output) = &mut self.output {
                prepared.append_to(output)?;
            }
        }
        self.counts.array(
            row.states.len(),
            size_of::<RuntimeCallableSpecializationState<RuntimeCallableStateSeedId>>()
                + size_of::<RuntimeCallableSpecializationState<RuntimeCallableStateId>>(),
        )?;
        self.count(row.states.len())?;
        for state in &row.states {
            self.state(state.source)?;
            self.state(state.target)?;
        }
        Ok(())
    }
    fn table(&mut self, rows: &[Definition]) -> Result<(), Error> {
        self.bytes(DOMAIN)?;
        self.bytes(&[1])?;
        self.count(rows.len())?;
        for (index, row) in rows.iter().enumerate() {
            self.count(index)?;
            self.row(row)?;
        }
        Ok(())
    }
}
