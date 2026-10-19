//! Strict private version-one task source frame. Expected keys are assertions;
//! every semantic child and issued key still comes from the common seal.

use super::{
    ExpectedTaskPlanKey, RuntimeTaskPlan, RuntimeTaskSemanticBinding, UnsealedRuntimePlanImage,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::body_semantic::request::codec::{
    RuntimeTaskRequestCodecError, RuntimeTaskRequestCodecLimits,
};
use crate::plan::construction::source::program::{
    PreparedRuntimeProgramSource, RuntimeProgramSourceError, RuntimeProgramSourceLimits,
};
use crate::plan::{
    RuntimeControlEffectContractId, RuntimePlan, RuntimeTaskPlanIndex, RuntimeTaskPlanSealLimits,
    RuntimeTaskPlanSeed, RuntimeTaskRequestTemplate,
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
    pub(in crate::plan) program: RuntimeProgramSourceLimits,
}

#[derive(Debug, thiserror::Error)]
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
    #[error("task source program is invalid: {0}")]
    Program(Box<RuntimeProgramSourceError>),
    #[error("task {ordinal} request source is invalid: {source}")]
    Request {
        ordinal: usize,
        source: RuntimeTaskRequestCodecError,
    },
    #[error("task source family/binding is invalid: {0}")]
    Binding(#[from] crate::plan::RuntimeTaskPlanValidationError),
    #[error("task source image could not be retained: {0}")]
    Image(super::RuntimeTaskPlanSealError),
}

impl From<TaskSemanticEncodingError> for RuntimeTaskSourceCodecError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}

impl From<RuntimeProgramSourceError> for RuntimeTaskSourceCodecError {
    fn from(error: RuntimeProgramSourceError) -> Self {
        match error {
            RuntimeProgramSourceError::Resource(cause) => Self::Resource(cause),
            other => Self::Program(Box::new(other)),
        }
    }
}

impl From<super::RuntimeTaskPlanSealError> for RuntimeTaskSourceCodecError {
    fn from(error: super::RuntimeTaskPlanSealError) -> Self {
        match error {
            super::RuntimeTaskPlanSealError::Encoding(cause)
            | super::RuntimeTaskPlanSealError::Body(
                crate::plan::RuntimeBodySemanticError::Encoding(cause),
            ) => Self::Resource(cause),
            other => Self::Image(other),
        }
    }
}

impl RuntimeTaskSourceCodecLimits {
    fn remaining_program(
        self,
        wire_overhead: usize,
        task_storage: usize,
    ) -> Result<RuntimeProgramSourceLimits, RuntimeTaskSourceCodecError> {
        limit("encoded bytes", wire_overhead, self.encoded_bytes)?;
        limit("decoded bytes", task_storage, self.decoded_bytes)?;
        Ok(RuntimeProgramSourceLimits {
            encoded_bytes: self
                .program
                .encoded_bytes
                .min(self.encoded_bytes - wire_overhead),
            decoded_bytes: self
                .program
                .decoded_bytes
                .min(self.decoded_bytes - task_storage),
            ..self.program
        })
    }
}

