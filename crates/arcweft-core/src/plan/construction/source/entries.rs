//! Original Entry inventory: launch paths, routes and complete typed roles.
//! Symbolic declarations re-enter the actual builder and acquire executable
//! authority only when the complete program passes its common seal.

mod encoding;
mod reading;
#[cfg(test)]
pub(super) mod tests;

use super::RuntimePlanSourceConstruction;
use crate::entry::schema::source::{RuntimeSchemaSourceError, RuntimeSchemaSourceLimits};
use crate::plan::RuntimeEntrySpec;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.entry-source.v1\0";
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeEntrySourceLimits {
    pub entries: u32,
    pub routes: u32,
    pub commands: u32,
    pub schema_nodes: u32,
    pub schema_depth: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeEntrySourceError {
    #[error("Entry source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid Entry source domain")]
    Domain,
    #[error("unsupported Entry source version {actual}")]
    Version { actual: u8 },
    #[error("Entry source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("Entry source arithmetic overflow")]
    Arithmetic,
    #[error("Entry source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("Entry source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("invalid Entry source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("Entry source contains an existing Entry inventory")]
    ExistingEntries,
    #[error("Entry source string is invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Entry source runtime path is invalid: {0}")]
    Path(#[from] crate::runtime_id::RuntimeIdError),
    #[error("Entry source identity is invalid: {0}")]
    Identity(#[from] crate::entry::RuntimeIdentityError),
    #[error("Entry source route path is invalid: {0}")]
    Route(#[from] crate::plan::RuntimeRoutePathError),
    #[error("Entry source schema is invalid: {0}")]
    Schema(#[from] RuntimeSchemaSourceError),
    #[error("Entry source construction failed: {0}")]
    Construction(Box<crate::plan::RuntimePlanBuildError>),
    #[error("Entry source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeEntrySourceError;
impl From<crate::plan::RuntimePlanBuildError> for Error {
    fn from(value: crate::plan::RuntimePlanBuildError) -> Self {
        Self::Construction(Box::new(value))
    }
}
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
    elements: usize,
    storage: usize,
    schemas: usize,
    routes: usize,
    commands: usize,
}
struct Counts {
    limits: RuntimeEntrySourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeEntrySourceLimits) -> Self {
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
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.storage(value.len())
    }
    fn entries(&mut self, count: usize) -> Result<(), Error> {
        limit("entries", count, self.limits.entries as usize)?;
        self.array::<RuntimeEntrySpec>(count)?;
        self.storage(
            count
                .checked_mul(size_of::<RuntimeEntrySpec>())
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn routes(&mut self, count: usize) -> Result<(), Error> {
        self.footprint.routes = self
            .footprint
            .routes
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit("routes", self.footprint.routes, self.limits.routes as usize)?;
        self.array::<crate::plan::RuntimeRouteSpec>(count)
    }
    fn commands(&mut self, count: usize) -> Result<(), Error> {
        self.footprint.commands = self
            .footprint
            .commands
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "commands",
            self.footprint.commands,
            self.limits.commands as usize,
        )?;
        self.array::<crate::entry::RuntimeCommandContract>(count)
    }
    fn schema_limits(&self) -> Result<RuntimeSchemaSourceLimits, Error> {
        Ok(RuntimeSchemaSourceLimits {
            nodes: u32::try_from(self.limits.schema_nodes as usize - self.footprint.schemas)
                .map_err(|_| Error::Arithmetic)?,
            depth: self.limits.schema_depth,
            elements: u32::try_from(self.limits.elements as usize - self.footprint.elements)
                .map_err(|_| Error::Arithmetic)?,
            encoded_bytes: self.limits.encoded_bytes,
            decoded_bytes: self.limits.decoded_bytes - self.footprint.storage,
        })
    }
    fn schema(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.schemas = self
            .footprint
            .schemas
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        limit(
            "schema nodes",
            self.footprint.schemas,
            self.limits.schema_nodes as usize,
        )?;
        self.elements(elements)?;
        self.storage(storage)
    }
}
pub(in crate::plan) struct PreparedRuntimeEntrySources<'a> {
    bytes: &'a [u8],
    limits: RuntimeEntrySourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeEntrySources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeEntrySourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let footprint = reading::inspect(bytes, limits, meter)?;
            Ok(Self {
                bytes,
                limits,
                footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
    ) -> Result<(), Error> {
        source.meter.status()?;
        let result = (|| {
            if !source.builder.entries.is_empty() {
                return Err(Error::ExistingEntries);
            }
            charge(&mut source.meter, self.bytes.len())?;
            let footprint = reading::inspect(self.bytes, self.limits, &mut source.meter)?;
            debug_assert_eq!(footprint, self.footprint);
            reading::materialize(self.bytes, self.limits, source)
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
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
