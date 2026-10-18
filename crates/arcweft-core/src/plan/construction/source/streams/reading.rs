//! Borrowed prefix rows. No operation, pattern or expression seed is retained.

use super::{Counts, DOMAIN, Error, limit, reference};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(super) struct Root<'a> {
    pub segments: &'a [u8],
    pub segment_count: usize,
    pub item: u32,
    pub error: u32,
    pub roots: usize,
    pub nodes: usize,
}
pub(super) struct Row<'a> {
    pub tag: u8,
    pub payload: &'a [u8],
    pub children: usize,
    pub depth: usize,
}
pub(super) struct Scope {
    remaining: usize,
    depth: usize,
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
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
    pub fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.invalid("flag")),
        }
    }
    pub fn header(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let n = self.count()?;
        counts.rows(n)?;
        Ok(n)
    }
    pub fn root(&mut self, counts: &mut Counts) -> Result<Root<'a>, Error> {
        let segment_count = self.count()?;
        if segment_count == 0 {
            return Err(self.invalid("empty Stream path"));
        }
        counts.segments(segment_count)?;
        let start = self.offset;
        for index in 0..segment_count {
            let text = self.text()?;
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Stream,
                index,
                segment_count,
                text,
            )?;
            counts.text(text.len())?;
        }
        let segments = &self.bytes[start..self.offset];
        let item = self.u32()?;
        let error = self.u32()?;
        reference("item type", item, counts.limits.expression.type_count)?;
        reference("error type", error, counts.limits.expression.type_count)?;
        let roots = self.count()?;
        let nodes = self.count()?;
        if roots > nodes || nodes > (self.bytes.len() - self.offset) / 9 {
            return Err(self.invalid("operation prefix size"));
        }
        counts.operations(nodes)?;
        counts.children(roots)?;
        Ok(Root {
            segments,
            segment_count,
            item,
            error,
            roots,
            nodes,
        })
    }
    pub fn body(
        &mut self,
        root: &Root<'_>,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
        mut retain: impl FnMut(Row<'a>),
    ) -> Result<(), Error> {
        if root.nodes == 0 {
            if root.roots != 0 {
                return Err(self.invalid("empty operation prefix"));
            }
            return Ok(());
        }
        let mut scopes = Vec::with_capacity(root.nodes);
        scopes.push(Scope {
            remaining: root.roots,
            depth: 0,
        });
        let mut remaining = root.roots;
        for index in 0..root.nodes {
            while scopes.last().is_some_and(|scope| scope.remaining == 0) {
                scopes.pop();
            }
            let scope = scopes
                .last_mut()
                .ok_or_else(|| self.invalid("extra operation root"))?;
            scope.remaining -= 1;
            let depth = scope.depth;
            limit("depth", depth, counts.limits.depth as usize)?;
            let tag = self.byte()?;
            let payload = self.frame()?;
            let children = self.count()?;
            let mut fields = Cursor::new(payload);
            let expected = fields.fields(tag, depth, counts, resources, meter)?;
            fields.finish()?;
            if expected != children {
                return Err(self.invalid("operation child count"));
            }
            remaining = remaining
                .checked_sub(1)
                .and_then(|n| n.checked_add(children))
                .ok_or(Error::Arithmetic)?;
            if remaining > root.nodes - index - 1 {
                return Err(self.invalid("unfilled operation prefix"));
            }
            counts.children(children)?;
            retain(Row {
                tag,
                payload,
                children,
                depth,
            });
            if children != 0 {
                scopes.push(Scope {
                    remaining: children,
                    depth: depth.checked_add(1).ok_or(Error::Arithmetic)?,
                });
            }
        }
        if remaining != 0 {
            return Err(self.invalid("incomplete operation prefix"));
        }
        Ok(())
    }
    fn pattern(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let bytes = self.frame()?;
        let prepared = RuntimePattern::preflight_sources(
            bytes,
            counts.pattern_limits(bytes.len()),
            resources,
            meter,
        )?;
        counts.child(
            prepared.node_count(),
            prepared.elements(),
            prepared.storage_bytes(),
        )
    }
    fn expression(
        &mut self,
        depth: usize,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let bytes = self.frame()?;
        let prepared = RuntimeExpr::preflight_sources(
            bytes,
            counts
                .expression_limits(depth.checked_add(1).ok_or(Error::Arithmetic)?, bytes.len())?,
            resources,
            meter,
        )?;
        counts.child(
            prepared.node_count(),
            prepared.elements(),
            prepared.storage_bytes(),
        )
    }
    fn fields(
        &mut self,
        tag: u8,
        depth: usize,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<usize, Error> {
        Ok(match tag {
            0 => {
                self.pattern(counts, resources, meter)?;
                self.expression(depth, counts, resources, meter)?;
                0
            }
            1 => {
                self.pattern(counts, resources, meter)?;
                self.expression(depth, counts, resources, meter)?;
                self.count()?
            }
            2 | 5 => {
                self.expression(depth, counts, resources, meter)?;
                0
            }
            3 => {
                self.expression(depth, counts, resources, meter)?;
                self.count()?
                    .checked_add(self.count()?)
                    .ok_or(Error::Arithmetic)?
            }
            4 => {
                self.expression(depth, counts, resources, meter)?;
                let n = self.count()?;
                counts.arms(n)?;
                let mut children = 0usize;
                for _ in 0..n {
                    self.pattern(counts, resources, meter)?;
                    if self.flag()? {
                        self.expression(depth, counts, resources, meter)?;
                    }
                    children = children
                        .checked_add(self.count()?)
                        .ok_or(Error::Arithmetic)?;
                }
                children
            }
            6 => 0,
            _ => return Err(self.invalid("operation tag")),
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
