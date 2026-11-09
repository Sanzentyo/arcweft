//! Flow transitions retain their target and complete source-ordered operands.

use serde::{Deserialize, Serialize};

use crate::value::{RuntimeFlowParameterBinding, RuntimeInputOrder, RuntimeInputOrderError};

use super::FlowRuntimeId;

/// The target is evaluated before the argument operands. A dynamic reference
/// selects an original Flow identity, never a reconstructed diagnostic label.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeFlowTransferTarget<E> {
    Static(FlowRuntimeId),
    Dynamic(E),
}

impl<E> RuntimeFlowTransferTarget<E> {
    pub(crate) fn borrowed(&self) -> RuntimeFlowTransferTarget<&E> {
        match self {
            Self::Static(flow) => RuntimeFlowTransferTarget::Static(flow.clone()),
            Self::Dynamic(selector) => RuntimeFlowTransferTarget::Dynamic(selector),
        }
    }

    pub(crate) fn identity<'a>(
        &'a self,
        view: impl FnOnce(&'a E) -> crate::value::RuntimeValueView<'a>,
    ) -> Result<&'a FlowRuntimeId, super::RuntimeFlowTargetError> {
        match self {
            Self::Static(flow) => Ok(flow),
            Self::Dynamic(selector) => FlowRuntimeId::from_reference_value(view(selector)),
        }
    }
}

/// Unsealed transfer data shared by source seeds and admitted Plan operations.
/// The Plan owns full target ABI admission; this carrier owns the permutation
/// between authored evaluation order and formal parameter storage order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFlowTransfer<E> {
    pub target: RuntimeFlowTransferTarget<E>,
    pub arguments: Box<[RuntimeFlowParameterBinding<E>]>,
}

impl<E> RuntimeFlowTransfer<E> {
    pub(crate) fn borrowed(&self) -> RuntimeFlowTransfer<&E> {
        RuntimeFlowTransfer {
            target: self.target.borrowed(),
            arguments: self
                .arguments
                .iter()
                .map(|argument| RuntimeFlowParameterBinding {
                    parameter: argument.parameter,
                    value: &argument.value,
                })
                .collect(),
        }
    }
    pub(crate) fn expressions(&self) -> impl Iterator<Item = &E> {
        let target = match &self.target {
            RuntimeFlowTransferTarget::Static(_) => None,
            RuntimeFlowTransferTarget::Dynamic(target) => Some(target),
        };
        target
            .into_iter()
            .chain(self.arguments.iter().map(|argument| &argument.value))
    }
    /// Positional/adapter-free syntax supplies no operands. Parameterized
    /// transfers must supply their complete coordinate-addressed inventory.
    pub fn static_without_arguments(target: FlowRuntimeId) -> Self {
        Self {
            target: RuntimeFlowTransferTarget::Static(target),
            arguments: Box::new([]),
        }
    }

    pub fn dynamic_without_arguments(target: E) -> Self {
        Self {
            target: RuntimeFlowTransferTarget::Dynamic(target),
            arguments: Box::new([]),
        }
    }

    /// The caller admits the operand count under its shared construction or
    /// execution budget before allocating this complete permutation.
    pub fn input_order(&self) -> Result<RuntimeInputOrder, RuntimeInputOrderError> {
        RuntimeInputOrder::try_new(
            self.arguments
                .iter()
                .map(|argument| argument.parameter.position())
                .collect::<Box<[_]>>(),
        )
    }

    /// Maps target first, then operands in authored order. Parameter
    /// coordinates remain attached to their own values across every lowering.
    pub(crate) fn try_map<T, Error>(
        self,
        mut map: impl FnMut(E) -> Result<T, Error>,
    ) -> Result<RuntimeFlowTransfer<T>, Error> {
        let target = match self.target {
            RuntimeFlowTransferTarget::Static(target) => RuntimeFlowTransferTarget::Static(target),
            RuntimeFlowTransferTarget::Dynamic(target) => {
                RuntimeFlowTransferTarget::Dynamic(map(target)?)
            }
        };
        let arguments = self
            .arguments
            .into_iter()
            .map(|argument| {
                Ok(RuntimeFlowParameterBinding {
                    parameter: argument.parameter,
                    value: map(argument.value)?,
                })
            })
            .collect::<Result<Box<[_]>, Error>>()?;
        Ok(RuntimeFlowTransfer { target, arguments })
    }
}

impl RuntimeFlowTransfer<crate::value::RuntimeExpr> {
    /// Argument evaluation is owned by caller operations. The transition
    /// consumes complete, distinct slots only after joint admission succeeds.
    pub(crate) fn input_local(
        argument: &RuntimeFlowParameterBinding<crate::value::RuntimeExpr>,
    ) -> Result<crate::runtime_id::RuntimeLocalDeclarationId, super::RuntimeFlowInvocationError>
    {
        if let crate::value::RuntimeExprKind::Local(read) = argument.value.kind()
            && read.mode() == crate::value::RuntimeLocalReadMode::Move
            && read.fields().is_empty()
        {
            return Ok(read.local());
        }
        Err(super::RuntimeFlowInvocationError::TransferInputSource {
            parameter: argument.parameter,
        })
    }

    pub(crate) fn validate_input_sources(&self) -> Result<(), super::RuntimeFlowInvocationError> {
        let mut unique = std::collections::BTreeSet::new();
        for argument in &self.arguments {
            let local = Self::input_local(argument)?;
            if !unique.insert(local) {
                return Err(super::RuntimeFlowInvocationError::DuplicateTransferInput { local });
            }
        }
        Ok(())
    }
}
