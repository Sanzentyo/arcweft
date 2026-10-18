//! Borrowed Host metadata and source-ordered argument frames, without values.

use super::{Counts, DOMAIN, Error};
use crate::step::{HostCallContractDigest, RuntimeHostCallMode};
use crate::task::{NeedProducerContractDigest, NeedProducerSiteDigest, RuntimeRequestRoleIdentity};

pub(super) struct Row<'a> {
    pub task: u32,
    pub producer_contract: NeedProducerContractDigest,
    pub producer_site: NeedProducerSiteDigest,
    pub public_id: &'a str,
    pub capability: &'a str,
    pub operation: &'a str,
    pub contract: Option<HostCallContractDigest>,
    pub result: u32,
    pub mode: RuntimeHostCallMode,
    pub deterministic: bool,
    pub binding: Option<&'a [u8]>,
    pub arguments: &'a [u8],
    pub argument_count: usize,
}
pub(super) enum ArgumentKind<'a> {
    Positional,
    Named(&'a str),
    Spread,
}
pub(super) struct Argument<'a> {
    pub identity: RuntimeRequestRoleIdentity,
    pub kind: ArgumentKind<'a>,
    pub value: &'a [u8],
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
    fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn flag(&mut self, kind: &'static str) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.invalid(kind)),
        }
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn count(&mut self) -> Result<usize, Error> {
        usize::try_from(self.u32()?).map_err(|_| Error::Arithmetic)
    }
    fn digest(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.raw(32)?.try_into().expect("source identity width"))
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let count = self.count()?;
        self.raw(count)
    }
    fn text(&mut self) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.frame()?)?)
    }
    pub fn argument(&mut self) -> Result<Argument<'a>, Error> {
        let identity = RuntimeRequestRoleIdentity::from_accepted_identity(self.digest()?);
        let kind = match self.byte()? {
            0 => ArgumentKind::Positional,
            1 => ArgumentKind::Named(self.text()?),
            2 => ArgumentKind::Spread,
            _ => return Err(self.invalid("argument kind")),
        };
        Ok(Argument {
            identity,
            kind,
            value: self.frame()?,
        })
    }
    pub fn row(&mut self, counts: &mut Counts) -> Result<Row<'a>, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let task = self.u32()?;
        if usize::try_from(task).map_err(|_| Error::Arithmetic)? >= counts.limits.task_count {
            return Err(Error::Reference {
                kind: "task",
                actual: task,
                maximum: counts.limits.task_count,
            });
        }
        let producer_contract = NeedProducerContractDigest::from_bytes(self.digest()?);
        let producer_site = NeedProducerSiteDigest::from_bytes(self.digest()?);
        let public_id = self.text()?;
        let capability = self.text()?;
        let operation = self.text()?;
        for text in [public_id, capability, operation] {
            counts.text(text.len())?;
        }
        let contract = if self.flag("optional host contract")? {
            Some(HostCallContractDigest::from_bytes(self.digest()?))
        } else {
            None
        };
        let result = self.u32()?;
        if result == 0
            || usize::try_from(result).map_err(|_| Error::Arithmetic)?
                > counts.limits.expression.type_count
        {
            return Err(Error::Reference {
                kind: "result type",
                actual: result,
                maximum: counts.limits.expression.type_count,
            });
        }
        let mode = match self.byte()? {
            0 => RuntimeHostCallMode::Immediate,
            1 => RuntimeHostCallMode::Suspend,
            _ => return Err(self.invalid("host mode")),
        };
        let deterministic = self.flag("determinism")?;
        let binding = if self.flag("optional binding")? {
            Some(self.frame()?)
        } else {
            None
        };
        let argument_count = self.count()?;
        counts.array(
            argument_count,
            size_of::<crate::plan::RuntimeHostArgumentSeed>()
                + size_of::<crate::task::RuntimeHostArgumentTemplate>(),
        )?;
        let start = self.offset;
        for _ in 0..argument_count {
            let argument = self.argument()?;
            if let ArgumentKind::Named(name) = argument.kind {
                counts.text(name.len())?;
            }
        }
        Ok(Row {
            task,
            producer_contract,
            producer_site,
            public_id,
            capability,
            operation,
            contract,
            result,
            mode,
            deterministic,
            binding,
            arguments: &self.bytes[start..self.offset],
            argument_count,
        })
    }
    pub fn finish(self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Trailing {
                offset: self.offset,
            })
        }
    }
}
