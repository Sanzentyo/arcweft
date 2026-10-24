//! One formatter grammar, stored live or inert without parallel DTOs.

use super::super::FiberStateError;
use super::super::input::FrameValueContext;
use super::super::{AwbcSaveResult, FiberCursor, RuntimeFormatContext, RuntimeProgramOwner};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::runtime_id::RuntimeFormatAttemptId;
use crate::value::RuntimeValueView;

#[cfg(test)]
mod tests;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeFmtParameterId, RuntimeValue};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Source-ordered operands retained by the instruction that owns the formatter.
/// Only the inert projection has a codec; a live formatter requires its program.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberFormatState;
/// let _: FiberFormatState = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberFormatState;
/// fn encode_live(state: &FiberFormatState) { let _ = serde_json::to_vec(state); }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FiberFormatState<V = RuntimeValue> {
    pub(in crate::awbc::fiber) storage: FormatStaging<V>,
}

/// A nested Flow operand transaction. Its active phase cannot retain a failure
/// or scope depth without the exact parameter whose evaluation owns them.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberFormatAttemptState;
/// let _: FiberFormatAttemptState = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberFormatAttemptState;
/// fn encode_live(state: &FiberFormatAttemptState) { let _ = serde_json::to_vec(state); }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FiberFormatAttemptState<V = RuntimeValue> {
    pub(in crate::awbc::fiber) storage: FormatAttempt<V>,
}

// The private records are the sole stored grammar. Concrete inert-only codecs
// delegate here; deriving their field traversal does not expose a live reader.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::awbc::fiber) struct FormatStaging<V> {
    pub(in crate::awbc::fiber) site: FiberCursor,
    pub(in crate::awbc::fiber) format_context: RuntimeFormatContext,
    pub(in crate::awbc::fiber) next_operand: usize,
    pub(in crate::awbc::fiber) values: Vec<Option<V>>,
    pub(in crate::awbc::fiber) first_recoverable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::awbc::fiber) struct FormatAttempt<V> {
    pub(in crate::awbc::fiber) attempt: RuntimeFormatAttemptId,
    pub(in crate::awbc::fiber) staging: FormatStaging<V>,
    pub(in crate::awbc::fiber) progress: FormatOperandProgress,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::awbc::fiber) enum FormatOperandProgress {
    Idle,
    Evaluating {
        #[serde(with = "parameter_id")]
        parameter: RuntimeFmtParameterId,
        scope_depth: usize,
        failure: Option<String>,
    },
}

mod parameter_id {
    use super::{Deserialize, Deserializer, RuntimeFmtParameterId, Serialize, Serializer};

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde field adapters are called with a borrowed field; this codec signature is fixed by derive"
    )]
    pub(super) fn serialize<S: Serializer>(
        parameter: &RuntimeFmtParameterId,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        u8::try_from(parameter.index())
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<RuntimeFmtParameterId, D::Error> {
        let parameter = u8::deserialize(deserializer)?;
        RuntimeFmtParameterId::from_index(usize::from(parameter)).ok_or_else(|| {
            serde::de::Error::custom(format_args!("unknown fmt parameter {parameter}"))
        })
    }
}

impl<V> FiberFormatState<V> {
    pub const fn format_context(&self) -> &RuntimeFormatContext {
        &self.storage.format_context
    }

    pub const fn site(&self) -> FiberCursor {
        self.storage.site
    }

    pub const fn next_operand(&self) -> usize {
        self.storage.next_operand
    }

    pub fn values(&self) -> &[Option<V>] {
        &self.storage.values
    }

    pub fn first_recoverable(&self) -> Option<&str> {
        self.storage.first_recoverable.as_deref()
    }
}

impl<V> FiberFormatAttemptState<V> {
    #[must_use]
    pub const fn attempt(&self) -> RuntimeFormatAttemptId {
        self.storage.attempt
    }

    #[must_use]
    pub const fn site(&self) -> FiberCursor {
        self.storage.staging.site
    }

    #[must_use]
    pub const fn next_operand(&self) -> usize {
        self.storage.staging.next_operand
    }

