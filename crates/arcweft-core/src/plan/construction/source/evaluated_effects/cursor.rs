//! Borrowed complete framing, with no seed or executable effect retention.

use super::registry_key::RegistryKeySource;
use super::{Counts, DOMAIN, Error};
use crate::effect::{RuntimeAssertionGuardId, RuntimeAssertionProfile};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::task::semantic::TaskSemanticMeter;

pub(super) enum Kind<'a> {
    Log(&'a str),
    Signal,
    Metric,
    Registry(RegistryKeySource<'a>),
    Event,
    Panic,
    Fail,
    Bail,
    Ensure,
    Drop(u8),
    Assert {
        guard: RuntimeAssertionGuardId,
        profile: RuntimeAssertionProfile,
        message: &'a str,
    },
}
pub(super) struct Row<'a> {
    pub kind: Kind<'a>,
    pub first: &'a [u8],
    pub second: Option<&'a [u8]>,
    pub fields: &'a [u8],
    pub field_count: usize,
}
impl Row<'_> {
    pub fn preflight(
        &self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        match &self.kind {
            Kind::Log(level) => counts.storage(level.len())?,
            Kind::Assert { message, .. } => counts.storage(message.len())?,
            Kind::Registry(key) => key.preflight(counts)?,
            _ => {}
        }
        counts.preflight_expression(self.first, resources, meter)?;
        if let Some(second) = self.second {
            counts.preflight_expression(second, resources, meter)?;
        }
        counts.fields(self.field_count)?;
        let mut fields = Cursor::new(self.fields);
        for _ in 0..self.field_count {
            counts.storage(fields.text()?.len())?;
            counts.preflight_expression(fields.frame()?, resources, meter)?;
        }
        fields.finish()
    }
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn invalid(&self, kind: &'static str) -> Error {
        Error::Invalid {
            kind,
            offset: self.offset,
        }
    }
    fn raw(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let result = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    pub(super) fn count(&mut self) -> Result<usize, Error> {
        Ok(u32::from_le_bytes(self.raw(4)?.try_into().expect("source u32 width")) as usize)
    }
    pub fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.count()?;
        self.raw(n)
    }
    pub fn text(&mut self) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.frame()?)?)
    }
    fn fields(&mut self) -> Result<(&'a [u8], usize), Error> {
        let count = self.count()?;
        let start = self.offset;
        for _ in 0..count {
            self.text()?;
            self.frame()?;
        }
        Ok((&self.bytes[start..self.offset], count))
    }
    pub fn row(&mut self) -> Result<Row<'a>, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let mut second = None;
        let mut fields = &self.bytes[0..0];
        let mut field_count = 0;
        let (kind, first) = match self.byte()? {
            0 => {
                let level = self.text()?;
                let first = self.frame()?;
                (fields, field_count) = self.fields()?;
                (Kind::Log(level), first)
            }
            tag @ (1 | 2) => {
                let first = self.frame()?;
                second = Some(self.frame()?);
                (if tag == 1 { Kind::Signal } else { Kind::Metric }, first)
            }
            3 => {
                let first = self.frame()?;
                (fields, field_count) = self.fields()?;
                (Kind::Event, first)
            }
            4 => (Kind::Panic, self.frame()?),
            5 => (Kind::Fail, self.frame()?),
            6 => (Kind::Bail, self.frame()?),
            7 => {
                let first = self.frame()?;
                second = Some(self.frame()?);
                (Kind::Ensure, first)
            }
            8 => {
                let first = self.frame()?;
                let policy = self.byte()?;
                if policy > 5 {
                    return Err(self.invalid("drop policy"));
                }
                if policy == 2 {
                    second = Some(self.frame()?);
                }
                (Kind::Drop(policy), first)
            }
            9 => {
                let guard = RuntimeAssertionGuardId::try_from_bytes(
                    self.raw(16)?.try_into().expect("source guard width"),
                )?;
                let profile = match self.byte()? {
                    0 => RuntimeAssertionProfile::Always,
                    1 => RuntimeAssertionProfile::DebugOnly,
                    _ => return Err(self.invalid("assertion profile")),
                };
                let message = self.text()?;
                (
                    Kind::Assert {
                        guard,
                        profile,
                        message,
                    },
                    self.frame()?,
                )
            }
            10 => (
                Kind::Registry(RegistryKeySource::new(self.frame()?)),
                self.frame()?,
            ),
            _ => return Err(self.invalid("effect tag")),
        };
        Ok(Row {
            kind,
            first,
            second,
            fields,
            field_count,
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
