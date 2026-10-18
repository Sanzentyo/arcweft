//! Private version-one source frame for the existing inert value algebra.
//! Complete framing and retained-storage counts precede inert allocation.
//! Decoded data still needs the original plan's literal/type admission.

mod agent;
mod metadata;
mod scalar;
mod sequence;
mod value;

#[cfg(test)]
mod tests;

use super::AwbcRuntimeValueSnapshot;
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::{RuntimeValue, RuntimeValueView};

const DOMAIN: &[u8] = b"arcweft.runtime.literal-source.v1\0";

#[derive(Clone, Copy)]
pub(crate) struct RuntimeLiteralSourceLimits {
    pub(crate) encoded_bytes: usize,
    pub(crate) decoded_bytes: usize,
    pub(crate) elements: u32,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum RuntimeLiteralSourceError {
    #[error("literal source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid literal source domain")]
    Domain,
    #[error("unsupported literal source version {actual}")]
    Version { actual: u8 },
    #[error("literal source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("literal source arithmetic overflow")]
    Arithmetic,
    #[error("literal source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("literal source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("literal source string at {offset} is not UTF-8: {source}")]
    Utf8 {
        offset: usize,
        source: std::str::Utf8Error,
    },
    #[error("unknown literal source {kind} tag {tag} at {offset}")]
    Tag {
        kind: &'static str,
        tag: u8,
        offset: usize,
    },
    #[error("invalid literal source {kind} at {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("{kind} at {offset} is not a static literal")]
    RuntimeCarrier { kind: &'static str, offset: usize },
    #[error("literal source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
    #[error("literal source value admission failed: {0}")]
    Value(#[from] super::AwbcRuntimeValueSnapshotError),
    #[error("literal source numeric shape at {offset} is invalid: {source}")]
    Dense {
        offset: usize,
        source: crate::math::RuntimeDenseNumericPayloadError,
    },
    #[error("literal source progress at {offset} is invalid: {source}")]
    Progress {
        offset: usize,
        source: arcweft_need::ProgressError,
    },
    #[error("literal source identity at {offset} is invalid: {source}")]
    Identity {
        offset: usize,
        source: crate::entry::RuntimeIdentityError,
    },
    #[error("literal source effect at {offset} is invalid: {source}")]
    Effect {
        offset: usize,
        source: arcweft_id::EffectIdError,
    },
    #[error("literal source record field {actual} at {offset} is not a nonzero field ID")]
    Field { offset: usize, actual: u32 },
    #[error("literal source runtime path at {offset} is invalid: {source}")]
    RuntimePath {
        offset: usize,
        source: crate::runtime_id::RuntimeIdError,
    },
    #[error("literal source public identity at {offset} is invalid: {source}")]
    PublicId {
        offset: usize,
        source: arcweft_id::IdError,
    },
    #[error("literal source public identity family at {offset} is invalid: {source}")]
    PublicFamily {
        offset: usize,
        source: arcweft_id::PublicIdFamilyError,
    },
    #[error("literal source imported entity at {offset} is invalid: {source}")]
    Imported {
        offset: usize,
        source: crate::value::RuntimeImportedProjectEntityError,
    },
    #[error("literal source character identity at {offset} is invalid: {source}")]
    Character {
        offset: usize,
        source: arcweft_character::id::CharacterIdError,
    },
    #[error("literal source Agent field at {offset} is invalid: {source}")]
    Agent {
        offset: usize,
        source: crate::value::RuntimeAgentConstructionError,
    },
    #[error("literal source predicate operands at {offset} are invalid: {source}")]
    PredicateOperands {
        offset: usize,
        source: crate::value::AgentPredicateOperandsError,
    },
}

impl From<TaskSemanticEncodingError> for RuntimeLiteralSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

impl RuntimeLiteralSourceError {
    fn reject_in_bank(self, meter: &mut TaskSemanticMeter) -> Self {
        let error = match meter.status() {
            Err(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(cause)
            }
            _ => self,
        };
        meter.reject_owner();
        error
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Footprint {
    storage: usize,
    scratch: usize,
    elements: usize,
    source_nodes: usize,
}

// These are physical source counts. Logical value, type and Choice validation
// stays with the original ValueResourceAdmission; representation-only tuple or
// column wrappers must not acquire a second logical value schema here.
struct Counts {
    limits: RuntimeLiteralSourceLimits,
    values: RuntimeSchemaLimits,
    footprint: Footprint,
}

impl Counts {
    fn new(limits: RuntimeLiteralSourceLimits, values: RuntimeSchemaLimits) -> Self {
        Self {
            limits,
            values,
            footprint: Footprint {
                storage: 0,
                scratch: 0,
                elements: 0,
                source_nodes: 0,
            },
        }
    }

    fn limit(
        kind: &'static str,
        actual: usize,
        maximum: usize,
    ) -> Result<(), RuntimeLiteralSourceError> {
        if actual > maximum {
            return Err(RuntimeLiteralSourceError::Limit {
                kind,
                actual,
                maximum,
            });
        }
        Ok(())
    }

    fn storage(&mut self, bytes: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(bytes)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        let total = self
            .footprint
            .storage
            .checked_add(self.footprint.scratch)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        Self::limit("decoded bytes", total, self.limits.decoded_bytes)
    }

    fn scratch(&mut self, bytes: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.footprint.scratch = self.footprint.scratch.max(bytes);
        let total = self
            .footprint
            .storage
            .checked_add(self.footprint.scratch)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        Self::limit("decoded bytes", total, self.limits.decoded_bytes)
    }

    fn elements(&mut self, count: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(count)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        Self::limit(
            "source elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )
    }

    fn array<T>(&mut self, count: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.elements(count)?;
        self.storage(
            count
                .checked_mul(size_of::<T>())
                .ok_or(RuntimeLiteralSourceError::Arithmetic)?,
        )
    }

    fn string_length(&self, length: usize) -> Result<(), RuntimeLiteralSourceError> {
        let maximum = match usize::try_from(self.values.max_string_bytes) {
            Ok(maximum) => maximum,
            Err(_) => usize::MAX,
        };
        Self::limit("string bytes", length, maximum)
    }

    fn string(&mut self, length: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.string_length(length)?;
        self.storage(length)
    }

    fn depth(&self, depth: usize) -> Result<(), RuntimeLiteralSourceError> {
        // The inert builtin payload grammar can contain a tuple wrapper for
        // one logical payload. Its framing depth is separate from the existing
        // logical/physical value admission, which still uses the original limit.
        let hard = 2 * crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH + 8;
        let maximum = (u64::from(self.values.max_depth) * 2 + 8).min(hard as u64) as usize;
        Self::limit("inert source depth", depth, maximum)
    }

    fn node(&mut self, depth: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.depth(depth)?;
        self.footprint.source_nodes = self
            .footprint
            .source_nodes
            .checked_add(1)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        self.elements(1)
    }
}

pub(crate) struct PreparedRuntimeLiteralEncoding<'a> {
    value: RuntimeValueView<'a>,
    limits: RuntimeLiteralSourceLimits,
    values: RuntimeSchemaLimits,
    encoded_bytes: usize,
    footprint: Footprint,
}

impl PreparedRuntimeLiteralEncoding<'_> {
    pub(crate) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(crate) const fn storage_bytes(&self) -> usize {
        self.footprint.storage + self.footprint.scratch
    }
    pub(crate) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(crate) const fn source_nodes(&self) -> usize {
        self.footprint.source_nodes
    }

    pub(crate) fn append_to(self, output: &mut Vec<u8>) -> Result<(), RuntimeLiteralSourceError> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(RuntimeLiteralSourceError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.limits, self.values);
        writer.frame(self.value)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.footprint, self.footprint);
        Ok(())
    }
}

/// Immutable complete source framing. It owns neither a runtime value nor a
/// program lease, and the caller can sum all child footprints before retaining.
pub(crate) struct PreparedRuntimeLiteralSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeLiteralSourceLimits,
    values: RuntimeSchemaLimits,
    footprint: Footprint,
}

impl PreparedRuntimeLiteralSource<'_> {
    pub(crate) const fn storage_bytes(&self) -> usize {
        self.footprint.storage + self.footprint.scratch
    }
    pub(crate) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(crate) const fn source_nodes(&self) -> usize {
        self.footprint.source_nodes
    }

