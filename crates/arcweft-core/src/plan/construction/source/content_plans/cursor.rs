//! Allocation-free application metadata; child expression frames stay borrowed.

use super::{Counts, DOMAIN, Error, RuntimeContentPlanSourceLimits, reference};
use crate::entry::{
    RuntimeDialogueContentTemplateDigest, schema::value_encoding::ValueResourceAdmission,
};
use crate::plan::{RuntimeDialogueContentTemplateKey, RuntimeDialogueValueRole, RuntimeLineId};
use crate::runtime_id::{
    RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteId, RuntimeDialogueValueSlotId,
    RuntimeIdFamily, RuntimeIdPath,
};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(super) struct Captures<'a> {
    pub bytes: &'a [u8],
    pub count: usize,
}
impl Captures<'_> {
    fn preflight(
        &self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        counts.captures(self.count)?;
        let mut cursor = Cursor::new(self.bytes, counts.limits);
        for _ in 0..self.count {
            let bytes = cursor.frame()?;
            let child = RuntimeExpr::preflight_sources(
                bytes,
                counts.child_limits(bytes.len()),
                resources,
                meter,
            )?;
            counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        }
        cursor.finish()
    }
}
pub(super) struct Value<'a> {
    pub slot: RuntimeDialogueValueSlotId,
    pub role: RuntimeDialogueValueRole,
    pub function: u32,
    pub captures: Captures<'a>,
}
pub(super) struct Effect<'a> {
    pub site: RuntimeDialogueEffectSiteId,
    pub state: u32,
    pub captures: Captures<'a>,
}
pub(super) struct Line<'a> {
    segments: &'a [u8],
    count: usize,
}
impl Line<'_> {
    pub fn materialize(
        &self,
        limits: RuntimeContentPlanSourceLimits,
    ) -> Result<RuntimeLineId, Error> {
        let mut cursor = Cursor::new(self.segments, limits);
        let mut segments = Vec::with_capacity(self.count);
        for index in 0..self.count {
            segments.push(
                RuntimeIdPath::preflight_stored_source_segment(
                    RuntimeIdFamily::Line,
                    index,
                    self.count,
                    cursor.text()?,
                )?
                .materialize(),
            );
        }
        cursor.finish()?;
        Ok(RuntimeLineId::from_source_path(
            RuntimeIdPath::from_source_segment_storage(RuntimeIdFamily::Line, segments)?,
        ))
    }
}
pub(super) struct Row<'a> {
    pub line: Line<'a>,
    pub template: RuntimeDialogueContentTemplateKey,
    pub values: &'a [u8],
    pub value_count: usize,
    pub effects: &'a [u8],
    pub effect_count: usize,
    pub marks: &'a [u8],
    pub mark_count: usize,
    pub group: Option<u32>,
}
impl Row<'_> {
    pub fn preflight(
        &self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        counts.line(self.line.count)?;
        let mut line = Cursor::new(self.line.segments, counts.limits);
        for _ in 0..self.line.count {
            counts.text(line.text()?.len(), 3)?;
        }
        line.finish()?;
        counts.values(self.value_count)?;
        let mut values = Cursor::new(self.values, counts.limits);
        for index in 0..self.value_count {
            values
                .value(index)?
                .captures
                .preflight(counts, resources, meter)?;
        }
        values.finish()?;
        counts.effects(self.effect_count)?;
        let mut effects = Cursor::new(self.effects, counts.limits);
        for index in 0..self.effect_count {
            effects
                .effect(index)?
                .captures
                .preflight(counts, resources, meter)?;
        }
        effects.finish()?;
        counts.marks(self.mark_count)?;
        let mut marks = Cursor::new(self.marks, counts.limits);
        for index in 0..self.mark_count {
            counts.text(marks.mark(index)?.len(), 1)?;
        }
        marks.finish()
    }
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeContentPlanSourceLimits,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8], limits: RuntimeContentPlanSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    pub fn invalid(&self, kind: &'static str) -> Error {
        Error::Invalid {
            kind,
            offset: self.offset,
        }
    }
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    pub fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    pub fn count(&mut self) -> Result<usize, Error> {
        Ok(self.u32()? as usize)
    }
    pub fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.count()?;
        self.raw(n)
    }
    pub fn text(&mut self) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.frame()?)?)
    }
    fn ordinal(&mut self, expected: usize) -> Result<(), Error> {
        if self.count()? != expected.checked_add(1).ok_or(Error::Arithmetic)? {
            return Err(self.invalid("ordinal"));
        }
        Ok(())
    }
    pub fn header(&mut self) -> Result<usize, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let n = self.count()?;
        super::limit("rows", n, self.limits.rows as usize)?;
        Ok(n)
    }
    fn captures(&mut self) -> Result<Captures<'a>, Error> {
        let count = self.count()?;
        let start = self.offset;
        for _ in 0..count {
            self.frame()?;
        }
        Ok(Captures {
            bytes: &self.bytes[start..self.offset],
            count,
        })
    }
    pub fn value(&mut self, index: usize) -> Result<Value<'a>, Error> {
        self.ordinal(index)?;
        let slot = RuntimeDialogueValueSlotId::from_zero_based(index).ok_or(Error::Arithmetic)?;
        let role = RuntimeDialogueValueRole::from_encoded(self.byte()?)
            .ok_or_else(|| self.invalid("value role"))?;
        let function = self.u32()?;
        reference("function", function, self.limits.function_count)?;
        Ok(Value {
            slot,
            role,
            function,
            captures: self.captures()?,
        })
    }
    pub fn effect(&mut self, index: usize) -> Result<Effect<'a>, Error> {
        self.ordinal(index)?;
        let site = RuntimeDialogueEffectSiteId::from_zero_based(index).ok_or(Error::Arithmetic)?;
        let state = self.u32()?;
        reference("state", state, self.limits.expression.state_count)?;
        Ok(Effect {
            site,
            state,
            captures: self.captures()?,
        })
    }
    pub fn mark(&mut self, index: usize) -> Result<&'a str, Error> {
        self.ordinal(index)?;
        self.text()
    }
    pub fn row(&mut self) -> Result<Row<'a>, Error> {
        let count = self.count()?;
        if count == 0 {
            return Err(self.invalid("empty Line path"));
        }
        let start = self.offset;
        for index in 0..count {
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Line,
                index,
                count,
                self.text()?,
            )?;
        }
        let line = Line {
            segments: &self.bytes[start..self.offset],
            count,
        };
        let id = self.u32()?;
        reference("template", id, self.limits.expression.template_count)?;
        let id = RuntimeDialogueContentTemplateId::from_nonzero(
            std::num::NonZeroU32::new(id).expect("checked template"),
        );
        let template = RuntimeDialogueContentTemplateKey::new(
            id,
            RuntimeDialogueContentTemplateDigest::from_bytes(
                self.raw(32)?.try_into().expect("source digest width"),
            ),
        );
        let value_count = self.count()?;
        let start = self.offset;
        for index in 0..value_count {
            self.value(index)?;
        }
        let values = &self.bytes[start..self.offset];
        let effect_count = self.count()?;
        let start = self.offset;
        for index in 0..effect_count {
            self.effect(index)?;
        }
        let effects = &self.bytes[start..self.offset];
        let mark_count = self.count()?;
        let start = self.offset;
        for index in 0..mark_count {
            self.mark(index)?;
        }
        let marks = &self.bytes[start..self.offset];
        if self.count()? != effect_count {
            return Err(self.invalid("effect site count"));
        }
        let group = match self.byte()? {
            0 => None,
            1 => {
                let id = self.u32()?;
                reference("Line group", id, self.limits.group_count)?;
                Some(id)
            }
            _ => return Err(self.invalid("group flag")),
        };
        Ok(Row {
            line,
            template,
            values,
            value_count,
            effects,
            effect_count,
            marks,
            mark_count,
            group,
        })
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
