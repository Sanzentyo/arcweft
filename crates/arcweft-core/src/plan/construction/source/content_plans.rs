//! Original content applications reference the one registered template ABI.
//! Declarations precede Line groups; their links bind only genuine group handles.

mod cursor;
mod decoding;
mod encoding;
#[cfg(test)]
pub(super) mod tests;

use super::expression::{RuntimeExpressionSourceError, RuntimeExpressionSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeDialogueContentApplicationKey, RuntimeDialogueContentPlan,
    RuntimeDialogueContentPlanSeed, RuntimeDialogueContentPlanSeedId, RuntimeDialogueEffectSite,
    RuntimeDialogueEffectSiteSeed, RuntimeDialogueMark, RuntimeDialogueValueSite,
    RuntimeDialogueValueSiteSeed, RuntimePlanBuildError,
};
use crate::runtime_id::{RuntimeDialogueContentPlanId, RuntimeIdSegment};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::RuntimeExpr;

const DOMAIN: &[u8] = b"arcweft.runtime.content-plan-sources.v1\0";
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeContentPlanSourceLimits {
    pub rows: u32,
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    /// Logical owned payload and named temporary arrays. Row/element bounds
    /// also bound standard-library index nodes and allocator overhead.
    pub decoded_bytes: usize,
    pub function_count: usize,
    pub group_count: usize,
    pub expression: RuntimeExpressionSourceLimits,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeContentPlanSourceError {
    #[error("content-plan source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid content-plan source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("content-plan source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("content-plan source arithmetic overflow")]
    Arithmetic,
    #[error("content-plan source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("content-plan source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("content-plan source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("content-plan source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("content-plan source has a foreign or reordered {kind} handle")]
    Handle { kind: &'static str },
    #[error("content-plan source runtime path is invalid: {0}")]
    Path(#[from] crate::runtime_id::RuntimeIdError),
    #[error("content-plan source contains invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("content-plan source expression is invalid: {0}")]
    Expression(#[from] RuntimeExpressionSourceError),
    #[error("content-plan source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("content-plan source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeContentPlanSourceError;
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
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Footprint {
    nodes: usize,
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeContentPlanSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeContentPlanSourceLimits) -> Self {
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
    fn rows(&mut self, n: usize) -> Result<(), Error> {
        limit("rows", n, self.limits.rows as usize)?;
        // Index entries contain cloned line keys. This counts their exact
        // logical payload, not an opaque estimate of BTreeMap node allocation.
        self.array(
            n,
            size_of::<RuntimeDialogueContentPlanSeed>()
                + size_of::<RuntimeDialogueContentPlan>()
                + size_of::<RuntimeDialogueContentPlanSeedId>()
                + size_of::<cursor::Row<'_>>()
                + 2 * size_of::<(
                    RuntimeDialogueContentApplicationKey,
                    RuntimeDialogueContentPlanId,
                )>(),
        )
    }
    fn line(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, 3 * size_of::<RuntimeIdSegment>())
    }
    fn text(&mut self, n: usize, copies: usize) -> Result<(), Error> {
        self.storage(n.checked_mul(copies).ok_or(Error::Arithmetic)?)
    }
    fn values(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            size_of::<RuntimeDialogueValueSiteSeed>() + size_of::<RuntimeDialogueValueSite>(),
        )
    }
    fn effects(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            size_of::<RuntimeDialogueEffectSiteSeed>() + size_of::<RuntimeDialogueEffectSite>(),
        )
    }
    fn marks(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, size_of::<String>() + size_of::<RuntimeDialogueMark>())
    }
    fn captures(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            size_of::<crate::plan::RuntimeExprSeed>() + size_of::<RuntimeExpr>(),
        )
    }
    fn child_limits(&self, n: usize) -> RuntimeExpressionSourceLimits {
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
}
pub(in crate::plan) struct PreparedRuntimeContentPlanSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeContentPlanSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeContentPlanSources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeContentPlanSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = cursor::Cursor::new(bytes, limits);
            let mut counts = Counts::new(limits);
            let n = cursor.header()?;
            counts.rows(n)?;
            for _ in 0..n {
                cursor.row()?.preflight(&mut counts, resources, meter)?;
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
