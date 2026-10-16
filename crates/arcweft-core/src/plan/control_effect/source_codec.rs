//! Bounded original C definitions. Source decoding never issues C proofs;
//! the common semantic pass retains that authority over the complete graph.

use super::{
    RuntimeControlEffectCancellation, RuntimeControlEffectCardinality,
    RuntimeControlEffectContract, RuntimeControlEffectContractDefinition,
    RuntimeControlEffectContractId, RuntimeControlEffectContractTable,
    RuntimeControlEffectIdentity, RuntimeControlEffectKind, RuntimeControlEffectOrdering,
    RuntimeControlEffectRow, RuntimeControlEffectTerminalBehavior, RuntimeTaskControlMode,
};
use crate::plan::RuntimePlanTypeTable;
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use std::num::NonZeroU32;

const DOMAIN: &[u8] = b"arcweft.runtime.control-effect-sources.v1\0";

/// Logical payload and peak row-buffer storage are bounded separately from
/// wire bytes; allocator bookkeeping is outside the logical byte count.
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeControlSourceLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) elements: usize,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeControlSourceError {
    #[error("control source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid control source domain")]
    Domain,
    #[error("unsupported control source version {actual}")]
    Version { actual: u8 },
    #[error("control source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("control source arithmetic overflow")]
    Arithmetic,
    #[error("control source is truncated")]
    Truncated,
    #[error("control source has trailing bytes")]
    Trailing,
    #[error("unknown control source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("control source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("control source {kind} coordinate {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("original control type count {actual} differs from preflight count {expected}")]
    TypeTableCount { expected: usize, actual: usize },
    #[error("control source output requires {required} bytes but only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl From<TaskSemanticEncodingError> for RuntimeControlSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

pub(in crate::plan) struct PreparedRuntimeControlEncoding<'a> {
    table: &'a RuntimeControlEffectContractTable,
    limits: RuntimeControlSourceLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeControlEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }

    pub(in crate::plan) fn append_to(
        self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeControlSourceError> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(RuntimeControlSourceError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.limits);
        writer.table(self.table)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.storage, self.storage_bytes);
        debug_assert_eq!(writer.counts.elements, self.elements);
        Ok(())
    }
}

