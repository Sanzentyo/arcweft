//! Borrowed receiver topology shared by live and inert Product admission.

use super::super::{AwbcProductActiveDialogue, AwbcProductDialoguePhase, AwbcProductStepExecutor};
use super::{ProductDialogueDispatch, ProductDialogueReceiver};
use crate::awbc::fiber::{FiberState, FiberStateError};
use crate::awbc::schema::{AwbcContentUnitId, AwbcDialogueResultTarget, AwbcProgram, AwbcTypeId};
use crate::line_task::LineRuntimeError;
use crate::runtime_id::{DialogueActivationId, RuntimePersistentFiberId};
use std::borrow::Borrow;
use std::collections::BTreeMap;

struct ReceiverEdge<'a> {
    activation: DialogueActivationId,
    content: AwbcContentUnitId,
    target_type: AwbcTypeId,
    result: &'a AwbcDialogueResultTarget,
    closing: bool,
    activation_fiber: Option<RuntimePersistentFiberId>,
}

struct ReceiverRow<'a, F> {
    fiber: Option<&'a F>,
    receiver: Option<ProductDialogueReceiver>,
    child: Option<ReceiverEdge<'a>>,
}

/// Admission reads actual frame references. An activation ID supplies the
/// parent coordinate, and the parent's original suspended site authenticates
/// that edge. Frames may be visited in any registry order.
pub(in crate::awbc::product_step) struct ProductDialogueReceivers<'a, F> {
    rows: BTreeMap<RuntimePersistentFiberId, ReceiverRow<'a, F>>,
}

impl<F> Default for ProductDialogueReceivers<'_, F> {
    fn default() -> Self {
        Self {
            rows: BTreeMap::new(),
        }
    }
}

impl<'a, V, O, N, B, R, G> ProductDialogueReceivers<'a, FiberState<V, O, N, B, R, G>> {
    pub(in crate::awbc::product_step) fn validate_result_discards(
        &self,
        program: &AwbcProgram,
        activation: &DialogueActivationId,
        value: crate::value::RuntimeValueView<'_>,
        retired: &std::collections::BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    ) -> Result<(), LineRuntimeError>
    where
        B: Borrow<crate::program_types::RuntimeFunctionEffectInstantiation>,
    {
        let invalid = || LineRuntimeError::InvalidRestoredResultState;
        let row = self
            .rows
            .get(&activation.owner_fiber())
            .ok_or_else(invalid)?;
        let edge = row
            .child
            .as_ref()
            .filter(|edge| &edge.activation == activation)
            .ok_or_else(invalid)?;
        let parent = row
            .fiber
            .and_then(|fiber| fiber.frames.last())
            .ok_or_else(invalid)?;
        crate::awbc::vm::validate_result_pattern_discards(
            program,
            parent.type_instantiation.as_ref().map(Borrow::borrow),
            edge.result.pattern,
            value,
            retired,
        )
        .map_err(|_| invalid())
    }

