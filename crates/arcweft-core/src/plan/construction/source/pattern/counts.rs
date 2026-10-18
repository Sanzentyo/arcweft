//! Retained seed/final storage and each actual scratch phase share one cap.

use super::{
    Footprint, Kind, PrefixScope, Row, RuntimePatternSourceError as Error,
    RuntimePatternSourceLimits, WireRest, limit,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::{
    MAX_RUNTIME_PATTERN_BINDING_DEPTH, RuntimePattern, RuntimePatternBindingStep,
    RuntimeRecordPatternField,
};
use crate::plan::{RuntimePatternSeed, RuntimeRecordPatternFieldSeed};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::AwbcRuntimeValueSnapshot;

pub(super) struct Counts {
    limits: RuntimePatternSourceLimits,
    pub(super) footprint: Footprint,
}

impl Counts {
    pub(super) fn new(limits: RuntimePatternSourceLimits, nodes: usize) -> Result<Self, Error> {
        limit("pattern nodes", nodes, limits.nodes as usize)?;
        if nodes == 0 {
            return Err(Error::Roots { actual: 0 });
        }
        let mut counts = Self {
            limits,
            footprint: Footprint {
                nodes,
                ..Footprint::default()
            },
        };
        counts.elements(nodes)?;
        counts.check()?;
        Ok(counts)
    }
    fn storage(&mut self, bytes: usize) -> Result<(), Error> {
        self.footprint.retained = self
            .footprint
            .retained
            .checked_add(bytes)
            .ok_or(Error::Arithmetic)?;
        self.check()
    }
    fn elements(&mut self, count: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "source elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )
    }
    pub(super) fn binding(&mut self, depth: usize) -> Result<(), Error> {
        let depth = depth.max(1);
        limit("binding depth", depth, MAX_RUNTIME_PATTERN_BINDING_DEPTH)?;
        self.footprint.bindings = self
            .footprint
            .bindings
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.footprint.binding_steps = self
            .footprint
            .binding_steps
            .checked_add(depth)
            .ok_or(Error::Arithmetic)?;
        self.elements(depth)?;
        self.storage(
            depth
                .checked_mul(size_of::<RuntimePatternBindingStep>())
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn rest(&mut self, rest: &WireRest, depth: usize) -> Result<(), Error> {
        if matches!(rest, WireRest::Bind(_)) {
            self.binding(depth.checked_add(1).ok_or(Error::Arithmetic)?)?;
        }
        Ok(())
    }
    pub(super) fn children(&mut self, count: usize, record: bool) -> Result<(), Error> {
        self.elements(count)?;
        let width = if record {
            self.footprint.record_fields = self
                .footprint
                .record_fields
                .checked_add(count)
                .ok_or(Error::Arithmetic)?;
            size_of::<RuntimeRecordPatternFieldSeed>() + size_of::<RuntimeRecordPatternField>()
        } else {
            size_of::<RuntimePatternSeed>() + size_of::<RuntimePattern>()
        };
        self.storage(count.checked_mul(width).ok_or(Error::Arithmetic)?)
    }
    pub(super) fn row(
        &mut self,
        row: &Row<'_>,
        depth: usize,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        match &row.kind {
            Kind::Bind { .. } | Kind::Typed(_) => self.binding(depth)?,
            Kind::Discard => {}
            Kind::Literal(bytes) | Kind::Entity(bytes) => {
                let literal = AwbcRuntimeValueSnapshot::preflight_literal_source(
                    bytes,
                    self.limits.literal(),
                    resources,
                    meter,
                )?;
                self.literal(literal.storage_bytes(), literal.elements())?;
            }
            Kind::Tuple(count) | Kind::Or(count) => self.children(*count, false)?,
            Kind::Record { fields, rest } => {
                self.children(fields.len() / 4, true)?;
                self.rest(rest, depth)?;
            }
            Kind::Sequence { items, rest } => {
                self.children(*items, false)?;
                self.rest(rest, depth)?;
            }
            Kind::Variant { payload, .. } => self.children(usize::from(*payload), false)?,
            Kind::Whole(_) => {
                self.binding(depth)?;
                self.children(1, false)?;
            }
        }
        self.check()
    }
    pub(super) fn literal(&mut self, storage: usize, elements: usize) -> Result<(), Error> {
        self.footprint.literals = self
            .footprint
            .literals
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.elements(elements)?;
        // The consumed inert graph and its admitted live projection can coexist.
        self.storage(storage.checked_mul(2).ok_or(Error::Arithmetic)?)
    }
    pub(super) fn storage_bytes(&self) -> Result<usize, Error> {
        let f = self.footprint;
        let prefix = f
            .nodes
            .checked_mul(size_of::<PrefixScope>())
            .ok_or(Error::Arithmetic)?;
        let decoding = f
            .nodes
            .checked_mul(size_of::<usize>() + size_of::<RuntimePatternSeed>())
            .and_then(|n| {
                f.record_fields
                    .checked_mul(size_of::<u32>())
                    .and_then(|fields| n.checked_add(fields))
            })
            .ok_or(Error::Arithmetic)?;
        let encoding =
            super::encoding::scratch_bytes(f.nodes, f.literals).ok_or(Error::Arithmetic)?;
        let declarations =
            RuntimePattern::binding_declaration_scratch_bytes(f.nodes).ok_or(Error::Arithmetic)?;
        let lowering = crate::plan::construction::lower::RuntimePlanBodyConstruction::pattern_source_scratch_bytes(
            f.nodes, f.bindings, f.record_fields,
        ).ok_or(Error::Arithmetic)?;
        f.retained
            .checked_add(
                prefix
                    .max(decoding)
                    .max(encoding)
                    .max(lowering)
                    .max(declarations),
            )
            .ok_or(Error::Arithmetic)
    }
    pub(super) fn check(&self) -> Result<(), Error> {
        limit(
            "decoded bytes",
            self.storage_bytes()?,
            self.limits.decoded_bytes,
        )
    }
}
