//! Original evaluated effects, restored only into the existing seed grammar.
//! The actual operation factory and common seal own executable admission.

mod cursor;
mod decoding;
mod encoding;
#[cfg(test)]
mod tests;

use super::expression::{RuntimeExpressionSourceError, RuntimeExpressionSourceLimits};
use crate::effect::{RuntimeEffectFieldExpr, RuntimeIdentityDecodeError};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{FlowOp, RuntimeEffectFieldSeed, RuntimeFlowOpSeed};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::RuntimeExpr;

const DOMAIN: &[u8] = b"arcweft.runtime.evaluated-effect-source.v1\0";
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeEvaluatedEffectSourceLimits {
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    /// Named seed/final payload and child-codec storage, not allocator RSS.
    pub decoded_bytes: usize,
    pub expression: RuntimeExpressionSourceLimits,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeEvaluatedEffectSourceError {
    #[error("evaluated-effect source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid evaluated-effect source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("evaluated-effect source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("evaluated-effect source arithmetic overflow")]
    Arithmetic,
    #[error("evaluated-effect source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("evaluated-effect source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("evaluated-effect source string is invalid: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("evaluated-effect source identity is invalid: {0}")]
    Identity(#[from] RuntimeIdentityDecodeError),
    #[error("evaluated-effect source expression is invalid: {0}")]
    Expression(#[from] RuntimeExpressionSourceError),
    #[error("evaluated-effect source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeEvaluatedEffectSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
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
    limits: RuntimeEvaluatedEffectSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeEvaluatedEffectSourceLimits) -> Result<Self, Error> {
        let mut result = Self {
            limits,
            footprint: Footprint::default(),
        };
        result.child(1, 0, 0)?;
        result.array(
            1,
            size_of::<RuntimeFlowOpSeed>() + size_of::<FlowOp>() + size_of::<cursor::Row<'_>>(),
        )?;
        Ok(result)
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
    fn fields(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            size_of::<RuntimeEffectFieldSeed>() + size_of::<RuntimeEffectFieldExpr>(),
        )
    }
    fn child(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
        self.array(elements, 0)?;
        self.storage(storage)
    }
    fn expression(&self, n: usize) -> RuntimeExpressionSourceLimits {
        RuntimeExpressionSourceLimits {
            nodes: self
                .limits
                .expression
                .nodes
                .min((self.limits.nodes as usize - self.footprint.nodes) as u32),
            elements: self
                .limits
                .expression
                .elements
                .min((self.limits.elements as usize - self.footprint.elements) as u32),
            encoded_bytes: self.limits.expression.encoded_bytes.min(n),
            decoded_bytes: self
                .limits
                .expression
                .decoded_bytes
                .min(self.limits.decoded_bytes - self.footprint.storage),
            ..self.limits.expression
        }
    }
    fn preflight_expression(
        &mut self,
        bytes: &[u8],
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let child =
            RuntimeExpr::preflight_sources(bytes, self.expression(bytes.len()), resources, meter)?;
        self.child(child.node_count(), child.elements(), child.storage_bytes())
    }
}
pub(in crate::plan) struct PreparedRuntimeEvaluatedEffectSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeEvaluatedEffectSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeEvaluatedEffectSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeEvaluatedEffectSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = cursor::Cursor::new(bytes);
            let row = cursor.row()?;
            cursor.finish()?;
            let mut counts = Counts::new(limits)?;
            row.preflight(&mut counts, resources, meter)?;
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
fn charge(meter: &mut TaskSemanticMeter, n: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(n)
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(3)
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
