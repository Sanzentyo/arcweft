//! Original canonical Flow operations, before frame placement or execution.
//!
//! One prefix forest contains ordinary bodies, observer transactions and Line
//! actions/cleanup. Complete borrowed inspection precedes seed/value storage.
//! The actual scoped operation factory remains the only semantic admission.

mod decoding;
mod encoding;
mod metadata;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use super::audio_commands::{RuntimeAudioCommandSourceError, RuntimeAudioCommandSourceLimits};
use super::evaluated_effects::{
    RuntimeEvaluatedEffectSourceError, RuntimeEvaluatedEffectSourceLimits,
};
use super::expression::{
    RuntimeExpressionSourceError, RuntimeExpressionSourceHandles, RuntimeExpressionSourceLimits,
};
use super::host_calls::{RuntimeHostCallSourceError, RuntimeHostCallSourceLimits};
use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use super::project_calls::{RuntimeProjectCallSourceError, RuntimeProjectCallSourceLimits};
use super::selected_producers::{
    RuntimeSelectedProducerSourceError, RuntimeSelectedProducerSourceLimits,
};
use super::static_effects::{RuntimeStaticEffectSourceError, RuntimeStaticEffectSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    FlowOp, RuntimeDialogueContentPlanSeedId, RuntimeFlowOpSeed, RuntimePlanBuildError,
    RuntimeTaskPlanBuildCoordinate,
};
use crate::runtime_id::RuntimeDeferSiteId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.flow-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeFlowSourceLimits {
    /// Operation nodes plus all nested expression/pattern/effect Source nodes.
    pub nodes: u32,
    /// Canonical operation nesting; expression nesting has its own owner limit.
    pub depth: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub task_count: usize,
    pub content_count: usize,
    pub defer_count: usize,
    pub expression: RuntimeExpressionSourceLimits,
}

