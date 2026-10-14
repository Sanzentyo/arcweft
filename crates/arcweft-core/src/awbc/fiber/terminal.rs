//! One terminal custody grammar, admitted before live/inert projection.

use super::input::FrameValueContext;
use super::{
    AwbcFiberRoot, AwbcFunctionKind, AwbcProgram, AwbcRuntimeTypeShape, AwbcSaveResult,
    FiberStateError, FiberStatus, FiberTrap, RuntimeProgramOwner, RuntimeValue,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeProgramValueType;
use crate::value::AwbcRuntimeValueSnapshot;
use crate::value::RuntimeValueView;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// The stable result of an owning fiber. Only inert values have a save codec.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberTerminalValue;
/// let _: FiberTerminalValue = serde_json::from_str("{} ").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberTerminalValue;
/// fn encode_live(result: &FiberTerminalValue) { let _ = serde_json::to_vec(result); }
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: super::InertRuntimeValueStorage",
        deserialize = "V: super::InertRuntimeValueStorage"
    )
)]
pub enum FiberTerminalValue<V = RuntimeValue> {
    Returned(Option<V>),
    /// The return payload has transferred to the caller.
    ReturnedTransferred,
    DialogueResultSelected(V),
    /// The selected payload has transferred to the line owner.
    DialogueResultTransferred,
    Cancelled,
    Trapped(FiberTrap),
}

impl FiberStatus {
    pub(super) fn validate_retained_shape<V>(
        self,
        suspended: bool,
        terminal: Option<&FiberTerminalValue<V>>,
        summary: Option<&str>,
    ) -> Result<(), FiberStateError> {
        let valid = match self {
            Self::Running => !suspended && terminal.is_none() && summary.is_none(),
            Self::Suspended => suspended && terminal.is_none() && summary.is_none(),
            Self::Returned => {
                !suspended
                    && match terminal {
                        Some(
                            FiberTerminalValue::Returned(Some(_))
                            | FiberTerminalValue::ReturnedTransferred,
                        ) => summary.is_some(),
                        Some(
                            FiberTerminalValue::Returned(None)
                            | FiberTerminalValue::DialogueResultSelected(_)
                            | FiberTerminalValue::DialogueResultTransferred,
                        ) => summary.is_none(),
                        _ => false,
                    }
            }
            Self::Cancelled => {
                !suspended
                    && summary.is_none()
                    && matches!(terminal, Some(FiberTerminalValue::Cancelled))
            }
            Self::Trapped => {
                !suspended
                    && summary.is_none()
                    && matches!(terminal, Some(FiberTerminalValue::Trapped(_)))
            }
        };
        if valid {
            Ok(())
        } else {
            Err(FiberStateError::InvalidStatus {
                actual: self,
                expected: self,
            })
        }
    }
}

