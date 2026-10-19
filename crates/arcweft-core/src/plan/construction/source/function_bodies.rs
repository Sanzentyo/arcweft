//! Complete original function definitions. Declaration permissions and actual
//! executable-body effects remain separate roles. Every row header is checked
//! against genuine reserved declarations before any body value is retained.

mod decoding;
mod encoding;
mod reading;
#[cfg(test)]
pub(super) mod tests;

use super::expression::{RuntimeExpressionSourceError, RuntimeExpressionSourceLimits};
use super::flow::{RuntimeFlowSourceError, RuntimeFlowSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeEffectSet, RuntimeEffectSetError, RuntimeFunctionSiteBody, RuntimeFunctionSiteBodySeed,
    RuntimePlanBuildError,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.function-bodies-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeFunctionBodySourceLimits {
    pub rows: u32,
    pub functions: usize,
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub flow: RuntimeFlowSourceLimits,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeFunctionBodySourceError {
    #[error("function body Source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid function body Source domain")]
    Domain,
    #[error("unsupported function body Source version {actual}")]
    Version { actual: u8 },
    #[error("function body Source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("function body Source arithmetic overflow")]
    Arithmetic,
    #[error("function body Source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("function body Source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("function body Source has invalid {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("function body Source ordinal {actual} differs from {expected}")]
    Ordinal { actual: u32, expected: u32 },
    #[error("function body Source {kind} has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("function body Source contains a foreign or reordered function handle")]
    Handle,
    #[error("function body Source declaration identity differs at row {row}")]
    Definition { row: usize },
    #[error("function body Source effect labels are not strictly ordered")]
    EffectOrder,
    #[error("an expression body cannot declare executable effects")]
    ExpressionEffects,
    #[error("function body Source text is invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("function body Source effect identity is invalid: {0}")]
    Effect(#[from] arcweft_id::EffectIdError),
    #[error("function body Source effect set is invalid: {0}")]
    EffectSet(#[from] RuntimeEffectSetError),
    #[error("function body Source expression is invalid: {0}")]
    Expression(Box<RuntimeExpressionSourceError>),
    #[error("function body Source Flow is invalid: {0}")]
    Flow(Box<RuntimeFlowSourceError>),
    #[error("function body Source construction failed: {0}")]
    Construction(Box<RuntimePlanBuildError>),
    #[error("function body Source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeFunctionBodySourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl From<RuntimeExpressionSourceError> for Error {
    fn from(error: RuntimeExpressionSourceError) -> Self {
        Self::Expression(Box::new(error))
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
#[derive(Clone, Copy)]
struct Counts {
    limits: RuntimeFunctionBodySourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeFunctionBodySourceLimits, rows: usize) -> Result<Self, Error> {
        limit("rows", rows, limits.rows as usize)?;
        namespace("function rows", limits.functions, rows)?;
        let mut counts = Self {
            limits,
            footprint: Footprint::default(),
        };
        counts.elements(rows)?;
        counts.storage(
            rows.checked_mul(
                size_of::<reading::Row<'_>>()
                    + size_of::<decoding::CheckedRow<'_>>()
                    + 2 * size_of::<RuntimeFunctionSiteBodySeed>()
                    + size_of::<RuntimeFunctionSiteBody>()
                    + 2 * size_of::<RuntimeEffectSet>(),
            )
            .ok_or(Error::Arithmetic)?,
        )?;
        Ok(counts)
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
    fn effects(&mut self, count: usize, bytes: usize) -> Result<(), Error> {
        self.elements(count)?;
        self.storage(
            count
                .checked_mul(size_of::<arcweft_id::EffectId>())
                .and_then(|n| n.checked_add(bytes))
                .and_then(|n| n.checked_mul(2))
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn child(&mut self, nodes: usize, elements: usize, bytes: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
        self.elements(elements)?;
        self.storage(bytes)
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
    fn expression(&self, bytes: usize) -> RuntimeExpressionSourceLimits {
        let flow = self.flow(bytes);
        RuntimeExpressionSourceLimits {
            nodes: flow.nodes.min(flow.expression.nodes),
            elements: flow.elements.min(flow.expression.elements),
            encoded_bytes: flow.encoded_bytes.min(flow.expression.encoded_bytes),
            decoded_bytes: flow.decoded_bytes.min(flow.expression.decoded_bytes),
            ..flow.expression
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeFunctionBodySources<'a> {
    bytes: &'a [u8],
    limits: RuntimeFunctionBodySourceLimits,
    rows: usize,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeFunctionBodySources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeFunctionBodySourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let (rows, footprint) = reading::inspect(bytes, limits, resources, meter, |_| {})?;
            Ok(Self {
                bytes,
                limits,
                rows,
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
}
fn namespace(kind: &'static str, expected: usize, actual: usize) -> Result<(), Error> {
    if expected == actual {
        Ok(())
    } else {
        Err(Error::Namespace {
            kind,
            actual,
            expected,
        })
    }
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(Error::Limit {
            kind,
            actual,
            maximum,
        })
    }
}
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(u64::try_from(bytes).map_err(|_| Error::Arithmetic)?)?;
    Ok(())
}
