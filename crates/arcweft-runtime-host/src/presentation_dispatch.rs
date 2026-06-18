use arcweft_id::PublicId;
use arcweft_presentation::input::{
    Action, ActionBatch, ActionTarget, HostEventBatch, InputEpoch, InteractionTarget,
};
use arcweft_presentation::interaction::InteractionState;
use arcweft_presentation::layer::LayerTree;
use arcweft_presentation::semantic::{SemanticActionError, SemanticRole, SemanticTree};
use thiserror::Error;

/// Host-side destination selected from routed presentation semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationActionDestination {
    Runtime,
    TextBox,
    Activity,
    UiEntity,
}

/// One routed presentation action with its host dispatch destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchedPresentationAction {
    destination: PresentationActionDestination,
    action: Action,
}

/// Ordered dispatch plan produced by the runtime host before handler execution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationActionDispatchPlan {
    actions: Vec<DispatchedPresentationAction>,
}

/// Output collected while running presentation action handlers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationActionHandlerOutput {
    actions: ActionBatch,
    host_events: HostEventBatch,
}

/// Rejection reason while partitioning presentation actions for host handlers.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PresentationActionDispatchError {
    #[error("semantic action was rejected: {0:?}")]
    Semantic(SemanticActionError),
    #[error("semantic target was not present: {0:?}")]
    UnknownTarget(InteractionTarget),
    #[error("activity action target did not resolve to an Activity semantic node: {0:?}")]
    ActivityTargetMismatch(InteractionTarget),
}

/// Handler-level failure reported while executing a dispatch plan.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PresentationActionHandlerError {
    #[error("presentation action `{action}` was unhandled by {destination:?}")]
    Unhandled {
        destination: PresentationActionDestination,
        action: PublicId,
    },
    #[error("presentation action `{action}` was rejected by {destination:?}: {reason}")]
    Rejected {
        destination: PresentationActionDestination,
        action: PublicId,
        reason: String,
    },
}

/// Error envelope for executing a routed presentation dispatch plan.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PresentationActionExecutionError {
    #[error("presentation action handler failed at dispatch index {index}: {source}")]
    Handler {
        index: usize,
        #[source]
        source: PresentationActionHandlerError,
    },
}

/// Runtime-host adapter contract for `TextBox`, `Activity`, UI, and runtime actions.
pub trait PresentationActionHandlers {
    fn handle_runtime_action(
        &mut self,
        action: &Action,
        output: &mut PresentationActionHandlerOutput,
    ) -> Result<(), PresentationActionHandlerError>;

    fn handle_textbox_action(
        &mut self,
        action: &Action,
        output: &mut PresentationActionHandlerOutput,
    ) -> Result<(), PresentationActionHandlerError>;

    fn handle_activity_action(
        &mut self,
        action: &Action,
        output: &mut PresentationActionHandlerOutput,
    ) -> Result<(), PresentationActionHandlerError>;

    fn handle_ui_entity_action(
        &mut self,
        action: &Action,
        output: &mut PresentationActionHandlerOutput,
    ) -> Result<(), PresentationActionHandlerError>;
}

/// Route an Agent semantic invocation and partition it for runtime-host handlers.
pub fn dispatch_semantic_invoke(
    epoch: InputEpoch,
    target: &InteractionTarget,
    action: &PublicId,
    semantics: &SemanticTree,
    layers: &LayerTree,
    state: &InteractionState,
) -> Result<DispatchedPresentationAction, PresentationActionDispatchError> {
    let action = semantics
        .route_and_lower_action(epoch, target, action, layers, state)
        .map_err(PresentationActionDispatchError::Semantic)?;
    dispatch_presentation_action(semantics, action)
}

/// Partition one routed presentation action by semantic role and action target.
pub fn dispatch_presentation_action(
    semantics: &SemanticTree,
    action: Action,
) -> Result<DispatchedPresentationAction, PresentationActionDispatchError> {
    let destination = match action.target() {
        ActionTarget::Runtime => PresentationActionDestination::Runtime,
        ActionTarget::Activity(target) => {
            let role = role_for_target(semantics, target)?;
            if role != SemanticRole::Activity {
                return Err(PresentationActionDispatchError::ActivityTargetMismatch(
                    target.clone(),
                ));
            }
            PresentationActionDestination::Activity
        }
        ActionTarget::Entity(target) => match role_for_target(semantics, target)? {
            SemanticRole::TextBox => PresentationActionDestination::TextBox,
            SemanticRole::Activity => PresentationActionDestination::Activity,
            SemanticRole::Button
            | SemanticRole::TextField
            | SemanticRole::TextArea
            | SemanticRole::Image
            | SemanticRole::Debug
            | SemanticRole::Custom => PresentationActionDestination::UiEntity,
        },
    };
    Ok(DispatchedPresentationAction {
        destination,
        action,
    })
}