    #[must_use]
    pub const fn active_parameter(&self) -> Option<RuntimeFmtParameterId> {
        match self.storage.progress {
            FormatOperandProgress::Idle => None,
            FormatOperandProgress::Evaluating { parameter, .. } => Some(parameter),
        }
    }

    pub fn values(&self) -> &[Option<V>] {
        &self.storage.staging.values
    }
}

impl<V> FormatStaging<V> {
    pub(in crate::awbc::fiber) fn validate_failure_message(
        reason: &str,
    ) -> Result<(), FiberStateError> {
        if !crate::entry::RuntimeSchemaLimits::engine_default().permits_string_bytes(reason.len()) {
            return Err(FiberStateError::Snapshot(
                crate::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "string_bytes",
                }
                .into(),
            ));
        }
        Ok(())
    }

    fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
        active_failure: bool,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        resources
            .work()
            .collection(self.values.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if !self.format_context.has_current_data() || self.site.function != frame.function {
            return Err(FiberStateError::InvalidFrame);
        }
        let count = super::super::format_operand_count_at_site(frame.program, self.site)?;
        if self.values.len() != count || self.next_operand > count {
            return Err(FiberStateError::InvalidFrame);
        }
        resources
            .metadata_string(self.format_context.active_locale().as_str())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if let Some(failure) = &self.first_recoverable {
            resources
                .metadata_string(failure)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        let mut had_failure = active_failure;
        for (ordinal, value) in self.values.iter().enumerate() {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if ordinal >= self.next_operand {
                if value.is_some() {
                    return Err(FiberStateError::InvalidFrame);
                }
            } else if let Some(value) = value {
                let ty =
                    super::super::format_operand_type_at_site(frame.program, self.site, ordinal)?;
                crate::program_types::RuntimeProgramValueType::Awbc {
                    program: frame.program,
                    ty,
                }
                .validate_view(view(value), frame.binding, resources)
                .map_err(|error| {
                    FiberStateError::Snapshot(
                        crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(
                            error.within_value_path(&format!("format.values[{ordinal}]")),
                        )),
                    )
                })?;
            } else {
                had_failure = true;
            }
        }
        if had_failure != self.first_recoverable.is_some() {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(())
    }

    fn try_map<T>(
        self,
        mut map: impl FnMut(V) -> AwbcSaveResult<T>,
    ) -> AwbcSaveResult<FormatStaging<T>> {
        Ok(FormatStaging {
            site: self.site,
            format_context: self.format_context,
            next_operand: self.next_operand,
            values: self
                .values
                .into_iter()
                .map(|value| value.map(&mut map).transpose())
                .collect::<Result<_, _>>()?,
            first_recoverable: self.first_recoverable,
        })
    }
}

impl<V> FiberFormatState<V> {
    pub(in crate::awbc::fiber) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
    ) -> Result<(), FiberStateError> {
        self.storage
            .validate_inputs(frame, resources, view, false)?;
        let staging = &self.storage;
        if let Some((method, result, result_type)) =
            super::super::format_project_method_at_site(frame.program, staging.site)?
        {
            if method.receiver != crate::awbc::schema::AwbcTraitReceiverMode::Owned
                || method.receiver_state_slot.is_some()
                || frame
                    .layout
                    .slots
                    .get(result.index())
                    .is_none_or(|slot| slot.ty != result_type)
                || ((staging.next_operand < staging.values.len()
                    || staging.first_recoverable.is_some())
                    && frame
                        .registers
                        .get(result.index())
                        .is_some_and(|storage| !storage.is_vacant()))
            {
                return Err(FiberStateError::InvalidFrame);
            }
        } else if matches!(
            super::super::instruction_at_site(frame.program, staging.site)?,
            crate::awbc::schema::AwbcInstruction::FormatContent {
                project_option: true,
                ..
            }
        ) {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(())
    }
}

