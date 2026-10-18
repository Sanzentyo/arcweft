//! Borrowed framing. Physical operand indices are zero based; type/state
//! references use their owning one-based namespaces.

use super::{DOMAIN, Error, RuntimeProjectCallSourceLimits, reference};
use crate::plan::RuntimeProjectCallAttachedPresenceSeed as Presence;
use crate::value::RuntimeCallArgumentMode;

pub(super) enum Ordinary<'a> {
    Fixed {
        parameter: u32,
        abi: u32,
        binding: u32,
        source: u32,
    },
    Rest {
        parameter: u32,
        abi: u32,
        binding: u32,
        sources: &'a [u8],
    },
}
pub(super) struct Attached {
    pub abi: u32,
    pub binding: u32,
    pub presence: Presence,
    pub source: Option<u32>,
}
pub(super) struct Row<'a> {
    pub state: u32,
    pub group: u32,
    pub callee: &'a [u8],
    pub operands: &'a [u8],
    pub operand_count: usize,
    pub ordinary: &'a [u8],
    pub ordinary_count: usize,
    pub attached: Option<Attached>,
    pub result: &'a [u8],
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeProjectCallSourceLimits,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8], limits: RuntimeProjectCallSourceLimits) -> Self {
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
    fn raw(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let result = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(result)
    }
    pub fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    pub fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.u32()? as usize;
        self.raw(n)
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.invalid("flag")),
        }
    }
    fn ty(&mut self) -> Result<u32, Error> {
        let id = self.u32()?;
        reference("type", id, self.limits.expression.type_count)?;
        Ok(id)
    }
    fn source(&mut self, n: usize) -> Result<u32, Error> {
        let index = self.u32()?;
        if index as usize >= n {
            return Err(Error::Reference {
                kind: "physical operand",
                actual: index,
                maximum: n,
            });
        }
        Ok(index)
    }
    pub fn operand(&mut self) -> Result<(RuntimeCallArgumentMode, &'a [u8]), Error> {
        let mode = match self.byte()? {
            0 => RuntimeCallArgumentMode::Value,
            1 => RuntimeCallArgumentMode::Spread,
            _ => return Err(self.invalid("argument mode")),
        };
        Ok((mode, self.frame()?))
    }
    pub fn ordinary(&mut self, index: usize, n: usize) -> Result<Ordinary<'a>, Error> {
        let parameter = self.u32()?;
        if parameter as usize != index {
            return Err(self.invalid("logical parameter ordinal"));
        }
        let abi = self.ty()?;
        let binding = self.ty()?;
        match self.byte()? {
            0 => Ok(Ordinary::Fixed {
                parameter,
                abi,
                binding,
                source: self.source(n)?,
            }),
            1 => {
                let count = self.u32()? as usize;
                let start = self.offset;
                let mut previous = None;
                for _ in 0..count {
                    let source = self.source(n)?;
                    if previous.is_some_and(|p| p >= source) {
                        return Err(self.invalid("rest source order"));
                    }
                    previous = Some(source);
                }
                Ok(Ordinary::Rest {
                    parameter,
                    abi,
                    binding,
                    sources: &self.bytes[start..self.offset],
                })
            }
            _ => Err(self.invalid("logical materialization")),
        }
    }
    fn attached(&mut self, n: usize) -> Result<Option<Attached>, Error> {
        if !self.flag()? {
            return Ok(None);
        }
        let abi = self.ty()?;
        let binding = self.ty()?;
        let presence = match self.byte()? {
            0 => Presence::RequiredPresent,
            1 => Presence::OptionalPresent,
            2 => Presence::OptionalOmitted,
            3 => Presence::DefaultedPresent,
            4 => Presence::DefaultedOmitted,
            _ => return Err(self.invalid("attached presence")),
        };
        let source = if self.flag()? {
            Some(self.source(n)?)
        } else {
            None
        };
        let present = matches!(
            presence,
            Presence::RequiredPresent | Presence::OptionalPresent | Presence::DefaultedPresent
        );
        if present != source.is_some() {
            return Err(self.invalid("attached source presence"));
        }
        Ok(Some(Attached {
            abi,
            binding,
            presence,
            source,
        }))
    }
    pub fn row(&mut self) -> Result<Row<'a>, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let state = self.u32()?;
        reference("callable state", state, self.limits.expression.state_count)?;
        let group = self.u32()?;
        let callee = self.frame()?;
        let operand_count = self.u32()? as usize;
        let start = self.offset;
        for _ in 0..operand_count {
            self.operand()?;
        }
        let operands = &self.bytes[start..self.offset];
        let ordinary_count = self.u32()? as usize;
        let start = self.offset;
        for index in 0..ordinary_count {
            self.ordinary(index, operand_count)?;
        }
        let ordinary = &self.bytes[start..self.offset];
        let attached = self.attached(operand_count)?;
        let result = self.frame()?;
        Ok(Row {
            state,
            group,
            callee,
            operands,
            operand_count,
            ordinary,
            ordinary_count,
            attached,
            result,
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
