//! Borrowed signature rows; all scalar tags and nested extents are checked.

use super::{DOMAIN, Error, RuntimeFunctionSourceLimits, limit, local_id, reference, type_id};
use crate::plan::{
    RuntimeEffectSet, RuntimeFunctionCaptureMode as Capture, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionInputOrigin as Origin, RuntimeFunctionInputSource as Source,
    RuntimeFunctionInputTransfer as Transfer, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing as Passing, RuntimeFunctionSemanticRole,
    RuntimeFunctionSiteBodyKind, RuntimeValueOwnershipRequirement,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use arcweft_id::EffectId;

pub(super) struct Row<'a> {
    pub(super) definition: RuntimeFunctionDefinitionIdentity,
    pub(super) role: RuntimeFunctionSemanticRole,
    pub(super) function_type: Option<RuntimePlanTypeId>,
    pub(super) result: RuntimePlanTypeId,
    pub(super) body_kind: RuntimeFunctionSiteBodyKind,
    pub(super) effects: &'a [u8],
    pub(super) effect_count: usize,
    pub(super) label_bytes: usize,
    pub(super) inputs: &'a [u8],
    pub(super) input_count: usize,
}
pub(super) struct Input<'a> {
    pub(super) transfer: Transfer,
    pub(super) origin: Origin,
    pub(super) source: Source,
    pub(super) local: RuntimeLocalDeclarationId,
    pub(super) ownership: RuntimeValueOwnershipRequirement,
    pub(super) unrestricted: &'a [u8],
    pub(super) pattern: &'a [u8],
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeFunctionSourceLimits,
}
impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: RuntimeFunctionSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn slice(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Arithmetic)?;
        let slice = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(slice)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.slice(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.slice(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn count(&mut self) -> Result<usize, Error> {
        let count = self.u32()? as usize;
        limit("source elements", count, self.limits.elements as usize)?;
        Ok(count)
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let len = self.u32()? as usize;
        self.slice(len)
    }
    fn identity(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.slice(32)?.try_into().expect("source identity width"))
    }
    fn local(&mut self) -> Result<RuntimeLocalDeclarationId, Error> {
        let id = self.u32()?;
        reference("local", id, self.limits.local_count)?;
        Ok(local_id(id))
    }
    fn ty(&mut self) -> Result<RuntimePlanTypeId, Error> {
        let id = self.u32()?;
        reference("type", id, self.limits.type_count)?;
        Ok(type_id(id))
    }
    fn passing(&mut self) -> Result<Passing, Error> {
        let tag = self.byte()?;
        Passing::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "parameter passing",
            tag,
        })
    }
    pub(super) fn header(&mut self) -> Result<usize, Error> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let rows = self.u32()? as usize;
        limit("function declarations", rows, self.limits.rows as usize)?;
        Ok(rows)
    }
    pub(super) fn row(&mut self, index: usize) -> Result<Row<'a>, Error> {
        let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
        let actual = self.u32()?;
        if actual != expected {
            return Err(Error::Ordinal { expected, actual });
        }
        let definition =
            RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.identity()?);
        let tag = self.byte()?;
        let role = RuntimeFunctionSemanticRole::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "semantic role",
            tag,
        })?;
        let function = self.u32()?;
        let function_type = if function == 0 {
            None
        } else {
            reference("function type", function, self.limits.type_count)?;
            Some(type_id(function))
        };
        let result = self.ty()?;
        let body_kind = match self.byte()? {
            0 => RuntimeFunctionSiteBodyKind::Expression,
            1 => RuntimeFunctionSiteBodyKind::Executable,
            tag => {
                return Err(Error::Tag {
                    kind: "body kind",
                    tag,
                });
            }
        };
        let start = self.offset;
        let (effect_count, label_bytes) = self.scan_effects()?;
        let effects = &self.bytes[start..self.offset];
        let input_count = self.count()?;
        let start = self.offset;
        for _ in 0..input_count {
            let _ = self.input()?;
        }
        let inputs = &self.bytes[start..self.offset];
        Ok(Row {
            definition,
            role,
            function_type,
            result,
            body_kind,
            effects,
            effect_count,
            label_bytes,
            inputs,
            input_count,
        })
    }
    fn scan_effects(&mut self) -> Result<(usize, usize), Error> {
        let count = self.count()?;
        let headers = count
            .checked_mul(size_of::<EffectId>())
            .ok_or(Error::Arithmetic)?;
        let mut previous = None;
        let mut bytes = 0usize;
        for _ in 0..count {
            let label = std::str::from_utf8(self.frame()?)?;
            bytes = bytes.checked_add(label.len()).ok_or(Error::Arithmetic)?;
            limit(
                "decoded bytes",
                headers
                    .checked_add(bytes)
                    .and_then(|n| n.checked_mul(2))
                    .ok_or(Error::Arithmetic)?,
                self.limits.decoded_bytes,
            )?;
            EffectId::validate_source(label)?;
            if previous.is_some_and(|old| old >= label) {
                return Err(Error::Order { kind: "effects" });
            }
            previous = Some(label);
        }
        Ok((count, bytes))
    }
    pub(super) fn effect_set(&mut self) -> Result<RuntimeEffectSet, Error> {
        let count = self.count()?;
        let mut effects = Vec::with_capacity(count);
        for _ in 0..count {
            effects.push(EffectId::parse(std::str::from_utf8(self.frame()?)?)?);
        }
        self.finish()?;
        Ok(RuntimeEffectSet::try_from_effects(effects)?)
    }
    pub(super) fn input(&mut self) -> Result<Input<'a>, Error> {
        let transfer = match self.byte()? {
            0 => Transfer::Transferred(match self.byte()? {
                0 => Capture::Copy,
                1 => Capture::SnapshotClone,
                2 => Capture::Move,
                tag => {
                    return Err(Error::Tag {
                        kind: "capture mode",
                        tag,
                    });
                }
            }),
            1 => Transfer::ExternalBinding,
            2 => Transfer::Formal,
            tag => {
                return Err(Error::Tag {
                    kind: "input transfer",
                    tag,
                });
            }
        };
        let origin = match self.byte()? {
            0 => Origin::Binding(self.identity()?),
            1 => Origin::Parameter(RuntimeFunctionParameterIdentity::from_accepted_identity(
                self.identity()?,
            )),
            2 => Origin::EvaluatedResult(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.identity()?),
            ),
            tag => {
                return Err(Error::Tag {
                    kind: "input origin",
                    tag,
                });
            }
        };
        let source = match self.byte()? {
            0 => Source::Capture {
                position: self.u32()?,
            },
            1 => Source::CapturedParameter {
                position: self.u32()?,
                passing: self.passing()?,
            },
            2 => Source::Parameter {
                position: self.u32()?,
                passing: self.passing()?,
            },
            tag => {
                return Err(Error::Tag {
                    kind: "input source",
                    tag,
                });
            }
        };
        let local = self.local()?;
        let ownership = match self.byte()? {
            0 => RuntimeValueOwnershipRequirement::Owned,
            1 => RuntimeValueOwnershipRequirement::Unrestricted,
            tag => {
                return Err(Error::Tag {
                    kind: "ownership",
                    tag,
                });
            }
        };
        let count = self.count()?;
        let unrestricted = self.slice(count.checked_mul(4).ok_or(Error::Arithmetic)?)?;
        let mut previous = 0;
        for field in unrestricted.chunks_exact(4) {
            let actual = u32::from_le_bytes(field.try_into().expect("source local width"));
            reference("unrestricted binding", actual, self.limits.local_count)?;
            if actual <= previous {
                return Err(Error::Order {
                    kind: "unrestricted bindings",
                });
            }
            previous = actual;
        }
        let pattern = self.frame()?;
        Ok(Input {
            transfer,
            origin,
            source,
            local,
            ownership,
            unrestricted,
            pattern,
        })
    }
    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
