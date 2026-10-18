//! Original static effects. Framing borrows bytes before retaining any request.
//! Expression-carrying Audio commands have their own genuine seed source owner.

mod encoding;
mod reading;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use crate::effect::LineEffectRequest;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::RuntimeLineEffectSeed;
use crate::plan::value_admission::PlanValueAuthority;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::{
    AwbcRuntimeValueSnapshot, RuntimeLiteralSourceError, RuntimeLiteralSourceLimits,
};

const DOMAIN: &[u8] = b"arcweft.runtime.static-effect-source.v1\0";
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeStaticEffectSourceLimits {
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    /// Named request/seed and literal-codec payload, rather than allocator RSS.
    pub decoded_bytes: usize,
    pub literal: RuntimeLiteralSourceLimits,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeStaticEffectSourceError {
    #[error("static-effect source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid static-effect source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("static-effect source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("static-effect source arithmetic overflow")]
    Arithmetic,
    #[error("static-effect source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("static-effect source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("static-effect source text is invalid: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("static-effect source identity is invalid: {0}")]
    Identity(#[from] crate::effect::RuntimeIdentityDecodeError),
    #[error("static-effect source literal is invalid: {0}")]
    Literal(#[from] RuntimeLiteralSourceError),
    #[error("static-effect source value is invalid: {0}")]
    Value(#[from] crate::value::AwbcRuntimeValueSnapshotError),
    #[error("static-effect source assignment is invalid: {0}")]
    Assignment(#[from] crate::effect::RuntimeAssignmentError),
    #[error("static-effect source needs {required} output bytes, only {remaining} reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeStaticEffectSourceError;
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
    limits: RuntimeStaticEffectSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeStaticEffectSourceLimits) -> Result<Self, Error> {
        let mut counts = Self {
            limits,
            footprint: Footprint::default(),
        };
        counts.child(
            1,
            1,
            size_of::<RuntimeLineEffectSeed>() + size_of::<LineEffectRequest>(),
        )?;
        Ok(counts)
    }
    fn child(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
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
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
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
    fn storage(&mut self, n: usize) -> Result<(), Error> {
        self.child(0, 0, n)
    }
    fn array<T>(&mut self, n: usize) -> Result<(), Error> {
        self.child(
            0,
            n,
            n.checked_mul(size_of::<T>()).ok_or(Error::Arithmetic)?,
        )
    }
    fn literal_limits(&self, encoded: usize) -> RuntimeLiteralSourceLimits {
        RuntimeLiteralSourceLimits {
            encoded_bytes: self.limits.literal.encoded_bytes.min(encoded),
            decoded_bytes: self
                .limits
                .literal
                .decoded_bytes
                .min(self.limits.decoded_bytes - self.footprint.storage),
            elements: self
                .limits
                .literal
                .elements
                .min((self.limits.elements as usize - self.footprint.elements) as u32),
        }
    }
}

/// One legitimate phase context for the same reader; no copied source algebra.
enum Context<'a, 'plan> {
    Inspect(&'a ValueResourceAdmission),
    Materialize {
        authority: PlanValueAuthority<'plan>,
        resources: &'a mut ValueResourceAdmission,
    },
}
impl Context<'_, '_> {
    fn retains(&self) -> bool {
        matches!(self, Self::Materialize { .. })
    }
    fn value(
        &mut self,
        bytes: &[u8],
        counts: &mut Counts,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Option<crate::value::RuntimeValue>, Error> {
        let resources: &ValueResourceAdmission = match self {
            Self::Inspect(resources) => resources,
            Self::Materialize { resources, .. } => resources,
        };
        let prepared = AwbcRuntimeValueSnapshot::preflight_literal_source(
            bytes,
            counts.literal_limits(bytes.len()),
            resources,
            meter,
        )?;
        counts.child(
            prepared.source_nodes(),
            prepared.elements(),
            prepared.storage_bytes(),
        )?;
        match self {
            Self::Inspect(_) => Ok(None),
            Self::Materialize {
                authority,
                resources,
            } => {
                let source = prepared.materialize(meter)?;
                Ok(Some(authority.materialize_static_literal_source_value(
                    source, resources,
                )?))
            }
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeStaticEffectSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeStaticEffectSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeStaticEffectSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeStaticEffectSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut counts = Counts::new(limits)?;
            let mut reader = reading::Reader::new(bytes);
            reader.request(&mut counts, &mut Context::Inspect(resources), meter)?;
            reader.finish()?;
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
    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeLineEffectSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            charge(&mut source.meter, self.bytes.len())?;
            let mut counts = Counts::new(self.limits)?;
            let mut reader = reading::Reader::new(self.bytes);
            let mut context = Context::Materialize {
                authority: PlanValueAuthority::Building {
                    types: &source.builder.types,
                    records: &source.builder.nominal_record_domains,
                    variants: &source.builder.variant_domains,
                },
                resources,
            };
            let effect = reader
                .request(&mut counts, &mut context, &mut source.meter)?
                .expect("materializing the preflighted effect retains its request");
            reader.finish()?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(RuntimeLineEffectSeed::Static(effect))
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
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