    pub(crate) fn materialize(
        self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<AwbcRuntimeValueSnapshot, RuntimeLiteralSourceError> {
        meter.status()?;
        let result = (|| {
            let mut reader = Reader::new(self.bytes, self.limits, self.values, meter);
            let value = reader
                .frame(true)?
                .expect("complete source frame retains its original value");
            debug_assert_eq!(reader.counts.footprint, self.footprint);
            Ok(value)
        })();
        result.map_err(|error: RuntimeLiteralSourceError| error.reject_in_bank(meter))
    }
}

impl AwbcRuntimeValueSnapshot {
    pub(crate) fn prepare_literal_source_encoding<'a>(
        &'a self,
        limits: RuntimeLiteralSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLiteralEncoding<'a>, RuntimeLiteralSourceError> {
        self.view()
            .prepare_literal_source_encoding(limits, resources, meter)
    }

    pub(crate) fn preflight_literal_source<'a>(
        bytes: &'a [u8],
        limits: RuntimeLiteralSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLiteralSource<'a>, RuntimeLiteralSourceError> {
        meter.status()?;
        let result = (|| {
            Counts::limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_source(meter, bytes.len())?;
            let values = resources.work().limits();
            let mut reader = Reader::new(bytes, limits, values, meter);
            reader.frame(false)?;
            Ok(PreparedRuntimeLiteralSource {
                bytes,
                limits,
                values,
                footprint: reader.counts.footprint,
            })
        })();
        result.map_err(|error: RuntimeLiteralSourceError| error.reject_in_bank(meter))
    }
}

