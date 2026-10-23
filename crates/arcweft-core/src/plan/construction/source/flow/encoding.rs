//! Counted canonical Flow emission uses the actual owned-body/value-root
//! projections. The same borrowed field grammar validates both sizing passes.

mod fields;

use super::{Counts, DOMAIN, Error, Footprint, RuntimeFlowSourceLimits, charge, limit, metadata};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{FlowOp, RuntimeFlowOwnedBodies, RuntimePlan};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct EncodedRuntimeFlowSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeFlowSource {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes.len() {
            return Err(Error::OutputCapacity {
                required: self.bytes.len(),
                remaining,
            });
        }
        output.extend_from_slice(&self.bytes);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn encode_flow_source(
        &self,
        ops: &[FlowOp],
        limits: RuntimeFlowSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeFlowSource, Error> {
        meter.status()?;
        let result = (|| {
            for (kind, expected, actual) in [
                (
                    "type",
                    limits.expression.type_count,
                    self.type_table().len(),
                ),
                (
                    "local",
                    limits.expression.local_count,
                    self.local_declarations().len(),
                ),
                ("task", limits.task_count, self.task_plans().len()),
                (
                    "content",
                    limits.content_count,
                    self.dialogue_content().rows().len(),
                ),
                ("defer", limits.defer_count, self.defer_sites().len()),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut operations = 0usize;
            walk(ops, limits, |op| {
                canonical(op)?;
                operations = operations.checked_add(1).ok_or(Error::Arithmetic)?;
                limit("operations", operations, limits.nodes as usize)
            })?;
            charge(meter, operations)?;
            let mut counted = Counts::new(limits, operations)?;
            let mut sink = Sink {
                output: None,
                length: 0,
                maximum: limits.encoded_bytes,
            };
            emit(
                self,
                ops,
                operations,
                &mut sink,
                &mut counted,
                resources,
                meter,
            )?;
            let length = sink.length;
            charge(meter, length)?;
            let mut bytes = Vec::with_capacity(length);
            let mut emitted = Counts::new(limits, operations)?;
            let mut sink = Sink {
                output: Some(&mut bytes),
                length: 0,
                maximum: length,
            };
            emit(
                self,
                ops,
                operations,
                &mut sink,
                &mut emitted,
                resources,
                meter,
            )?;
            debug_assert_eq!(sink.length, length);
            debug_assert_eq!(counted.footprint, emitted.footprint);
            Ok(EncodedRuntimeFlowSource {
                bytes,
                footprint: counted.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
fn canonical(op: &FlowOp) -> Result<(), Error> {
    match op {
        FlowOp::LetScope { .. } => Err(Error::Continuation),
        _ => Ok(()),
    }
}
enum Work<'a> {
    Ops(std::slice::Iter<'a, FlowOp>, usize),
    Bodies(RuntimeFlowOwnedBodies<'a>, usize),
}
fn walk<'a>(
    ops: &'a [FlowOp],
    limits: RuntimeFlowSourceLimits,
    mut visit: impl FnMut(&'a FlowOp) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut work = vec![Work::Ops(ops.iter(), 1)];
    while let Some(next) = work.pop() {
        match next {
            Work::Ops(mut ops, depth) => {
                if let Some(op) = ops.next() {
                    limit("depth", depth, limits.depth as usize)?;
                    work.push(Work::Ops(ops, depth));
                    visit(op)?;
                    work.push(Work::Bodies(op.owned_bodies(), depth));
                }
            }
            Work::Bodies(mut bodies, depth) => {
                if let Some((_, body)) = bodies.next() {
                    work.push(Work::Bodies(bodies, depth));
                    work.push(Work::Ops(
                        body.iter(),
                        depth.checked_add(1).ok_or(Error::Arithmetic)?,
                    ));
                }
            }
        }
    }
    Ok(())
}
struct Sink<'a> {
    output: Option<&'a mut Vec<u8>>,
    length: usize,
    maximum: usize,
}
impl Sink<'_> {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)?;
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.raw(&[byte])
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(value)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
}
fn emit(
    plan: &RuntimePlan,
    ops: &[FlowOp],
    operations: usize,
    sink: &mut Sink<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    sink.raw(DOMAIN)?;
    sink.byte(1)?;
    sink.count(ops.len())?;
    sink.count(operations)?;
    counts.body(ops.len())?;
    walk(ops, counts.limits, |op| {
        canonical(op)?;
        let packet = fields::encode(op, plan, counts, resources, meter)?;
        // All metadata/value frames in this one node are bounded temporary
        // Source bytes. Their complete aggregate resource check precedes the
        // complete output allocation in the first pass and all seed creation.
        let mut cursor = metadata::Cursor::new(&packet);
        let row = cursor.operation(counts, resources, meter)?;
        cursor.finish()?;
        debug_assert_eq!(row.tag, op.semantic_tag());
        sink.raw(&packet)
    })
}
