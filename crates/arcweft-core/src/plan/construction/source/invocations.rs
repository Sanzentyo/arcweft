//! Original invocation declarations bind real function handles before
//! their bodies. Contracts and selectors are data, never executable proofs.

mod cursor;
mod encoding;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use crate::entry::{
    RuntimeCallableExecutable, RuntimeFlowExecutable, RuntimeFlowExecutableParameter,
    RuntimeFlowSchema,
};
use crate::plan::{
    RuntimeCallableExecutableSeed, RuntimeCallableExecutableSeedCode, RuntimeFunctionSiteSeedId,
    RuntimePlanBuildError,
};
use crate::runtime_id::{RuntimeDeferSiteId, RuntimeFunctionSiteId, RuntimeIdSegment};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.invocation-declarations.v1\0";
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeInvocationSourceLimits {
    pub flows: u32,
    pub schemas: u32,
    pub flow_executables: u32,
    pub callables: u32,
    pub defers: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub type_count: usize,
    pub function_count: usize,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeInvocationSourceError {
    #[error("invocation source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid invocation source domain")]
    Domain,
    #[error("unsupported invocation source version {actual}")]
    Version { actual: u8 },
    #[error("invocation source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("invocation source arithmetic overflow")]
    Arithmetic,
    #[error("invocation source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("invocation source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("invalid invocation source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("invocation source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("invocation source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("invocation source contains a foreign or reordered {kind} handle")]
    Handle { kind: &'static str },
    #[error("invocation source string is invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("invocation source runtime path is invalid: {0}")]
    Path(#[from] crate::runtime_id::RuntimeIdError),
    #[error("invocation source identity is invalid: {0}")]
    Identity(#[from] crate::entry::RuntimeIdentityError),
    #[error("invocation source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("invocation source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeInvocationSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl Error {
    fn reject(self, meter: &mut TaskSemanticMeter) -> Self {
        let result = match meter.status() {
            Err(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(cause)
            }
            _ => self,
        };
        meter.reject_owner();
        result
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeInvocationSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeInvocationSourceLimits) -> Self {
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
    fn text(&mut self, n: usize) -> Result<(), Error> {
        self.storage(n.checked_mul(2).ok_or(Error::Arithmetic)?)
    }
    fn segments(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, 2 * size_of::<RuntimeIdSegment>())
    }
    fn roots(&mut self, n: usize) -> Result<(), Error> {
        limit("Flows", n, self.limits.flows as usize)?;
        self.array(
            n,
            size_of::<super::super::ReservedFlowRoot>() + size_of::<crate::plan::RuntimeFlow>(),
        )?;
        self.storage(
            crate::plan::flows::RuntimeFlowTable::source_index_storage_bytes(n, 0)
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn parameters(&mut self, n: usize) -> Result<(), Error> {
        self.array(
            n,
            2 * size_of::<crate::runtime_id::RuntimeLocalDeclarationId>(),
        )
    }
    fn schemas(&mut self, n: usize) -> Result<(), Error> {
        limit("Flow schemas", n, self.limits.schemas as usize)?;
        self.array(n, size_of::<RuntimeFlowSchema>())?;
        self.storage(
            crate::plan::flows::RuntimeFlowTable::source_index_storage_bytes(0, n)
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn schema_parameters(&mut self, n: usize) -> Result<(), Error> {
        self.array(n, size_of::<RuntimeFlowExecutableParameter>())
    }
    fn flow_executables(&mut self, n: usize) -> Result<(), Error> {
        limit("Flow executables", n, self.limits.flow_executables as usize)?;
        self.array(n, size_of::<RuntimeFlowExecutable>())
    }
    fn callables(&mut self, n: usize) -> Result<(), Error> {
        limit("callable executables", n, self.limits.callables as usize)?;
        self.array(
            n,
            size_of::<RuntimeCallableExecutableSeed>() + size_of::<RuntimeCallableExecutable>(),
        )
    }
    fn defers(&mut self, n: usize) -> Result<(), Error> {
        limit("defer sites", n, self.limits.defers as usize)?;
        self.array(
            n,
            2 * size_of::<RuntimeFunctionSiteId>() + size_of::<RuntimeDeferSiteId>(),
        )
    }
}
pub(in crate::plan) struct PreparedRuntimeInvocationSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeInvocationSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeInvocationSources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeInvocationSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = cursor::Cursor::new(bytes, limits);
            let mut counts = Counts::new(limits);
            let roots = cursor.header(&mut counts)?;
            for _ in 0..roots {
                cursor.root(&mut counts)?;
            }
            let schemas = cursor.schema_count(&mut counts)?;
            for _ in 0..schemas {
                cursor.schema(&mut counts)?;
            }
            let flows = cursor.flow_executable_count(&mut counts)?;
            for _ in 0..flows {
                cursor.flow_executable(&mut counts)?;
            }
            let callables = cursor.callable_count(&mut counts)?;
            for _ in 0..callables {
                cursor.callable(&mut counts)?;
            }
            let defers = cursor.defer_count(&mut counts)?;
            for _ in 0..defers {
                cursor.function()?;
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
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        functions: &[RuntimeFunctionSiteSeedId],
    ) -> Result<Box<[RuntimeDeferSiteId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            namespace(source, self.limits, functions)?;
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = cursor::Cursor::new(self.bytes, self.limits);
            let mut counts = Counts::new(self.limits);
            let roots = cursor.header(&mut counts)?;
            for _ in 0..roots {
                let row = cursor.root(&mut counts)?;
                let function = &functions[row.function as usize - 1];
                let (_, sources, ..) = function
                    .resolve(&source.builder.issuer)
                    .expect("original function handles authenticated");
                let actual = sources
                    .iter()
                    .filter(|source| {
                        matches!(
                            source,
                            crate::plan::RuntimeFunctionInputSource::Parameter { .. }
                        )
                    })
                    .count();
                if row.parameters != actual {
                    return Err(Error::Namespace {
                        kind: "Flow parameters",
                        expected: row.parameters,
                        actual,
                    });
                }
                source
                    .builder
                    .bind_flow_function_seed(row.flow.materialize(self.limits)?, function)?;
            }
            let schemas = cursor.schema_count(&mut counts)?;
            for _ in 0..schemas {
                let row = cursor.schema(&mut counts)?;
                let flow = row.flow.materialize(self.limits)?;
                let mut parameters = Vec::with_capacity(row.parameter_count);
                let mut inputs = cursor::Cursor::new(row.parameters, self.limits);
                for _ in 0..row.parameter_count {
                    parameters.push(inputs.parameter()?.materialize());
                }
                inputs.finish()?;
                source
                    .builder
                    .push_flow_schema(RuntimeFlowSchema { flow, parameters })?;
            }
            let flows = cursor.flow_executable_count(&mut counts)?;
            for _ in 0..flows {
                let row = cursor.flow_executable(&mut counts)?;
                source.builder.push_flow_executable(RuntimeFlowExecutable {
                    flow: row.flow.materialize(self.limits)?,
                    contract: row.contract,
                    controller: row.controller.map(|role| role.materialize()).transpose()?,
                })?;
            }
            let callables = cursor.callable_count(&mut counts)?;
            for _ in 0..callables {
                let row = cursor.callable(&mut counts)?;
                let code = match row.code {
                    cursor::Code::Function(id) => RuntimeCallableExecutableSeedCode::FunctionSite(
                        functions[id as usize - 1].clone(),
                    ),
                    cursor::Code::Flow(flow) => RuntimeCallableExecutableSeedCode::ControllerFlow(
                        flow.materialize(self.limits)?,
                    ),
                };
                let role = row.role.materialize()?;
                source
                    .builder
                    .push_callable_executable_seed(RuntimeCallableExecutableSeed {
                        callable: role.callable,
                        contract: role.contract,
                        code,
                    })?;
            }
            let defers = cursor.defer_count(&mut counts)?;
            let mut handles = Vec::with_capacity(defers);
            for index in 0..defers {
                let function = cursor.function()?;
                let handle = source
                    .builder
                    .reserve_defer_site_seed(&functions[function as usize - 1])?;
                debug_assert_eq!(handle.index(), index);
                handles.push(handle);
            }
            cursor.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(handles.into_boxed_slice())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
fn namespace(
    source: &mut RuntimePlanSourceConstruction,
    limits: RuntimeInvocationSourceLimits,
    functions: &[RuntimeFunctionSiteSeedId],
) -> Result<(), Error> {
    for (kind, expected, actual) in [
        ("type", limits.type_count, source.builder.types.len()),
        (
            "function",
            limits.function_count,
            source.builder.function_sites.len(),
        ),
        ("function handles", limits.function_count, functions.len()),
        ("existing Flow", 0, source.builder.flows.len()),
        ("existing Flow schema", 0, source.builder.flow_schemas.len()),
        (
            "existing Flow executable",
            0,
            source.builder.flow_executables.len(),
        ),
        (
            "existing callable executable",
            0,
            source.builder.callable_executables.len(),
        ),
        ("existing defer", 0, source.builder.defer_sites.len()),
    ] {
        if expected != actual {
            return Err(Error::Namespace {
                kind,
                expected,
                actual,
            });
        }
    }
    source
        .meter
        .charge_work(u64::try_from(functions.len()).map_err(|_| Error::Arithmetic)?)?;
    for (index, handle) in functions.iter().enumerate() {
        if handle.id().get().get() as usize != index + 1
            || handle.resolve(&source.builder.issuer).is_none()
        {
            return Err(Error::Handle { kind: "function" });
        }
    }
    Ok(())
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
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(bytes)
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(3)
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