impl<V> FiberTerminalValue<V> {
    #[allow(
        clippy::too_many_lines,
        reason = "one closed terminal custody algebra admits original result owners and all metadata before projecting any retained value"
    )]
    pub(super) fn validate_inputs(
        &self,
        program: &AwbcProgram,
        root: AwbcFiberRoot,
        frame: Option<&FrameValueContext<'_, V>>,
        frame_count: usize,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let expected_root = root.validate(program)?;
        if let Some(expected) = expected_root
            && frame.is_some_and(|frame| frame.function != expected)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let expected = match self {
            Self::Returned(_) | Self::ReturnedTransferred => {
                let value = match self {
                    Self::Returned(value) => value.as_ref(),
                    _ => None,
                };
                if root == AwbcFiberRoot::Empty
                    && frame_count == 0
                    && matches!(self, Self::Returned(None))
                {
                    return Ok(());
                }
                if frame_count > 1 {
                    return Err(FiberStateError::InvalidFrame);
                }
                // A detached Program result is owned by its exact pure-program
                // binding after execution frames leave custody. Entry/function
                // results are owned by their retained root frame.
                let function_id = match root {
                    AwbcFiberRoot::Program(_) => {
                        expected_root.ok_or(FiberStateError::InvalidFrame)?
                    }
                    _ => frame.ok_or(FiberStateError::InvalidFrame)?.function,
                };
                resources
                    .work()
                    .charge(0)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                let function = program
                    .functions
                    .get(function_id.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                resources
                    .work()
                    .charge(0)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                let signature = program
                    .signatures
                    .get(function.signature.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                match (self, value, signature.result) {
                    (Self::ReturnedTransferred, None, Some(expected)) => {
                        resources
                            .work()
                            .charge(0)
                            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                        program
                            .runtime_types
                            .get(expected.index())
                            .ok_or(FiberStateError::InvalidFrame)?;
                        None
                    }
                    (Self::Returned(_), Some(value), Some(expected)) => {
                        Some((value, expected, frame.and_then(|frame| frame.binding)))
                    }
                    (Self::Returned(_), None, None) => None,
                    // A Flow completion has Unit ABI and no returned payload.
                    (Self::Returned(_), None, Some(expected)) => {
                        resources
                            .work()
                            .charge(0)
                            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                        let row = program
                            .runtime_types
                            .get(expected.index())
                            .ok_or(FiberStateError::InvalidFrame)?;
                        if function.kind != AwbcFunctionKind::Flow
                            || !matches!(row.shape(), AwbcRuntimeTypeShape::Unit)
                        {
                            return Err(FiberStateError::ReturnValueMismatch);
                        }
                        None
                    }
                    _ => return Err(FiberStateError::ReturnValueMismatch),
                }
            }
            Self::DialogueResultSelected(_) | Self::DialogueResultTransferred => {
                if frame_count != 1 {
                    return Err(FiberStateError::InvalidFrame);
                }
                let frame = frame.ok_or(FiberStateError::InvalidFrame)?;
                resources
                    .work()
                    .charge(0)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                let function = program
                    .functions
                    .get(frame.function.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                if !matches!(
                    function.kind,
                    AwbcFunctionKind::LineTask | AwbcFunctionKind::LineCancellationHandler
                ) {
                    return Err(FiberStateError::InvalidFrame);
                }
                // Charge the complete original owner lookup inventory before
                // traversing it; no value descendants are constructed here.
                resources
                    .work()
                    .collection(program.line_task_groups.len())
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                for group in &program.line_task_groups {
                    resources
                        .work()
                        .charge(0)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    resources
                        .work()
                        .collection(group.cancel_handlers.len())
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    for _ in &group.cancel_handlers {
                        resources
                            .work()
                            .charge(0)
                            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    }
                    resources
                        .work()
                        .collection(group.nodes.len as usize)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    for _ in 0..group.nodes.len {
                        resources
                            .work()
                            .charge(0)
                            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    }
                }
                let group = program
                    .line_task_group_for_function(
                        frame.function,
                        "terminal.dialogue_result_selected",
                    )
                    .map_err(|_| FiberStateError::InvalidFrame)?;
                if let Self::DialogueResultSelected(value) = self {
                    Some((value, group.result_type, frame.binding))
                } else {
                    resources
                        .work()
                        .charge(0)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    program
                        .runtime_types
                        .get(group.result_type.index())
                        .ok_or(FiberStateError::InvalidFrame)?;
                    None
                }
            }
            Self::Cancelled => None,
            Self::Trapped(trap) => {
                if let Some(message) = &trap.message {
                    resources
                        .metadata_string(message)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                }
                if let Some(source) = trap.source_map {
                    resources
                        .work()
                        .charge(0)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                    program
                        .source_map
                        .get(source.index())
                        .ok_or(FiberStateError::InvalidFrame)?;
                }
                None
            }
        };
        if let Some((value, expected, binding)) = expected {
            RuntimeProgramValueType::Awbc {
                program,
                ty: expected,
            }
            .validate_view(view(value), binding, resources)
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error),
                ))
            })?;
        }
        Ok(())
    }
}

impl<V> FiberTerminalValue<V> {
    pub(super) fn validate_summary(
        &self,
        original: Option<crate::program_types::RuntimeProgramTypes<'_>>,
        summary: Option<&str>,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
    ) -> Result<(), FiberStateError> {
        if let Self::Returned(Some(value)) = self
            && summary
                != Some(
                    view(value)
                        .label(original)
                        .map_err(|error| FiberStateError::Snapshot(error.into()))?
                        .as_ref(),
                )
        {
            return Err(FiberStateError::InvalidStatus {
                actual: FiberStatus::Returned,
                expected: FiberStatus::Returned,
            });
        }
        Ok(())
    }
}

impl FiberTerminalValue {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberTerminalValue<AwbcRuntimeValueSnapshot>> {
        Ok(match self {
            Self::Returned(value) => FiberTerminalValue::Returned(
                value
                    .as_ref()
                    .map(|value| {
                        AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
                    })
                    .transpose()?,
            ),
            Self::DialogueResultSelected(value) => FiberTerminalValue::DialogueResultSelected(
                AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)?,
            ),
            Self::Cancelled => FiberTerminalValue::Cancelled,
            Self::ReturnedTransferred => FiberTerminalValue::ReturnedTransferred,
            Self::DialogueResultTransferred => FiberTerminalValue::DialogueResultTransferred,
            Self::Trapped(trap) => FiberTerminalValue::Trapped(trap.clone()),
        })
    }
}

impl FiberTerminalValue<AwbcRuntimeValueSnapshot> {
    pub(super) fn into_live_admitted(
        self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberTerminalValue> {
        Ok(match self {
            Self::Returned(value) => FiberTerminalValue::Returned(
                value
                    .map(|value| value.into_admitted_runtime_value(owner))
                    .transpose()?,
            ),
            Self::DialogueResultSelected(value) => FiberTerminalValue::DialogueResultSelected(
                value.into_admitted_runtime_value(owner)?,
            ),
            Self::Cancelled => FiberTerminalValue::Cancelled,
            Self::ReturnedTransferred => FiberTerminalValue::ReturnedTransferred,
            Self::DialogueResultTransferred => FiberTerminalValue::DialogueResultTransferred,
            Self::Trapped(trap) => FiberTerminalValue::Trapped(trap),
        })
    }
}
