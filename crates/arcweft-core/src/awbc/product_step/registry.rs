//! Registry reach through the executor's actual activation and cleanup owners.

use super::dialogue::{ProductDialogueReceiver, ProductDialogueStore};
use super::{
    AwbcProductStepExecutor, AwbcTypeId, ProductChildFiber, ProductChildFiberOwner,
    ProductStepError,
};
use crate::awbc::fiber::{AwbcFiberLexicalScopeId, FiberState, FiberStateError};
use crate::awbc::schema::AwbcProgram;
use crate::effect::{RuntimePreparedRegistryMutation, RuntimeRegistryOwner, RuntimeScopeRegistry};
use crate::line_task::RuntimeDialogueActivationState;
use crate::runtime_id::DialogueActivationId;

/// The real parent held while this executable callback is running. It is
/// borrowed from the child's owner, never persisted as another ownership graph.
#[derive(Clone, Copy)]
pub(super) enum ProductRegistryOrigin {
    Dialogue(DialogueActivationId),
    Deferred(AwbcFiberLexicalScopeId),
}

struct ReachedProductFlow<'a> {
    owner: RuntimeRegistryOwner,
    rows: &'a RuntimeScopeRegistry,
    activation_frame: Option<DialogueActivationId>,
}

impl ProductRegistryOrigin {
    pub(super) fn for_child(owner: &ProductChildFiberOwner) -> Option<Self> {
        match owner {
            ProductChildFiberOwner::FlowDeferred { owner, .. } => Some(Self::Deferred(*owner)),
            _ => owner.line_activation().map(Self::Dialogue),
        }
    }

    fn owner_fiber(self) -> u64 {
        match self {
            Self::Dialogue(activation) => activation.owner_fiber().get(),
            Self::Deferred(owner) => owner.fiber.get().get(),
        }
    }

    fn invalid(self) -> FiberStateError {
        match self {
            Self::Dialogue(activation) => FiberStateError::InvalidDialogueReceiver { activation },
            Self::Deferred(_) => FiberStateError::InvalidFrame,
        }
    }

