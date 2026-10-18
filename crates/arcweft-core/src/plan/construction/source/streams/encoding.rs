//! Source bytes are emitted only after the complete counted traversal succeeds.
//! Nested grammars keep their existing owners and the original work bank.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeStreamSourceLimits, charge, limit, reference,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::RuntimePlan;
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath};
use crate::stream::StreamOp;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(in crate::plan) struct EncodedRuntimeStreamSources {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeStreamSources {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
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
    pub(in crate::plan) fn encode_stream_sources(
        &self,
        limits: RuntimeStreamSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeStreamSources, Error> {
        meter.status()?;
        let result = (|| {
            if self.type_table().len() != limits.expression.type_count
                || self.local_declarations().len() != limits.expression.local_count
            {
                return Err(Error::Namespace);
            }
            let mut counts = Counts::new(limits);
            let mut writer = Writer {
                output: None,
                length: 0,
                maximum: limits.encoded_bytes,
            };
            emit(self, &mut writer, &mut counts, resources, meter)?;
            let n = writer.length;
            charge(meter, n)?;
            let mut bytes = Vec::with_capacity(n);
            let mut emitted = Counts::new(limits);
            let mut writer = Writer {
                output: Some(&mut bytes),
                length: 0,
                maximum: n,
            };
            emit(self, &mut writer, &mut emitted, resources, meter)?;
            debug_assert_eq!(writer.length, n);
            debug_assert_eq!(emitted.footprint, counts.footprint);
            Ok(EncodedRuntimeStreamSources {
                bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    length: usize,
    maximum: usize,
}
impl Writer<'_> {
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
    fn byte(&mut self, value: u8) -> Result<(), Error> {
        self.raw(&[value])
    }
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(n)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.count(bytes.len())?;
        self.raw(bytes)
    }
    fn remaining(&self, header: usize) -> Result<usize, Error> {
        let remaining = self.maximum - self.length;
        limit("encoded bytes", header, remaining)?;
        Ok(remaining - header)
    }
}
fn gather<'a>(
    ops: &'a [StreamOp],
    counts: &Counts,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<(&'a StreamOp, usize)>, Error> {
    let mut rows = Vec::new();
    let mut pending = Vec::new();
    let check = |n: usize| -> Result<(), Error> {
        limit(
            "nodes",
            counts
                .footprint
                .nodes
                .checked_add(n)
                .ok_or(Error::Arithmetic)?,
            counts.limits.nodes as usize,
        )?;
        limit(
            "decoded bytes",
            counts
                .footprint
                .storage
                .checked_add(Counts::operation_storage(n)?)
                .ok_or(Error::Arithmetic)?,
            counts.limits.decoded_bytes,
        )
    };
    check(ops.len())?;
    pending.reserve_exact(ops.len());
    pending.extend(ops.iter().rev().map(|op| (op, 0usize)));
    while let Some((op, depth)) = pending.pop() {
        limit("depth", depth, counts.limits.depth as usize)?;
        meter.charge_work(1)?;
        rows.reserve_exact(1);
        rows.push((op, depth));
        let next = depth.checked_add(1).ok_or(Error::Arithmetic)?;
        let mut push = |body: &'a [StreamOp]| -> Result<(), Error> {
            let total = rows
                .len()
                .checked_add(pending.len())
                .and_then(|n| n.checked_add(body.len()))
                .ok_or(Error::Arithmetic)?;
            check(total)?;
            pending.reserve_exact(body.len());
            pending.extend(body.iter().rev().map(|op| (op, next)));
            Ok(())
        };
        match op {
            StreamOp::ForNext { body, .. } => push(body)?,
            StreamOp::If {
                then_ops, else_ops, ..
            } => {
                push(else_ops)?;
                push(then_ops)?;
            }
            StreamOp::Match { arms, .. } => {
                for arm in arms.iter().rev() {
                    push(&arm.ops)?;
                }
            }
            _ => {}
        }
    }
    Ok(rows)
}
struct Payload<'a> {
    bytes: Vec<u8>,
    maximum: usize,
    counts: &'a mut Counts,
    resources: &'a ValueResourceAdmission,
    meter: &'a mut TaskSemanticMeter,
    depth: usize,
}
impl Payload<'_> {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let n = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", n, self.maximum)?;
        self.bytes.reserve_exact(bytes.len());
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(n)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn frame_capacity(&self) -> Result<usize, Error> {
        let remaining = self.maximum - self.bytes.len();
        limit("encoded bytes", 4, remaining)?;
        Ok(remaining - 4)
    }
    fn expression(&mut self, expr: &RuntimeExpr) -> Result<(), Error> {
        let encoded = expr.encode_sources(
            self.counts.expression_limits(
                self.depth.checked_add(1).ok_or(Error::Arithmetic)?,
                self.frame_capacity()?,
            )?,
            self.resources,
            self.meter,
        )?;
        self.counts.child(
            encoded.node_count(),
            encoded.elements(),
            encoded.storage_bytes(),
        )?;
        self.count(encoded.bytes().len())?;
        self.raw(encoded.bytes())
    }
    fn pattern(&mut self, pattern: &RuntimePattern) -> Result<(), Error> {
        let encoded = pattern.prepare_sources_encoding(
            self.counts.pattern_limits(self.frame_capacity()?),
            self.resources,
            self.meter,
        )?;
        self.counts.child(
            encoded.node_count(),
            encoded.elements(),
            encoded.storage_bytes(),
        )?;
        let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
        encoded.append_to(&mut bytes)?;
        self.count(bytes.len())?;
        self.raw(&bytes)
    }
    fn fields(&mut self, op: &StreamOp) -> Result<usize, Error> {
        Ok(match op {
            StreamOp::Let { pattern, expr } => {
                self.pattern(pattern)?;
                self.expression(expr)?;
                0
            }
            StreamOp::ForNext {
                pattern,
                source,
                body,
            } => {
                self.pattern(pattern)?;
                self.expression(source)?;
                self.count(body.len())?;
                body.len()
            }
            StreamOp::Yield { expr } => {
                self.expression(expr)?;
                0
            }
            StreamOp::If {
                condition,
                then_ops,
                else_ops,
            } => {
                self.expression(condition)?;
                self.count(then_ops.len())?;
                self.count(else_ops.len())?;
                then_ops
                    .len()
                    .checked_add(else_ops.len())
                    .ok_or(Error::Arithmetic)?
            }
            StreamOp::Match { scrutinee, arms } => {
                self.expression(scrutinee)?;
                self.count(arms.len())?;
                self.counts.arms(arms.len())?;
                let mut children = 0usize;
                for arm in arms {
                    self.pattern(&arm.pattern)?;
                    self.raw(&[u8::from(arm.guard.is_some())])?;
                    if let Some(guard) = &arm.guard {
                        self.expression(guard)?;
                    }
                    self.count(arm.ops.len())?;
                    children = children
                        .checked_add(arm.ops.len())
                        .ok_or(Error::Arithmetic)?;
                }
                children
            }
            StreamOp::Close { source } => {
                self.expression(source)?;
                0
            }
            StreamOp::Return => 0,
        })
    }
}
fn emit(
    plan: &RuntimePlan,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    counts.rows(plan.stream_plans().len())?;
    writer.count(plan.stream_plans().len())?;
    for stream in plan.stream_plans() {
        let path = stream.id().path();
        let segments = path.segments();
        if segments.is_empty() {
            return Err(Error::Invalid {
                kind: "empty Stream path",
                offset: writer.length,
            });
        }
        counts.segments(segments.len())?;
        writer.count(segments.len())?;
        for (index, segment) in segments.iter().enumerate() {
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Stream,
                index,
                segments.len(),
                segment.as_str(),
            )?;
            counts.text(segment.as_str().len())?;
            writer.frame(segment.as_str().as_bytes())?;
        }
        for (kind, ty) in [
            ("item type", stream.item_ty()),
            ("error type", stream.error_ty()),
        ] {
            reference(kind, ty.get().get(), counts.limits.expression.type_count)?;
            writer.count(ty.get().get() as usize)?;
        }
        let rows = gather(stream.ops(), counts, meter)?;
        counts.operations(rows.len())?;
        counts.children(stream.ops().len())?;
        writer.count(stream.ops().len())?;
        writer.count(rows.len())?;
        for (op, depth) in rows {
            let maximum = writer.remaining(9)?;
            let mut payload = Payload {
                bytes: Vec::new(),
                maximum,
                counts,
                resources,
                meter,
                depth,
            };
            let children = payload.fields(op)?;
            payload.counts.children(children)?;
            writer.byte(op.semantic_tag())?;
            writer.frame(&payload.bytes)?;
            writer.count(children)?;
        }
    }
    Ok(())
}
