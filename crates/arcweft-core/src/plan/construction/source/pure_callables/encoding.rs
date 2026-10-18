//! Complete borrowed headers; counting and emission share the same wire writer.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimePureCallableSourceLimits, charge, limit, reference,
};
use crate::plan::{
    RuntimeCallableParameter, RuntimePlan, RuntimePureHelperOrigin, RuntimeReceiverMode,
};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct PreparedRuntimePureCallableEncoding<'a> {
    plan: &'a RuntimePlan,
    limits: RuntimePureCallableSourceLimits,
    bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimePureCallableEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes {
            return Err(Error::OutputCapacity {
                required: self.bytes,
                remaining,
            });
        }
        let mut writer = Writer {
            output: Some(output),
            meter: None,
            bytes: 0,
            maximum: self.bytes,
        };
        let mut counts = Counts::new(self.limits);
        emit(self.plan, self.limits, &mut writer, &mut counts)?;
        debug_assert_eq!(writer.bytes, self.bytes);
        debug_assert_eq!(counts.footprint, self.footprint);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn prepare_pure_callable_declaration_sources_encoding(
        &self,
        limits: RuntimePureCallableSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimePureCallableEncoding<'_>, Error> {
        meter.status()?;
        let result = (|| {
            for (kind, expected, actual) in [
                ("type", limits.type_count, self.type_table().len()),
                ("local", limits.local_count, self.local_declarations().len()),
                (
                    "function",
                    limits.function_count,
                    self.function_sites().len(),
                ),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut writer = Writer {
                output: None,
                meter: Some(&mut *meter),
                bytes: 0,
                maximum: limits.encoded_bytes,
            };
            let mut counts = Counts::new(limits);
            emit(self, limits, &mut writer, &mut counts)?;
            Ok(PreparedRuntimePureCallableEncoding {
                plan: self,
                limits,
                bytes: writer.bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    bytes: usize,
    maximum: usize,
}
impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let next = self
            .bytes
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", next, self.maximum)?;
        if let Some(meter) = self.meter.as_deref_mut() {
            charge(meter, bytes.len())?;
        }
        if let Some(output) = self.output.as_deref_mut() {
            output.extend_from_slice(bytes);
        }
        self.bytes = next;
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.bytes(&[n])
    }
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.u32(u32::try_from(n).map_err(|_| Error::Arithmetic)?)
    }
    fn u32(&mut self, n: u32) -> Result<(), Error> {
        self.bytes(&n.to_le_bytes())
    }
    fn index(&mut self, n: usize) -> Result<(), Error> {
        self.bytes(
            &u64::try_from(n)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn optional_index(&mut self, n: Option<usize>) -> Result<(), Error> {
        self.byte(u8::from(n.is_some()))?;
        if let Some(n) = n {
            self.index(n)?;
        }
        Ok(())
    }
    fn string(&mut self, text: &str) -> Result<(), Error> {
        self.count(text.len())?;
        self.bytes(text.as_bytes())
    }
    fn optional_string(&mut self, text: Option<&str>) -> Result<(), Error> {
        self.byte(u8::from(text.is_some()))?;
        if let Some(text) = text {
            self.string(text)?;
        }
        Ok(())
    }
    fn inputs(
        &mut self,
        inputs: &[RuntimeCallableParameter],
        limits: RuntimePureCallableSourceLimits,
    ) -> Result<(), Error> {
        self.count(inputs.len())?;
        for input in inputs {
            let local = input.local().get().get();
            reference("local", local, limits.local_count)?;
            self.bytes(input.identity().as_bytes())?;
            self.u32(local)?;
            self.byte(input.passing().semantic_tag())?;
            self.byte(input.abi().semantic_tag())?;
        }
        Ok(())
    }
}
fn emit(
    plan: &RuntimePlan,
    limits: RuntimePureCallableSourceLimits,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
) -> Result<(), Error> {
    limit(
        "helpers",
        plan.pure_helpers().len(),
        limits.helpers as usize,
    )?;
    limit(
        "methods",
        plan.trait_methods().len(),
        limits.methods as usize,
    )?;
    limit(
        "program bindings",
        plan.pure_programs().len(),
        limits.bindings as usize,
    )?;
    writer.bytes(DOMAIN)?;
    writer.byte(1)?;
    writer.count(plan.pure_helpers().len())?;
    for (index, row) in plan.pure_helpers().iter().enumerate() {
        if row.id.0 != index {
            return Err(Error::Ordinal {
                expected: u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?,
                actual: u32::try_from(row.id.0 + 1).map_err(|_| Error::Arithmetic)?,
            });
        }
        counts.helper(row.inputs.len(), row.name.len())?;
        reference("result type", row.expr.ty().get().get(), limits.type_count)?;
        writer.count(index + 1)?;
        writer.bytes(row.definition.as_bytes())?;
        writer.u32(row.expr.ty().get().get())?;
        writer.byte(row.output_type.semantic_tag())?;
        writer.string(&row.name)?;
        writer.byte(u8::from(row.scalar_eval_supported))?;
        writer.byte(match row.origin {
            RuntimePureHelperOrigin::Annotated => 0,
            RuntimePureHelperOrigin::Inferred => 1,
        })?;
        writer.inputs(&row.inputs, limits)?;
    }
    writer.count(plan.trait_methods().len())?;
    for (index, row) in plan.trait_methods().iter().enumerate() {
        if row.id.0 != index {
            return Err(Error::Ordinal {
                expected: u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?,
                actual: u32::try_from(row.id.0 + 1).map_err(|_| Error::Arithmetic)?,
            });
        }
        let identity = &row.identity;
        let labels = [
            identity.trait_name.as_ref().map_or(0, String::len),
            identity.self_type.len(),
            identity.method_name.len(),
            identity.monomorph_label.len(),
        ]
        .into_iter()
        .try_fold(0usize, |sum, n| sum.checked_add(n).ok_or(Error::Arithmetic))?;
        counts.method(row.inputs.len(), labels)?;
        reference("result type", row.body.ty().get().get(), limits.type_count)?;
        writer.count(index + 1)?;
        writer.bytes(row.definition.as_bytes())?;
        writer.u32(row.body.ty().get().get())?;
        writer.byte(row.output_type.semantic_tag())?;
        writer.byte(match row.receiver {
            RuntimeReceiverMode::Owned => 0,
            RuntimeReceiverMode::SharedRef => 1,
            RuntimeReceiverMode::MutRef => 2,
        })?;
        writer.index(identity.impl_id)?;
        writer.optional_index(identity.trait_id)?;
        writer.optional_index(identity.witness)?;
        writer.optional_string(identity.trait_name.as_deref())?;
        writer.string(&identity.self_type)?;
        writer.string(&identity.method_name)?;
        writer.string(&identity.monomorph_label)?;
        writer.inputs(&row.inputs, limits)?;
    }
    writer.count(plan.pure_programs().len())?;
    for (index, row) in plan.pure_programs().iter().enumerate() {
        let function = row.site().get().get();
        reference("function", function, limits.function_count)?;
        let actual = plan
            .function_sites()
            .get(row.site())
            .expect("checked original function")
            .inputs()
            .len();
        if actual != row.input_types().len() {
            return Err(Error::Namespace {
                kind: "program binding inputs",
                expected: row.input_types().len(),
                actual,
            });
        }
        counts.binding(actual)?;
        writer.count(index + 1)?;
        writer.bytes(&row.program().as_bytes())?;
        writer.u32(function)?;
        writer.count(actual)?;
    }
    Ok(())
}