/// Partition an ordered action batch without changing action order.
pub fn dispatch_presentation_action_batch(
    semantics: &SemanticTree,
    actions: ActionBatch,
) -> Result<PresentationActionDispatchPlan, PresentationActionDispatchError> {
    actions
        .into_vec()
        .into_iter()
        .map(|action| dispatch_presentation_action(semantics, action))
        .collect::<Result<Vec<_>, _>>()
        .map(|actions| PresentationActionDispatchPlan { actions })
}

/// Execute a dispatch plan against host-owned handler implementations.
pub fn execute_presentation_action_plan(
    plan: &PresentationActionDispatchPlan,
    handlers: &mut impl PresentationActionHandlers,
) -> Result<PresentationActionHandlerOutput, PresentationActionExecutionError> {
    let mut output = PresentationActionHandlerOutput::default();
    for (index, dispatched) in plan.as_slice().iter().enumerate() {
        let result = match dispatched.destination() {
            PresentationActionDestination::Runtime => {
                handlers.handle_runtime_action(dispatched.action(), &mut output)
            }
            PresentationActionDestination::TextBox => {
                handlers.handle_textbox_action(dispatched.action(), &mut output)
            }
            PresentationActionDestination::Activity => {
                handlers.handle_activity_action(dispatched.action(), &mut output)
            }
            PresentationActionDestination::UiEntity => {
                handlers.handle_ui_entity_action(dispatched.action(), &mut output)
            }
        };
        result.map_err(|source| PresentationActionExecutionError::Handler { index, source })?;
    }
    Ok(output)
}

impl DispatchedPresentationAction {
    pub const fn new(destination: PresentationActionDestination, action: Action) -> Self {
        Self {
            destination,
            action,
        }
    }

    pub const fn destination(&self) -> PresentationActionDestination {
        self.destination
    }

    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub fn into_action(self) -> Action {
        self.action
    }
}

impl PresentationActionDispatchPlan {
    pub fn push(&mut self, action: DispatchedPresentationAction) {
        self.actions.push(action);
    }

    pub fn as_slice(&self) -> &[DispatchedPresentationAction] {
        &self.actions
    }

    pub fn into_vec(self) -> Vec<DispatchedPresentationAction> {
        self.actions
    }
}

impl PresentationActionHandlerOutput {
    pub fn push_action(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn push_host_event(&mut self, event: arcweft_presentation::input::HostEvent) {
        self.host_events.push(event);
    }

    pub fn actions(&self) -> &ActionBatch {
        &self.actions
    }

    pub fn host_events(&self) -> &HostEventBatch {
        &self.host_events
    }

    pub fn into_parts(self) -> (ActionBatch, HostEventBatch) {
        (self.actions, self.host_events)
    }
}

impl PresentationActionHandlerError {
    pub fn unhandled(destination: PresentationActionDestination, action: &Action) -> Self {
        Self::Unhandled {
            destination,
            action: action.kind().clone(),
        }
    }