pub(in crate::plan) struct RuntimeFlowSourceHandles<'a> {
    pub expression: &'a RuntimeExpressionSourceHandles<'a>,
    pub contents: &'a [RuntimeDialogueContentPlanSeedId],
    pub defers: &'a [RuntimeDeferSiteId],
    pub tasks: &'a [RuntimeTaskPlanBuildCoordinate],
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeFlowSourceError {
    #[error("Flow Source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid Flow Source domain")]
    Domain,
    #[error("unsupported Flow Source version {actual}")]
    Version { actual: u8 },
    #[error("Flow Source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("Flow Source arithmetic overflow")]
    Arithmetic,
    #[error("Flow Source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("Flow Source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("Flow Source has an incomplete or excessive operation prefix")]
    Prefix,
    #[error("Flow Source contains a runtime-only operation")]
    Continuation,
    #[error("invalid Flow Source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("Flow Source {kind} coordinate {actual} is outside {maximum} rows")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("Flow Source contains a foreign or reordered {kind} handle")]
    Handle { kind: &'static str },
    #[error("Flow Source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("Flow Source text is invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Flow Source path is invalid: {0}")]
    Path(#[from] crate::runtime_id::RuntimeIdError),
    #[error("Flow Source identity is invalid: {0}")]
    Identity(#[from] crate::entry::RuntimeIdentityError),
    #[error("Flow Source expression is invalid: {0}")]
    Expression(Box<RuntimeExpressionSourceError>),
    #[error("Flow Source pattern is invalid: {0}")]
    Pattern(Box<RuntimePatternSourceError>),
    #[error("Flow Source audio command is invalid: {0}")]
    Audio(Box<RuntimeAudioCommandSourceError>),
    #[error("Flow Source static effect is invalid: {0}")]
    Static(Box<RuntimeStaticEffectSourceError>),
    #[error("Flow Source evaluated effect is invalid: {0}")]
    Evaluated(Box<RuntimeEvaluatedEffectSourceError>),
    #[error("Flow Source Host call is invalid: {0}")]
    Host(Box<RuntimeHostCallSourceError>),
    #[error("Flow Source selected producer is invalid: {0}")]
    Selected(Box<RuntimeSelectedProducerSourceError>),
    #[error("Flow Source project call is invalid: {0}")]
    Project(Box<RuntimeProjectCallSourceError>),
    #[error("Flow Source declaration name is invalid: {0}")]
    Name(#[from] arcweft_id::DeclarationNameError),
    #[error("Flow Source character identity is invalid: {0}")]
    Character(#[from] arcweft_character::id::CharacterIdError),
    #[error("Flow Source construction failed: {0}")]
    Construction(Box<RuntimePlanBuildError>),
    #[error("Flow Source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeFlowSourceError;
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
impl From<RuntimePlanBuildError> for Error {
    fn from(error: RuntimePlanBuildError) -> Self {
        Self::Construction(Box::new(error))
    }
}
impl From<RuntimePatternSourceError> for Error {
    fn from(error: RuntimePatternSourceError) -> Self {
        Self::Pattern(Box::new(error))
    }
}
impl From<RuntimeAudioCommandSourceError> for Error {
    fn from(error: RuntimeAudioCommandSourceError) -> Self {
        Self::Audio(Box::new(error))
    }
}
impl From<RuntimeStaticEffectSourceError> for Error {
    fn from(error: RuntimeStaticEffectSourceError) -> Self {
        Self::Static(Box::new(error))
    }
}
impl From<RuntimeEvaluatedEffectSourceError> for Error {
    fn from(error: RuntimeEvaluatedEffectSourceError) -> Self {
        Self::Evaluated(Box::new(error))
    }
}
impl From<RuntimeHostCallSourceError> for Error {
    fn from(error: RuntimeHostCallSourceError) -> Self {
        Self::Host(Box::new(error))
    }
}
impl From<RuntimeSelectedProducerSourceError> for Error {
    fn from(error: RuntimeSelectedProducerSourceError) -> Self {
        Self::Selected(Box::new(error))
    }
}
impl From<RuntimeProjectCallSourceError> for Error {
    fn from(error: RuntimeProjectCallSourceError) -> Self {
        Self::Project(Box::new(error))
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
    limits: RuntimeFlowSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeFlowSourceLimits, operations: usize) -> Result<Self, Error> {
        let mut counts = Self {
            limits,
            footprint: Footprint::default(),
        };
        limit("operation nodes", operations, limits.nodes as usize)?;
        counts.footprint.nodes = operations;
        // All rows, seed-stack slots, final operation slots and bounded prefix
        // counters are admitted before the first traversal allocation/value.
        counts.storage(
            operations
                .checked_mul(
                    size_of::<metadata::Row<'_>>()
                        + 2 * size_of::<RuntimeFlowOpSeed>()
                        + size_of::<FlowOp>()
                        + 2 * size_of::<usize>(),
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
    fn body(&mut self, count: usize) -> Result<(), Error> {
        self.elements(count)
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.storage(value.len())
    }
    fn child(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
        self.elements(elements)?;
        self.storage(storage)
    }
    fn expression(&self, bytes: usize) -> RuntimeExpressionSourceLimits {
        RuntimeExpressionSourceLimits {
            nodes: (self.limits.nodes
                - u32::try_from(self.footprint.nodes).expect("admitted nodes"))
            .min(self.limits.expression.nodes),
            elements: (self.limits.elements
                - u32::try_from(self.footprint.elements).expect("admitted elements"))
            .min(self.limits.expression.elements),
            encoded_bytes: bytes.min(self.limits.expression.encoded_bytes),
            decoded_bytes: (self.limits.decoded_bytes - self.footprint.storage)
                .min(self.limits.expression.decoded_bytes),
            ..self.limits.expression
        }
    }
    fn pattern(&self, bytes: usize) -> RuntimePatternSourceLimits {
        let limits = self.expression(bytes);
        RuntimePatternSourceLimits {
            nodes: limits.nodes,
            elements: limits.elements,
            encoded_bytes: limits.encoded_bytes,
            decoded_bytes: limits.decoded_bytes,
            type_count: limits.type_count,
            local_count: limits.local_count,
        }
    }
    fn audio(&self, bytes: usize) -> RuntimeAudioCommandSourceLimits {
        let expression = self.expression(bytes);
        RuntimeAudioCommandSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: expression.encoded_bytes,
            decoded_bytes: expression.decoded_bytes,
            expression,
        }
    }
    fn static_effect(&self, bytes: usize) -> RuntimeStaticEffectSourceLimits {
        let expression = self.expression(bytes);
        RuntimeStaticEffectSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: expression.encoded_bytes,
            decoded_bytes: expression.decoded_bytes,
            literal: crate::value::RuntimeLiteralSourceLimits {
                encoded_bytes: expression.encoded_bytes,
                decoded_bytes: expression.decoded_bytes,
                elements: expression.elements,
            },
        }
    }
    fn evaluated(&self, bytes: usize) -> RuntimeEvaluatedEffectSourceLimits {
        let expression = self.expression(bytes);
        RuntimeEvaluatedEffectSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: expression.encoded_bytes,
            decoded_bytes: expression.decoded_bytes,
            expression,
        }
    }
    fn host(&self, bytes: usize) -> RuntimeHostCallSourceLimits {
        let expression = self.expression(bytes);
        RuntimeHostCallSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: expression.encoded_bytes,
            decoded_bytes: expression.decoded_bytes,
            expression,
            task_count: self.limits.task_count,
        }
    }
    fn selected(&self, bytes: usize) -> RuntimeSelectedProducerSourceLimits {
        let expression = self.expression(bytes);
        RuntimeSelectedProducerSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: expression.encoded_bytes,
            decoded_bytes: expression.decoded_bytes,
            expression,
            task_count: self.limits.task_count,
        }
    }
    fn project(&self, bytes: usize) -> RuntimeProjectCallSourceLimits {
        let expression = self.expression(bytes);
        RuntimeProjectCallSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: expression.encoded_bytes,
            decoded_bytes: expression.decoded_bytes,
            expression,
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeFlowSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeFlowSourceLimits,
    roots: usize,
    operations: usize,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeFlowSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeFlowSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let (roots, operations, footprint) = inspect(bytes, limits, resources, meter, |_| {})?;
            Ok(Self {
                bytes,
                limits,
                roots,
                operations,
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
    ) -> Result<Vec<RuntimeFlowOpSeed>, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits)?;
            charge(&mut source.meter, self.bytes.len())?;
            let mut rows = Vec::with_capacity(self.operations);
            let (_, _, footprint) = inspect(
                self.bytes,
                self.limits,
                resources,
                &mut source.meter,
                |row| rows.push(row),
            )?;
            debug_assert_eq!(footprint, self.footprint);
            let mut seeds = Vec::with_capacity(self.operations);
            for row in rows.into_iter().rev() {
                let op = decoding::Decoder {
                    source,
                    handles,
                    resources,
                    limits: self.limits,
                    seeds: &mut seeds,
                }
                .operation(row)?;
                seeds.push(op);
            }
            let roots = (0..self.roots)
                .map(|_| seeds.pop().expect("complete admitted Flow prefix"))
                .collect();
            debug_assert!(seeds.is_empty());
            Ok(roots)
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}

fn inspect<'a>(
    bytes: &'a [u8],
    limits: RuntimeFlowSourceLimits,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
    mut visit: impl FnMut(metadata::Row<'a>),
) -> Result<(usize, usize, Footprint), Error> {
    let mut cursor = metadata::Cursor::new(bytes);
    let (roots, operations) = cursor.header()?;
    if roots > operations {
        return Err(Error::Prefix);
    }
    let mut counts = Counts::new(limits, operations)?;
    counts.body(roots)?;
    let mut pending = Vec::new();
    if roots != 0 {
        pending.push(roots);
    }
    let mut remaining = roots;
    for index in 0..operations {
        while pending.last() == Some(&0) {
            pending.pop();
        }
        let depth = pending.len();
        limit("depth", depth, limits.depth as usize)?;
        let current = pending.last_mut().ok_or(Error::Prefix)?;
        *current -= 1;
        remaining = remaining.checked_sub(1).ok_or(Error::Prefix)?;
        let row = cursor.operation(&mut counts, resources, meter)?;
        remaining = remaining
            .checked_add(row.children)
            .ok_or(Error::Arithmetic)?;
        if remaining > operations - index - 1 {
            return Err(Error::Prefix);
        }
        if row.children != 0 {
            pending.push(row.children);
        }
        visit(row);
    }
    if remaining != 0 {
        return Err(Error::Prefix);
    }
    cursor.finish()?;
    Ok((roots, operations, counts.footprint))
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