    fn row(
        &mut self,
        owner: RuntimePersistentFiberId,
    ) -> &mut ReceiverRow<'a, FiberState<V, O, N, B, R, G>> {
        self.rows.entry(owner).or_insert(ReceiverRow {
            fiber: None,
            receiver: None,
            child: None,
        })
    }

    pub(in crate::awbc::product_step) fn register_fiber(
        &mut self,
        fiber: &'a FiberState<V, O, N, B, R, G>,
        receiver: ProductDialogueReceiver,
    ) -> Result<(), FiberStateError> {
        if matches!(receiver, ProductDialogueReceiver::Fiber(instance) | ProductDialogueReceiver::LineChild { instance, .. } if instance != fiber.instance)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let owner = RuntimePersistentFiberId::from_allocated(fiber.instance.get().get());
        let row = self.row(owner);
        if row.fiber.is_some() {
            return Err(FiberStateError::InvalidFrame);
        }
        row.fiber = Some(fiber);
        row.receiver = Some(receiver);
        Ok(())
    }

    pub(in crate::awbc::product_step) fn register_dialogue<V2, O2, E, L, P, H>(
        &mut self,
        expected: DialogueActivationId,
        frame: &'a AwbcProductActiveDialogue<
            V2,
            O2,
            E,
            AwbcProductDialoguePhase<FiberState<V, O, N, B, R, G>, L, P>,
            H,
        >,
    ) -> Result<(), FiberStateError> {
        let invalid = || FiberStateError::InvalidDialogueReceiver {
            activation: expected,
        };
        if frame.activation != expected
            || AwbcProductStepExecutor::runtime_dialogue_content_id(frame.content)
                .map_err(|_| invalid())?
                != expected.content()
        {
            return Err(invalid());
        }
        if let Some(fiber) = frame.activation_fiber()
            && fiber.instance.get().get() <= expected.owner_fiber().get()
        {
            // An activation allocates its fiber after its receiver. This also
            // rules out cycles when an activation fiber receives another line.
            return Err(invalid());
        }
        let row = self.row(expected.owner_fiber());
        if row.child.is_some() {
            return Err(invalid());
        }
        row.child = Some(ReceiverEdge {
            activation: expected,
            content: frame.content,
            target_type: frame.target_type,
            result: &frame.result,
            closing: matches!(frame.phase, AwbcProductDialoguePhase::Closing(_)),
            activation_fiber: frame
                .activation_fiber()
                .map(|fiber| RuntimePersistentFiberId::from_allocated(fiber.instance.get().get())),
        });
        Ok(())
    }

    pub(in crate::awbc::product_step) fn validate(
        &self,
        program: &AwbcProgram,
    ) -> Result<(), FiberStateError> {
        let mut root = None;
        let mut depth = 0;
        for (owner, row) in &self.rows {
            let receiver = row.receiver.ok_or_else(|| {
                row.child
                    .as_ref()
                    .map_or(FiberStateError::InvalidFrame, |edge| {
                        FiberStateError::InvalidDialogueReceiver {
                            activation: edge.activation,
                        }
                    })
            })?;
            if let ProductDialogueReceiver::Activation(parent)
            | ProductDialogueReceiver::LineChild { parent, .. } = receiver
            {
                let parent_edge = self
                    .rows
                    .get(&parent.owner_fiber())
                    .and_then(|row| row.child.as_ref())
                    .filter(|edge| edge.activation == parent)
                    .ok_or(FiberStateError::InvalidDialogueReceiver { activation: parent })?;
                let invalid = FiberStateError::InvalidDialogueReceiver { activation: parent };
                match receiver {
                    ProductDialogueReceiver::Activation(_)
                        if parent_edge.activation_fiber != Some(*owner) =>
                    {
                        return Err(invalid);
                    }
                    ProductDialogueReceiver::LineChild { .. }
                        if *owner <= parent.owner_fiber()
                            || parent_edge
                                .activation_fiber
                                .is_some_and(|fiber| *owner <= fiber) =>
                    {
                        return Err(invalid);
                    }
                    _ => {}
                }
            }
            if let Some(edge) = &row.child {
                let invalid = FiberStateError::InvalidDialogueReceiver {
                    activation: edge.activation,
                };
                depth += 1;
                if !crate::entry::RuntimeSchemaLimits::engine_default().permits_depth(depth)
                    || (matches!(receiver, ProductDialogueReceiver::Fiber(_))
                        && root.replace(edge.activation).is_some())
                {
                    return Err(invalid);
                }
                let parent = row.fiber.ok_or(FiberStateError::InvalidDialogueReceiver {
                    activation: edge.activation,
                })?;
                parent.validate_dialogue_receiver(
                    program,
                    edge.activation,
                    edge.content,
                    edge.target_type,
                    edge.result,
                    edge.closing,
                )?;
            }
            if let Some(fiber) = row.fiber {
                fiber.validate_dialogue_receiver_presence(
                    row.child.as_ref().map(|edge| edge.activation),
                )?;
            }
        }
        Ok(())
    }

    pub(in crate::awbc::product_step) fn receiver(
        &self,
        activation: DialogueActivationId,
    ) -> Result<ProductDialogueReceiver, FiberStateError> {
        self.rows.get(&activation.owner_fiber())
            .filter(|row| row.fiber.is_some_and(|fiber| matches!(fiber.suspension.as_ref().map(|suspension| &suspension.reason), Some(crate::awbc::fiber::FiberSuspensionReason::DialogueActive { activation: child }) if *child == activation)))
            .and_then(|row| row.receiver)
            .ok_or(FiberStateError::InvalidDialogueReceiver { activation })
    }

    /// Follow actual owned receiver edges from the root toward waiting work.
    /// A line's child dialogue progresses before its caller's cleanup can join.
    pub(in crate::awbc::product_step) fn dispatch(
        &self,
    ) -> Result<Option<ProductDialogueDispatch>, FiberStateError> {
        for (owner, row) in &self.rows {
            if matches!(row.receiver, Some(ProductDialogueReceiver::Fiber(_)))
                && let Some(dispatch) = self.dispatch_row(*owner, 0)?
            {
                return Ok(Some(dispatch));
            }
        }
        Ok(None)
    }

    fn dispatch_row(
        &self,
        owner: RuntimePersistentFiberId,
        depth: usize,
    ) -> Result<Option<ProductDialogueDispatch>, FiberStateError> {
        if !crate::entry::RuntimeSchemaLimits::engine_default().permits_depth(depth) {
            return Err(FiberStateError::InvalidFrame);
        }
        let row = self.rows.get(&owner).ok_or(FiberStateError::InvalidFrame)?;
        let fiber = row.fiber.ok_or(FiberStateError::InvalidFrame)?;
        let Some(suspension) = &fiber.suspension else {
            return Ok(None);
        };
        match suspension.reason {
            crate::awbc::fiber::FiberSuspensionReason::Dialogue { .. } => Ok(Some(
                ProductDialogueDispatch::Start(row.receiver.ok_or(FiberStateError::InvalidFrame)?),
            )),
            crate::awbc::fiber::FiberSuspensionReason::DialogueActive { activation } => {
                let edge = row
                    .child
                    .as_ref()
                    .filter(|edge| edge.activation == activation)
                    .ok_or(FiberStateError::InvalidDialogueReceiver { activation })?;
                for (child_owner, child) in &self.rows {
                    if matches!(child.receiver, Some(ProductDialogueReceiver::LineChild { parent, .. }) if parent == activation)
                        && let Some(dispatch) = self.dispatch_row(*child_owner, depth + 1)?
                    {
                        return Ok(Some(dispatch));
                    }
                }
                if let Some(activation_fiber) = edge.activation_fiber
                    && let Some(dispatch) = self.dispatch_row(activation_fiber, depth + 1)?
                {
                    return Ok(Some(dispatch));
                }
                let resume = suspension
                    .declared_resume()
                    .ok_or(FiberStateError::InvalidDialogueReceiver { activation })?;
                Ok(Some(ProductDialogueDispatch::Resume { activation, resume }))
            }
            _ => Ok(None),
        }
    }
}
