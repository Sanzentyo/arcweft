//! Borrowed complete framing, with no seed or executable effect retention.

use super::registry_key::RegistryKeySource;
use super::{Counts, DOMAIN, Error};
use crate::effect::{
    RuntimeAssertionGuardId, RuntimeAssertionProfile, RuntimeDropPolicyKind,
    RuntimeRegistryReadKind,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimeSemanticTypeId;
use crate::task::semantic::TaskSemanticMeter;

pub(super) enum Kind<'a> {
    Wait,
    Log(&'a str),
    Signal,
    Metric,
    Registry(RegistryKeySource<'a>),
    RegistryDrop {
        key: RegistryKeySource<'a>,
        presence: RuntimeRegistryReadKind,
        item_type: RuntimeSemanticTypeId,
        policy: RuntimeDropPolicyKind,
    },
    Event,
    Action,
    Panic,
    Fail,
    Bail,
    Ensure,
    Drop(RuntimeDropPolicyKind),
    Assert {
        guard: RuntimeAssertionGuardId,
        profile: RuntimeAssertionProfile,
        message: &'a str,
    },
}
pub(super) struct Row<'a> {
    pub kind: Kind<'a>,
    pub first: Option<&'a [u8]>,
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
            Kind::Registry(key) | Kind::RegistryDrop { key, .. } => key.preflight(counts)?,
            _ => {}
        }
        if let Some(first) = self.first {
            counts.preflight_expression(first, resources, meter)?;
        }
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
    fn drop_policy(&mut self) -> Result<(RuntimeDropPolicyKind, Option<&'a [u8]>), Error> {
        let policy = RuntimeDropPolicyKind::from_wire_tag(self.byte()?)
            .ok_or_else(|| self.invalid("drop policy"))?;
        let fade = (policy == RuntimeDropPolicyKind::Stop)
            .then(|| self.frame())
            .transpose()?;
        Ok((policy, fade))
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
                (Kind::Log(level), Some(first))
            }
            tag @ (1 | 2) => {
                let first = self.frame()?;
                second = Some(self.frame()?);
                (
                    if tag == 1 { Kind::Signal } else { Kind::Metric },
                    Some(first),
                )
            }
            3 => {
                let first = self.frame()?;
                (fields, field_count) = self.fields()?;
                (Kind::Event, Some(first))
            }
            12 => (Kind::Action, Some(self.frame()?)),
            4 => (Kind::Panic, Some(self.frame()?)),
            5 => (Kind::Fail, Some(self.frame()?)),
            6 => (Kind::Bail, Some(self.frame()?)),
            7 => {
                let first = self.frame()?;
                second = Some(self.frame()?);
                (Kind::Ensure, Some(first))
            }
            8 => {
                let first = self.frame()?;
                let policy;
                (policy, second) = self.drop_policy()?;
                (Kind::Drop(policy), Some(first))
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
                    Some(self.frame()?),
                )
            }
            10 => (
                Kind::Registry(RegistryKeySource::new(self.frame()?)),
                Some(self.frame()?),
            ),
            11 => (Kind::Wait, Some(self.frame()?)),
            13 => {
                let key = RegistryKeySource::new(self.frame()?);
                let presence = match self.byte()? {
                    0 => RuntimeRegistryReadKind::Required,
                    1 => RuntimeRegistryReadKind::Optional,
                    _ => return Err(self.invalid("registry drop presence")),
                };
                let item_type = RuntimeSemanticTypeId::from_bytes(
                    self.raw(32)?
                        .try_into()
                        .expect("source semantic type width"),
                );
                let (policy, fade) = self.drop_policy()?;
                (
                    Kind::RegistryDrop {
                        key,
                        presence,
                        item_type,
                        policy,
                    },
                    fade,
                )
            }
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