impl RuntimeValue {
    /// Borrows the original storage throughout measurement and emission. This
    /// encodes source data; original plan/type admission remains a separate
    /// requirement before a decoded value may enter an executable body.
    pub(crate) fn prepare_literal_source_encoding<'a>(
        &'a self,
        limits: RuntimeLiteralSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLiteralEncoding<'a>, RuntimeLiteralSourceError> {
        self.view()
            .prepare_literal_source_encoding(limits, resources, meter)
    }
}

impl crate::value::RuntimeEntityReference {
    /// Borrows the original typed identity for an executable source operand.
    pub(crate) fn prepare_literal_source_encoding<'a>(
        &'a self,
        limits: RuntimeLiteralSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLiteralEncoding<'a>, RuntimeLiteralSourceError> {
        RuntimeValueView::Scalar(crate::value::RuntimeScalarView::EntityRef(self))
            .prepare_literal_source_encoding(limits, resources, meter)
    }
}

impl<'a> RuntimeValueView<'a> {
    fn prepare_literal_source_encoding(
        self,
        limits: RuntimeLiteralSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLiteralEncoding<'a>, RuntimeLiteralSourceError> {
        meter.status()?;
        let result = (|| {
            let values = resources.work().limits();
            let mut writer = Writer::new(None, Some(&mut *meter), limits, values);
            writer.frame(self)?;
            Ok(PreparedRuntimeLiteralEncoding {
                value: self,
                limits,
                values,
                encoded_bytes: writer.length,
                footprint: writer.counts.footprint,
            })
        })();
        result.map_err(|error: RuntimeLiteralSourceError| error.reject_in_bank(meter))
    }
}

struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    length: usize,
    counts: Counts,
}

impl<'a> Writer<'a> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        limits: RuntimeLiteralSourceLimits,
        values: RuntimeSchemaLimits,
    ) -> Self {
        Self {
            output,
            meter,
            length: 0,
            counts: Counts::new(limits, values),
        }
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeLiteralSourceError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        Counts::limit(
            "encoded bytes",
            self.length,
            self.counts.limits.encoded_bytes,
        )?;
        if let Some(meter) = &mut self.meter {
            charge_source(meter, bytes.len())?;
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn tag(&mut self, value: u8) -> Result<(), RuntimeLiteralSourceError> {
        self.bytes(&[value])
    }
    fn boolean(&mut self, value: bool) -> Result<(), RuntimeLiteralSourceError> {
        self.tag(u8::from(value))
    }
    fn u32(&mut self, value: u32) -> Result<(), RuntimeLiteralSourceError> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), RuntimeLiteralSourceError> {
        self.bytes(&value.to_le_bytes())
    }
    fn count(&mut self, count: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.u32(u32::try_from(count).map_err(|_| RuntimeLiteralSourceError::Arithmetic)?)
    }
    fn string(&mut self, text: &str) -> Result<(), RuntimeLiteralSourceError> {
        self.counts.string(text.len())?;
        self.count(text.len())?;
        self.bytes(text.as_bytes())
    }
    fn node(&mut self, depth: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.counts.node(depth)?;
        if let Some(meter) = &mut self.meter {
            meter.charge_work(1)?;
        }
        Ok(())
    }
    fn frame(&mut self, value: RuntimeValueView<'_>) -> Result<(), RuntimeLiteralSourceError> {
        self.bytes(DOMAIN)?;
        self.tag(1)?;
        self.counts.storage(size_of::<AwbcRuntimeValueSnapshot>())?;
        self.value(value, 0)
    }
}

