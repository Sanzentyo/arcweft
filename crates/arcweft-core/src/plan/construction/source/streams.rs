//! Complete original Stream operations enter the existing construction owner.
//! All prefix framing and nested value graphs are admitted before seed storage.

mod decoding;
mod encoding;
mod reading;
#[cfg(test)]
mod tests;

use super::expression::{RuntimeExpressionSourceError, RuntimeExpressionSourceLimits};
use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{RuntimePlanBuildError, RuntimeStreamOpSeed, RuntimeStreamPlanSeed};
use crate::runtime_id::RuntimeIdSegment;
use crate::stream::{StreamMatchArm, StreamOp, StreamPlan};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.stream-sources.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeStreamSourceLimits {
    pub rows: u32,
    /// Operations, expression nodes and directly framed pattern nodes.
    /// Patterns/literals inside expressions also share element/storage caps.
    pub nodes: u32,
    pub depth: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub expression: RuntimeExpressionSourceLimits,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeStreamSourceError {
    #[error("Stream source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid Stream source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("Stream source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("Stream source arithmetic overflow")]
    Arithmetic,
    #[error("Stream source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("Stream source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("Stream source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("Stream source needs an empty Stream namespace")]
    Namespace,
    #[error("Stream source path is invalid: {0}")]
    Path(#[from] crate::runtime_id::RuntimeIdError),
    #[error("Stream source contains invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Stream source expression is invalid: {0}")]
    Expression(#[from] RuntimeExpressionSourceError),
    #[error("Stream source pattern is invalid: {0}")]
    Pattern(#[from] RuntimePatternSourceError),
    #[error("Stream source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("Stream source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeStreamSourceError;
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
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Footprint {
    nodes: usize,
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeStreamSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeStreamSourceLimits) -> Self {
        Self {
            limits,
            footprint: Footprint::default(),
        }
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
    fn array(&mut self, n: usize, width: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(n)
            .ok_or(Error::Arithmetic)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        self.storage(n.checked_mul(width).ok_or(Error::Arithmetic)?)
    }
    fn nodes(&mut self, n: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(n)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)
    }
    fn rows(&mut self, n: usize) -> Result<(), Error> {
        limit("rows", n, self.limits.rows as usize)?;
        self.array(
            n,
            size_of::<reading::Root<'_>>()
                + size_of::<StreamPlan>()
                + size_of::<RuntimeStreamPlanSeed>(),
        )
    }
    fn operation_storage(n: usize) -> Result<usize, Error> {
        n.checked_mul(
            size_of::<reading::Row<'_>>()
                + size_of::<Option<RuntimeStreamOpSeed>>()
                + size_of::<StreamOp>()
                + size_of::<reading::Scope>()
                + 2 * size_of::<(&StreamOp, usize)>(),
        )
        .ok_or(Error::Arithmetic)
    }
    fn operations(&mut self, n: usize) -> Result<(), Error> {
        self.nodes(n)?;
        self.array(n, 0)?;
        self.storage(Self::operation_storage(n)?)
    }
    fn children(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, size_of::<RuntimeStreamOpSeed>() + size_of::<StreamOp>())
    }
    fn arms(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            size_of::<crate::plan::RuntimeStreamMatchArmSeed>() + size_of::<StreamMatchArm>(),
        )
    }
    fn segments(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, 2 * size_of::<RuntimeIdSegment>())
    }
    fn text(&mut self, n: usize) -> Result<(), Error> {
        self.storage(n.checked_mul(2).ok_or(Error::Arithmetic)?)
    }
    fn expression_limits(
        &self,
        depth: usize,
        encoded_bytes: usize,
    ) -> Result<RuntimeExpressionSourceLimits, Error> {
        limit("depth", depth, self.limits.depth as usize)?;
        Ok(RuntimeExpressionSourceLimits {
            nodes: self
                .limits
                .expression
                .nodes
                .min((self.limits.nodes as usize - self.footprint.nodes) as u32),
            depth: self
                .limits
                .expression
                .depth
                .min(self.limits.depth - depth as u32),
            elements: self
                .limits
                .expression
                .elements
                .min((self.limits.elements as usize - self.footprint.elements) as u32),
            encoded_bytes: self.limits.expression.encoded_bytes.min(encoded_bytes),
            decoded_bytes: self
                .limits
                .expression
                .decoded_bytes
                .min(self.limits.decoded_bytes - self.footprint.storage),
            ..self.limits.expression
        })
    }
    fn pattern_limits(&self, encoded_bytes: usize) -> RuntimePatternSourceLimits {
        RuntimePatternSourceLimits {
            nodes: self
                .limits
                .expression
                .nodes
                .min((self.limits.nodes as usize - self.footprint.nodes) as u32),
            elements: (self.limits.elements as usize - self.footprint.elements) as u32,
            encoded_bytes,
            decoded_bytes: self.limits.decoded_bytes - self.footprint.storage,
            type_count: self.limits.expression.type_count,
            local_count: self.limits.expression.local_count,
        }
    }
    fn child(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.nodes(nodes)?;
        self.array(elements, 0)?;
        self.storage(storage)
    }
}
pub(in crate::plan) struct PreparedRuntimeStreamSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeStreamSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeStreamSources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeStreamSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut reader = reading::Cursor::new(bytes);
            let mut counts = Counts::new(limits);
            let rows = reader.header(&mut counts)?;
            for _ in 0..rows {
                let root = reader.root(&mut counts)?;
                reader.body(&root, &mut counts, resources, meter, |_| {})?;
            }
            reader.finish()?;
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
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
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
fn charge(meter: &mut TaskSemanticMeter, n: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(n)
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(3)
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
