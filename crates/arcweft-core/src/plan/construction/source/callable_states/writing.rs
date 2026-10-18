//! Bounded emission borrows the original state graph; references stay numeric
//! source coordinates until the real builder resolves their whole namespace.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeCallableSourceLimits, limit, reference};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableAttachedContract as Attached, RuntimeCallableDefault as Default,
    RuntimeCallableInputSource as Input, RuntimeCallableParameterCoordinate as Coordinate,
    RuntimeCallableParameterInput as Parameter, RuntimeCallableParameterKind as Kind,
    RuntimeCallablePartialTransition as Partial, RuntimeCallablePosition as Position,
    RuntimeCallableRetainedInput as Retained, RuntimeCallableRetainedRole as Role,
    RuntimeCallableState, RuntimeCallableStateSeedId, RuntimeCallableStateTable,
    RuntimeCallableTransition as Transition,
};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId, RuntimePlanTypeId};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct PreparedRuntimeCallableEncoding<'a> {
    table: &'a RuntimeCallableStateTable,
    limits: RuntimeCallableSourceLimits,
    encoded_bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeCallableEncoding<'_> {
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
        let mut writer = Writer::new(Some(output), None, self.limits, self.table.len())?;
        writer.table(self.table)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.footprint, self.footprint);
        Ok(())
    }
}
impl RuntimeCallableStateTable {
    pub(in crate::plan) fn prepare_sources_encoding<'a>(
        &'a self,
        limits: RuntimeCallableSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeCallableEncoding<'a>, Error> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer::new(None, Some(&mut *meter), limits, self.len())?;
            writer.table(self)?;
            Ok(PreparedRuntimeCallableEncoding {
                table: self,
                limits,
                encoded_bytes: writer.length,
                footprint: writer.counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    length: usize,
    counts: Counts,
}
impl<'a> Writer<'a> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        limits: RuntimeCallableSourceLimits,
        rows: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            output,
            meter,
            length: 0,
            counts: Counts::new(limits, rows)?,
        })
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit(
            "encoded bytes",
            self.length,
            self.counts.limits.encoded_bytes,
        )?;
        if let Some(meter) = &mut self.meter {
            meter.charge_work(u64::try_from(bytes.len()).map_err(|_| Error::Arithmetic)?)?;
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn tag(&mut self, tag: u8) -> Result<(), Error> {
        self.bytes(&[tag])
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
    fn function(&mut self, function: RuntimeFunctionSiteId) -> Result<(), Error> {
        reference(
            "function",
            function.get().get() as usize,
            self.counts.limits.function_count,
            true,
        )?;
        self.u32(function.get().get())
    }
    fn state(&mut self, state: RuntimeCallableStateId) -> Result<(), Error> {
        reference("state", state.index(), self.counts.footprint.rows, false)?;
        self.count(state.index())
    }
    fn coordinate(&mut self, value: Coordinate) -> Result<(), Error> {
        self.u32(value.group)?;
        self.u32(value.parameter)
    }
    fn coordinates(&mut self, values: &[Coordinate]) -> Result<(), Error> {
        self.counts.array::<Coordinate>(values.len())?;
        self.count(values.len())?;
        for &value in values {
            self.coordinate(value)?;
        }
        Ok(())
    }
    fn inputs(&mut self, values: &[Input]) -> Result<(), Error> {
        self.counts.array::<Input>(values.len())?;
        self.count(values.len())?;
        for &input in values {
            match input {
                Input::Retained { position } => {
                    self.tag(0)?;
                    self.u32(position)?;
                }
                Input::Argument { position } => {
                    self.tag(1)?;
                    self.u32(position)?;
                }
                Input::Attached => self.tag(2)?,
            }
        }
        Ok(())
    }
    fn row(&mut self, row: &RuntimeCallableState) -> Result<(), Error> {
        self.ty(row.function_type)?;
        self.state(row.origin)?;
        match &row.position {
            Position::Unapplied => self.tag(0)?,
            Position::WithinGroup { group, bound } => {
                self.tag(1)?;
                self.u32(*group)?;
                self.coordinates(bound)?;
            }
            Position::AfterGroup { completed } => {
                self.tag(2)?;
                self.u32(*completed)?;
            }
        }
        self.counts
            .mapped::<Retained<RuntimeSemanticTypeId>, Retained<RuntimePlanTypeId>>(
                row.retained.len(),
            )?;
        self.count(row.retained.len())?;
        for input in &row.retained {
            match input.role {
                Role::Capture { position } => {
                    self.tag(0)?;
                    self.u32(position)?;
                }
                Role::Parameter(coordinate) => {
                    self.tag(1)?;
                    self.coordinate(coordinate)?;
                }
            }
            self.ty(input.ty)?;
        }
        self.counts
            .mapped::<Parameter<RuntimeSemanticTypeId>, Parameter<RuntimePlanTypeId>>(
                row.parameters.len(),
            )?;
        self.count(row.parameters.len())?;
        for input in &row.parameters {
            self.coordinate(input.coordinate)?;
            self.tag(match input.kind {
                Kind::Fixed => 0,
                Kind::Rest => 1,
            })?;
            self.ty(input.abi_ty)?;
            self.ty(input.binding_ty)?;
        }
        self.ty(row.result)?;
        match &row.attached {
            Attached::None => self.tag(0)?,
            Attached::Required { ty } => {
                self.tag(1)?;
                self.ty(*ty)?;
            }
            Attached::Optional { value, binding } => {
                self.tag(2)?;
                self.ty(*value)?;
                self.ty(*binding)?;
            }
            Attached::Defaulted { ty, default } => {
                self.tag(3)?;
                self.ty(*ty)?;
                match default {
                    Default::RequiresSpecialization => self.tag(0)?,
                    Default::Body { function, captures } => {
                        self.tag(1)?;
                        self.function(*function)?;
                        self.inputs(captures)?;
                    }
                }
            }
        }
        match &row.transition {
            Transition::RequiresSpecialization => self.tag(0)?,
            Transition::Retain { state, values } => {
                self.tag(1)?;
                self.state(*state)?;
                self.inputs(values)?;
            }
            Transition::Invoke {
                function,
                captures,
                arguments,
            } => {
                self.tag(2)?;
                self.function(*function)?;
                self.inputs(captures)?;
                self.inputs(arguments)?;
            }
        }
        self.counts
            .mapped::<Partial<RuntimeCallableStateSeedId>, Partial<RuntimeCallableStateId>>(
                row.partials.len(),
            )?;
        self.count(row.partials.len())?;
        for partial in &row.partials {
            self.coordinates(&partial.parameters)?;
            self.state(partial.state)?;
            self.inputs(&partial.values)?;
        }
        Ok(())
    }
    fn table(&mut self, table: &RuntimeCallableStateTable) -> Result<(), Error> {
        self.bytes(DOMAIN)?;
        self.tag(1)?;
        self.count(table.len())?;
        for (id, row) in table.iter_with_ids() {
            self.count(id.index())?;
            self.row(row)?;
        }
        Ok(())
    }
}
