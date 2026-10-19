use super::super::flow::PreparedRuntimeFlowSource;
use super::{
    BodyOwner, Counts, DOMAIN, Error, Footprint, Namespaces, RuntimeFunctionBodySourceLimits,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(super) struct Row<'a> {
    pub owner: BodyOwner,
    pub definition: [u8; 32],
    pub kind: RuntimeFunctionSiteBodyKind,
    pub effects: &'a [u8],
    pub payload: &'a [u8],
    pub prefix: Footprint,
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("Source u32 width"),
        ))
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let count = self.u32()? as usize;
        self.raw(count)
    }
    pub fn text(&mut self) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.frame()?)?)
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Trailing {
                offset: self.offset,
            })
        }
    }
    pub fn header(&mut self) -> Result<Namespaces, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        Ok(Namespaces {
            functions: self.u32()? as usize,
            helpers: self.u32()? as usize,
            methods: self.u32()? as usize,
        })
    }
    pub fn row(
        &mut self,
        owner: BodyOwner,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Row<'a>, Error> {
        let actual = self.u32()?;
        let expected = u32::try_from(owner.index().checked_add(1).ok_or(Error::Arithmetic)?)
            .map_err(|_| Error::Arithmetic)?;
        if actual != expected {
            return Err(Error::Ordinal { actual, expected });
        }
        let definition = self.raw(32)?.try_into().expect("definition identity width");
        let kind = match self.byte()? {
            0 => RuntimeFunctionSiteBodyKind::Expression,
            1 => RuntimeFunctionSiteBodyKind::Executable,
            tag => return Err(Error::Tag { kind: "body", tag }),
        };
        if !matches!(owner, BodyOwner::Function(_))
            && kind != RuntimeFunctionSiteBodyKind::Expression
        {
            return Err(Error::PureBodyKind {
                owner: owner.label(),
            });
        }
        let start = self.offset;
        let count = self.u32()? as usize;
        counts.effects(count, 0)?;
        if kind == RuntimeFunctionSiteBodyKind::Expression && count != 0 {
            return Err(Error::ExpressionEffects);
        }
        let mut previous = None;
        for _ in 0..count {
            let effect = self.text()?;
            counts.effects(0, effect.len())?;
            arcweft_id::EffectId::validate_source(effect)?;
            if previous.is_some_and(|previous| previous >= effect) {
                return Err(Error::EffectOrder);
            }
            previous = Some(effect);
        }
        let effects = &self.bytes[start..self.offset];
        let payload = self.frame()?;
        let prefix = counts.footprint;
        match kind {
            RuntimeFunctionSiteBodyKind::Expression => {
                let child = RuntimeExpr::preflight_sources(
                    payload,
                    counts.expression(payload.len()),
                    resources,
                    meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
            }
            RuntimeFunctionSiteBodyKind::Executable => {
                let child = PreparedRuntimeFlowSource::preflight(
                    payload,
                    counts.flow(payload.len()),
                    resources,
                    meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
            }
        }
        Ok(Row {
            owner,
            definition,
            kind,
            effects,
            payload,
            prefix,
        })
    }
}
pub(super) fn inspect<'a>(
    bytes: &'a [u8],
    limits: RuntimeFunctionBodySourceLimits,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
    mut visit: impl FnMut(Row<'a>),
) -> Result<(usize, Footprint), Error> {
    let mut cursor = Cursor::new(bytes);
    let namespaces = cursor.header()?;
    let mut counts = Counts::new(limits, namespaces)?;
    for owner in namespaces.owners() {
        visit(cursor.row(owner, &mut counts, resources, meter)?);
    }
    cursor.finish()?;
    Ok((namespaces.total()?, counts.footprint))
}