    pub fn rejected(
        destination: PresentationActionDestination,
        action: &Action,
        reason: impl Into<String>,
    ) -> Self {
        Self::Rejected {
            destination,
            action: action.kind().clone(),
            reason: reason.into(),
        }
    }
}

fn role_for_target(
    semantics: &SemanticTree,
    target: &InteractionTarget,
) -> Result<SemanticRole, PresentationActionDispatchError> {
    semantics
        .find(target)
        .map(arcweft_presentation::semantic::SemanticNode::role)
        .ok_or_else(|| PresentationActionDispatchError::UnknownTarget(target.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{
        AgentInput, HostEvent, HostEventSource, RawInputEvent, RawInputKind,
    };
    use arcweft_presentation::layer::{
        LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, RenderPhase,
    };
    use arcweft_presentation::router::{InputRouter, RouteDecision};
    use arcweft_presentation::semantic::SemanticNode;

    fn public_id(name: &str) -> PublicId {
        PublicId::try_new(name).unwrap()
    }

    fn layer_id(name: &str) -> LayerId {
        LayerId::new(public_id(&format!("layer.{name}")))
    }

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(public_id(&format!("target.{name}")))
    }

    fn order(phase: RenderPhase, z: i32) -> LayerOrder {
        LayerOrder {
            phase,
            z,
            stable_index: 0,
        }
    }

    #[test]
    fn dispatch_batch_partitions_textbox_activity_ui_and_runtime_actions() {
        let textbox = target("textbox.main");
        let activity = target("activity.truck");
        let button = target("ui.button");
        let mut semantics = SemanticTree::default();
        semantics.push(SemanticNode::new(
            layer_id("dialogue"),
            textbox.clone(),
            SemanticRole::TextBox,
            HitRect::new(0.0, 0.0, 100.0, 20.0),
        ));
        semantics.push(SemanticNode::new(
            layer_id("world"),
            activity.clone(),
            SemanticRole::Activity,
            HitRect::new(0.0, 0.0, 100.0, 100.0),
        ));
        semantics.push(SemanticNode::new(
            layer_id("ui"),
            button.clone(),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 20.0, 20.0),
        ));

        let mut batch = ActionBatch::default();
        batch.push(Action::new(
            ActionTarget::Entity(textbox),
            public_id("action.advance"),
        ));
        batch.push(Action::new(
            ActionTarget::Activity(activity),
            public_id("action.pause"),
        ));
        batch.push(Action::new(
            ActionTarget::Entity(button),
            public_id("action.select"),
        ));
        batch.push(Action::new(
            ActionTarget::Runtime,
            public_id("action.open_menu"),
        ));

        let plan = dispatch_presentation_action_batch(&semantics, batch).unwrap();
        let destinations = plan
            .as_slice()
            .iter()
            .map(DispatchedPresentationAction::destination)
            .collect::<Vec<_>>();
        assert_eq!(
            destinations,
            vec![
                PresentationActionDestination::TextBox,
                PresentationActionDestination::Activity,
                PresentationActionDestination::UiEntity,
                PresentationActionDestination::Runtime,
            ]
        );
    }

    #[test]
    fn dispatch_semantic_invoke_routes_through_modal_policy_before_partitioning() {
        let root = layer_id("root");
        let world = layer_id("world");
        let modal = layer_id("modal");
        let activity = target("activity.truck");
        let button = target("modal.close");
        let pause = public_id("action.pause");
        let close = public_id("action.close");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(
                    world.clone(),
                    LayerKind::Activity,
                    order(RenderPhase::World, 0),
                )
                .with_parent(root.clone())
                .with_input_policy(LayerInputPolicy::HitTest),
            )
            .unwrap();
        layers
            .insert(
                LayerNode::new(
                    modal.clone(),
                    LayerKind::Modal,
                    order(RenderPhase::Modal, 0),
                )
                .with_parent(root)
                .with_input_policy(LayerInputPolicy::Modal),
            )
            .unwrap();

        let mut semantics = SemanticTree::default();
        semantics.push(
            SemanticNode::new(
                world,
                activity.clone(),
                SemanticRole::Activity,
                HitRect::new(0.0, 0.0, 100.0, 100.0),
            )
            .with_action(pause.clone()),
        );
        semantics.push(
            SemanticNode::new(
                modal.clone(),
                button.clone(),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 20.0, 20.0),
            )
            .with_action(close.clone()),
        );

        assert_eq!(
            dispatch_semantic_invoke(
                InputEpoch(1),
                &activity,
                &pause,
                &semantics,
                &layers,
                &InteractionState::default(),
            ),
            Err(PresentationActionDispatchError::Semantic(
                SemanticActionError::RejectedByRouter(RouteDecision::BlockedByModal { modal })
            ))
        );

        let dispatched = dispatch_semantic_invoke(
            InputEpoch(2),
            &button,
            &close,
            &semantics,
            &layers,
            &InteractionState::default(),
        )
        .unwrap();
        assert_eq!(
            dispatched.destination(),
            PresentationActionDestination::UiEntity
        );

