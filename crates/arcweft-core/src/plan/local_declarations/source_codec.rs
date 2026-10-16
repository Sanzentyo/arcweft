//! Private local request source frame. Physical placement is derived from the
//! actual decoded executable tables by the existing construction authority;
//! it is deliberately not accepted as wire-supplied placement evidence.

use super::{
    PendingRuntimeLocalDeclaration, RuntimeGeneratedLocalOrigin, RuntimeLocalBindingDeclaration,
    RuntimeLocalBindingKind, RuntimeLocalBindingStorage, RuntimeLocalDeclaration,
    RuntimeLocalDeclarationSource, RuntimeLocalDeclarationTable,
    RuntimeLocalDeclarationTableBuilder,
};
use crate::plan::{
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionParameterIdentity, RuntimePlanTypeTable,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use std::num::NonZeroU32;

const DOMAIN: &[u8] = b"arcweft.runtime.local-declarations.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeLocalSourceLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) type_count: usize,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeLocalSourceError {
    #[error("local source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid local source domain")]
    Domain,
    #[error("unsupported local source version {actual}")]
    Version { actual: u8 },
    #[error("local source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("local source arithmetic overflow")]
    Arithmetic,
    #[error("local source is truncated")]
    Truncated,
    #[error("local source has trailing data")]
    Trailing,
    #[error("unknown local source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("local source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("local source type coordinate {actual} is outside 1..={maximum}")]
    Type { actual: u32, maximum: usize },
    #[error("original local type table count {actual} differs from preflight count {expected}")]
    TypeTableCount { expected: usize, actual: usize },
    #[error("local type {ty} is invalid in original function context {context:?}")]
    Context {
        ty: RuntimePlanTypeId,
        context: Option<RuntimePlanTypeId>,
    },
    #[error("local source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl From<TaskSemanticEncodingError> for RuntimeLocalSourceError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

pub(in crate::plan) struct PreparedRuntimeLocalEncoding<'a> {
    table: &'a RuntimeLocalDeclarationTable,
    limits: RuntimeLocalSourceLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
}

impl PreparedRuntimeLocalEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) fn append_to(
        self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeLocalSourceError> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(RuntimeLocalSourceError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.limits);
        writer.table(self.table)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.storage, self.storage_bytes);
        Ok(())
    }
}

/// Complete bounded declaration source data. This capability cannot produce a
/// final local table: executable placement still belongs to the original bodies.
pub(in crate::plan) struct PreparedRuntimeLocalSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeLocalSourceLimits,
    rows: usize,
    storage_bytes: usize,
}