/// Preflighted original definitions, without any cached/completed digest.
pub(in crate::plan) struct PreparedRuntimeControlSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeControlSourceLimits,
    rows: usize,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeControlSources<'_> {
    pub(in crate::plan) const fn row_count(&self) -> usize {
        self.rows
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }

    pub(in crate::plan) fn materialize(
        self,
        types: &RuntimePlanTypeTable,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeControlEffectContractTable, RuntimeControlSourceError> {
        meter.status()?;
        let result = (|| {
            if types.len() != self.limits.type_count {
                return Err(RuntimeControlSourceError::TypeTableCount {
                    expected: self.limits.type_count,
                    actual: types.len(),
                });
            }
            // Recheck the whole immutable source before retaining any row.
            Reader::new(self.bytes, self.limits).table(false)?;
            let mut reader = Reader::new(self.bytes, self.limits);
            let table = reader
                .table(true)?
                .expect("retaining preflighted C definitions");
            debug_assert_eq!(table.len(), self.rows);
            debug_assert_eq!(reader.counts.storage, self.storage_bytes);
            debug_assert_eq!(reader.counts.elements, self.elements);
            Ok(table)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl RuntimeControlEffectContractTable {
    pub(in crate::plan) fn prepare_sources_encoding(
        &self,
        limits: RuntimeControlSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeControlEncoding<'_>, RuntimeControlSourceError> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer::new(None, Some(&mut *meter), limits);
            writer.table(self)?;
            Ok(PreparedRuntimeControlEncoding {
                table: self,
                limits,
                encoded_bytes: writer.length,
                storage_bytes: writer.counts.storage,
                elements: writer.counts.elements,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn encode_sources(
        &self,
        limits: RuntimeControlSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<u8>, RuntimeControlSourceError> {
        let prepared = self.prepare_sources_encoding(limits, meter)?;
        let mut output = Vec::with_capacity(prepared.encoded_bytes());
        prepared.append_to(&mut output)?;
        Ok(output)
    }

    pub(in crate::plan) fn preflight_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimeControlSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeControlSources<'a>, RuntimeControlSourceError> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut reader = Reader::new(bytes, limits);
            reader.table(false)?;
            Ok(PreparedRuntimeControlSources {
                bytes,
                limits,
                rows: reader.rows,
                storage_bytes: reader.counts.storage,
                elements: reader.counts.elements,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

struct Counts {
    limits: RuntimeControlSourceLimits,
    storage: usize,
    elements: usize,
}

impl Counts {
    fn new(limits: RuntimeControlSourceLimits) -> Self {
        Self {
            limits,
            storage: 0,
            elements: 0,
        }
    }
    fn array<T>(&mut self, count: usize, copies: usize) -> Result<(), RuntimeControlSourceError> {
        self.elements = self
            .elements
            .checked_add(count)
            .ok_or(RuntimeControlSourceError::Arithmetic)?;
        limit("elements", self.elements, self.limits.elements)?;
        let storage = count
            .checked_mul(size_of::<T>())
            .and_then(|n| n.checked_mul(copies))
            .ok_or(RuntimeControlSourceError::Arithmetic)?;
        self.storage = self
            .storage
            .checked_add(storage)
            .ok_or(RuntimeControlSourceError::Arithmetic)?;
        limit("decoded bytes", self.storage, self.limits.decoded_bytes)
    }
    fn rows(&mut self, count: usize) -> Result<(), RuntimeControlSourceError> {
        limit("rows", count, self.limits.rows as usize)?;
        // Vec-to-Arc publication temporarily retains both outer row buffers.
        // Nested boxes are moved; their logical payload is counted once.
        self.array::<RuntimeControlEffectContract>(count, 2)
    }
}

struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    counts: Counts,
    length: usize,
}

impl<'a> Writer<'a> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        limits: RuntimeControlSourceLimits,
    ) -> Self {
        Self {
            output,
            meter,
            counts: Counts::new(limits),
            length: 0,
        }
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeControlSourceError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(RuntimeControlSourceError::Arithmetic)?;
        limit(
            "encoded bytes",
            self.length,
            self.counts.limits.encoded_bytes,
        )?;
        if let Some(meter) = &mut self.meter {
            charge(meter, bytes.len())?;
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn tag(&mut self, tag: u8) -> Result<(), RuntimeControlSourceError> {
        self.bytes(&[tag])
    }
    fn count(&mut self, count: usize) -> Result<(), RuntimeControlSourceError> {
        self.bytes(
            &u32::try_from(count)
                .map_err(|_| RuntimeControlSourceError::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn ty(&mut self, ty: RuntimePlanTypeId) -> Result<(), RuntimeControlSourceError> {
        reference("type", ty.get().get(), self.counts.limits.type_count)?;
        self.bytes(&ty.get().get().to_le_bytes())
    }
    fn table(
        &mut self,
        table: &RuntimeControlEffectContractTable,
    ) -> Result<(), RuntimeControlSourceError> {
        self.counts.rows(table.len())?;
        self.bytes(DOMAIN)?;
        self.tag(1)?;
        self.count(table.len())?;
        // `iter` exposes completed proofs; source admission must also work
        // for definitions that have not reached the common seal yet.
        for (ordinal, row) in table.rows.iter().enumerate() {
            self.count(ordinal)?;
            self.tag(row.mode().semantic_tag())?;
            self.counts
                .array::<RuntimeControlEffectRow<RuntimePlanTypeId>>(row.effects().len(), 1)?;
            self.count(row.effects().len())?;
            for (ordinal, effect) in row.effects().iter().enumerate() {
                self.count(ordinal)?;
                self.tag(effect.kind.semantic_tag())?;
                self.tag(u8::from(effect.identity.is_some()))?;
                if let Some(identity) = effect.identity {
                    self.bytes(identity.as_bytes())?;
                }
                self.counts
                    .array::<RuntimePlanTypeId>(effect.inputs.len(), 1)?;
                self.count(effect.inputs.len())?;
                for ty in &effect.inputs {
                    self.ty(*ty)?;
                }
                self.tag(u8::from(effect.output.is_some()))?;
                if let Some(ty) = effect.output {
                    self.ty(ty)?;
                }
                self.tag(effect.cardinality.semantic_tag())?;
                self.tag(effect.ordering.semantic_tag())?;
                self.tag(effect.cancellation.semantic_tag())?;
                self.tag(effect.terminal.semantic_tag())?;
            }
            self.counts
                .array::<RuntimeControlEffectContractId>(row.children().len(), 1)?;
            self.count(row.children().len())?;
            for (ordinal, child) in row.children().iter().enumerate() {
                self.count(ordinal)?;
                let child = u32::try_from(child.index())
                    .ok()
                    .and_then(|n| n.checked_add(1))
                    .ok_or(RuntimeControlSourceError::Arithmetic)?;
                reference("child", child, table.len())?;
                self.bytes(&child.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    counts: Counts,
    rows: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], limits: RuntimeControlSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            counts: Counts::new(limits),
            rows: 0,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], RuntimeControlSourceError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RuntimeControlSourceError::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeControlSourceError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn tag(&mut self) -> Result<u8, RuntimeControlSourceError> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, RuntimeControlSourceError> {
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(RuntimeControlSourceError::Tag {
                kind: "option",
                tag,
            }),
        }
    }
    fn count(&mut self) -> Result<u32, RuntimeControlSourceError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four-byte source ordinal"),
        ))
    }
    fn ordinal(&mut self, expected: u32) -> Result<(), RuntimeControlSourceError> {
        let actual = self.count()?;
        if actual != expected {
            return Err(RuntimeControlSourceError::Ordinal { expected, actual });
        }
        Ok(())
    }
    fn ty(&mut self) -> Result<RuntimePlanTypeId, RuntimeControlSourceError> {
        let actual = self.count()?;
        reference("type", actual, self.counts.limits.type_count)?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            NonZeroU32::new(actual).expect("checked type coordinate"),
        ))
    }
    fn tagged<T>(
        &mut self,
        kind: &'static str,
        decode: impl FnOnce(u8) -> Option<T>,
    ) -> Result<T, RuntimeControlSourceError> {
        let tag = self.tag()?;
        decode(tag).ok_or(RuntimeControlSourceError::Tag { kind, tag })
    }
    fn effect(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeControlEffectRow<RuntimePlanTypeId>>, RuntimeControlSourceError> {
        let kind = self.tagged("effect kind", RuntimeControlEffectKind::from_semantic_tag)?;
        let identity = if self.boolean()? {
            Some(RuntimeControlEffectIdentity::from_checked_digest(
                self.take(32)?
                    .try_into()
                    .expect("32-byte original endpoint identity"),
            ))
        } else {
            None
        };
        let count = self.count()? as usize;
        self.counts.array::<RuntimePlanTypeId>(count, 1)?;
        let mut inputs = materialize.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let ty = self.ty()?;
            if let Some(inputs) = &mut inputs {
                inputs.push(ty);
            }
        }
        let output = if self.boolean()? {
            Some(self.ty()?)
        } else {
            None
        };
        let cardinality = self.tagged(
            "cardinality",
            RuntimeControlEffectCardinality::from_semantic_tag,
        )?;
        let ordering = self.tagged("ordering", RuntimeControlEffectOrdering::from_semantic_tag)?;
        let cancellation = self.tagged(
            "cancellation",
            RuntimeControlEffectCancellation::from_semantic_tag,
        )?;
        let terminal = self.tagged(
            "terminal",
            RuntimeControlEffectTerminalBehavior::from_semantic_tag,
        )?;
        Ok(inputs.map(|inputs| RuntimeControlEffectRow {
            kind,
            identity,
            inputs: inputs.into_boxed_slice(),
            output,
            cardinality,
            ordering,
            cancellation,
            terminal,
        }))
    }
    fn table(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeControlEffectContractTable>, RuntimeControlSourceError> {
        if self.take(DOMAIN.len())? != DOMAIN {
            return Err(RuntimeControlSourceError::Domain);
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeControlSourceError::Version { actual: version });
        }
        let count = self.count()?;
        self.rows = count as usize;
        self.counts.rows(self.rows)?;
        let mut rows = materialize.then(|| Vec::with_capacity(self.rows));
        for ordinal in 0..count {
            self.ordinal(ordinal)?;
            let mode = self.tagged("mode", RuntimeTaskControlMode::from_semantic_tag)?;
            let effects_count = self.count()?;
            self.counts
                .array::<RuntimeControlEffectRow<RuntimePlanTypeId>>(effects_count as usize, 1)?;
            let mut effects = materialize.then(|| Vec::with_capacity(effects_count as usize));
            for ordinal in 0..effects_count {
                self.ordinal(ordinal)?;
                let effect = self.effect(materialize)?;
                if let Some(effects) = &mut effects {
                    effects.push(effect.expect("retaining effect definition"));
                }
            }
            let children_count = self.count()?;
            self.counts
                .array::<RuntimeControlEffectContractId>(children_count as usize, 1)?;
            let mut children = materialize.then(|| Vec::with_capacity(children_count as usize));
            for ordinal in 0..children_count {
                self.ordinal(ordinal)?;
                let child = self.count()?;
                reference("child", child, self.rows)?;
                if let Some(children) = &mut children {
                    children.push(
                        RuntimeControlEffectContractId::for_index((child - 1) as usize)
                            .expect("checked original child coordinate"),
                    );
                }
            }
            if let Some(rows) = &mut rows {
                rows.push(RuntimeControlEffectContract::new(
                    RuntimeControlEffectContractDefinition {
                        mode,
                        effects: effects
                            .expect("retaining original effects")
                            .into_boxed_slice(),
                        children: children
                            .expect("retaining original children")
                            .into_boxed_slice(),
                    },
                ));
            }
        }
        if self.offset != self.bytes.len() {
            return Err(RuntimeControlSourceError::Trailing);
        }
        Ok(rows.map(RuntimeControlEffectContractTable::from_definitions))
    }
}

fn reference(
    kind: &'static str,
    actual: u32,
    maximum: usize,
) -> Result<(), RuntimeControlSourceError> {
    if actual == 0 || actual as usize > maximum {
        return Err(RuntimeControlSourceError::Reference {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn limit(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimeControlSourceError> {
    if actual > maximum {
        return Err(RuntimeControlSourceError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), RuntimeControlSourceError> {
    meter.charge_work(
        u64::try_from(bytes)
            .ok()
            .and_then(|n| n.checked_mul(3))
            .ok_or(RuntimeControlSourceError::Arithmetic)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