impl<V> FiberFormatAttemptState<V> {
    pub(in crate::awbc::fiber) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let Some((attempt, manifest)) =
            super::format_attempt_operands_at_site(frame.program, self.storage.staging.site)?
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if attempt != self.storage.attempt || manifest.is_empty() {
            return Err(FiberStateError::InvalidFrame);
        }
        let active_failure = match &self.storage.progress {
            FormatOperandProgress::Idle => false,
            FormatOperandProgress::Evaluating {
                parameter,
                scope_depth,
                failure,
            } => {
                resources
                    .work()
                    .charge(0)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                if manifest
                    .get(self.storage.staging.next_operand)
                    .is_none_or(|operand| operand.parameter != *parameter)
                    || *scope_depth > frame.scope_count
                {
                    return Err(FiberStateError::InvalidFrame);
                }
                let _ = super::format_attempt_completion_cursor(
                    frame.program,
                    frame.function,
                    attempt,
                    *parameter,
                    resources.work(),
                )?;
                if let Some(failure) = failure {
                    resources
                        .metadata_string(failure)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                }
                failure.is_some()
            }
        };
        self.storage
            .staging
            .validate_inputs(frame, resources, view, active_failure)
    }
}

impl FormatStaging<RuntimeValue> {
    pub(in crate::awbc::fiber) fn new(
        site: FiberCursor,
        format_context: &RuntimeFormatContext,
        operand_count: usize,
    ) -> Result<Self, FiberStateError> {
        if !format_context.has_current_data() {
            return Err(FiberStateError::InvalidFrame);
        }
        let mut resources =
            ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default());
        resources
            .work()
            .collection(operand_count)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        resources
            .metadata_string(format_context.active_locale().as_str())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        for _ in 0..operand_count {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        Ok(Self {
            site,
            format_context: format_context.clone(),
            next_operand: 0,
            values: vec![None; operand_count],
            first_recoverable: None,
        })
    }

    fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FormatStaging<AwbcRuntimeValueSnapshot>> {
        Ok(FormatStaging {
            site: self.site,
            format_context: self.format_context.clone(),
            next_operand: self.next_operand,
            values: self
                .values
                .iter()
                .map(|value| {
                    value
                        .as_ref()
                        .map(|value| {
                            AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
                        })
                        .transpose()
                })
                .collect::<Result<_, _>>()?,
            first_recoverable: self.first_recoverable.clone(),
        })
    }
}

impl FiberFormatState {
    pub(in crate::awbc::fiber) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberFormatState<AwbcRuntimeValueSnapshot>> {
        Ok(FiberFormatState {
            storage: self.storage.snapshot_admitted(owner)?,
        })
    }
}

impl FiberFormatState<AwbcRuntimeValueSnapshot> {
    pub(in crate::awbc::fiber) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberFormatState> {
        let owner = owner.into();
        Ok(FiberFormatState {
            storage: self
                .storage
                .try_map(|value| value.into_admitted_runtime_value(owner))?,
        })
    }
}

impl FiberFormatAttemptState {
    pub(in crate::awbc::fiber) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberFormatAttemptState<AwbcRuntimeValueSnapshot>> {
        Ok(FiberFormatAttemptState {
            storage: FormatAttempt {
                attempt: self.storage.attempt,
                staging: self.storage.staging.snapshot_admitted(owner)?,
                progress: self.storage.progress.clone(),
            },
        })
    }
}

impl FiberFormatAttemptState<AwbcRuntimeValueSnapshot> {
    pub(in crate::awbc::fiber) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberFormatAttemptState> {
        let owner = owner.into();
        Ok(FiberFormatAttemptState {
            storage: FormatAttempt {
                attempt: self.storage.attempt,
                staging: self
                    .storage
                    .staging
                    .try_map(|value| value.into_admitted_runtime_value(owner))?,
                progress: self.storage.progress,
            },
        })
    }
}

impl Serialize for FiberFormatState<AwbcRuntimeValueSnapshot> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.storage.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FiberFormatState<AwbcRuntimeValueSnapshot> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            storage: FormatStaging::deserialize(deserializer)?,
        })
    }
}

impl Serialize for FiberFormatAttemptState<AwbcRuntimeValueSnapshot> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.storage.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FiberFormatAttemptState<AwbcRuntimeValueSnapshot> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            storage: FormatAttempt::deserialize(deserializer)?,
        })
    }
}
