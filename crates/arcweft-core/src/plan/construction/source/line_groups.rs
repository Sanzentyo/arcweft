//! Original Line graph definitions, before liveness or execution authority.
//!
//! Group frames contain original definition/result/cleanup metadata, typed
//! handle sites, activation Flow Source, a preorder node prefix, cancellation
//! branches and the three outcome cleanup bodies. Captures and activation
//! exports are derived by the actual Line factory. Content Source remains the
//! sole owner of attachments; a Mark carries its genuine content/local identity.
//!
//! This module is not registered until its reader, writer, reconstruction and
//! actual-group tests are complete.

mod decoding;
mod encoding;
mod reading;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use super::flow::{RuntimeFlowSourceError, RuntimeFlowSourceHandles, RuntimeFlowSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{RuntimeLineTaskGroupSeed, RuntimeLineTaskGroupSeedId, RuntimePlanBuildError};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.line-group-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeLineGroupSourceLimits {
    pub groups: u32,
    /// Line nodes plus all nodes in every nested Flow Source frame.
    pub nodes: u32,
    /// Line topology depth; Flow and expression depths retain their own limits.
    pub depth: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub flow: RuntimeFlowSourceLimits,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeLineGroupSourceError {
    #[error("Line Source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid Line Source domain")]
    Domain,
    #[error("unsupported Line Source version {actual}")]
    Version { actual: u8 },
    #[error("Line Source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("Line Source arithmetic overflow")]
    Arithmetic,
    #[error("Line Source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("Line Source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("Line Source has an incomplete or excessive preorder prefix")]
    Prefix,
    #[error("Line Source has invalid {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("Line Source {kind} coordinate {actual} is outside {maximum} rows")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("Line Source contains a foreign or reordered {kind} handle")]
    Handle { kind: &'static str },
    #[error("Line Source needs an empty original group table, found {actual} groups")]
    ExistingGroups { actual: usize },
    #[error("Line Source text is invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Line Source character identity is invalid: {0}")]
    Character(#[from] arcweft_character::id::CharacterIdError),
    #[error("Line Source input identity is invalid: {0}")]
    Input(#[from] arcweft_interaction_model::id::IdentifierError),
    #[error("Line Source Flow is invalid: {0}")]
    Flow(Box<RuntimeFlowSourceError>),
    #[error("Line Source construction failed: {0}")]
    Construction(Box<RuntimePlanBuildError>),
    #[error("Line Source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeLineGroupSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl From<RuntimeFlowSourceError> for Error {
    fn from(error: RuntimeFlowSourceError) -> Self {
        Self::Flow(Box::new(error))
    }
}
impl From<RuntimePlanBuildError> for Error {
    fn from(error: RuntimePlanBuildError) -> Self {
        Self::Construction(Box::new(error))
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
}
struct Counts {
    limits: RuntimeLineGroupSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeLineGroupSourceLimits, groups: usize) -> Result<Self, Error> {
        limit("groups", groups, limits.groups as usize)?;
        let mut counts = Self {
            limits,
            footprint: Footprint::default(),
        };
        counts.array::<RuntimeLineTaskGroupSeed>(groups)?;
        counts.storage(
            groups
                .checked_mul(
                    size_of::<RuntimeLineTaskGroupSeedId>()
                        + size_of::<crate::line_task::LineTaskGroup>()
                        + size_of::<reading::Group<'_>>(),
                )
                .ok_or(Error::Arithmetic)?,
        )?;
        Ok(counts)
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
    fn text(&mut self, text: &str) -> Result<(), Error> {
        self.storage(text.len())
    }
    fn nodes(&mut self, count: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)
    }
    fn child(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.nodes(nodes)?;
        self.elements(elements)?;
        self.storage(storage)
    }
    fn flow(&self, bytes: usize) -> RuntimeFlowSourceLimits {
        RuntimeFlowSourceLimits {
            nodes: self.limits.flow.nodes.min(
                self.limits.nodes - u32::try_from(self.footprint.nodes).expect("admitted nodes"),
            ),
            elements: self.limits.flow.elements.min(
                self.limits.elements
                    - u32::try_from(self.footprint.elements).expect("admitted elements"),
            ),
            encoded_bytes: bytes.min(self.limits.flow.encoded_bytes),
            decoded_bytes: self
                .limits
                .flow
                .decoded_bytes
                .min(self.limits.decoded_bytes - self.footprint.storage),
            ..self.limits.flow
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeLineGroupSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeLineGroupSourceLimits,
    groups: usize,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeLineGroupSources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeLineGroupSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let (groups, footprint) = reading::inspect(bytes, limits, resources, meter)?;
            Ok(Self {
                bytes,
                limits,
                groups,
                footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
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
        handles: &RuntimeFlowSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<Box<[RuntimeLineTaskGroupSeedId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            // Authenticate the complete original namespace and every content/
            // mark pair before any Flow value or Line seed is reconstructed.
            charge(&mut source.meter, self.bytes.len())?;
            let groups = decoding::authenticate(&self, source, handles, resources)?;
            let mut result = Vec::with_capacity(self.groups);
            for group in groups {
                let seed = decoding::group(group, source, handles, resources)?;
                result.push(source.builder.push_line_task_group_seed(seed)?);
            }
            Ok(result.into_boxed_slice())
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
