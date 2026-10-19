//! Original direct Host calls re-enter the actual construction owner.
//! Borrowed framing and all nested values pass admission before seed storage.

mod encoding;
mod reading;
#[cfg(test)]
pub(super) mod tests;

use super::RuntimePlanSourceConstruction;
use super::expression::{
    RuntimeExpressionSourceError, RuntimeExpressionSourceHandles, RuntimeExpressionSourceLimits,
};
use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::{
    FlowOp, RuntimeFlowOpSeed, RuntimeHostCallTargetSeed, RuntimePlanBuildError,
    RuntimeTaskPlanBuildCoordinate,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{HostCallProducerDefinition, NamedHostArg, RuntimeHostArgumentTemplate};
use crate::value::RuntimeExpr;

const DOMAIN: &[u8] = b"arcweft.runtime.host-call-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeHostCallSourceLimits {
    /// Cumulative nested expression and directly framed pattern nodes.
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub task_count: usize,
    pub expression: RuntimeExpressionSourceLimits,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeHostCallSourceError {
    #[error("Host call source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid Host call source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("Host call source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("Host call source arithmetic overflow")]
    Arithmetic,
    #[error("Host call source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("Host call source has trailing bytes at byte {offset}")]
    Trailing { offset: usize },
    #[error("Host call source {kind} coordinate {actual} is outside its {maximum} rows")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("Host call source has a foreign or reordered task coordinate")]
    TaskCoordinate,
    #[error("Host call source is not a direct Host call")]
    Operation,
    #[error("Host call source text is not UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Host call source expression is invalid: {0}")]
    Expression(#[source] Box<RuntimeExpressionSourceError>),
    #[error("Host call source binding is invalid: {0}")]
    Pattern(#[source] Box<RuntimePatternSourceError>),
    #[error("Host call source construction failed: {0}")]
    Construction(#[source] Box<RuntimePlanBuildError>),
    #[error("Host call source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeHostCallSourceError;
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
    limits: RuntimeHostCallSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeHostCallSourceLimits) -> Result<Self, Error> {
        let mut counts = Self {
            limits,
            footprint: Footprint::default(),
        };
        counts.storage(size_of::<RuntimeFlowOpSeed>() + size_of::<FlowOp>())?;
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
    fn array(&mut self, count: usize, width: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        self.storage(count.checked_mul(width).ok_or(Error::Arithmetic)?)
    }
    fn text(&mut self, bytes: usize) -> Result<(), Error> {
        self.storage(bytes.checked_mul(2).ok_or(Error::Arithmetic)?)
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
                - u32::try_from(self.footprint.nodes).expect("bounded source nodes"))
            .min(self.limits.expression.nodes),
            elements: (self.limits.elements
                - u32::try_from(self.footprint.elements).expect("bounded source elements"))
            .min(self.limits.expression.elements),
            encoded_bytes: bytes.min(self.limits.expression.encoded_bytes),
            decoded_bytes: (self.limits.decoded_bytes - self.footprint.storage)
                .min(self.limits.expression.decoded_bytes),
            ..self.limits.expression
        }
    }
    fn pattern(&self, bytes: usize) -> RuntimePatternSourceLimits {
        let child = self.expression(bytes);
        RuntimePatternSourceLimits {
            nodes: child.nodes,
            elements: child.elements,
            encoded_bytes: bytes,
            decoded_bytes: child.decoded_bytes,
            type_count: child.type_count,
            local_count: child.local_count,
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeHostCallSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeHostCallSourceLimits,
    footprint: Footprint,
    task_ordinal: u32,
}
impl<'a> PreparedRuntimeHostCallSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeHostCallSourceLimits,
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
            if let Some(binding) = row.binding {
                let child = RuntimePattern::preflight_sources(
                    binding,
                    counts.pattern(binding.len()),
                    resources,
                    meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
            }
            let mut arguments = reading::Cursor::new(row.arguments);
            for _ in 0..row.argument_count {
                let argument = arguments.argument()?;
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
                task_ordinal: row.task,
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
    pub(in crate::plan::construction::source) const fn task_ordinal(&self) -> u32 {
        self.task_ordinal
    }

    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeFlowOpSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            let count = source
                .builder
                .task_plan_definitions
                .lock()
                .map_err(|_| RuntimePlanBuildError::Poisoned)?
                .as_ref()
                .ok_or(Error::TaskCoordinate)?
                .len();
            if count != self.limits.task_count
                || !source
                    .builder
                    .task_coordinate_owner(u32::try_from(count).map_err(|_| Error::Arithmetic)?)
                    .contains(coordinate)
            {
                return Err(Error::TaskCoordinate);
            }
            // The caller's current allowance may differ from the first inspection.
            // Re-admit the entire borrowed packet before copying any metadata or value.
            let checked = Self::preflight(self.bytes, self.limits, resources, &mut source.meter)?;
            debug_assert_eq!(checked.footprint, self.footprint);
            let mut counts = Counts::new(self.limits)?;
            let mut cursor = reading::Cursor::new(self.bytes);
            let row = cursor.row(&mut counts)?;
            cursor.finish()?;
            if coordinate.ordinal() != row.task {
                return Err(Error::TaskCoordinate);
            }
            let result_type = source
                .builder
                .types
                .get(RuntimePlanTypeId::from_accepted_ordinal(
                    std::num::NonZeroU32::new(row.result)
                        .expect("preflighted positive type coordinate"),
                ))
                .expect("authenticated original type namespace")
                .semantic_identity();
            let binding = if let Some(bytes) = row.binding {
                let child = RuntimePattern::preflight_sources(
                    bytes,
                    counts.pattern(bytes.len()),
                    resources,
                    &mut source.meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
                Some(child.materialize(source, resources)?)
            } else {
                None
            };
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
                arguments.push(match argument.kind {
                    reading::ArgumentKind::Positional => {
                        RuntimeHostArgumentTemplate::Positional(argument.identity, value)
                    }
                    reading::ArgumentKind::Named(name) => RuntimeHostArgumentTemplate::Named(
                        argument.identity,
                        NamedHostArg {
                            name: name.to_owned(),
                            value,
                        },
                    ),
                    reading::ArgumentKind::Spread => {
                        RuntimeHostArgumentTemplate::Spread(argument.identity, value)
                    }
                });
            }
            cursor.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(RuntimeFlowOpSeed::HostCall {
                binding,
                target: RuntimeHostCallTargetSeed {
                    producer: HostCallProducerDefinition {
                        contract: row.producer_contract,
                        plan: coordinate.clone(),
                        site: row.producer_site,
                    },
                    public_id: row.public_id.to_owned(),
                    capability: row.capability.to_owned(),
                    operation: row.operation.to_owned(),
                    contract: row.contract,
                    args: arguments,
                    result: result_type,
                    mode: row.mode,
                    deterministic: row.deterministic,
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
