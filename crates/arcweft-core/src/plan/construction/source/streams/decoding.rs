//! The complete preflight precedes this temporary prefix index and seed arena.

use super::super::RuntimePlanSourceConstruction;
use super::super::expression::RuntimeExpressionSourceHandles;
use super::reading::{Cursor, Row};
use super::{Counts, Error, PreparedRuntimeStreamSources, charge};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::{RuntimePattern, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeExprSeed, RuntimePatternSeed, RuntimeStreamMatchArmSeed, RuntimeStreamOpSeed as Op,
    RuntimeStreamPlanSeed,
};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimePlanTypeId};
use crate::stream::StreamRuntimeId;
use crate::value::RuntimeExpr;

impl PreparedRuntimeStreamSources<'_> {
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            if !source.builder.stream_plans.is_empty() {
                return Err(Error::Namespace);
            }
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = Cursor::new(self.bytes);
            let mut counts = Counts::new(self.limits);
            let n = cursor.header(&mut counts)?;
            for _ in 0..n {
                let root = cursor.root(&mut counts)?;
                let mut rows = Vec::with_capacity(root.nodes);
                cursor.body(&root, &mut counts, resources, &mut source.meter, |row| {
                    rows.push(row)
                })?;
                let mut seeds = Vec::with_capacity(root.nodes);
                for row in rows.into_iter().rev() {
                    let seed = Decoder {
                        source,
                        handles,
                        resources,
                        seeds: &mut seeds,
                        limits: self.limits,
                    }
                    .operation(row)?;
                    seeds.push(seed);
                }
                let ops = (0..root.roots)
                    .map(|_| seeds.pop().expect("complete operation prefix"))
                    .collect();
                if !seeds.is_empty() {
                    return Err(cursor.invalid("remaining operation seeds"));
                }
                let mut segments = Cursor::new(root.segments);
                let mut path = Vec::with_capacity(root.segment_count);
                for index in 0..root.segment_count {
                    path.push(
                        RuntimeIdPath::preflight_stored_source_segment(
                            RuntimeIdFamily::Stream,
                            index,
                            root.segment_count,
                            segments.text()?,
                        )?
                        .materialize(),
                    );
                }
                segments.finish()?;
                let seed = RuntimeStreamPlanSeed {
                    id: StreamRuntimeId::from_source_path(
                        RuntimeIdPath::from_source_segment_storage(RuntimeIdFamily::Stream, path)?,
                    ),
                    item_ty: semantic(source, root.item),
                    error_ty: semantic(source, root.error),
                    ops,
                };
                source.builder.push_stream_plan_seed(seed)?;
            }
            cursor.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
fn semantic(source: &RuntimePlanSourceConstruction, raw: u32) -> RuntimeSemanticTypeId {
    let id = RuntimePlanTypeId::from_accepted_ordinal(
        std::num::NonZeroU32::new(raw).expect("preflighted type"),
    );
    source
        .builder
        .types
        .get(id)
        .expect("original type namespace")
        .semantic_identity()
}
struct Decoder<'a, 'b> {
    source: &'a mut RuntimePlanSourceConstruction,
    handles: &'a RuntimeExpressionSourceHandles<'b>,
    resources: &'a mut ValueResourceAdmission,
    seeds: &'a mut Vec<Op>,
    limits: super::RuntimeStreamSourceLimits,
}
impl Decoder<'_, '_> {
    fn pattern(&mut self, fields: &mut Cursor<'_>) -> Result<RuntimePatternSeed, Error> {
        let bytes = fields.frame()?;
        let counts = Counts::new(self.limits);
        Ok(RuntimePattern::preflight_sources(
            bytes,
            counts.pattern_limits(bytes.len()),
            self.resources,
            &mut self.source.meter,
        )?
        .materialize(self.source, self.resources)?)
    }
    fn expression(
        &mut self,
        fields: &mut Cursor<'_>,
        depth: usize,
    ) -> Result<RuntimeExprSeed, Error> {
        let bytes = fields.frame()?;
        let counts = Counts::new(self.limits);
        Ok(RuntimeExpr::preflight_sources(
            bytes,
            counts
                .expression_limits(depth.checked_add(1).ok_or(Error::Arithmetic)?, bytes.len())?,
            self.resources,
            &mut self.source.meter,
        )?
        .materialize(self.source, self.handles, self.resources)?)
    }
    fn children(&mut self, n: usize) -> Vec<Op> {
        (0..n)
            .map(|_| self.seeds.pop().expect("preflighted child operation"))
            .collect()
    }
    fn operation(&mut self, row: Row<'_>) -> Result<Op, Error> {
        if row.children > self.seeds.len() {
            return Err(Error::Invalid {
                kind: "operation operands",
                offset: 0,
            });
        }
        let before = self.seeds.len();
        let mut fields = Cursor::new(row.payload);
        let op = match row.tag {
            0 => Op::Let {
                pattern: self.pattern(&mut fields)?,
                expr: self.expression(&mut fields, row.depth)?,
            },
            1 => Op::ForNext {
                pattern: self.pattern(&mut fields)?,
                source: self.expression(&mut fields, row.depth)?,
                body: self.children(fields.count()?),
            },
            2 => Op::Yield {
                expr: self.expression(&mut fields, row.depth)?,
            },
            3 => {
                let condition = self.expression(&mut fields, row.depth)?;
                let left = fields.count()?;
                let right = fields.count()?;
                Op::If {
                    condition,
                    then_ops: self.children(left),
                    else_ops: self.children(right),
                }
            }
            4 => {
                let scrutinee = self.expression(&mut fields, row.depth)?;
                let n = fields.count()?;
                let mut arms = Vec::with_capacity(n);
                for _ in 0..n {
                    let pattern = self.pattern(&mut fields)?;
                    let guard = if fields.flag()? {
                        Some(self.expression(&mut fields, row.depth)?)
                    } else {
                        None
                    };
                    let ops = self.children(fields.count()?);
                    arms.push(RuntimeStreamMatchArmSeed {
                        pattern,
                        guard,
                        ops,
                    });
                }
                Op::Match { scrutinee, arms }
            }
            5 => Op::Close {
                source: self.expression(&mut fields, row.depth)?,
            },
            6 => Op::Return,
            _ => return Err(fields.invalid("operation tag")),
        };
        fields.finish()?;
        debug_assert_eq!(before - self.seeds.len(), row.children);
        Ok(op)
    }
}
