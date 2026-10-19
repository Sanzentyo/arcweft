//! Original pure-helper/method signatures and domain program bindings.
//! The actual construction reserves handles before any expression body exists.

mod cursor;
mod encoding;
#[cfg(test)]
pub(super) mod tests;

use super::RuntimePlanSourceConstruction;
use crate::plan::{
    RuntimeCallableParameter, RuntimeCallableParameterSeed, RuntimeFunctionSiteSeedId,
    RuntimePlanBuildError, RuntimePureHelper, RuntimePureHelperDeclarationSeed,
    RuntimePureHelperSeedId, RuntimePureProgramBinding, RuntimePureProgramBindingSeed,
    RuntimeTraitMethod, RuntimeTraitMethodDeclarationSeed, RuntimeTraitMethodSeedId,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use cursor::Cursor;

const DOMAIN: &[u8] = b"arcweft.runtime.pure-callable-declarations.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimePureCallableSourceLimits {
    pub(in crate::plan) helpers: u32,
    pub(in crate::plan) methods: u32,
    pub(in crate::plan) bindings: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
    pub(in crate::plan) local_count: usize,
    pub(in crate::plan) function_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub(in crate::plan) enum RuntimePureCallableSourceError {
    #[error("pure callable source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid pure callable source domain")]
    Domain,
    #[error("unsupported pure callable source version {actual}")]
    Version { actual: u8 },
    #[error("pure callable source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("pure callable source arithmetic overflow")]
    Arithmetic,
    #[error("pure callable source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("pure callable source has trailing bytes")]
    Trailing,
    #[error("unknown pure callable source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("pure callable source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("pure callable source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("pure callable source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("pure callable source contains a foreign function handle")]
    ForeignFunction,
    #[error("pure callable source string is not UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("pure callable source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("pure callable source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimePureCallableSourceError;
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
    limits: RuntimePureCallableSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimePureCallableSourceLimits) -> Self {
        Self {
            limits,
            footprint: Footprint::default(),
        }
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
    fn storage(&mut self, count: usize) -> Result<(), Error> {
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn inputs(&mut self, count: usize) -> Result<(), Error> {
        self.elements(count)?;
        // Includes the actual seed array, reserved ABI, original resolver
        // scratch and physical-ABI validation scratch before they are moved.
        self.storage(
            count
                .checked_mul(
                    size_of::<RuntimeCallableParameterSeed>()
                        + size_of::<RuntimeCallableParameter>()
                        + size_of::<(
                            crate::runtime_id::RuntimeLocalDeclarationId,
                            crate::runtime_id::RuntimePlanTypeId,
                        )>()
                        + size_of::<(crate::runtime_id::RuntimeLocalDeclarationId, usize)>()
                        + size_of::<crate::plan::RuntimePureInputType>(),
                )
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn helper(&mut self, inputs: usize, labels: usize) -> Result<(), Error> {
        self.elements(1)?;
        self.inputs(inputs)?;
        self.storage(
            size_of::<super::super::ReservedPureHelper>() + size_of::<RuntimePureHelper>(),
        )?;
        self.storage(
            RuntimePureHelperSeedId::source_storage_bytes(inputs).ok_or(Error::Arithmetic)?,
        )?;
        self.storage(labels)
    }
    fn method(&mut self, inputs: usize, labels: usize) -> Result<(), Error> {
        self.elements(1)?;
        self.inputs(inputs)?;
        self.storage(
            size_of::<super::super::ReservedTraitMethod>() + size_of::<RuntimeTraitMethod>(),
        )?;
        self.storage(
            RuntimeTraitMethodSeedId::source_storage_bytes(inputs).ok_or(Error::Arithmetic)?,
        )?;
        self.storage(labels)
    }
    fn binding(&mut self, inputs: usize) -> Result<(), Error> {
        self.elements(1)?;
        self.elements(inputs)?;
        self.storage(
            size_of::<RuntimePureProgramBinding>() + size_of::<RuntimePureProgramBindingSeed>(),
        )?;
        self.storage(
            inputs
                .checked_mul(size_of::<crate::pattern::RuntimeSemanticTypeId>())
                .ok_or(Error::Arithmetic)?,
        )
    }
}

pub(in crate::plan) struct PreparedRuntimePureCallableSources<'a> {
    bytes: &'a [u8],
    limits: RuntimePureCallableSourceLimits,
    footprint: Footprint,
}
pub(in crate::plan) struct RuntimePureCallableSourceHandles {
    pub(in crate::plan) helpers: Box<[RuntimePureHelperSeedId]>,
    pub(in crate::plan) methods: Box<[RuntimeTraitMethodSeedId]>,
}
impl<'a> PreparedRuntimePureCallableSources<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimePureCallableSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = Cursor::new(bytes, limits);
            let mut counts = Counts::new(limits);
            let helpers = cursor.header()?;
            for index in 0..helpers {
                let row = cursor.helper(index)?;
                counts.helper(row.inputs.len() / cursor::INPUT_BYTES, row.name.len())?;
            }
            let methods = cursor.method_count()?;
            for index in 0..methods {
                let row = cursor.method(index)?;
                counts.method(row.inputs.len() / cursor::INPUT_BYTES, row.label_bytes()?)?;
            }
            let bindings = cursor.binding_count()?;
            for index in 0..bindings {
                let row = cursor.binding(index)?;
                counts.binding(row.inputs)?;
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
    pub(in crate::plan) fn reserve(
        self,
        source: &mut RuntimePlanSourceConstruction,
        functions: &[RuntimeFunctionSiteSeedId],
    ) -> Result<RuntimePureCallableSourceHandles, Error> {
        source.meter.status()?;
        let result = (|| {
            charge(&mut source.meter, self.bytes.len())?;
            for (kind, expected, actual) in [
                ("type", self.limits.type_count, source.builder.types.len()),
                ("local", self.limits.local_count, source.locals.len()),
                (
                    "function",
                    self.limits.function_count,
                    source.builder.function_sites.len(),
                ),
                (
                    "function handles",
                    self.limits.function_count,
                    functions.len(),
                ),
                ("existing helper", 0, source.builder.pure_helpers.len()),
                ("existing method", 0, source.builder.trait_methods.len()),
                (
                    "existing program binding",
                    0,
                    source.builder.pure_programs.len(),
                ),
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
                    return Err(Error::ForeignFunction);
                }
            }
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let helpers = cursor.header()?;
            let mut helper_handles = Vec::with_capacity(helpers);
            for index in 0..helpers {
                let row = cursor.helper(index)?;
                let inputs = source.pure_source_inputs(row.inputs, self.limits)?;
                let result = source
                    .builder
                    .types
                    .get(row.result)
                    .expect("checked original result type")
                    .semantic_identity();
                helper_handles.push(source.builder.reserve_pure_helper_seed(
                    RuntimePureHelperDeclarationSeed {
                        definition: row.definition,
                        name: row.name.to_owned(),
                        inputs,
                        result,
                        output_abi: row.output,
                        scalar_eval_supported: row.scalar,
                        origin: row.origin,
                    },
                )?);
            }
            let methods = cursor.method_count()?;
            let mut method_handles = Vec::with_capacity(methods);
            for index in 0..methods {
                let row = cursor.method(index)?;
                let inputs = source.pure_source_inputs(row.inputs, self.limits)?;
                let result = source
                    .builder
                    .types
                    .get(row.result)
                    .expect("checked original method result type")
                    .semantic_identity();
                method_handles.push(source.builder.reserve_trait_method_seed(
                    RuntimeTraitMethodDeclarationSeed {
                        definition: row.definition,
                        identity: row.identity(),
                        receiver: row.receiver,
                        inputs,
                        result,
                        output_abi: row.output,
                    },
                )?);
            }
            let bindings = cursor.binding_count()?;
            for index in 0..bindings {
                let row = cursor.binding(index)?;
                let handle = &functions[row.function as usize - 1];
                let (_, _, inputs, ..) = handle
                    .resolve(&source.builder.issuer)
                    .ok_or(Error::ForeignFunction)?;
                if row.inputs != inputs.len() {
                    return Err(Error::Namespace {
                        kind: "program binding inputs",
                        expected: row.inputs,
                        actual: inputs.len(),
                    });
                }
                // The owning factory checks every earlier domain binding.
                source
                    .meter
                    .charge_work(u64::try_from(index).map_err(|_| Error::Arithmetic)?)?;
                source
                    .builder
                    .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
                        program: row.program,
                        site: handle.clone(),
                    })?;
            }
            cursor.finish()?;
            Ok(RuntimePureCallableSourceHandles {
                helpers: helper_handles.into_boxed_slice(),
                methods: method_handles.into_boxed_slice(),
            })
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
impl RuntimePlanSourceConstruction {
    fn pure_source_inputs(
        &self,
        bytes: &[u8],
        limits: RuntimePureCallableSourceLimits,
    ) -> Result<Box<[RuntimeCallableParameterSeed]>, Error> {
        let mut cursor = Cursor::new(bytes, limits);
        let mut inputs = Vec::with_capacity(bytes.len() / cursor::INPUT_BYTES);
        for _ in 0..bytes.len() / cursor::INPUT_BYTES {
            let row = cursor.input()?;
            inputs.push(RuntimeCallableParameterSeed {
                identity: row.identity,
                local: self
                    .local(row.local)
                    .expect("preflighted actual local handle")
                    .clone(),
                passing: row.passing,
                abi: row.abi,
            });
        }
        cursor.finish()?;
        Ok(inputs.into_boxed_slice())
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
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(bytes)
            .ok()
            .and_then(|n| n.checked_mul(3))
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
