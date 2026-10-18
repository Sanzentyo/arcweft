//! Borrowed original headers and prepared input patterns share one bank.

use super::super::pattern::PreparedRuntimePatternEncoding;
use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeFunctionSourceLimits, charge, limit, reference,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeFunctionCaptureMode as Capture, RuntimeFunctionInputBinding,
    RuntimeFunctionInputOrigin as Origin, RuntimeFunctionInputSource as Source,
    RuntimeFunctionInputTransfer as Transfer, RuntimeFunctionSiteBody, RuntimeFunctionSiteTable,
    RuntimeValueOwnershipRequirement,
};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct PreparedRuntimeFunctionEncoding<'a> {
    table: &'a RuntimeFunctionSiteTable,
    patterns: Vec<PreparedRuntimePatternEncoding<'a>>,
    encoded_bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeFunctionEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn pattern_nodes(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(Error::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let start = output.len();
        output.extend_from_slice(DOMAIN);
        output.push(1);
        put(output, ordinal(self.table.len())?);
        let mut patterns = self.patterns.into_iter();
        for (id, row) in self.table.iter_with_ids() {
            put(output, id.get().get());
            output.extend_from_slice(row.definition().as_bytes());
            output.push(row.role().semantic_tag());
            put(output, row.function_type().map_or(0, |ty| ty.get().get()));
            put(output, row.result().get().get());
            output.push(match row.body() {
                RuntimeFunctionSiteBody::Expression(_) => 0,
                RuntimeFunctionSiteBody::Executable(_) => 1,
            });
            put(output, ordinal(row.invocation_effects().len())?);
            for effect in row.invocation_effects().iter() {
                put(output, ordinal(effect.as_str().len())?);
                output.extend_from_slice(effect.as_str().as_bytes());
            }
            put(output, ordinal(row.inputs().len())?);
            for input in row.inputs() {
                emit_input(input, output)?;
                let pattern = patterns
                    .next()
                    .expect("one prepared source for every original input");
                put(output, ordinal(pattern.encoded_bytes())?);
                pattern.append_to(output)?;
            }
        }
        debug_assert!(patterns.next().is_none());
        debug_assert_eq!(output.len() - start, self.encoded_bytes);
        Ok(())
    }
}