    fn reached_flow<'a>(
        self,
        program: &AwbcProgram,
        dialogues: &'a ProductDialogueStore,
        main: &'a FiberState,
        children: impl Iterator<Item = &'a ProductChildFiber> + Clone,
    ) -> Result<ReachedProductFlow<'a>, FiberStateError> {
        let mut origin = self;
        loop {
            let owner = origin.owner_fiber();
            let active = dialogues.active_frames().find(|frame| {
                frame
                    .activation_fiber()
                    .is_some_and(|fiber| fiber.instance.get().get() == owner)
            });
            let child = children
                .clone()
                .find(|child| child.fiber.instance.get().get() == owner);
            let fiber = if let Some(frame) = active {
                frame.activation_fiber()
            } else if main.instance.get().get() == owner {
                Some(main)
            } else {
                child.map(|child| &child.fiber)
            }
            .ok_or_else(|| self.invalid())?;
            if let Self::Deferred(scope) = origin {
                let deferred = scope.active_scope(fiber).ok_or_else(|| self.invalid())?;
                if deferred.defer_inflight.is_none() {
                    return Err(self.invalid());
                }
            }
            if let Some(index) = fiber.reached_flow_frame_index(program) {
                let frame = &fiber.frames[index];
                return Ok(ReachedProductFlow {
                    owner: RuntimeRegistryOwner::AwbcFlow {
                        fiber: fiber.instance,
                        frame: frame.instance,
                    },
                    rows: &frame.flow_registry,
                    activation_frame: active.map(|frame| frame.activation),
                });
            }
            let parent = active
                .map(|frame| Self::Dialogue(frame.activation))
                .or_else(|| child.and_then(|child| Self::for_child(&child.owner)))
                .ok_or_else(|| self.invalid())?;
            if parent.owner_fiber() >= owner {
                return Err(self.invalid());
            }
            origin = parent;
        }
    }

    pub(super) fn reached_registry<'a>(
        self,
        program: &AwbcProgram,
        scope: &arcweft_id::LifetimeScopeKind,
        line: Option<&'a RuntimeDialogueActivationState<AwbcTypeId>>,
        dialogues: &'a ProductDialogueStore,
        main: &'a FiberState,
        children: impl Iterator<Item = &'a ProductChildFiber> + Clone,
    ) -> Result<(RuntimeRegistryOwner, &'a RuntimeScopeRegistry), FiberStateError> {
        match (scope, self) {
            (arcweft_id::LifetimeScopeKind::Line, Self::Dialogue(activation)) => Ok((
                RuntimeRegistryOwner::Dialogue(activation),
                line.ok_or_else(|| self.invalid())?
                    .line_registry()
                    .map_err(|_| self.invalid())?,
            )),
            (arcweft_id::LifetimeScopeKind::Flow, _) => {
                let reached = self.reached_flow(program, dialogues, main, children)?;
                Ok((reached.owner, reached.rows))
            }
            _ => Err(self.invalid()),
        }
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn reserve_reached_registry_mutation(
        &mut self,
        origin: ProductRegistryOrigin,
        owner: RuntimeRegistryOwner,
        line: Option<&mut RuntimeDialogueActivationState<AwbcTypeId>>,
        write: &RuntimePreparedRegistryMutation,
    ) -> Result<(), ProductStepError> {
        self.edit_reached_registry(origin, owner, line, false, |rows| {
            write.reserve(rows).map_err(FiberStateError::Snapshot)
        })
    }

    pub(super) fn commit_reached_registry_mutation(
        &mut self,
        origin: ProductRegistryOrigin,
        owner: RuntimeRegistryOwner,
        line: Option<&mut RuntimeDialogueActivationState<AwbcTypeId>>,
        write: RuntimePreparedRegistryMutation,
    ) {
        self.edit_reached_registry(origin, owner, line, true, |rows| {
            write.commit(rows).map_err(FiberStateError::Snapshot)
        })
        .expect("prepared mutation retains its reached registry");
    }

    fn edit_reached_registry(
        &mut self,
        origin: ProductRegistryOrigin,
        owner: RuntimeRegistryOwner,
        line: Option<&mut RuntimeDialogueActivationState<AwbcTypeId>>,
        publish: bool,
        edit: impl FnOnce(&mut RuntimeScopeRegistry) -> Result<(), FiberStateError>,
    ) -> Result<(), ProductStepError> {
        if let ProductRegistryOrigin::Dialogue(activation) = origin
            && owner == RuntimeRegistryOwner::Dialogue(activation)
        {
            return Ok(edit(
                line.ok_or_else(|| origin.invalid())?.line_registry_mut()?,
            )?);
        }
        let RuntimeRegistryOwner::AwbcFlow { fiber, frame } = owner else {
            return Err(FiberStateError::InvalidFrame.into());
        };
        let reached = origin.reached_flow(
            &self.program,
            &self.dialogues,
            &self.fiber,
            self.child_fibers.iter(),
        )?;
        if reached.owner != owner {
            return Err(FiberStateError::InvalidFrame.into());
        }
        let edit_fiber = |parent: &mut FiberState| {
            let index = parent
                .reached_flow_frame_index(&self.program)
                .ok_or_else(|| origin.invalid())?;
            let reached = &mut parent.frames[index];
            if reached.instance != frame {
                return Err(FiberStateError::InvalidFrame);
            }
            edit(&mut reached.flow_registry)
        };
        if let Some(id) = reached.activation_frame {
            self.dialogues
                .preflight_receiver_edit(ProductDialogueReceiver::Activation(id))?;
            let mut parent = self.dialogues.begin_transaction(&id)?;
            let result = parent
                .frame_mut()
                .receiving_fiber_mut()
                .ok_or_else(|| origin.invalid())
                .and_then(edit_fiber);
            if publish && result.is_ok() {
                self.dialogues.commit_receiver(parent);
            } else {
                self.dialogues.restore_transaction(parent)?;
            }
            return Ok(result?);
        }
        let parent = std::iter::once(&mut self.fiber)
            .chain(self.child_fibers.iter_mut().map(|child| &mut child.fiber))
            .find(|parent| parent.instance == fiber)
            .ok_or_else(|| origin.invalid())?;
        Ok(edit_fiber(parent)?)
    }
}