impl PreparedRuntimeLocalSources<'_> {
    pub(in crate::plan) const fn row_count(&self) -> usize {
        self.rows
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) fn materialize(
        self,
        types: &RuntimePlanTypeTable,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeLocalDeclarationTableBuilder, RuntimeLocalSourceError> {
        meter.status()?;
        let result = (|| {
            if types.len() != self.limits.type_count {
                return Err(RuntimeLocalSourceError::TypeTableCount {
                    expected: self.limits.type_count,
                    actual: types.len(),
                });
            }
            // The shared type owner validates every context before the first
            // pending-row allocation. No temporary lexical scope is cloned.
            Reader::new(self.bytes, self.limits).table(false, Some(types))?;
            let mut reader = Reader::new(self.bytes, self.limits);
            let builder = reader
                .table(true, Some(types))?
                .expect("retaining bounded local request sources");
            debug_assert_eq!(builder.len(), self.rows);
            debug_assert_eq!(reader.storage, self.storage_bytes);
            Ok(builder)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl RuntimeLocalDeclarationTable {
    pub(in crate::plan) fn prepare_sources_encoding(
        &self,
        limits: RuntimeLocalSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLocalEncoding<'_>, RuntimeLocalSourceError> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer::new(None, Some(&mut *meter), limits);
            writer.table(self)?;
            Ok(PreparedRuntimeLocalEncoding {
                table: self,
                limits,
                encoded_bytes: writer.length,
                storage_bytes: writer.storage,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn encode_sources(
        &self,
        limits: RuntimeLocalSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<u8>, RuntimeLocalSourceError> {
        let prepared = self.prepare_sources_encoding(limits, meter)?;
        let mut output = Vec::with_capacity(prepared.encoded_bytes());
        prepared.append_to(&mut output)?;
        Ok(output)
    }
}

impl RuntimeLocalDeclarationTableBuilder {
    pub(in crate::plan) fn preflight_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimeLocalSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeLocalSources<'a>, RuntimeLocalSourceError> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut reader = Reader::new(bytes, limits);
            reader.table(false, None)?;
            Ok(PreparedRuntimeLocalSources {
                bytes,
                limits,
                rows: reader.rows,
                storage_bytes: reader.storage,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    limits: RuntimeLocalSourceLimits,
    length: usize,
    storage: usize,
}

impl<'a> Writer<'a> {
    fn new(
        output: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        limits: RuntimeLocalSourceLimits,
    ) -> Self {
        Self {
            output,
            meter,
            limits,
            length: 0,
            storage: 0,
        }
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeLocalSourceError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(RuntimeLocalSourceError::Arithmetic)?;
        limit("encoded bytes", self.length, self.limits.encoded_bytes)?;
        if let Some(meter) = &mut self.meter {
            charge(meter, bytes.len())?;
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn tag(&mut self, tag: u8) -> Result<(), RuntimeLocalSourceError> {
        self.bytes(&[tag])
    }
    fn count(&mut self, count: usize) -> Result<(), RuntimeLocalSourceError> {
        self.bytes(
            &u32::try_from(count)
                .map_err(|_| RuntimeLocalSourceError::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn ty(&mut self, ty: RuntimePlanTypeId) -> Result<(), RuntimeLocalSourceError> {
        let id = ty.get().get();
        check_type(id, self.limits.type_count)?;
        self.bytes(&id.to_le_bytes())
    }
    fn source(
        &mut self,
        source: RuntimeLocalDeclarationSource,
    ) -> Result<(), RuntimeLocalSourceError> {
        match source {
            RuntimeLocalDeclarationSource::Binding {
                identity,
                declaration,
            } => {
                self.tag(0)?;
                self.bytes(&identity)?;
                self.tag(declaration.kind().semantic_tag())?;
                self.tag(u8::from(declaration.is_mutable()))?;
                self.tag(declaration.storage().semantic_tag())?;
            }
            RuntimeLocalDeclarationSource::Parameter(identity) => {
                self.tag(1)?;
                self.bytes(identity.as_bytes())?;
            }
            RuntimeLocalDeclarationSource::EvaluatedResult(identity) => {
                self.tag(2)?;
                self.bytes(identity.as_bytes())?;
            }
            RuntimeLocalDeclarationSource::Generated(identity) => {
                self.tag(3)?;
                self.bytes(identity.as_bytes())?;
            }
        }
        Ok(())
    }
    fn table(
        &mut self,
        table: &RuntimeLocalDeclarationTable,
    ) -> Result<(), RuntimeLocalSourceError> {
        self.storage = storage(table.len(), self.limits)?;
        self.bytes(DOMAIN)?;
        self.tag(1)?;
        self.count(table.len())?;
        for (ordinal, row) in table.declarations().enumerate() {
            self.count(ordinal)?;
            self.source(row.source())?;
            self.ty(row.ty())?;
            self.tag(u8::from(row.context().is_some()))?;
            if let Some(context) = row.context() {
                self.ty(context)?;
            }
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeLocalSourceLimits,
    storage: usize,
    rows: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], limits: RuntimeLocalSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
            storage: 0,
            rows: 0,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], RuntimeLocalSourceError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RuntimeLocalSourceError::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeLocalSourceError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn tag(&mut self) -> Result<u8, RuntimeLocalSourceError> {
        Ok(self.take(1)?[0])
    }
    fn boolean(&mut self) -> Result<bool, RuntimeLocalSourceError> {
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(RuntimeLocalSourceError::Tag {
                kind: "option/bool",
                tag,
            }),
        }
    }
    fn count(&mut self) -> Result<u32, RuntimeLocalSourceError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four-byte source ordinal"),
        ))
    }
    fn identity(&mut self) -> Result<[u8; 32], RuntimeLocalSourceError> {
        Ok(self.take(32)?.try_into().expect("32-byte source identity"))
    }
    fn ty(&mut self) -> Result<RuntimePlanTypeId, RuntimeLocalSourceError> {
        let id = self.count()?;
        check_type(id, self.limits.type_count)?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            NonZeroU32::new(id).expect("checked original type coordinate"),
        ))
    }
    fn source(&mut self) -> Result<RuntimeLocalDeclarationSource, RuntimeLocalSourceError> {
        Ok(match self.tag()? {
            0 => {
                let identity = self.identity()?;
                let tag = self.tag()?;
                let kind = RuntimeLocalBindingKind::from_semantic_tag(tag).ok_or(
                    RuntimeLocalSourceError::Tag {
                        kind: "binding kind",
                        tag,
                    },
                )?;
                let mutable = self.boolean()?;
                let tag = self.tag()?;
                let storage = RuntimeLocalBindingStorage::from_semantic_tag(tag).ok_or(
                    RuntimeLocalSourceError::Tag {
                        kind: "binding storage",
                        tag,
                    },
                )?;
                RuntimeLocalDeclarationSource::Binding {
                    identity,
                    declaration: RuntimeLocalBindingDeclaration::new(kind, mutable, storage),
                }
            }
            1 => RuntimeLocalDeclarationSource::Parameter(
                RuntimeFunctionParameterIdentity::from_accepted_identity(self.identity()?),
            ),
            2 => RuntimeLocalDeclarationSource::EvaluatedResult(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.identity()?),
            ),
            3 => RuntimeLocalDeclarationSource::Generated(
                RuntimeGeneratedLocalOrigin::from_accepted_identity(self.identity()?),
            ),
            tag => {
                return Err(RuntimeLocalSourceError::Tag {
                    kind: "origin",
                    tag,
                });
            }
        })
    }
    fn table(
        &mut self,
        materialize: bool,
        types: Option<&RuntimePlanTypeTable>,
    ) -> Result<Option<RuntimeLocalDeclarationTableBuilder>, RuntimeLocalSourceError> {
        if self.take(DOMAIN.len())? != DOMAIN {
            return Err(RuntimeLocalSourceError::Domain);
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeLocalSourceError::Version { actual: version });
        }
        let count = self.count()?;
        self.rows = count as usize;
        self.storage = storage(self.rows, self.limits)?;
        let mut builder = materialize.then(|| RuntimeLocalDeclarationTableBuilder {
            declarations: Vec::with_capacity(self.rows),
            maximum: count,
            reserved: count,
            sealed: false,
        });
        for ordinal in 0..count {
            let actual = self.count()?;
            if actual != ordinal {
                return Err(RuntimeLocalSourceError::Ordinal {
                    expected: ordinal,
                    actual,
                });
            }
            let source = self.source()?;
            let ty = self.ty()?;
            let context = if self.boolean()? {
                Some(self.ty()?)
            } else {
                None
            };
            if let Some(types) = types {
                let valid = types.get(ty).is_some_and(|row| match context {
                    None => row.admits_local_context(None),
                    Some(context) => types
                        .get(context)
                        .is_some_and(|owner| row.admits_local_context(Some(owner))),
                });
                if !valid {
                    return Err(RuntimeLocalSourceError::Context { ty, context });
                }
            }
            if let Some(builder) = &mut builder {
                builder
                    .materialize(source, ty, context)
                    .ok_or(RuntimeLocalSourceError::Arithmetic)?;
            }
        }
        if self.offset != self.bytes.len() {
            return Err(RuntimeLocalSourceError::Trailing);
        }
        Ok(builder)
    }
}

fn storage(
    rows: usize,
    limits: RuntimeLocalSourceLimits,
) -> Result<usize, RuntimeLocalSourceError> {
    limit("rows", rows, limits.rows as usize)?;
    let bytes = rows
        .checked_mul(
            size_of::<PendingRuntimeLocalDeclaration>() + size_of::<RuntimeLocalDeclaration>(),
        )
        .ok_or(RuntimeLocalSourceError::Arithmetic)?;
    limit("decoded bytes", bytes, limits.decoded_bytes)?;
    Ok(bytes)
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), RuntimeLocalSourceError> {
    if actual > maximum {
        return Err(RuntimeLocalSourceError::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn check_type(actual: u32, maximum: usize) -> Result<(), RuntimeLocalSourceError> {
    if actual == 0 || actual as usize > maximum {
        return Err(RuntimeLocalSourceError::Type { actual, maximum });
    }
    Ok(())
}
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), RuntimeLocalSourceError> {
    meter.charge_work(
        u64::try_from(bytes)
            .ok()
            .and_then(|n| n.checked_mul(3))
            .ok_or(RuntimeLocalSourceError::Arithmetic)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
