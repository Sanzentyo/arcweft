//! Original content manifests and source-ordered formatter declarations.
//! Admission uses the actual template and formatter construction authorities.

mod cursor;
mod encoding;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use crate::plan::{
    RuntimeDialogueContentEffectSlot, RuntimeDialogueContentEffectSlotSeed,
    RuntimeDialogueContentSlot, RuntimeDialogueContentSlotSeed,
    RuntimeDialogueContentTemplateManifest, RuntimeDialogueContentTemplateManifestSeed,
    RuntimeFormatAttempt, RuntimeFormatAttemptDeclarationSeed, RuntimeFormatAttemptOperand,
    RuntimeFormatAttemptOperandSeed, RuntimeFormatAttemptSeedId, RuntimePlanBuildError,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use cursor::Cursor;

const DOMAIN: &[u8] = b"arcweft.runtime.content-declarations.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeContentDeclarationSourceLimits {
    pub(in crate::plan) templates: u32,
    pub(in crate::plan) attempts: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
}
#[derive(Debug, thiserror::Error)]
pub(in crate::plan) enum RuntimeContentDeclarationSourceError {
    #[error("content declaration source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid content declaration source domain")]
    Domain,
    #[error("unsupported content declaration source version {actual}")]
    Version { actual: u8 },
    #[error("content declaration source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("content declaration source arithmetic overflow")]
    Arithmetic,
    #[error("content declaration source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("content declaration source has trailing bytes")]
    Trailing,
    #[error("unknown content declaration source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("content declaration source {kind} ordinal {actual} differs from {expected}")]
    Ordinal {
        kind: &'static str,
        expected: u32,
        actual: u32,
    },
    #[error("content declaration source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("content declaration source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("content declaration source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("content declaration source plain-text pointer disagrees with its actual manifest")]
    PlainTextPointer,
    #[error(
        "content declaration source needs {required} output bytes, only {remaining} are reserved"
    )]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeContentDeclarationSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl Error {
    fn reject(self, meter: &mut TaskSemanticMeter) -> Self {
        let result = match meter.status() {
            Err(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(cause)
            }
            _ => self,
        };
        meter.reject_owner();
        result
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeContentDeclarationSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeContentDeclarationSourceLimits) -> Self {
        Self {
            limits,
            footprint: Footprint::default(),
        }
    }
    fn elements(&mut self, n: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(n)
            .ok_or(Error::Arithmetic)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )
    }
    fn storage(&mut self, n: usize) -> Result<(), Error> {
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(n)
            .ok_or(Error::Arithmetic)?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn template(&mut self, slots: usize, effects: usize) -> Result<(), Error> {
        self.elements(1)?;
        self.elements(slots)?;
        self.elements(effects)?;
        self.storage(
            size_of::<RuntimeDialogueContentTemplateManifestSeed>()
                + 2 * size_of::<RuntimeDialogueContentTemplateManifest>(),
        )?;
        self.storage(
            slots
                .checked_mul(
                    size_of::<RuntimeDialogueContentSlotSeed>()
                        + size_of::<RuntimeDialogueContentSlot>(),
                )
                .ok_or(Error::Arithmetic)?,
        )?;
        self.storage(
            effects
                .checked_mul(
                    size_of::<RuntimeDialogueContentEffectSlotSeed>()
                        + size_of::<RuntimeDialogueContentEffectSlot>(),
                )
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn captures(&mut self, n: usize) -> Result<(), Error> {
        self.elements(n)?;
        self.storage(
            n.checked_mul(
                size_of::<crate::pattern::RuntimeSemanticTypeId>()
                    + size_of::<crate::runtime_id::RuntimePlanTypeId>(),
            )
            .ok_or(Error::Arithmetic)?,
        )
    }
    fn attempt(&mut self, n: usize) -> Result<(), Error> {
        self.elements(1)?;
        self.elements(n)?;
        self.storage(
            size_of::<RuntimeFormatAttemptDeclarationSeed>()
                + 2 * size_of::<RuntimeFormatAttempt>()
                + size_of::<RuntimeFormatAttemptSeedId>(),
        )?;
        self.storage(
            n.checked_mul(
                size_of::<RuntimeFormatAttemptOperandSeed>()
                    + size_of::<RuntimeFormatAttemptOperand>(),
            )
            .ok_or(Error::Arithmetic)?,
        )
    }
}

pub(in crate::plan) struct PreparedRuntimeContentDeclarationSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeContentDeclarationSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeContentDeclarationSources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeContentDeclarationSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = Cursor::new(bytes, limits);
            let (templates, _) = cursor.header()?;
            let mut counts = Counts::new(limits);
            for index in 0..templates {
                let row = cursor.template(index)?;
                counts.template(row.slot_count, row.effect_count)?;
                let mut effects = Cursor::new(row.effects, limits);
                for effect in 0..row.effect_count {
                    let effect = effects.effect(effect)?;
                    counts.captures(effect.captures.len() / 4)?;
                }
                effects.finish()?;
            }
            let attempts = cursor.attempt_count()?;
            for index in 0..attempts {
                let row = cursor.attempt(index, templates)?;
                counts.attempt(row.operand_count)?;
            }
            cursor.finish()?;
            Ok(Self {
                bytes,
                limits,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) fn reserve(
        self,
        source: &mut RuntimePlanSourceConstruction,
    ) -> Result<Box<[RuntimeFormatAttemptSeedId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            charge(&mut source.meter, self.bytes.len())?;
            for (kind, expected, actual) in [
                ("type", self.limits.type_count, source.builder.types.len()),
                (
                    "existing template",
                    0,
                    source.builder.dialogue_content.template_count(),
                ),
                (
                    "existing formatter",
                    0,
                    source.builder.format_attempts.len(),
                ),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let (templates, plain) = cursor.header()?;
            for index in 0..templates {
                let row = cursor.template(index)?;
                let seed = source.content_template_source_seed(&row, self.limits)?;
                if Some(row.id) == plain {
                    source
                        .builder
                        .register_plain_text_context_template_seed(seed)?;
                } else {
                    source
                        .builder
                        .register_dialogue_content_template_seed(seed)?;
                }
            }
            let attempts = cursor.attempt_count()?;
            let mut handles = Vec::with_capacity(attempts);
            for index in 0..attempts {
                let row = cursor.attempt(index, templates)?;
                let mut inputs = Cursor::new(row.operands, self.limits);
                let mut operands = Vec::with_capacity(row.operand_count);
                for _ in 0..row.operand_count {
                    let (parameter, ty) = inputs.operand()?;
                    let ty = source
                        .builder
                        .types
                        .get(ty)
                        .expect("preflighted original formatter type")
                        .semantic_identity();
                    operands.push(RuntimeFormatAttemptOperandSeed { parameter, ty });
                }
                inputs.finish()?;
                handles.push(source.builder.reserve_format_attempt_seed(
                    RuntimeFormatAttemptDeclarationSeed {
                        template: row.template,
                        operands: operands.into_boxed_slice(),
                    },
                )?);
            }
            cursor.finish()?;
            Ok(handles.into_boxed_slice())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
impl RuntimePlanSourceConstruction {
    fn content_template_source_seed(
        &self,
        row: &cursor::Template<'_>,
        limits: RuntimeContentDeclarationSourceLimits,
    ) -> Result<RuntimeDialogueContentTemplateManifestSeed, Error> {
        let mut slots = Cursor::new(row.slots, limits);
        let mut slot_seeds = Vec::with_capacity(row.slot_count);
        for index in 0..row.slot_count {
            slot_seeds.push(slots.slot(index)?);
        }
        slots.finish()?;
        let mut effects = Cursor::new(row.effects, limits);
        let mut effect_seeds = Vec::with_capacity(row.effect_count);
        for index in 0..row.effect_count {
            let effect = effects.effect(index)?;
            let mut captures = Vec::with_capacity(effect.captures.len() / 4);
            let mut types = Cursor::new(effect.captures, limits);
            for _ in 0..effect.captures.len() / 4 {
                let ty = types.ty()?;
                captures.push(
                    self.builder
                        .types
                        .get(ty)
                        .expect("preflighted original capture type")
                        .semantic_identity(),
                );
            }
            types.finish()?;
            effect_seeds.push(RuntimeDialogueContentEffectSlotSeed {
                site: effect.site,
                trigger: effect.trigger,
                capture_types: captures.into_boxed_slice(),
            });
        }
        effects.finish()?;
        Ok(RuntimeDialogueContentTemplateManifestSeed {
            id: row.id,
            digest: row.digest,
            slots: slot_seeds.into_boxed_slice(),
            effects: effect_seeds.into_boxed_slice(),
        })
    }
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual > maximum {
        return Err(Error::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn reference(kind: &'static str, actual: u32, maximum: usize) -> Result<(), Error> {
    if actual == 0 || actual as usize > maximum {
        return Err(Error::Reference {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(bytes)
            .ok()
            .and_then(|n| n.checked_mul(3))
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
