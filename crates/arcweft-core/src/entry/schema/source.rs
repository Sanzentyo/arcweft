//! Original schema data for construction, independently of layout digests.
//!
//! A complete borrowed scan precedes strings, schema nodes and aggregate
//! materialization. The iterative grammar retains all source policies and
//! references; it grants no nominal, executable or program authority.

mod encoding;
mod reading;
#[cfg(test)]
mod tests;

use super::RuntimeTypeSchema;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.schema-source.v1\0";

#[derive(Clone, Copy)]
pub(crate) struct RuntimeSchemaSourceLimits {
    pub nodes: u32,
    pub depth: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum RuntimeSchemaSourceError {
    #[error("schema source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid schema source domain")]
    Domain,
    #[error("unsupported schema source version {actual}")]
    Version { actual: u8 },
    #[error("schema source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("schema source arithmetic overflow")]
    Arithmetic,
    #[error("schema source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("schema source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("invalid schema source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("invalid schema source field coordinate {actual}")]
    Field { actual: u32 },
    #[error("schema source string is invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("schema source identity is invalid: {0}")]
    Identity(#[from] crate::entry::RuntimeIdentityError),
    #[error("schema source builtin payload inventory is invalid: {0}")]
    Builtin(#[from] super::RuntimeBuiltinSchemaError),
    #[error("schema source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeSchemaSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(value: TaskSemanticEncodingError) -> Self {
        Self::Resource(value)
    }
}
impl Error {
    fn reject(self, meter: &mut TaskSemanticMeter) -> Self {
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    nodes: usize,
    elements: usize,
    storage: usize,
    depth: usize,
}
struct Counts {
    limits: RuntimeSchemaSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeSchemaSourceLimits) -> Self {
        Self {
            limits,
            footprint: Footprint::default(),
        }
    }
    fn storage(&mut self, bytes: usize) -> Result<(), Error> {
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(bytes)
            .ok_or(Error::Arithmetic)?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn elements(&mut self, count: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )
    }
    fn array<T>(&mut self, count: usize) -> Result<(), Error> {
        self.elements(count)?;
        self.storage(count.checked_mul(size_of::<T>()).ok_or(Error::Arithmetic)?)
    }
    fn text(&mut self, length: usize) -> Result<(), Error> {
        self.storage(length)
    }
    fn node(&mut self, depth: usize) -> Result<(), Error> {
        limit("depth", depth, self.limits.depth as usize)?;
        self.footprint.depth = self.footprint.depth.max(depth);
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
        // Conservative peak storage includes the final node, a child scratch
        // slot, one active construction frame and the borrowed scan's counter.
        // Aggregate field slots below are additionally admitted, never omitted
        // because their inline child happens to have been counted here.
        self.storage(
            2 * size_of::<RuntimeTypeSchema>()
                + size_of::<reading::Frame<'_>>()
                + size_of::<usize>(),
        )
    }
}

pub(crate) struct PreparedRuntimeSchemaSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeSchemaSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeSchemaSource<'a> {
    pub(crate) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeSchemaSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let footprint = reading::inspect(bytes, limits)?;
            Ok(Self {
                bytes,
                limits,
                footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
    pub(crate) const fn nodes(&self) -> usize {
        self.footprint.nodes
    }
    pub(crate) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(crate) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(crate) fn materialize(
        self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTypeSchema, Error> {
        meter.status()?;
        let result = (|| {
            charge(meter, self.bytes.len())?;
            // The borrowed immutable packet already passed every metadata,
            // child inventory, depth and allocation check. Reconstruction
            // cannot publish a partial schema or bypass an owning constructor.
            Ok(reading::materialize(
                self.bytes,
                self.limits,
                self.footprint,
            ))
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual > maximum {
        Err(Error::Limit {
            kind,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(u64::try_from(bytes).map_err(|_| Error::Arithmetic)?)?;
    Ok(())
}
