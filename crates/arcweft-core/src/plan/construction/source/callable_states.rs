//! Original reusable callable graph data. The common plan seal remains the
//! authority for origins, partial projections, bodies and attached defaults.

mod reading;
#[cfg(test)]
pub(super) mod tests;
mod writing;

use super::RuntimePlanSourceConstruction;
use crate::plan::{
    RuntimeCallableStateSeedId, RuntimeCallableStateTable, RuntimeFunctionSiteSeedId,
    RuntimePlanBuildError,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.callable-state-sources.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeCallableSourceLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
    pub(in crate::plan) function_count: usize,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeCallableSourceError {
    #[error("callable source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid callable source domain")]
    Domain,
    #[error("unsupported callable source version {actual}")]
    Version { actual: u8 },
    #[error("callable source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("callable source arithmetic overflow")]
    Arithmetic,
    #[error("callable source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("callable source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("unknown callable source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("callable source {kind} reference {actual} is outside its {maximum}-row namespace")]
    Reference {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("callable source row ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("callable source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("callable source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("callable source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeCallableSourceError;
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
    rows: usize,
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeCallableSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeCallableSourceLimits, rows: usize) -> Result<Self, Error> {
        limit("callable states", rows, limits.rows as usize)?;
        let mut result = Self {
            limits,
            footprint: Footprint {
                rows,
                ..Footprint::default()
            },
        };
        result.array_width(rows, 0)?;
        let storage = crate::plan::construction::callable_states::RuntimeCallableStateBuilder::source_storage_bytes(rows)
            .ok_or(Error::Arithmetic)?;
        result.storage(storage)?;
        Ok(result)
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
    fn array<T>(&mut self, count: usize) -> Result<(), Error> {
        self.array_width(count, size_of::<T>())
    }
    fn mapped<S, F>(&mut self, count: usize) -> Result<(), Error> {
        self.array_width(count, size_of::<S>() + size_of::<F>())
    }
    fn array_width(&mut self, count: usize, width: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "source elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        self.storage(count.checked_mul(width).ok_or(Error::Arithmetic)?)
    }
}

pub(in crate::plan) struct PreparedRuntimeCallableSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeCallableSourceLimits,
    footprint: Footprint,
}
impl PreparedRuntimeCallableSources<'_> {
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn row_count(&self) -> usize {
        self.footprint.rows
    }

    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        functions: &[RuntimeFunctionSiteSeedId],
    ) -> Result<Box<[RuntimeCallableStateSeedId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            charge(&mut source.meter, self.bytes.len())?;
            for (kind, expected, actual) in [
                ("type", self.limits.type_count, source.builder.types.len()),
                ("function", self.limits.function_count, functions.len()),
                (
                    "reserved function",
                    self.limits.function_count,
                    source.builder.function_sites.len(),
                ),
                (
                    "existing callable",
                    0,
                    source.builder.callable_states.borrow().states.len(),
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
            for (index, function) in functions.iter().enumerate() {
                if function.resolve(&source.builder.issuer).is_none()
                    || function.id().get().get() as usize != index + 1
                {
                    return Err(Error::Construction(
                        RuntimePlanBuildError::ForeignFunctionSiteSeed,
                    ));
                }
            }
            let mut reader = reading::Reader::new(self.bytes, self.limits, self.footprint.rows)?;
            reader.header()?;
            let mut states = Vec::with_capacity(self.footprint.rows);
            for _ in 0..self.footprint.rows {
                states.push(source.builder.reserve_callable_state_seed()?);
            }
            for index in 0..self.footprint.rows {
                let row = reader
                    .row(
                        index,
                        Some(&reading::Owner {
                            types: &source.builder.types,
                            functions,
                            states: &states,
                        }),
                    )?
                    .expect("source rows materialize only under the actual construction owner");
                source
                    .builder
                    .define_callable_state_seed(&states[index], row)?;
            }
            reader.finish()?;
            debug_assert_eq!(reader.footprint(), self.footprint);
            source
                .builder
                .callable_states
                .borrow_mut()
                .complete_source()?;
            Ok(states.into_boxed_slice())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
impl RuntimeCallableStateTable {
    pub(in crate::plan) fn preflight_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimeCallableSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeCallableSources<'a>, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let rows = reading::Reader::header_count(bytes, limits)?;
            let mut reader = reading::Reader::new(bytes, limits, rows)?;
            reader.header()?;
            for index in 0..rows {
                let _ = reader.row(index, None)?;
            }
            reader.finish()?;
            Ok(PreparedRuntimeCallableSources {
                bytes,
                limits,
                footprint: reader.footprint(),
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
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
fn reference(
    kind: &'static str,
    actual: usize,
    maximum: usize,
    one_based: bool,
) -> Result<(), Error> {
    if (one_based && (actual == 0 || actual > maximum)) || (!one_based && actual >= maximum) {
        return Err(Error::Reference {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn charge(meter: &mut TaskSemanticMeter, length: usize) -> Result<(), Error> {
    let length = u64::try_from(length).map_err(|_| Error::Arithmetic)?;
    meter.charge_work(length.checked_mul(3).ok_or(Error::Arithmetic)?)?;
    Ok(())
}