        let raw = RawInputEvent::new(
            InputEpoch(3),
            RawInputKind::Agent(AgentInput {
                action: close,
                target: Some(button.clone()),
            }),
        );
        let hits = semantics.to_hit_tree();
        assert!(matches!(
            InputRouter::route(&raw, &layers, &hits, &InteractionState::default()).decision(),
            RouteDecision::Routed(event) if event.target() == &button
        ));
    }

    #[test]
    fn activity_action_rejects_non_activity_semantic_target() {
        let button = target("ui.button");
        let mut semantics = SemanticTree::default();
        semantics.push(SemanticNode::new(
            layer_id("ui"),
            button.clone(),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 20.0, 20.0),
        ));

        assert_eq!(
            dispatch_presentation_action(
                &semantics,
                Action::new(
                    ActionTarget::Activity(button.clone()),
                    public_id("action.pause")
                ),
            ),
            Err(PresentationActionDispatchError::ActivityTargetMismatch(
                button
            ))
        );
    }

    #[test]
    fn execute_plan_calls_handlers_in_dispatch_order_and_collects_output() {
        let textbox = target("textbox.main");
        let activity = target("activity.truck");
        let button = target("ui.button");
        let mut plan = PresentationActionDispatchPlan::default();
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::TextBox,
            action: Action::new(
                ActionTarget::Entity(textbox.clone()),
                public_id("action.advance"),
            ),
        });
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::Activity,
            action: Action::new(
                ActionTarget::Activity(activity.clone()),
                public_id("action.pause"),
            ),
        });
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::UiEntity,
            action: Action::new(ActionTarget::Entity(button), public_id("action.select")),
        });
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::Runtime,
            action: Action::new(ActionTarget::Runtime, public_id("action.open_menu")),
        });

        let mut handlers = RecordingHandlers::default();
        let output = execute_presentation_action_plan(&plan, &mut handlers).unwrap();

        assert_eq!(
            handlers.calls,
            vec![
                "textbox:action.advance",
                "activity:action.pause",
                "ui:action.select",
                "runtime:action.open_menu",
            ]
        );
        assert_eq!(
            output.actions().as_slice()[0].target(),
            &ActionTarget::Entity(textbox)
        );
        assert_eq!(
            output.host_events().as_slice()[0].source(),
            &HostEventSource::Activity(activity)
        );
    }

    #[test]
    fn execute_plan_reports_handler_failure_index_without_running_later_actions() {
        let button = target("ui.button");
        let mut plan = PresentationActionDispatchPlan::default();
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::Runtime,
            action: Action::new(ActionTarget::Runtime, public_id("action.first")),
        });
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::UiEntity,
            action: Action::new(ActionTarget::Entity(button), public_id("action.fail")),
        });
        plan.push(DispatchedPresentationAction {
            destination: PresentationActionDestination::Runtime,
            action: Action::new(ActionTarget::Runtime, public_id("action.never")),
        });

        let mut handlers = RecordingHandlers {
            fail_ui: true,
            ..RecordingHandlers::default()
        };

        assert_eq!(
            execute_presentation_action_plan(&plan, &mut handlers),
            Err(PresentationActionExecutionError::Handler {
                index: 1,
                source: PresentationActionHandlerError::Rejected {
                    destination: PresentationActionDestination::UiEntity,
                    action: public_id("action.fail"),
                    reason: "test rejection".to_owned(),
                },
            })
        );
        assert_eq!(handlers.calls, vec!["runtime:action.first"]);
    }

    #[derive(Default)]
    struct RecordingHandlers {
        calls: Vec<String>,
        fail_ui: bool,
    }

    impl RecordingHandlers {
        fn record(&mut self, family: &str, action: &Action) {
            self.calls
                .push(format!("{family}:{}", action.kind().as_str()));
        }
    }

    impl PresentationActionHandlers for RecordingHandlers {
        fn handle_runtime_action(
            &mut self,
            action: &Action,
            _output: &mut PresentationActionHandlerOutput,
        ) -> Result<(), PresentationActionHandlerError> {
            self.record("runtime", action);
            Ok(())
        }

        fn handle_textbox_action(
            &mut self,
            action: &Action,
            output: &mut PresentationActionHandlerOutput,
        ) -> Result<(), PresentationActionHandlerError> {
            self.record("textbox", action);
            if let ActionTarget::Entity(target) = action.target() {
                output.push_action(Action::new(
                    ActionTarget::Entity(target.clone()),
                    public_id("action.textbox.after_advance"),
                ));
            }
            Ok(())
        }

        fn handle_activity_action(
            &mut self,
            action: &Action,
            output: &mut PresentationActionHandlerOutput,
        ) -> Result<(), PresentationActionHandlerError> {
            self.record("activity", action);
            if let ActionTarget::Activity(target) = action.target() {
                output.push_host_event(HostEvent::new(
                    HostEventSource::Activity(target.clone()),
                    public_id("host.activity.action_handled"),
                ));
            }
            Ok(())
        }

        fn handle_ui_entity_action(
            &mut self,
            action: &Action,
            _output: &mut PresentationActionHandlerOutput,
        ) -> Result<(), PresentationActionHandlerError> {
            if self.fail_ui {
                return Err(PresentationActionHandlerError::rejected(
                    PresentationActionDestination::UiEntity,
                    action,
                    "test rejection",
                ));
            }
            self.record("ui", action);
            Ok(())
        }
    }
}