impl RuntimeFunctionSiteTable {
    pub(in crate::plan) fn prepare_declaration_sources_encoding<'a>(
        &'a self,
        limits: RuntimeFunctionSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeFunctionEncoding<'a>, Error> {
        meter.status()?;
        let result = (|| {
            let mut counts = Counts::new(limits, self.len())?;
            let mut length = DOMAIN.len() + 1 + 4;
            limit("encoded bytes", length, limits.encoded_bytes)?;
            for row in self.iter() {
                meter.charge_work(1)?;
                counts.row(row.inputs().len(), row.invocation_effects().len(), 0)?;
                if let Some(ty) = row.function_type() {
                    reference("function type", ty.get().get(), limits.type_count)?;
                }
                reference("type", row.result().get().get(), limits.type_count)?;
                length = length
                    .checked_add(4 + 32 + 1 + 4 + 4 + 1 + 4 + 4)
                    .ok_or(Error::Arithmetic)?;
                for effect in row.invocation_effects().iter() {
                    meter.charge_work(1)?;
                    ordinal(effect.as_str().len())?;
                    counts.storage(
                        effect
                            .as_str()
                            .len()
                            .checked_mul(2)
                            .ok_or(Error::Arithmetic)?,
                    )?;
                    length = length
                        .checked_add(4)
                        .and_then(|n| n.checked_add(effect.as_str().len()))
                        .ok_or(Error::Arithmetic)?;
                }
                for input in row.inputs() {
                    meter.charge_work(1)?;
                    reference("local", input.input_local().get().get(), limits.local_count)?;
                    counts.unrestricted(input.unrestricted_bindings().len())?;
                    meter.charge_work(
                        u64::try_from(input.unrestricted_bindings().len())
                            .map_err(|_| Error::Arithmetic)?,
                    )?;
                    let mut previous = 0;
                    for id in input.unrestricted_bindings() {
                        reference("unrestricted binding", id.get().get(), limits.local_count)?;
                        if id.get().get() <= previous {
                            return Err(Error::Order {
                                kind: "unrestricted bindings",
                            });
                        }
                        previous = id.get().get();
                    }
                    length = length
                        .checked_add(input_bytes(input)?)
                        .ok_or(Error::Arithmetic)?;
                }
                limit("encoded bytes", length, limits.encoded_bytes)?;
            }
            // All signature containers are bounded before preparing child rows.
            let mut patterns = Vec::with_capacity(counts.footprint.inputs);
            for row in self.iter() {
                for input in row.inputs() {
                    let pattern = input.pattern().prepare_sources_encoding(
                        counts.pattern_limits(limits.encoded_bytes - length),
                        resources,
                        meter,
                    )?;
                    ordinal(pattern.encoded_bytes())?;
                    counts.pattern(
                        pattern.storage_bytes(),
                        pattern.elements(),
                        pattern.node_count(),
                    )?;
                    length = length
                        .checked_add(pattern.encoded_bytes())
                        .ok_or(Error::Arithmetic)?;
                    limit("encoded bytes", length, limits.encoded_bytes)?;
                    patterns.push(pattern);
                }
            }
            charge(meter, length)?;
            Ok(PreparedRuntimeFunctionEncoding {
                table: self,
                patterns,
                encoded_bytes: length,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}

fn input_bytes(input: &RuntimeFunctionInputBinding) -> Result<usize, Error> {
    let transfer = match input.transfer() {
        Transfer::Transferred(_) => 2,
        Transfer::ExternalBinding | Transfer::Formal => 1,
    };
    let source = match input.source() {
        Source::Capture { .. } => 5,
        Source::CapturedParameter { .. } | Source::Parameter { .. } => 6,
    };
    input
        .unrestricted_bindings()
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(transfer + 33 + source + 4 + 1 + 4 + 4))
        .ok_or(Error::Arithmetic)
}
fn emit_input(input: &RuntimeFunctionInputBinding, out: &mut Vec<u8>) -> Result<(), Error> {
    match input.transfer() {
        Transfer::Transferred(mode) => {
            out.push(0);
            out.push(match mode {
                Capture::Copy => 0,
                Capture::SnapshotClone => 1,
                Capture::Move => 2,
            });
        }
        Transfer::ExternalBinding => out.push(1),
        Transfer::Formal => out.push(2),
    }
    match input.origin() {
        Origin::Binding(id) => {
            out.push(0);
            out.extend_from_slice(&id);
        }
        Origin::Parameter(id) => {
            out.push(1);
            out.extend_from_slice(id.as_bytes());
        }
        Origin::EvaluatedResult(id) => {
            out.push(2);
            out.extend_from_slice(id.as_bytes());
        }
    }
    match input.source() {
        Source::Capture { position } => {
            out.push(0);
            put(out, position);
        }
        Source::CapturedParameter { position, passing } => {
            out.push(1);
            put(out, position);
            out.push(passing.semantic_tag());
        }
        Source::Parameter { position, passing } => {
            out.push(2);
            put(out, position);
            out.push(passing.semantic_tag());
        }
    }
    put(out, input.input_local().get().get());
    out.push(match input.ownership() {
        RuntimeValueOwnershipRequirement::Owned => 0,
        RuntimeValueOwnershipRequirement::Unrestricted => 1,
    });
    put(out, ordinal(input.unrestricted_bindings().len())?);
    for id in input.unrestricted_bindings() {
        put(out, id.get().get());
    }
    Ok(())
}
fn ordinal(value: usize) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| Error::Arithmetic)
}
fn put(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
