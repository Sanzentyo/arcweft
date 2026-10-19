//! Original specialization relations borrow the actual type namespace and
//! become issuer-bound seeds. Only the common program seal validates the
//! complete substitution and state correspondence.

mod reading;
#[cfg(test)]
pub(super) mod tests;
mod writing;

use super::RuntimePlanSourceConstruction;
use crate::plan::type_scope::function_codec::{
    RuntimeFunctionTypeSourceError, RuntimeFunctionTypeSourceLimits,
};
use crate::plan::type_table::RuntimePlanTypeTableBuilder;
use crate::plan::{
    RuntimeCallableSpecializationDefinition, RuntimeCallableSpecializationSeed,
    RuntimeCallableSpecializationSeedId, RuntimeCallableStateSeedId, RuntimeFunctionTypeContract,
    RuntimePlan, RuntimePlanBuildError, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection,
    RuntimePlanTypeTable,
};
use crate::runtime_id::{RuntimeCallableStateId, RuntimePlanTypeId};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

const DOMAIN: &[u8] = b"arcweft.runtime.callable-specialization-sources.v1\0";
type Definition =
    RuntimeCallableSpecializationDefinition<RuntimePlanTypeId, RuntimeCallableStateId>;

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeSpecializationSourceLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
    pub(in crate::plan) state_count: usize,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeSpecializationSourceError {
    #[error("specialization source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid specialization source domain")]
    Domain,
    #[error("unsupported specialization source version {actual}")]
    Version { actual: u8 },
    #[error("specialization source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("specialization source arithmetic overflow")]
    Arithmetic,
    #[error("specialization source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("specialization source has trailing bytes")]
    Trailing,
    #[error("specialization source row ordinal {actual} differs from {expected}")]
    Ordinal { actual: u32, expected: u32 },
    #[error("unknown specialization constant source tag {tag}")]
    Tag { tag: u8 },
    #[error("specialization {kind} reference {actual} is outside the {maximum}-row namespace")]
    Reference {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("specialization source {kind} namespace differs from the actual construction")]
    Namespace { kind: &'static str },
    #[error("specialization target {ty} is not a root function declaration")]
    Target { ty: RuntimePlanTypeId },
    #[error("specialization effect source failed: {0}")]
    Effect(#[from] RuntimeFunctionTypeSourceError),
    #[error("specialization construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("specialization source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeSpecializationSourceError;
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

#[derive(Clone, Copy)]
// Both phases borrow their owning type table; no source catalog is retained.
enum Types<'a> {
    Program(&'a RuntimePlanTypeTable),
    Construction(&'a RuntimePlanTypeTableBuilder),
}
impl<'a> Types<'a> {
    fn get(self, id: RuntimePlanTypeId) -> &'a RuntimePlanTypeDeclaration {
        match self {
            Self::Program(table) => table.get(id),
            Self::Construction(table) => table.get(id),
        }
        .expect("source reference bounds use the actual immutable type namespace")
    }
    fn target(
        self,
        ty: RuntimePlanTypeId,
    ) -> Result<
        (
            &'a RuntimeFunctionTypeContract,
            &'a crate::plan::RuntimeTypeScope,
        ),
        Error,
    > {
        let row = self.get(ty);
        match row.projection() {
            RuntimePlanTypeProjection::Function { contract, .. } if row.scope().is_root() => {
                Ok((contract, row.scope()))
            }
            _ => Err(Error::Target { ty }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    rows: usize,
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeSpecializationSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeSpecializationSourceLimits, rows: usize) -> Result<Self, Error> {
        limit("rows", rows, limits.rows as usize)?;
        let mut counts = Self {
            limits,
            footprint: Footprint {
                rows,
                ..Footprint::default()
            },
        };
        counts.array(
            rows,
            size_of::<RuntimeCallableSpecializationSeed>()
                + size_of::<Definition>()
                + size_of::<RuntimeCallableSpecializationSeedId>(),
        )?;
        Ok(counts)
    }
    fn array(&mut self, count: usize, width: usize) -> Result<(), Error> {
        self.child(count, count.checked_mul(width).ok_or(Error::Arithmetic)?)
    }
    fn child(&mut self, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(elements)
            .ok_or(Error::Arithmetic)?;
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(storage)
            .ok_or(Error::Arithmetic)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn effect_limits(&self, bytes: usize) -> Result<RuntimeFunctionTypeSourceLimits, Error> {
        let remaining = (self.limits.elements as usize)
            .checked_sub(self.footprint.elements)
            .ok_or(Error::Arithmetic)?;
        Ok(RuntimeFunctionTypeSourceLimits {
            encoded_bytes: bytes,
            decoded_bytes: self
                .limits
                .decoded_bytes
                .checked_sub(self.footprint.storage)
                .ok_or(Error::Arithmetic)?,
            decision_nodes: remaining as u32,
            overrides: remaining as u32,
        })
    }
}

pub(in crate::plan) struct PreparedRuntimeSpecializationSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeSpecializationSourceLimits,
    footprint: Footprint,
}
impl PreparedRuntimeSpecializationSources<'_> {
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) fn materialize(
        self,
        source: &mut RuntimePlanSourceConstruction,
        states: &[RuntimeCallableStateSeedId],
    ) -> Result<Box<[RuntimeCallableSpecializationSeedId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            namespace(source, self.limits)?;
            if states.len() != self.limits.state_count {
                return Err(Error::Namespace {
                    kind: "state handles",
                });
            }
            charge(&mut source.meter, self.bytes.len())?;
            source
                .meter
                .charge_work(u64::try_from(states.len()).map_err(|_| Error::Arithmetic)?)?;
            for (index, state) in states.iter().enumerate() {
                if state
                    .resolve(&source.builder.issuer)
                    .is_none_or(|id| id.index() != index)
                {
                    return Err(Error::Construction(
                        RuntimePlanBuildError::ForeignCallableStateSeed,
                    ));
                }
            }
            source
                .builder
                .callable_specializations
                .reserve_exact(self.footprint.rows);
            let mut handles = Vec::with_capacity(self.footprint.rows);
            let mut reader = reading::Reader::new(
                self.bytes,
                self.limits,
                Types::Construction(&source.builder.types),
            )?;
            for index in 0..self.footprint.rows {
                let row = reader
                    .row(index, Some(states), &mut source.meter)?
                    .expect("retaining actual specialization seeds");
                // End the immutable type-table borrow before each actual owner mutation.
                let (offset, counts) = reader.into_parts();
                handles.push(source.builder.push_callable_specialization_seed(row)?);
                reader = reading::Reader::resume(
                    self.bytes,
                    Types::Construction(&source.builder.types),
                    offset,
                    counts,
                );
            }
            reader.finish()?;
            debug_assert_eq!(reader.counts.footprint, self.footprint);
            Ok(handles.into_boxed_slice())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
impl RuntimePlanSourceConstruction {
    pub(in crate::plan) fn preflight_specialization_sources<'a>(
        &mut self,
        bytes: &'a [u8],
        limits: RuntimeSpecializationSourceLimits,
    ) -> Result<PreparedRuntimeSpecializationSources<'a>, Error> {
        self.meter.status()?;
        let result = (|| {
            namespace(self, limits)?;
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(&mut self.meter, bytes.len())?;
            let mut reader =
                reading::Reader::new(bytes, limits, Types::Construction(&self.builder.types))?;
            for index in 0..reader.counts.footprint.rows {
                let _ = reader.row(index, None, &mut self.meter)?;
            }
            reader.finish()?;
            Ok(PreparedRuntimeSpecializationSources {
                bytes,
                limits,
                footprint: reader.counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(&mut self.meter))
    }
}
fn namespace(
    source: &RuntimePlanSourceConstruction,
    limits: RuntimeSpecializationSourceLimits,
) -> Result<(), Error> {
    for (kind, actual, expected) in [
        ("type", source.builder.types.len(), limits.type_count),
        (
            "state",
            source.builder.callable_states.borrow().states.len(),
            limits.state_count,
        ),
        (
            "existing specialization",
            source.builder.callable_specializations.len(),
            0,
        ),
    ] {
        if actual != expected {
            return Err(Error::Namespace { kind });
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
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(bytes)
            .ok()
            .and_then(|n| n.checked_mul(3))
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
