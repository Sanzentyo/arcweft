//! Complete original function signatures, admitted before any function body.
//! Reservation uses the actual builder, binder and input-pattern authority.

mod cursor;
mod encoding;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::{
    RuntimeFunctionInputBindingSeed, RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId,
    RuntimeFunctionSiteTable, RuntimePlanBuildError, RuntimePlanBuilder,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use cursor::Cursor;

const DOMAIN: &[u8] = b"arcweft.runtime.function-declarations.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeFunctionSourceLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) pattern_nodes: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
    pub(in crate::plan) local_count: usize,
}
impl RuntimeFunctionSourceLimits {
    fn pattern(self) -> RuntimePatternSourceLimits {
        RuntimePatternSourceLimits {
            nodes: self.pattern_nodes,
            elements: self.elements,
            encoded_bytes: self.encoded_bytes,
            decoded_bytes: self.decoded_bytes,
            type_count: self.type_count,
            local_count: self.local_count,
        }
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeFunctionSourceError {
    #[error("function source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid function declaration source domain")]
    Domain,
    #[error("unsupported function declaration source version {actual}")]
    Version { actual: u8 },
    #[error("function source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("function source arithmetic overflow")]
    Arithmetic,
    #[error("function source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("function source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("unknown function source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("function source {kind} coordinate {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("function source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("function source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("function source {kind} is not strictly ordered")]
    Order { kind: &'static str },
    #[error("function source effect is not UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("function source effect identity is invalid: {0}")]
    Effect(#[from] arcweft_id::EffectIdError),
    #[error("function source closed effect set is invalid: {0}")]
    EffectSet(#[from] crate::plan::RuntimeEffectSetError),
    #[error("function source input pattern is invalid: {0}")]
    Pattern(#[from] RuntimePatternSourceError),
    #[error("function source reservation failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("function source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
impl From<TaskSemanticEncodingError> for RuntimeFunctionSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl RuntimeFunctionSourceError {
    fn reject(self, meter: &mut TaskSemanticMeter) -> Self {
        let result = match meter.status() {
            Err(error) if error != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(error)
            }
            _ => self,
        };
        meter.reject_owner();
        result
    }
}
type Error = RuntimeFunctionSourceError;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    rows: usize,
    inputs: usize,
    elements: usize,
    nodes: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeFunctionSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn pattern_limits(&self, encoded_bytes: usize) -> RuntimePatternSourceLimits {
        RuntimePatternSourceLimits {
            nodes: self.limits.pattern_nodes - self.footprint.nodes as u32,
            elements: self.limits.elements - self.footprint.elements as u32,
            encoded_bytes,
            decoded_bytes: self.limits.decoded_bytes - self.footprint.storage,
            type_count: self.limits.type_count,
            local_count: self.limits.local_count,
        }
    }
    fn new(limits: RuntimeFunctionSourceLimits, rows: usize) -> Result<Self, Error> {
        limit("function declarations", rows, limits.rows as usize)?;
        let mut counts = Self {
            limits,
            footprint: Footprint {
                rows,
                ..Footprint::default()
            },
        };
        counts.elements(rows)?;
        counts.storage(
            RuntimePlanBuilder::function_source_storage_bytes(rows, 0).ok_or(Error::Arithmetic)?,
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
            "source elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )
    }
    fn row(&mut self, inputs: usize, effects: usize, labels: usize) -> Result<(), Error> {
        self.footprint.inputs = self
            .footprint
            .inputs
            .checked_add(inputs)
            .ok_or(Error::Arithmetic)?;
        self.elements(inputs)?;
        self.elements(effects)?;
        self.storage(
            RuntimePlanBuilder::function_source_storage_bytes(0, inputs)
                .ok_or(Error::Arithmetic)?,
        )?;
        self.storage(
            inputs
                .checked_mul(size_of::<super::pattern::PreparedRuntimePatternEncoding<'_>>())
                .ok_or(Error::Arithmetic)?,
        )?;
        // The original signature handle and reserved definition each own a set.
        let effect_bytes = effects
            .checked_mul(size_of::<arcweft_id::EffectId>())
            .and_then(|n| n.checked_add(labels))
            .and_then(|n| n.checked_mul(2))
            .ok_or(Error::Arithmetic)?;
        self.storage(effect_bytes)
    }
    fn unrestricted(&mut self, count: usize) -> Result<(), Error> {
        self.elements(count)?;
        self.storage(
            count
                .checked_mul(
                    size_of::<super::super::RuntimeLocalSeedId>()
                        + size_of::<RuntimeLocalDeclarationId>(),
                )
                .ok_or(Error::Arithmetic)?,
        )
    }
    fn pattern(&mut self, storage: usize, elements: usize, nodes: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        limit(
            "pattern nodes",
            self.footprint.nodes,
            self.limits.pattern_nodes as usize,
        )?;
        self.elements(elements)?;
        self.storage(storage)
    }
}

pub(in crate::plan) struct PreparedRuntimeFunctionSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeFunctionSourceLimits,
    footprint: Footprint,
}
impl PreparedRuntimeFunctionSources<'_> {
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn row_count(&self) -> usize {
        self.footprint.rows
    }
    pub(in crate::plan) const fn pattern_nodes(&self) -> usize {
        self.footprint.nodes
    }

    pub(in crate::plan) fn reserve(
        self,
        source: &mut RuntimePlanSourceConstruction,
        resources: &mut ValueResourceAdmission,
    ) -> Result<Box<[RuntimeFunctionSiteSeedId]>, Error> {
        source.meter.status()?;
        let result = (|| {
            charge(&mut source.meter, self.bytes.len())?;
            for (kind, expected, actual) in [
                ("type", self.limits.type_count, source.builder.types.len()),
                ("local", self.limits.local_count, source.locals.len()),
                ("existing function", 0, source.builder.function_sites.len()),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut cursor = Cursor::new(self.bytes, self.limits);
            let rows = cursor.header()?;
            let mut handles = Vec::with_capacity(rows);
            for index in 0..rows {
                let row = cursor.row(index)?;
                let effects = Cursor::new(row.effects, self.limits).effect_set()?;
                let mut input_cursor = Cursor::new(row.inputs, self.limits);
                let mut inputs = Vec::with_capacity(row.input_count);
                for _ in 0..row.input_count {
                    let input = input_cursor.input()?;
                    let pattern = RuntimePattern::preflight_sources(
                        input.pattern,
                        self.limits.pattern(),
                        resources,
                        &mut source.meter,
                    )?;
                    // The original owner checks each unrestricted local against
                    // the complete pattern inventory before reserving the ABI.
                    let work = pattern
                        .node_count()
                        .checked_mul(2)
                        .and_then(|n| n.checked_mul(input.unrestricted.len() / 4))
                        .and_then(|n| u64::try_from(n).ok())
                        .ok_or(Error::Arithmetic)?;
                    source.meter.charge_work(work)?;
                    let pattern = pattern.materialize(source, resources)?;
                    let unrestricted_bindings = input
                        .unrestricted
                        .chunks_exact(4)
                        .map(|bytes| {
                            let id =
                                u32::from_le_bytes(bytes.try_into().expect("source local width"));
                            source
                                .local(local_id(id))
                                .expect("preflighted actual local handle")
                                .clone()
                        })
                        .collect();
                    inputs.push(RuntimeFunctionInputBindingSeed {
                        transfer: input.transfer,
                        origin: input.origin,
                        source: input.source,
                        input_local: source
                            .local(input.local)
                            .expect("preflighted actual input handle")
                            .clone(),
                        pattern,
                        ownership: input.ownership,
                        unrestricted_bindings,
                    });
                }
                input_cursor.finish()?;
                let semantic = |ty| {
                    source
                        .builder
                        .types
                        .get(ty)
                        .expect("original source type table")
                        .semantic_identity()
                };
                let declaration = RuntimeFunctionSiteDeclarationSeed {
                    definition: row.definition,
                    role: row.role,
                    function_type: row.function_type.map(semantic),
                    inputs: inputs.into_boxed_slice(),
                    result: semantic(row.result),
                    body_kind: row.body_kind,
                    effects,
                };
                let handle = source.builder.reserve_function_site_seed(declaration)?;
                debug_assert_eq!(handle.id().get().get() as usize, index + 1);
                handles.push(handle);
            }
            cursor.finish()?;
            Ok(handles.into_boxed_slice())
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}

impl RuntimeFunctionSiteTable {
    pub(in crate::plan) fn preflight_declaration_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimeFunctionSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeFunctionSources<'a>, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = Cursor::new(bytes, limits);
            let rows = cursor.header()?;
            let mut counts = Counts::new(limits, rows)?;
            for index in 0..rows {
                let row = cursor.row(index)?;
                counts.row(row.input_count, row.effect_count, row.label_bytes)?;
                let mut input_cursor = Cursor::new(row.inputs, limits);
                for _ in 0..row.input_count {
                    let input = input_cursor.input()?;
                    counts.unrestricted(input.unrestricted.len() / 4)?;
                    let pattern = RuntimePattern::preflight_sources(
                        input.pattern,
                        counts.pattern_limits(input.pattern.len()),
                        resources,
                        meter,
                    )?;
                    counts.pattern(
                        pattern.storage_bytes(),
                        pattern.elements(),
                        pattern.node_count(),
                    )?;
                }
                input_cursor.finish()?;
            }
            cursor.finish()?;
            Ok(PreparedRuntimeFunctionSources {
                bytes,
                limits,
                footprint: counts.footprint,
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
fn local_id(id: u32) -> RuntimeLocalDeclarationId {
    RuntimeLocalDeclarationId::from_accepted_ordinal(
        std::num::NonZeroU32::new(id).expect("source local is checked"),
    )
}
fn type_id(id: u32) -> RuntimePlanTypeId {
    RuntimePlanTypeId::from_accepted_ordinal(
        std::num::NonZeroU32::new(id).expect("source type is checked"),
    )
}
fn charge(meter: &mut TaskSemanticMeter, length: usize) -> Result<(), Error> {
    let work = u64::try_from(length)
        .ok()
        .and_then(|n| n.checked_mul(3))
        .ok_or(Error::Arithmetic)?;
    meter.charge_work(work)?;
    Ok(())
}
