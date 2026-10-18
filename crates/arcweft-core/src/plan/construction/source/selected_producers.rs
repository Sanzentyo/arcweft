//! Source of one selected Need start. The wire carries original declarations
//! and a construction coordinate, never a completed task key or launch proof.

mod encoding;
mod reading;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use super::expression::{RuntimeExpressionSourceError, RuntimeExpressionSourceLimits};
use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::{RuntimeFlowOpSeed, RuntimePlanBuildError, RuntimeTaskPlanBuildCoordinate};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{NeedProducerPlanError, NeedProducerTaskPlan, RuntimeHostArgumentTemplate};
use crate::value::RuntimeExpr;

const DOMAIN: &[u8] = b"arcweft.runtime.selected-producer-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeSelectedProducerSourceLimits {
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub task_count: usize,
    pub expression: RuntimeExpressionSourceLimits,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeSelectedProducerSourceError {
    #[error("selected producer source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid selected producer source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("selected producer source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("selected producer source arithmetic overflow")]
    Arithmetic,
    #[error("selected producer source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("selected producer source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("selected producer source {kind} coordinate {actual} is outside its {maximum} rows")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("selected producer source has a foreign or reordered task coordinate")]
    TaskCoordinate,
    #[error("selected producer source is not a selected Need start")]
    Operation,
    #[error("selected producer source text is not UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("selected producer source expression is invalid: {0}")]
    Expression(#[source] Box<RuntimeExpressionSourceError>),
    #[error("selected producer source binding is invalid: {0}")]
    Pattern(#[source] Box<RuntimePatternSourceError>),
    #[error("selected producer source request is invalid: {0}")]
    Plan(#[from] NeedProducerPlanError),
    #[error("selected producer source construction failed: {0}")]
    Construction(#[source] Box<RuntimePlanBuildError>),
    #[error(
        "selected producer source needs {required} output bytes, only {remaining} are reserved"
    )]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeSelectedProducerSourceError;
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
impl From<RuntimePatternSourceError> for Error {
    fn from(error: RuntimePatternSourceError) -> Self {
        Self::Pattern(Box::new(error))
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
    limits: RuntimeSelectedProducerSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeSelectedProducerSourceLimits) -> Result<Self, Error> {
        let mut result = Self {
            limits,
            footprint: Footprint::default(),
        };
        // Both construction and admitted operation storage, including the
        // selected static request. Child codecs account for their own graphs.
        result.storage(
            size_of::<RuntimeFlowOpSeed>()
                + size_of::<crate::plan::FlowOp>()
                + 2 * size_of::<NeedProducerTaskPlan>(),
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
    fn text(&mut self, n: usize) -> Result<(), Error> {
        self.storage(n.checked_mul(2).ok_or(Error::Arithmetic)?)
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
    fn expression(&self, bytes: usize) -> RuntimeExpressionSourceLimits {
        RuntimeExpressionSourceLimits {
            nodes: (self.limits.nodes
                - u32::try_from(self.footprint.nodes)
                    .expect("node count is bounded by its u32 limit"))
            .min(self.limits.expression.nodes),
            elements: (self.limits.elements
                - u32::try_from(self.footprint.elements)
                    .expect("element count is bounded by its u32 limit"))
            .min(self.limits.expression.elements),
            encoded_bytes: bytes.min(self.limits.expression.encoded_bytes),
            decoded_bytes: (self.limits.decoded_bytes - self.footprint.storage)
                .min(self.limits.expression.decoded_bytes),
            ..self.limits.expression
        }
    }
    fn pattern(&self, bytes: usize) -> RuntimePatternSourceLimits {
        let expression = self.expression(bytes);
        RuntimePatternSourceLimits {
            nodes: expression.nodes,
            elements: expression.elements,
            encoded_bytes: bytes,
            decoded_bytes: expression.decoded_bytes,
            type_count: expression.type_count,
            local_count: expression.local_count,
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeSelectedProducerSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeSelectedProducerSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeSelectedProducerSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeSelectedProducerSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut counts = Counts::new(limits)?;
            let mut cursor = reading::Cursor::new(bytes);
            let row = cursor.row(&mut counts)?;
            cursor.finish()?;
            let pattern = RuntimePattern::preflight_sources(
                row.binding,
                counts.pattern(row.binding.len()),
                resources,
                meter,
            )?;
            counts.child(
                pattern.node_count(),
                pattern.elements(),
                pattern.storage_bytes(),
            )?;
            let mut arguments = reading::Cursor::new(row.arguments);
            let mut names = row.schema.bindings();
            for _ in 0..row.argument_count {
                let argument = arguments.argument()?;
                names.check_next(argument.name)?;
                let child = RuntimeExpr::preflight_sources(
                    argument.value,
                    counts.expression(argument.value.len()),
                    resources,
                    meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
            }
            arguments.finish()?;
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

    /// The caller supplies a real reservation from this original builder.
    /// Neither a source ordinal nor an expected digest can mint that handle.
    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
        handles: &super::expression::RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeFlowOpSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            let actual = source
                .builder
                .task_plan_definitions
                .lock()
                .map_err(|_| RuntimePlanBuildError::Poisoned)?
                .as_ref()
                .ok_or(Error::TaskCoordinate)?
                .len();
            if actual != self.limits.task_count
                || !source
                    .builder
                    .task_coordinate_owner(u32::try_from(actual).map_err(|_| Error::Arithmetic)?)
                    .contains(coordinate)
            {
                return Err(Error::TaskCoordinate);
            }
            charge(&mut source.meter, self.bytes.len())?;
            let mut counts = Counts::new(self.limits)?;
            let mut cursor = reading::Cursor::new(self.bytes);
            let row = cursor.row(&mut counts)?;
            cursor.finish()?;
            if coordinate.ordinal() != row.task {
                return Err(Error::TaskCoordinate);
            }
            let plan = row.schema.materialize(source)?;
            let pattern = RuntimePattern::preflight_sources(
                row.binding,
                counts.pattern(row.binding.len()),
                resources,
                &mut source.meter,
            )?;
            counts.child(
                pattern.node_count(),
                pattern.elements(),
                pattern.storage_bytes(),
            )?;
            let binding = pattern.materialize(source, resources)?;
            let mut arguments = Vec::with_capacity(row.argument_count);
            let mut cursor = reading::Cursor::new(row.arguments);
            for _ in 0..row.argument_count {
                let argument = cursor.argument()?;
                let child = RuntimeExpr::preflight_sources(
                    argument.value,
                    counts.expression(argument.value.len()),
                    resources,
                    &mut source.meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
                let value = child.materialize(source, handles, resources)?;
                arguments.push(match argument.name {
                    Some(name) => {
                        RuntimeHostArgumentTemplate::named(argument.identity, name, value)
                    }
                    None => RuntimeHostArgumentTemplate::positional(argument.identity, value),
                });
            }
            cursor.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(RuntimeFlowOpSeed::StartNeedProducer {
                binding,
                target: crate::plan::RuntimeNeedProducerStartTargetSeed {
                    plan,
                    task_plan: coordinate.clone(),
                    arguments,
                },
            })
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
    meter.charge_work(
        u64::try_from(bytes)
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(3)
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