impl RuntimeTaskSourceCodecError {
    /// Child wire/admission errors retain their context. Exhaustion belongs
    /// to the original shared bank, regardless of which child reached it.
    fn reject_in_bank(self, meter: &mut TaskSemanticMeter) -> Self {
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
struct References {
    functions: usize,
    types: usize,
    controls: usize,
    lines: usize,
}

impl References {
    fn original(plan: &RuntimePlan) -> Self {
        Self {
            functions: plan.function_sites().len(),
            types: plan.type_table().len(),
            controls: plan.control_effect_contracts().len(),
            lines: plan.line_task_groups().len(),
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
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Vec<u8>, RuntimeTaskSourceCodecError> {
        meter.status()?;
        let result = (|| {
            let (row_length, row_storage, references) = {
                let mut count =
                    Writer::new(None, Some(&mut *meter), limits, References::original(self));
                count.task_rows(self)?;
                (count.length, count.storage, count.references)
            };
            let overhead = DOMAIN
                .len()
                .checked_add(1 + 4)
                .and_then(|bytes| bytes.checked_add(row_length))
                .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
            limit("encoded bytes", overhead, limits.encoded_bytes)?;
            let program_limits = limits.remaining_program(overhead, row_storage)?;
            let program = self.encode_program_source(program_limits, resources, meter)?;
            let length = overhead
                .checked_add(program.bytes().len())
                .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
            charge_passes(meter, length, 2)?;
            let mut bytes = Vec::with_capacity(length);
            let mut writer = Writer::new(Some(&mut bytes), None, limits, references);
            writer.header()?;
            writer.frame(program.bytes().len(), |output| {
                output.extend_from_slice(program.bytes());
                Ok(())
            })?;
            storage(
                &mut writer.storage,
                1,
                program.storage_bytes(),
                limits.decoded_bytes,
            )?;
            writer.task_rows(self)?;
            debug_assert_eq!(writer.length, length);
            debug_assert_eq!(writer.storage, row_storage + program.storage_bytes());
            Ok(bytes)
        })();
        result.map_err(|error: RuntimeTaskSourceCodecError| error.reject_in_bank(meter))
    }
}

impl UnsealedRuntimePlanImage {
    /// Complete original Source enters once and reserves its own genuine
    /// namespace. Expected bytes are assertions checked only by the common seal.
    pub(in crate::plan) fn decode_task_sources(
        bytes: &[u8],
        limits: RuntimeTaskSourceCodecLimits,
        seal_limits: RuntimeTaskPlanSealLimits,
        mut meter: TaskSemanticMeter,
        resources: &mut ValueResourceAdmission,
    ) -> Result<Self, RuntimeTaskSourceCodecError> {
        meter.constrain(
            seal_limits.max_semantic_work,
            seal_limits.max_transcript_bytes,
        )?;
        let limits = RuntimeTaskSourceCodecLimits {
            rows: limits.rows.min(seal_limits.max_task_plan_rows),
            request_roles: limits.request_roles.min(seal_limits.max_request_roles),
            path_steps: limits.path_steps.min(seal_limits.max_children_per_row),
            ..limits
        };
        let checked = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_passes(&mut meter, bytes.len(), 3)?;
            // These zero counts are cursor initialization only. No task row is
            // read until its assertions come from the complete Source header.
            let mut count = Reader::new(
                bytes,
                limits,
                References {
                    functions: 0,
                    types: 0,
                    controls: 0,
                    lines: 0,
                },
                0,
            );
            count.header()?;
            let length = count.u32()? as usize;
            let program_bytes = count.slice(length)?;
            let namespaces =
                PreparedRuntimeProgramSource::namespaces(program_bytes, limits.program)?;
            limit("rows", namespaces.tasks, limits.rows as usize)?;
            limit(
                "control rows",
                namespaces.controls,
                seal_limits.max_control_effect_rows as usize,
            )?;
            count.references = References {
                functions: namespaces.functions,
                types: namespaces.types,
                controls: namespaces.controls,
                lines: namespaces.groups,
            };
            count.original_rows = u32::try_from(namespaces.tasks)
                .map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?;
            let row_offset = count.offset;
            count.task_rows(false)?;
            limit(
                "View rows",
                count.views,
                seal_limits.max_view_bindings as usize,
            )?;
            let program = PreparedRuntimeProgramSource::preflight(
                program_bytes,
                limits.remaining_program(bytes.len() - length, count.storage)?,
                resources,
                &mut meter,
            )?;
            Ok((
                program,
                count.references,
                count.original_rows,
                row_offset,
                count.storage,
            ))
        })();
        let (program, references, task_count, row_offset, task_storage) = checked
            .map_err(|error: RuntimeTaskSourceCodecError| error.reject_in_bank(&mut meter))?;
        let mut original = program.materialize(meter, resources)?;
        let retained = (|| {
            let mut reader = Reader::new(bytes, limits, references, task_count);
            reader.offset = row_offset;
            let rows = reader
                .task_rows(true)?
                .expect("materializing a checked task frame retains its rows");
            debug_assert_eq!(reader.storage, task_storage);
            for (ordinal, row) in rows.0.into_vec().into_iter().enumerate() {
                let request = row.request_template.try_map_types(|ty| {
                    original.source.type_identity(ty).ok_or(
                        RuntimeTaskSourceCodecError::Reference {
                            kind: "type",
                            actual: ty.get().get(),
                            maximum: references.types,
                        },
                    )
                })?;
                let binding = row.binding.try_complete_line(|group| {
                    original.groups.get(group.index()).cloned().ok_or(
                        RuntimeTaskSourceCodecError::Reference {
                            kind: "line",
                            actual: group.get().get(),
                            maximum: references.lines,
                        },
                    )
                })?;
                let function = original
                    .functions
                    .get(row.producer_function.get().get() as usize - 1)
                    .cloned()
                    .ok_or(RuntimeTaskSourceCodecError::Reference {
                        kind: "function",
                        actual: row.producer_function.get().get(),
                        maximum: references.functions,
                    })?;
                let control = original.source.control(row.control_effect).cloned().ok_or(
                    RuntimeTaskSourceCodecError::Reference {
                        kind: "control",
                        actual: row.control_effect.index() as u32 + 1,
                        maximum: references.controls,
                    },
                )?;
                original.source.builder.define_task_plan(
                    &original.tasks[ordinal],
                    RuntimeTaskPlanSeed::new(
                        function, row.family, row.class, request, control, binding,
                    ),
                )?;
            }
            Ok(rows.1)
        })();
        let expected = retained.map_err(|error: RuntimeTaskSourceCodecError| {
            match original.source.reject_owner() {
                TaskSemanticEncodingError::OwnerRejected => error,
                cause => RuntimeTaskSourceCodecError::Resource(cause),
            }
        })?;
        let mut image = original.source.prepare_image(seal_limits)?;
        if expected.len() != image.task_plans.len() {
            return Err(super::RuntimeTaskPlanSealError::ExpectedKeyCount {
                rows: image.task_plans.len(),
                keys: expected.len(),
            }
            .into());
        }
        image.expected_task_plan_keys = Some(expected);
        Ok(image)
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
        std::mem::size_of::<RuntimeTaskPlan>() + std::mem::size_of::<RuntimeTaskPlanSeed>(),
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

    fn header(&mut self) -> Result<(), RuntimeTaskSourceCodecError> {
        self.put(DOMAIN)?;
        self.put(&[1])
    }

    fn task_rows(&mut self, plan: &RuntimePlan) -> Result<(), RuntimeTaskSourceCodecError> {
        let count = plan.task_plans().len();
        limit("rows", count, self.limits.rows as usize)?;
        row_storage(&mut self.storage, count, self.limits.decoded_bytes)?;
        self.count(count)?;
        for ordinal in 0..count {
            let index =
                u32::try_from(ordinal).map_err(|_| RuntimeTaskSourceCodecError::Arithmetic)?;
            let source = plan
                .task_plans()
                .by_index(RuntimeTaskPlanIndex(index))
                .expect("a sealed task table has dense source rows");
            let row = source.definition();
            let key = source.key();
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
            prepared
                .mapped_type_storage_bytes::<crate::pattern::RuntimeSemanticTypeId>()
                .map_err(|source| request_error(ordinal, source))?,
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
    fn extent(&mut self, length: usize) -> Result<(), RuntimeTaskSourceCodecError> {
        if let Some(meter) = &mut self.meter {
            meter.charge_work(length as u64)?;
        }
        let next = self
            .length
            .checked_add(length)
            .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
        limit("encoded bytes", next, self.limits.encoded_bytes)?;
        self.length = next;
        Ok(())
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), RuntimeTaskSourceCodecError> {
        self.extent(bytes.len())?;
        if let Some(output) = &mut self.bytes {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn frame(
        &mut self,
        length: usize,
        append: impl FnOnce(&mut Vec<u8>) -> Result<(), RuntimeTaskSourceCodecError>,
    ) -> Result<(), RuntimeTaskSourceCodecError> {
        self.count(length)?;
        self.extent(length)?;
        let output = self
            .bytes
            .as_mut()
            .expect("prepared frames are appended only after counting");
        let start = output.len();
        append(output)?;
        debug_assert_eq!(output.len() - start, length);
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
    views: usize,
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
            views: 0,
        }
    }

    fn header(&mut self) -> Result<(), RuntimeTaskSourceCodecError> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(RuntimeTaskSourceCodecError::Domain);
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeTaskSourceCodecError::Version { actual: version });
        }
        Ok(())
    }

    fn task_rows(
        &mut self,
        materialize: bool,
    ) -> Result<Option<DecodedRows>, RuntimeTaskSourceCodecError> {
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
                request
                    .mapped_type_storage_bytes::<crate::pattern::RuntimeSemanticTypeId>()
                    .map_err(|source| request_error(ordinal as usize, source))?,
                self.limits.decoded_bytes,
            )?;
            let control = reference("control", self.u32()?, self.references.controls)?;
            let binding = self.binding()?;
            family.validate_runtime_task_binding(&binding)?;
            if matches!(binding, RuntimeTaskSemanticBinding::View) {
                self.views = self
                    .views
                    .checked_add(1)
                    .ok_or(RuntimeTaskSourceCodecError::Arithmetic)?;
            }
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
