//! Strict private version-one task source frame. Expected keys are assertions;
//! every semantic child and issued key still comes from the common seal.

use super::{
    ExpectedTaskPlanKey, RuntimeTaskPlan, RuntimeTaskSemanticBinding, UnsealedRuntimePlanImage,
};
use crate::plan::body_semantic::request::codec::{
    RuntimeTaskRequestCodecError, RuntimeTaskRequestCodecLimits,
};
use crate::plan::{
    RuntimeControlEffectContractId, RuntimePlan, RuntimePlanInventory,
    RuntimeTaskPlanCoordinateOwner, RuntimeTaskPlanIndex, RuntimeTaskRequestTemplate,
};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLineTaskGroupId};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{NeedProducerFamily, TaskClass};
use std::num::NonZeroU32;

const DOMAIN: &[u8] = b"arcweft.runtime.task-sources.v1\0";
const ROW_FIXED_BYTES: usize = 4 + 4 + 1 + 1 + 4 + 4 + 1 + 32;

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeTaskSourceCodecLimits {
    pub(in crate::plan) rows: u32,
    pub(in crate::plan) encoded_bytes: usize,
    pub(in crate::plan) decoded_bytes: usize,
    pub(in crate::plan) request_roles: u32,
    pub(in crate::plan) path_steps: u32,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeTaskSourceCodecError {
    #[error("task source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid task source domain")]
    Domain,
    #[error("unsupported task source version {actual}")]
    Version { actual: u8 },
    #[error("task source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("task source count arithmetic overflow")]
    Arithmetic,
    #[error("task source input is truncated")]
    Truncated,
    #[error("task source input has trailing bytes")]
    Trailing,
    #[error("unknown task source {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("task source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("task source {kind} coordinate {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("task source count {actual} differs from original namespace count {expected}")]
    NamespaceCount { expected: u32, actual: u32 },
    #[error("task {ordinal} request source is invalid: {source}")]
    Request {
        ordinal: usize,
        source: RuntimeTaskRequestCodecError,
    },
    #[error("task source family/binding is invalid: {0}")]
    Binding(#[from] crate::plan::RuntimeTaskPlanValidationError),
    #[error("task source image could not be retained: {0}")]
    Image(#[from] super::RuntimeTaskPlanSealError),
}

impl From<TaskSemanticEncodingError> for RuntimeTaskSourceCodecError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy)]
struct References {
    functions: usize,
    types: usize,
    controls: usize,
    lines: usize,
}

impl References {
    fn original(inventory: &RuntimePlanInventory) -> Self {
        Self {
            functions: inventory.function_sites.len(),
            types: inventory.type_table.len(),
            controls: inventory.control_effect_contracts.len(),
            lines: inventory.line_task_groups.len(),
        }
    }

    fn request(self, limits: RuntimeTaskSourceCodecLimits) -> RuntimeTaskRequestCodecLimits {
        RuntimeTaskRequestCodecLimits {
            roles: limits.request_roles,
            path_steps: limits.path_steps,
            encoded_bytes: limits.encoded_bytes,
            decoded_bytes: limits.decoded_bytes,
            type_count: self.types,
        }
    }
}

impl RuntimePlan {
    pub(in crate::plan) fn encode_task_sources(
        &self,
        limits: RuntimeTaskSourceCodecLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<u8>, RuntimeTaskSourceCodecError> {
        meter.status()?;
        let result = (|| {
            let (length, measured_storage, references) = {
                let mut count = Writer::new(
                    None,
                    Some(&mut *meter),
                    limits,
                    References::original(&self.inventory),
                );
                count.program(self)?;
                (count.length, count.storage, count.references)
            };
            charge_passes(meter, length, 2)?;
            let mut bytes = Vec::with_capacity(length);
            let mut writer = Writer::new(Some(&mut bytes), None, limits, references);
            writer.program(self)?;
            debug_assert_eq!(writer.length, length);
            debug_assert_eq!(writer.storage, measured_storage);
            Ok(bytes)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl UnsealedRuntimePlanImage {
    /// The original inventory and namespace enter once. No decoded data is
    /// published, and no expected bytes can become a key or a semantic child.
    pub(in crate::plan) fn decode_task_sources(
        inventory: RuntimePlanInventory,
        coordinate_owner: RuntimeTaskPlanCoordinateOwner,
        bytes: &[u8],
        limits: RuntimeTaskSourceCodecLimits,
        mut meter: TaskSemanticMeter,
    ) -> Result<Self, RuntimeTaskSourceCodecError> {
        meter.status()?;
        let decoded = (|| {
            coordinate_owner.ensure_unsealed()?;
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_passes(&mut meter, bytes.len(), 3)?;
            let references = References::original(&inventory);
            let mut count = Reader::new(bytes, limits, references, coordinate_owner.len());
            count.program(false)?;
            let mut reader = Reader::new(bytes, limits, references, coordinate_owner.len());
            let rows = reader
                .program(true)?
                .expect("materializing a checked task frame retains its rows");
            debug_assert_eq!(reader.storage, count.storage);
            Ok::<_, RuntimeTaskSourceCodecError>(rows)
        })();
        let (rows, expected) = match decoded {
            Ok(rows) => rows,
            Err(error) => {
                meter.reject_owner();
                return Err(error);
            }
        };
        Self::new(inventory, coordinate_owner, rows, Some(expected), meter).map_err(Into::into)
    }
}

fn charge_passes(
    meter: &mut TaskSemanticMeter,
    bytes: usize,
    passes: u64,
) -> Result<(), RuntimeTaskSourceCodecError> {
    let units = u64::try_from(bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(passes))
        .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
    meter.charge_work(units)?;
    Ok(())
}

fn limit(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimeTaskSourceCodecError> {
    if actual > maximum {
        Err(RuntimeTaskSourceCodecError::Limit {
            kind,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn storage(
    total: &mut usize,
    count: usize,
    element: usize,
    maximum: usize,
) -> Result<(), RuntimeTaskSourceCodecError> {
    let next = count
        .checked_mul(element)
        .and_then(|bytes| total.checked_add(bytes))
        .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
    limit("decoded bytes", next, maximum)?;
    *total = next;
    Ok(())
}

fn row_storage(
    total: &mut usize,
    count: usize,
    maximum: usize,
) -> Result<(), RuntimeTaskSourceCodecError> {
    storage(
        total,
        count,
        std::mem::size_of::<RuntimeTaskPlan>(),
        maximum,
    )?;
    storage(
        total,
        count,
        std::mem::size_of::<ExpectedTaskPlanKey>(),
        maximum,
    )
}

fn request_error(
    ordinal: usize,
    source: RuntimeTaskRequestCodecError,
) -> RuntimeTaskSourceCodecError {
    match source {
        RuntimeTaskRequestCodecError::Resource(error) => {
            RuntimeTaskSourceCodecError::Resource(error)
        }
        source => RuntimeTaskSourceCodecError::Request { ordinal, source },
    }
}

fn reference(
    kind: &'static str,
    raw: u32,
    maximum: usize,
) -> Result<NonZeroU32, RuntimeTaskSourceCodecError> {
    if raw == 0 || raw as usize > maximum {
        return Err(RuntimeTaskSourceCodecError::Reference {
            kind,
            actual: raw,
            maximum,
        });
    }
    Ok(NonZeroU32::new(raw).expect("a checked source coordinate is nonzero"))
}

struct Writer<'a> {
    bytes: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    length: usize,
    storage: usize,
    limits: RuntimeTaskSourceCodecLimits,
    references: References,
}

impl<'a> Writer<'a> {
    fn new(
        bytes: Option<&'a mut Vec<u8>>,
        meter: Option<&'a mut TaskSemanticMeter>,
        limits: RuntimeTaskSourceCodecLimits,
        references: References,
    ) -> Self {
        Self {
            bytes,
            meter,
            length: 0,
            storage: 0,
            limits,
            references,
        }
    }

    fn program(&mut self, plan: &RuntimePlan) -> Result<(), RuntimeTaskSourceCodecError> {
        let count = plan.task_plans().len();
        limit("rows", count, self.limits.rows as usize)?;
        row_storage(&mut self.storage, count, self.limits.decoded_bytes)?;
        self.put(DOMAIN)?;
        self.put(&[1])?;
        self.count(count)?;
        for ordinal in 0..count {
            let index =
                u32::try_from(ordinal).map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?;
            let (row, key) = plan
                .task_plans()
                .by_index(RuntimeTaskPlanIndex(index))
                .expect("a sealed task table has dense source rows");
            row.family().validate_runtime_task_binding(row.binding())?;
            self.count(ordinal)?;
            let function = row.producer_function().get().get();
            reference("function", function, self.references.functions)?;
            self.u32(function)?;
            self.put(&[row.family().semantic_tag(), row.class().semantic_tag()])?;
            self.request(ordinal, row.request_template())?;
            let control = row
                .control_effect()
                .index()
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
            reference("control", control, self.references.controls)?;
            self.u32(control)?;
            self.put(&[row.binding().semantic_tag()])?;
            match row.binding() {
                crate::plan::RuntimeTaskSemanticBinding::Timeout { contract } => {
                    self.put(contract.as_bytes())?;
                }
                crate::plan::RuntimeTaskSemanticBinding::Line { plan } => {
                    let group = plan.group().get().get();
                    reference("line", group, self.references.lines)?;
                    self.u32(group)?;
                }
                _ => {}
            }
            self.put(key.as_bytes())?;
        }
        Ok(())
    }

    fn request(
        &mut self,
        ordinal: usize,
        definition: &RuntimeTaskRequestTemplate,
    ) -> Result<(), RuntimeTaskSourceCodecError> {
        let limits = self.references.request(self.limits);
        let prepared = match &mut self.meter {
            Some(meter) => definition.prepare_encoding_with_meter(limits, meter),
            None => definition.prepare_encoding(limits),
        }
        .map_err(|source| request_error(ordinal, source))?;
        storage(
            &mut self.storage,
            1,
            prepared.storage_bytes(),
            self.limits.decoded_bytes,
        )?;
        let length = prepared.encoded_bytes();
        self.count(length)?;
        let next = self
            .length
            .checked_add(length)
            .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
        limit("encoded bytes", next, self.limits.encoded_bytes)?;
        if let Some(bytes) = &mut self.bytes {
            prepared
                .append_to(bytes)
                .map_err(|source| request_error(ordinal, source))?;
        }
        self.length = next;
        Ok(())
    }

    fn count(&mut self, value: usize) -> Result<(), RuntimeTaskSourceCodecError> {
        self.u32(u32::try_from(value).map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?)
    }
    fn u32(&mut self, value: u32) -> Result<(), RuntimeTaskSourceCodecError> {
        self.put(&value.to_le_bytes())
    }
    fn put(&mut self, bytes: &[u8]) -> Result<(), RuntimeTaskSourceCodecError> {
        if let Some(meter) = &mut self.meter {
            meter.charge_work(bytes.len() as u64)?;
        }
        let next = self
            .length
            .checked_add(bytes.len())
            .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
        limit("encoded bytes", next, self.limits.encoded_bytes)?;
        if let Some(output) = &mut self.bytes {
            output.extend_from_slice(bytes);
        }
        self.length = next;
        Ok(())
    }
}

type DecodedRows = (Box<[RuntimeTaskPlan]>, Box<[ExpectedTaskPlanKey]>);

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    storage: usize,
    limits: RuntimeTaskSourceCodecLimits,
    references: References,
    original_rows: u32,
}

impl<'a> Reader<'a> {
    fn new(
        bytes: &'a [u8],
        limits: RuntimeTaskSourceCodecLimits,
        references: References,
        original_rows: u32,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            storage: 0,
            limits,
            references,
            original_rows,
        }
    }

    fn program(
        &mut self,
        materialize: bool,
    ) -> Result<Option<DecodedRows>, RuntimeTaskSourceCodecError> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(RuntimeTaskSourceCodecError::Domain);
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeTaskSourceCodecError::Version { actual: version });
        }
        let count = self.u32()?;
        limit("rows", count as usize, self.limits.rows as usize)?;
        if count != self.original_rows {
            return Err(RuntimeTaskSourceCodecError::NamespaceCount {
                expected: self.original_rows,
                actual: count,
            });
        }
        let minimum = (count as usize)
            .checked_mul(ROW_FIXED_BYTES)
            .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
        if minimum > self.bytes.len() - self.offset {
            return Err(RuntimeTaskSourceCodecError::Truncated);
        }
        row_storage(&mut self.storage, count as usize, self.limits.decoded_bytes)?;
        let mut rows = materialize.then(|| Vec::with_capacity(count as usize));
        let mut keys = materialize.then(|| Vec::with_capacity(count as usize));
        for ordinal in 0..count {
            let actual = self.u32()?;
            if actual != ordinal {
                return Err(RuntimeTaskSourceCodecError::Ordinal {
                    expected: ordinal,
                    actual,
                });
            }
            let function = reference("function", self.u32()?, self.references.functions)?;
            let family_tag = self.tag()?;
            let family = NeedProducerFamily::from_semantic_tag(family_tag).ok_or(
                RuntimeTaskSourceCodecError::Tag {
                    kind: "family",
                    tag: family_tag,
                },
            )?;
            let class_tag = self.tag()?;
            let class = TaskClass::from_semantic_tag(class_tag).ok_or(
                RuntimeTaskSourceCodecError::Tag {
                    kind: "class",
                    tag: class_tag,
                },
            )?;
            let request_len = self.u32()? as usize;
            let request_bytes = self.slice(request_len)?;
            let request = RuntimeTaskRequestTemplate::preflight_source(
                request_bytes,
                self.references.request(self.limits),
            )
            .map_err(|source| request_error(ordinal as usize, source))?;
            storage(
                &mut self.storage,
                1,
                request.storage_bytes(),
                self.limits.decoded_bytes,
            )?;
            let control = reference("control", self.u32()?, self.references.controls)?;
            let binding = self.binding()?;
            family.validate_runtime_task_binding(&binding)?;
            let expected = ExpectedTaskPlanKey::new(self.take()?);
            if let (Some(rows), Some(keys)) = (&mut rows, &mut keys) {
                rows.push(RuntimeTaskPlan {
                    producer_function: RuntimeFunctionSiteId::from_accepted_ordinal(function),
                    family,
                    class,
                    request_template: request
                        .materialize()
                        .map_err(|source| request_error(ordinal as usize, source))?,
                    control_effect: RuntimeControlEffectContractId::for_index(
                        (control.get() - 1) as usize,
                    )
                    .expect("a checked control coordinate fits its original table"),
                    binding,
                });
                keys.push(expected);
            }
        }
        if self.offset != self.bytes.len() {
            return Err(RuntimeTaskSourceCodecError::Trailing);
        }
        Ok(rows
            .zip(keys)
            .map(|(rows, keys)| (rows.into_boxed_slice(), keys.into_boxed_slice())))
    }

    fn binding(&mut self) -> Result<RuntimeTaskSemanticBinding, RuntimeTaskSourceCodecError> {
        Ok(match self.tag()? {
            0 => RuntimeTaskSemanticBinding::Ordinary,
            1 => RuntimeTaskSemanticBinding::View,
            2 => RuntimeTaskSemanticBinding::AwaitManyBase,
            3 => RuntimeTaskSemanticBinding::AwaitManyChild,
            4 => RuntimeTaskSemanticBinding::Timeout {
                contract: crate::task::NeedTimeoutContractDigest::from_bytes(self.take()?),
            },
            5 => {
                let group = reference("line", self.u32()?, self.references.lines)?;
                RuntimeTaskSemanticBinding::Line {
                    plan: RuntimeLineTaskGroupId::from_accepted_ordinal(group),
                }
            }
            tag => {
                return Err(RuntimeTaskSourceCodecError::Tag {
                    kind: "binding",
                    tag,
                });
            }
        })
    }

    fn slice(&mut self, length: usize) -> Result<&'a [u8], RuntimeTaskSourceCodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeTaskSourceCodecError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], RuntimeTaskSourceCodecError> {
        self.slice(N)?
            .try_into()
            .map_err(|_| RuntimeTaskSourceCodecError::Truncated)
    }
    fn tag(&mut self) -> Result<u8, RuntimeTaskSourceCodecError> {
        Ok(self.take::<1>()?[0])
    }
    fn u32(&mut self) -> Result<u32, RuntimeTaskSourceCodecError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
}

#[cfg(test)]
mod tests;
