//! Borrowed source framing; schema identities remain distinct from type IDs.

use super::{DOMAIN, Error, RuntimeContentDeclarationSourceLimits, limit, reference};
use crate::entry::RuntimeDialogueContentTemplateDigest;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeDialogueContentEffectTrigger, RuntimeDialogueContentSlotSeed, RuntimeDialogueValueRole,
};
use crate::runtime_id::{
    RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteId, RuntimeDialogueValueSlotId,
    RuntimePlanTypeId,
};
use crate::time::LogicalDuration;
use crate::value::RuntimeFmtParameterId;

pub(super) struct Template<'a> {
    pub(super) id: RuntimeDialogueContentTemplateId,
    pub(super) digest: RuntimeDialogueContentTemplateDigest,
    pub(super) slots: &'a [u8],
    pub(super) slot_count: usize,
    pub(super) effects: &'a [u8],
    pub(super) effect_count: usize,
}
pub(super) struct Effect<'a> {
    pub(super) site: RuntimeDialogueEffectSiteId,
    pub(super) trigger: RuntimeDialogueContentEffectTrigger,
    pub(super) captures: &'a [u8],
}
pub(super) struct Attempt<'a> {
    pub(super) template: RuntimeDialogueContentTemplateId,
    pub(super) operands: &'a [u8],
    pub(super) operand_count: usize,
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeContentDeclarationSourceLimits,
}
impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: RuntimeContentDeclarationSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn slice(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.slice(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.slice(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.slice(8)?.try_into().expect("source u64 width"),
        ))
    }
    fn identity(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.slice(32)?.try_into().expect("source identity width"))
    }
    fn count(&mut self, kind: &'static str, max: usize) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        limit(kind, n, max)?;
        Ok(n)
    }
    fn ordinal(&mut self, kind: &'static str, index: usize) -> Result<u32, Error> {
        let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
        let actual = self.u32()?;
        if actual != expected {
            return Err(Error::Ordinal {
                kind,
                expected,
                actual,
            });
        }
        Ok(actual)
    }
    pub(super) fn ty(&mut self) -> Result<RuntimePlanTypeId, Error> {
        let raw = self.u32()?;
        reference("type", raw, self.limits.type_count)?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            std::num::NonZeroU32::new(raw).expect("checked original type coordinate"),
        ))
    }
    pub(super) fn header(
        &mut self,
    ) -> Result<(usize, Option<RuntimeDialogueContentTemplateId>), Error> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let templates = self.count("templates", self.limits.templates as usize)?;
        let plain = self.u32()?;
        let plain = if plain == 0 {
            None
        } else {
            reference("plain-text template", plain, templates)?;
            Some(RuntimeDialogueContentTemplateId::from_nonzero(
                std::num::NonZeroU32::new(plain).expect("checked template coordinate"),
            ))
        };
        Ok((templates, plain))
    }
    pub(super) fn template(&mut self, index: usize) -> Result<Template<'a>, Error> {
        self.ordinal("template", index)?;
        let id = RuntimeDialogueContentTemplateId::from_zero_based(index)
            .expect("checked template ordinal");
        let digest = RuntimeDialogueContentTemplateDigest::from_bytes(self.identity()?);
        let slot_count = self.count("slots", self.limits.elements as usize)?;
        let start = self.offset;
        for slot in 0..slot_count {
            self.slot(slot)?;
        }
        let slots = &self.bytes[start..self.offset];
        let effect_count = self.count("effects", self.limits.elements as usize)?;
        let start = self.offset;
        for effect in 0..effect_count {
            self.effect(effect)?;
        }
        let effects = &self.bytes[start..self.offset];
        Ok(Template {
            id,
            digest,
            slots,
            slot_count,
            effects,
            effect_count,
        })
    }
    pub(super) fn slot(&mut self, index: usize) -> Result<RuntimeDialogueContentSlotSeed, Error> {
        self.ordinal("slot", index)?;
        let slot =
            RuntimeDialogueValueSlotId::from_zero_based(index).expect("checked slot ordinal");
        let tag = self.byte()?;
        let role = RuntimeDialogueValueRole::from_encoded(tag).ok_or(Error::Tag {
            kind: "slot role",
            tag,
        })?;
        let semantic_type = RuntimeSemanticTypeId::from_bytes(self.identity()?);
        Ok(RuntimeDialogueContentSlotSeed {
            slot,
            role,
            semantic_type,
        })
    }
    pub(super) fn effect(&mut self, index: usize) -> Result<Effect<'a>, Error> {
        self.ordinal("effect", index)?;
        let site =
            RuntimeDialogueEffectSiteId::from_zero_based(index).expect("checked effect ordinal");
        let trigger = match self.byte()? {
            0 => RuntimeDialogueContentEffectTrigger::Content,
            1 => RuntimeDialogueContentEffectTrigger::Delay {
                duration: LogicalDuration::from_nanos(self.u64()?),
            },
            tag => {
                return Err(Error::Tag {
                    kind: "effect trigger",
                    tag,
                });
            }
        };
        let captures = self.count("captures", self.limits.elements as usize)?;
        let start = self.offset;
        for _ in 0..captures {
            self.ty()?;
        }
        Ok(Effect {
            site,
            trigger,
            captures: &self.bytes[start..self.offset],
        })
    }
    pub(super) fn attempt_count(&mut self) -> Result<usize, Error> {
        self.count("formatters", self.limits.attempts as usize)
    }
    pub(super) fn attempt(&mut self, index: usize, templates: usize) -> Result<Attempt<'a>, Error> {
        self.ordinal("formatter", index)?;
        let template = self.u32()?;
        reference("formatter template", template, templates)?;
        let template = RuntimeDialogueContentTemplateId::from_nonzero(
            std::num::NonZeroU32::new(template).expect("checked template coordinate"),
        );
        let operand_count = self.count("formatter operands", self.limits.elements as usize)?;
        let start = self.offset;
        for _ in 0..operand_count {
            self.operand()?;
        }
        Ok(Attempt {
            template,
            operands: &self.bytes[start..self.offset],
            operand_count,
        })
    }
    pub(super) fn operand(&mut self) -> Result<(RuntimeFmtParameterId, RuntimePlanTypeId), Error> {
        let tag = self.byte()?;
        let parameter = RuntimeFmtParameterId::from_index(tag as usize).ok_or(Error::Tag {
            kind: "formatter parameter",
            tag,
        })?;
        Ok((parameter, self.ty()?))
    }
    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing);
        }
        Ok(())
    }
}
