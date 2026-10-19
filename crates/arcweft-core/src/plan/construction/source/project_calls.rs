//! Project-call source data retains the caller's physical/logical argument map.
//! Callable states own transitions, captures and defaults; the actual operation
//! factory and common seal remain the only executable admission authority.

mod cursor;
mod decoding;
mod encoding;
#[cfg(test)]
pub(super) mod tests;

use super::expression::{RuntimeExpressionSourceError, RuntimeExpressionSourceLimits};
use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::{
    FlowOp, RuntimeFlowOpSeed, RuntimePlanBuildError, RuntimeProjectCallOperand,
    RuntimeProjectCallOperandSeed, RuntimeProjectCallOrdinaryMaterialization,
    RuntimeProjectCallOrdinaryMaterializationSeed, RuntimeProjectCallSite,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::RuntimeExpr;

const DOMAIN: &[u8] = b"arcweft.runtime.project-call-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeProjectCallSourceLimits {
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    /// Named seed/final payload and child-codec working storage, not allocator RSS.
    pub decoded_bytes: usize,
    pub expression: RuntimeExpressionSourceLimits,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeProjectCallSourceError {
    #[error("project-call source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid project-call source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("project-call source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("project-call source arithmetic overflow")]
    Arithmetic,
    #[error("project-call source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("project-call source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error(
        "project-call source {kind} reference {actual} is outside its namespace of {maximum} rows"
    )]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("project-call source expression is invalid: {0}")]
    Expression(#[from] RuntimeExpressionSourceError),
    #[error("project-call source pattern is invalid: {0}")]
    Pattern(#[from] RuntimePatternSourceError),
    #[error("project-call source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("project-call source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeProjectCallSourceError;
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
    limits: RuntimeProjectCallSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeProjectCallSourceLimits) -> Result<Self, Error> {
        let mut result = Self {
            limits,
            footprint: Footprint::default(),
        };
        result.child(1, 0, 0)?;
        result.array(
            1,
            size_of::<RuntimeFlowOpSeed>()
                + size_of::<FlowOp>()
                + size_of::<RuntimeProjectCallSite>()
                + size_of::<cursor::Row<'_>>(),
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
    fn operands(&mut self, n: usize) -> Result<(), Error> {
        // The existing admission owner also keeps logical source-set entries
        // and a temporary source-index copy. Tree/allocator overhead is governed
        // by the element cap, rather than an opaque physical-byte estimate.
        self.array(
            n,
            size_of::<RuntimeProjectCallOperandSeed>()
                + size_of::<RuntimeProjectCallOperand>()
                + 2 * size_of::<u32>(),
        )
    }
    fn ordinary(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            size_of::<RuntimeProjectCallOrdinaryMaterializationSeed>()
                + size_of::<RuntimeProjectCallOrdinaryMaterialization>(),
        )
    }
    fn sources(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, 2 * size_of::<u32>())
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
    fn pattern(&self, n: usize) -> RuntimePatternSourceLimits {
        let expr = self.expression(n);
        RuntimePatternSourceLimits {
            nodes: expr.nodes,
            elements: expr.elements,
            encoded_bytes: expr.encoded_bytes,
            decoded_bytes: expr.decoded_bytes,
            type_count: expr.type_count,
            local_count: expr.local_count,
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
pub(in crate::plan) struct PreparedRuntimeProjectCallSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeProjectCallSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeProjectCallSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeProjectCallSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = cursor::Cursor::new(bytes, limits);
            let row = cursor.row()?;
            cursor.finish()?;
            let mut counts = Counts::new(limits)?;
            counts.preflight_expression(row.callee, resources, meter)?;
            counts.operands(row.operand_count)?;
            let mut operands = cursor::Cursor::new(row.operands, limits);
            for _ in 0..row.operand_count {
                let (_, expr) = operands.operand()?;
                counts.preflight_expression(expr, resources, meter)?;
            }
            operands.finish()?;
            counts.ordinary(row.ordinary_count)?;
            let mut ordinary = cursor::Cursor::new(row.ordinary, limits);
            for index in 0..row.ordinary_count {
                if let cursor::Ordinary::Rest { sources, .. } =
                    ordinary.ordinary(index, row.operand_count)?
                {
                    counts.sources(sources.len() / 4)?;
                }
            }
            ordinary.finish()?;
            let pattern = RuntimePattern::preflight_sources(
                row.result,
                counts.pattern(row.result.len()),
                resources,
                meter,
            )?;
            counts.child(
                pattern.node_count(),
                pattern.elements(),
                pattern.storage_bytes(),
            )?;
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