struct Reader<'a, 'meter> {
    bytes: &'a [u8],
    offset: usize,
    counts: Counts,
    meter: &'meter mut TaskSemanticMeter,
}

impl<'a, 'meter> Reader<'a, 'meter> {
    fn new(
        bytes: &'a [u8],
        limits: RuntimeLiteralSourceLimits,
        values: RuntimeSchemaLimits,
        meter: &'meter mut TaskSemanticMeter,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            counts: Counts::new(limits, values),
            meter,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], RuntimeLiteralSourceError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?;
        let bytes =
            self.bytes
                .get(self.offset..end)
                .ok_or(RuntimeLiteralSourceError::Truncated {
                    offset: self.offset,
                })?;
        self.offset = end;
        Ok(bytes)
    }
    fn tag(&mut self) -> Result<u8, RuntimeLiteralSourceError> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, RuntimeLiteralSourceError> {
        let offset = self.offset;
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(RuntimeLiteralSourceError::Tag {
                kind: "bool/option",
                tag,
                offset,
            }),
        }
    }
    fn u32(&mut self) -> Result<u32, RuntimeLiteralSourceError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four source bytes"),
        ))
    }
    fn u64(&mut self) -> Result<u64, RuntimeLiteralSourceError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight source bytes"),
        ))
    }
    fn u128(&mut self) -> Result<u128, RuntimeLiteralSourceError> {
        Ok(u128::from_le_bytes(
            self.take(16)?.try_into().expect("sixteen source bytes"),
        ))
    }
    fn digest(&mut self) -> Result<[u8; 32], RuntimeLiteralSourceError> {
        Ok(self.take(32)?.try_into().expect("32 source identity bytes"))
    }
    fn count(&mut self) -> Result<usize, RuntimeLiteralSourceError> {
        usize::try_from(self.u32()?).map_err(|_| RuntimeLiteralSourceError::Arithmetic)
    }
    fn string(&mut self) -> Result<&'a str, RuntimeLiteralSourceError> {
        let length = self.count()?;
        self.counts.string(length)?;
        let offset = self.offset;
        std::str::from_utf8(self.take(length)?)
            .map_err(|source| RuntimeLiteralSourceError::Utf8 { offset, source })
    }
    fn node(&mut self, depth: usize) -> Result<(), RuntimeLiteralSourceError> {
        self.counts.node(depth)?;
        self.meter.charge_work(1)?;
        Ok(())
    }
    fn primitives<T>(
        &mut self,
        count: usize,
        width: usize,
        materialize: bool,
        mut decode: impl FnMut(usize, &[u8]) -> Result<T, RuntimeLiteralSourceError>,
    ) -> Result<Option<Vec<T>>, RuntimeLiteralSourceError> {
        self.counts.array::<T>(count)?;
        let offset = self.offset;
        let bytes = self.take(
            count
                .checked_mul(width)
                .ok_or(RuntimeLiteralSourceError::Arithmetic)?,
        )?;
        let mut values = materialize.then(|| Vec::with_capacity(count));
        for (index, chunk) in bytes.chunks_exact(width).enumerate() {
            let value = decode(offset + index * width, chunk)?;
            if let Some(values) = &mut values {
                values.push(value);
            }
        }
        Ok(values)
    }
    fn frame(
        &mut self,
        materialize: bool,
    ) -> Result<Option<AwbcRuntimeValueSnapshot>, RuntimeLiteralSourceError> {
        if self.take(DOMAIN.len())? != DOMAIN {
            return Err(RuntimeLiteralSourceError::Domain);
        }
        let actual = self.tag()?;
        if actual != 1 {
            return Err(RuntimeLiteralSourceError::Version { actual });
        }
        self.counts.storage(size_of::<AwbcRuntimeValueSnapshot>())?;
        let value = self.value(materialize, 0)?;
        if self.offset != self.bytes.len() {
            return Err(RuntimeLiteralSourceError::Trailing {
                offset: self.offset,
            });
        }
        Ok(value)
    }
}

fn charge_source(
    meter: &mut TaskSemanticMeter,
    bytes: usize,
) -> Result<(), RuntimeLiteralSourceError> {
    let bytes = u64::try_from(bytes).map_err(|_| RuntimeLiteralSourceError::Arithmetic)?;
    meter.charge_work(
        bytes
            .checked_mul(3)
            .ok_or(RuntimeLiteralSourceError::Arithmetic)?,
    )?;
    Ok(())
}
