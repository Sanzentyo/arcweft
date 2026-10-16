//! Complete private source frame for the existing type declaration table.
//! No executable row digest or task proof is accepted from this frame.

mod policy;
mod projection;

use super::{
    RuntimePlanTypeDeclaration, RuntimePlanTypeTable, RuntimePlanTypeTableError,
    RuntimePlanTypeTableRow, plan_type_id_for_index, validate_candidate_graph,
};
use crate::entry::RuntimeNominalDeclarationId;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::RuntimeTypeScope;
use crate::plan::type_scope::function_codec::{
    RuntimeFunctionTypeSourceError, RuntimeFunctionTypeSourceLimits,
};
use crate::plan::type_scope::source_codec::{
    RuntimeTypeScopeCodecError, RuntimeTypeScopeCodecLimits,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use std::{collections::BTreeSet, num::NonZeroU32};

const DOMAIN: &[u8] = b"arcweft.runtime.type-sources.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeTypeSourceLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) elements: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) policy_depth: u32,
}

impl RuntimeTypeSourceLimits {
    fn scope(self) -> RuntimeTypeScopeCodecLimits {
        RuntimeTypeScopeCodecLimits {
            encoded_bytes: self.encoded_bytes,
            decoded_bytes: self.decoded_bytes,
        }
    }
    fn function(self) -> RuntimeFunctionTypeSourceLimits {
        RuntimeFunctionTypeSourceLimits {
            encoded_bytes: self.encoded_bytes,
            decoded_bytes: self.decoded_bytes,
            decision_nodes: self.elements,
            overrides: self.elements,
        }
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeTypeSourceError {
    #[error("type source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid type source domain")]
    Domain,
    #[error("unsupported type source version {actual}")]
    Version { actual: u8 },
    #[error("type source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("type source count arithmetic overflow")]
    Arithmetic,
    #[error("type source is truncated")]
    Truncated,
    #[error("type source has trailing data")]
    Trailing,
    #[error("unknown type source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("unknown dense numeric source parts {parts:?}")]
    DenseNumeric { parts: [u8; 2] },
    #[error("type source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("type source reference {actual} is outside 1..={maximum}")]
    Reference { actual: u32, maximum: usize },
    #[error("type source string is not UTF-8")]
    Utf8,
    #[error("invalid nominal type source identity: {0}")]
    Identity(#[from] crate::entry::RuntimeIdentityError),
    #[error("invalid type source lexical scope: {0}")]
    Scope(#[from] RuntimeTypeScopeCodecError),
    #[error("invalid bound type source reference: {0}")]
    Bound(#[from] crate::plan::RuntimeTypeScopeError),
    #[error("invalid function type source: {0}")]
    Function(#[from] RuntimeFunctionTypeSourceError),
    #[error("type source graph admission failed: {0}")]
    Admission(Box<RuntimePlanTypeTableError>),
    #[error("type source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl From<TaskSemanticEncodingError> for RuntimeTypeSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

struct Counts {
    limits: RuntimeTypeSourceLimits,
    storage: usize,
    elements: usize,
}

impl Counts {
    fn new(limits: RuntimeTypeSourceLimits) -> Self {
        Self {
            limits,
            storage: 0,
            elements: 0,
        }
    }
    fn storage(&mut self, bytes: usize) -> Result<(), RuntimeTypeSourceError> {
        self.storage = self
            .storage
            .checked_add(bytes)
            .ok_or(RuntimeTypeSourceError::Arithmetic)?;
        limit("decoded bytes", self.storage, self.limits.decoded_bytes)
    }
    fn array<T>(&mut self, count: usize) -> Result<(), RuntimeTypeSourceError> {
        self.elements(count)?;
        self.storage(
            count
                .checked_mul(size_of::<T>())
                .ok_or(RuntimeTypeSourceError::Arithmetic)?,
        )
    }
    fn elements(&mut self, count: usize) -> Result<(), RuntimeTypeSourceError> {
        self.elements = self
            .elements
            .checked_add(count)
            .ok_or(RuntimeTypeSourceError::Arithmetic)?;
        limit(
            "source elements",
            self.elements,
            self.limits.elements as usize,
        )
    }
    fn rows(&mut self, count: usize) -> Result<(), RuntimeTypeSourceError> {
        limit("type rows", count, self.limits.rows as usize)?;
        self.elements(count)?;
        // The retained declarations and final dense rows coexist during the
        // existing graph admission, along with its logical graph work tables.
        let row = size_of::<RuntimePlanTypeDeclaration>()
            + size_of::<RuntimePlanTypeTableRow>()
            + size_of::<RuntimeSemanticTypeId>()
            + size_of::<Vec<usize>>()
            + 4 * size_of::<usize>();
        self.storage(
            count
                .checked_mul(row)
                .ok_or(RuntimeTypeSourceError::Arithmetic)?,
        )
    }
    fn reference(&mut self) -> Result<(), RuntimeTypeSourceError> {
        self.elements(1)?;
        self.storage(2 * (size_of::<usize>() + size_of::<&RuntimePlanTypeId>()))
    }
    fn scope_admission(&mut self, count: usize) -> Result<(), RuntimeTypeSourceError> {
        self.storage(
            count
                .checked_mul(size_of::<crate::plan::RuntimeTypeBinder>())
                .ok_or(RuntimeTypeSourceError::Arithmetic)?,
        )
    }
    fn policy_admission(&mut self) -> Result<(), RuntimeTypeSourceError> {
        type Use = crate::entry::schema::RuntimeCodecUse;
        self.storage(
            2 * (size_of::<(usize, &Use, usize)>() + size_of::<(RuntimePlanTypeId, &Use)>()),
        )
    }
    fn policy_depth(&self, depth: usize) -> Result<(), RuntimeTypeSourceError> {
        let maximum = self
            .limits
            .policy_depth
            .min(crate::entry::RuntimeSchemaLimits::engine_default().max_depth)
            as usize;
        limit("codec policy depth", depth, maximum)
    }
}

pub(in crate::plan) struct PreparedRuntimeTypeTableEncoding<'a> {
    table: &'a RuntimePlanTypeTable,
    limits: RuntimeTypeSourceLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeTypeTableEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }
    pub(in crate::plan) fn append_to(
        self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeTypeSourceError> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(RuntimeTypeSourceError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.limits, self.table.len());
        writer.table(self.table)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.storage, self.storage_bytes);
        debug_assert_eq!(writer.counts.elements, self.elements);
        Ok(())
    }
}

/// Complete immutable frame data. The containing artifact can inspect every
/// table's counts before retaining any type/graph arrays; no proof is issued.
pub(in crate::plan) struct PreparedRuntimeTypeTableSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeTypeSourceLimits,
    storage_bytes: usize,
    elements: usize,
    rows: usize,
}

impl PreparedRuntimeTypeTableSource<'_> {
    pub(in crate::plan) const fn row_count(&self) -> usize {
        self.rows
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }
    pub(in crate::plan) fn materialize(
        self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimePlanTypeTable, RuntimeTypeSourceError> {
        meter.status()?;
        let result = (|| {
            let mut reader = Reader::new(self.bytes, self.limits, meter);
            let rows = reader
                .table(true)?
                .expect("retaining a complete type source frame");
            debug_assert_eq!(reader.counts.storage, self.storage_bytes);
            debug_assert_eq!(reader.counts.elements, self.elements);
            debug_assert_eq!(rows.len(), self.rows);
            let mut identities = BTreeSet::new();
            for row in &rows {
                if !identities.insert(row.semantic_identity) {
                    return Err(RuntimeTypeSourceError::Admission(Box::new(
                        RuntimePlanTypeTableError::ConflictingProjection {
                            semantic_identity: row.semantic_identity,
                        },
                    )));
                }
            }
            validate_candidate_graph(&rows)
                .map_err(|error| RuntimeTypeSourceError::Admission(Box::new(error)))?;
            let rows = rows
                .into_iter()
                .enumerate()
                .map(|(index, declaration)| {
                    Ok(RuntimePlanTypeTableRow {
                        id: plan_type_id_for_index(index)
                            .map_err(|error| RuntimeTypeSourceError::Admission(Box::new(error)))?,
                        declaration,
                    })
                })
                .collect::<Result<Vec<_>, RuntimeTypeSourceError>>()?
                .into_boxed_slice();
            Ok(RuntimePlanTypeTable { rows })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl RuntimePlanTypeTable {
    pub(in crate::plan) fn prepare_sources_encoding(
        &self,
        limits: RuntimeTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeTypeTableEncoding<'_>, RuntimeTypeSourceError> {
        meter.status()?;
        let result = (|| {
            let mut counter = Writer::new(None, Some(&mut *meter), limits, self.len());
            counter.table(self)?;
            Ok(PreparedRuntimeTypeTableEncoding {
                table: self,
                limits,
                encoded_bytes: counter.length,
                storage_bytes: counter.counts.storage,
                elements: counter.counts.elements,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn encode_sources(
        &self,
        limits: RuntimeTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<u8>, RuntimeTypeSourceError> {
        let prepared = self.prepare_sources_encoding(limits, meter)?;
        let mut output = Vec::with_capacity(prepared.encoded_bytes());
        prepared.append_to(&mut output)?;
        Ok(output)
    }

    pub(in crate::plan) fn preflight_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimeTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeTypeTableSource<'a>, RuntimeTypeSourceError> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_source(meter, bytes.len())?;
            let mut reader = Reader::new(bytes, limits, meter);
            reader.table(false)?;
            Ok(PreparedRuntimeTypeTableSource {
                bytes,
                limits,
                storage_bytes: reader.counts.storage,
                elements: reader.counts.elements,
                rows: reader.type_count,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn decode_sources(
        bytes: &[u8],
        limits: RuntimeTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeTypeSourceError> {
        Self::preflight_sources(bytes, limits, meter)?.materialize(meter)
    }
}

struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    length: usize,
    type_count: usize,
    counts: Counts,
}

impl<'a> Writer<'a> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        limits: RuntimeTypeSourceLimits,
        type_count: usize,
    ) -> Self {
        Self {
            output,
            meter,
            length: 0,
            type_count,
            counts: Counts::new(limits),
        }
    }
    fn extent(&mut self, bytes: usize) -> Result<(), RuntimeTypeSourceError> {
        self.length = self
            .length
            .checked_add(bytes)
            .ok_or(RuntimeTypeSourceError::Arithmetic)?;
        limit(
            "encoded bytes",
            self.length,
            self.counts.limits.encoded_bytes,
        )
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeTypeSourceError> {
        self.extent(bytes.len())?;
        if let Some(meter) = &mut self.meter {
            charge_source(meter, bytes.len())?;
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn tag(&mut self, tag: u8) -> Result<(), RuntimeTypeSourceError> {
        self.bytes(&[tag])
    }
    fn count(&mut self, count: usize) -> Result<(), RuntimeTypeSourceError> {
        let count = u32::try_from(count).map_err(|_| RuntimeTypeSourceError::Arithmetic)?;
        self.bytes(&count.to_le_bytes())
    }
    fn string(&mut self, value: &str) -> Result<(), RuntimeTypeSourceError> {
        self.counts.storage(value.len())?;
        self.count(value.len())?;
        self.bytes(value.as_bytes())
    }
    fn reference(&mut self, id: RuntimePlanTypeId) -> Result<(), RuntimeTypeSourceError> {
        let id = id.get().get();
        check_reference(id, self.type_count)?;
        self.counts.reference()?;
        self.bytes(&id.to_le_bytes())
    }
    fn table(&mut self, table: &RuntimePlanTypeTable) -> Result<(), RuntimeTypeSourceError> {
        self.counts.rows(table.len())?;
        self.bytes(DOMAIN)?;
        self.tag(1)?;
        self.count(table.len())?;
        for (ordinal, row) in table.declarations().enumerate() {
            self.count(ordinal)?;
            self.bytes(row.semantic_identity().as_bytes())?;
            self.tag(u8::from(row.nominal_declaration().is_some()))?;
            if let Some(nominal) = row.nominal_declaration() {
                self.bytes(nominal.as_bytes())?;
            }
            let prepared = if let Some(meter) = &mut self.meter {
                row.scope()
                    .prepare_source_encoding(self.counts.limits.scope(), meter)?
            } else {
                row.scope()
                    .prepare_source_in_frame(self.counts.limits.scope())?
            };
            self.count(prepared.encoded_bytes())?;
            self.extent(prepared.encoded_bytes())?;
            self.counts.storage(prepared.storage_bytes())?;
            self.counts.elements(row.scope().binders().len())?;
            if let Some(output) = &mut self.output {
                prepared.append_to(output)?;
            }
            self.projection(row.projection(), row.scope())?;
            self.tag(u8::from(row.data_codec().is_some()))?;
            if let Some(codec) = row.data_codec() {
                self.policy(codec, 0)?;
            }
        }
        Ok(())
    }
}

struct Reader<'a, 'meter> {
    bytes: &'a [u8],
    offset: usize,
    type_count: usize,
    counts: Counts,
    meter: &'meter mut TaskSemanticMeter,
}

impl<'a, 'meter> Reader<'a, 'meter> {
    fn new(
        bytes: &'a [u8],
        limits: RuntimeTypeSourceLimits,
        meter: &'meter mut TaskSemanticMeter,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            type_count: 0,
            counts: Counts::new(limits),
            meter,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], RuntimeTypeSourceError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RuntimeTypeSourceError::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeTypeSourceError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn tag(&mut self) -> Result<u8, RuntimeTypeSourceError> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, RuntimeTypeSourceError> {
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(RuntimeTypeSourceError::Tag {
                kind: "option/bool",
                tag,
            }),
        }
    }
    fn u16(&mut self) -> Result<u16, RuntimeTypeSourceError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two-byte coordinate"),
        ))
    }
    fn u32(&mut self) -> Result<u32, RuntimeTypeSourceError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four-byte coordinate"),
        ))
    }
    fn count(&mut self) -> Result<usize, RuntimeTypeSourceError> {
        usize::try_from(self.u32()?).map_err(|_| RuntimeTypeSourceError::Arithmetic)
    }
    fn digest(&mut self) -> Result<[u8; 32], RuntimeTypeSourceError> {
        Ok(self.take(32)?.try_into().expect("32-byte data identity"))
    }
    fn string(&mut self) -> Result<&'a str, RuntimeTypeSourceError> {
        let length = self.count()?;
        self.counts.storage(length)?;
        std::str::from_utf8(self.take(length)?).map_err(|_| RuntimeTypeSourceError::Utf8)
    }
    fn reference(&mut self) -> Result<RuntimePlanTypeId, RuntimeTypeSourceError> {
        let id = self.u32()?;
        check_reference(id, self.type_count)?;
        self.counts.reference()?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            NonZeroU32::new(id).expect("checked nonzero source coordinate"),
        ))
    }
    fn table(
        &mut self,
        materialize: bool,
    ) -> Result<Option<Vec<RuntimePlanTypeDeclaration>>, RuntimeTypeSourceError> {
        if self.take(DOMAIN.len())? != DOMAIN {
            return Err(RuntimeTypeSourceError::Domain);
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeTypeSourceError::Version { actual: version });
        }
        let count = self.count()?;
        self.type_count = count;
        self.counts.rows(count)?;
        let mut rows = materialize.then(|| Vec::with_capacity(count));
        for ordinal in 0..count {
            let expected =
                u32::try_from(ordinal).map_err(|_| RuntimeTypeSourceError::Arithmetic)?;
            let actual = self.u32()?;
            if actual != expected {
                return Err(RuntimeTypeSourceError::Ordinal { expected, actual });
            }
            let semantic_identity = RuntimeSemanticTypeId::from_bytes(self.digest()?);
            let nominal_declaration = if self.boolean()? {
                Some(RuntimeNominalDeclarationId::from_bytes(self.digest()?))
            } else {
                None
            };
            let scope_length = self.count()?;
            let scope = RuntimeTypeScope::preflight_source_in_frame(
                self.take(scope_length)?,
                self.counts.limits.scope(),
            )?;
            self.counts.storage(scope.storage_bytes())?;
            self.counts.elements(scope.as_scope_ref().binder_count())?;
            let projection = self.projection(materialize, scope.as_scope_ref())?;
            let data_codec = if self.boolean()? {
                self.policy(materialize, 0)?
            } else {
                None
            };
            if let Some(rows) = &mut rows {
                rows.push(RuntimePlanTypeDeclaration {
                    nominal_declaration,
                    semantic_identity,
                    scope: scope.materialize(),
                    projection: projection.expect("retaining checked projection source"),
                    data_codec,
                });
            }
        }
        if self.offset != self.bytes.len() {
            return Err(RuntimeTypeSourceError::Trailing);
        }
        Ok(rows)
    }
}

fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), RuntimeTypeSourceError> {
    if actual > maximum {
        return Err(RuntimeTypeSourceError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn check_reference(actual: u32, maximum: usize) -> Result<(), RuntimeTypeSourceError> {
    if actual == 0 || actual as usize > maximum {
        return Err(RuntimeTypeSourceError::Reference { actual, maximum });
    }
    Ok(())
}
fn charge_source(
    meter: &mut TaskSemanticMeter,
    bytes: usize,
) -> Result<(), RuntimeTypeSourceError> {
    let work = u64::try_from(bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(3))
        .ok_or(RuntimeTypeSourceError::Arithmetic)?;
    meter.charge_work(work)?;
    Ok(())
}

#[cfg(test)]
mod tests;
