use arcweft_presentation::input::{
    Action, ActionBatch, HostEvent, HostEventBatch, HostEventSource, InputEvent, InteractionTarget,
};
use thiserror::Error;

/// Borrowed Activity input selected from routed presentation input and host events.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityStepInputRef<'a> {
    target: &'a InteractionTarget,
    input_events: Vec<&'a InputEvent>,
    host_events: Vec<&'a HostEvent>,
}

/// Output collected from one Activity step.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityStepOutput {
    actions: ActionBatch,
    host_events: HostEventBatch,
}

/// Mutable sink passed to an Activity while it is stepped by the runtime host.
pub struct ActivityStepOutputSink<'a> {
    output: &'a mut ActivityStepOutput,
}

/// Host-owned Activity implementation boundary.
pub trait ActivityHost {
    fn step(
        &mut self,
        input: ActivityStepInputRef<'_>,
        output: &mut ActivityStepOutputSink<'_>,
    ) -> Result<(), ActivityHostError>;
}

/// Runtime-host registry for concrete Activity instances.
#[derive(Default)]
pub struct ActivityHostRegistry {
    hosts: Vec<ActivityHostRegistration>,
}

struct ActivityHostRegistration {
    target: InteractionTarget,
    host: Box<dyn ActivityHost>,
}

/// Failure reported by an Activity host implementation.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ActivityHostError {
    #[error("activity rejected step: {0}")]
    Rejected(String),
}

/// Error while registering Activity hosts.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ActivityHostRegistrationError {
    #[error("duplicate Activity host target: {0:?}")]
    DuplicateTarget(InteractionTarget),
}

/// Error while stepping an Activity through the runtime host.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ActivityHostStepError {
    #[error("unknown Activity host target: {0:?}")]
    UnknownTarget(InteractionTarget),
    #[error("Activity host failed for {target:?}: {source}")]
    Host {
        target: InteractionTarget,
        #[source]
        source: ActivityHostError,
    },
}

impl<'a> ActivityStepInputRef<'a> {
    pub fn new(
        target: &'a InteractionTarget,
        input_events: &'a [InputEvent],
        host_events: &'a [HostEvent],
    ) -> Self {
        Self {
            target,
            input_events: input_events
                .iter()
                .filter(|event| event.target() == target)
                .collect(),
            host_events: host_events
                .iter()
                .filter(|event| {
                    matches!(event.source(), HostEventSource::Activity(activity) if activity == target)
                })
                .collect(),
        }
    }

    pub const fn target(&self) -> &InteractionTarget {
        self.target
    }

    pub fn input_events(&self) -> &[&'a InputEvent] {
        &self.input_events
    }

    pub fn host_events(&self) -> &[&'a HostEvent] {
        &self.host_events
    }
}

impl ActivityStepOutput {
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

impl ActivityStepOutputSink<'_> {
    pub fn push_action(&mut self, action: Action) {
        self.output.actions.push(action);
    }

    pub fn push_host_event(&mut self, event: HostEvent) {
        self.output.host_events.push(event);
    }
}

impl ActivityHostRegistry {
    pub fn register(
        &mut self,
        target: InteractionTarget,
        host: impl ActivityHost + 'static,
    ) -> Result<(), ActivityHostRegistrationError> {
        if self.hosts.iter().any(|entry| entry.target == target) {
            return Err(ActivityHostRegistrationError::DuplicateTarget(target));
        }
        self.hosts.push(ActivityHostRegistration {
            target,
            host: Box::new(host),
        });
        Ok(())
    }

    pub fn targets(&self) -> impl Iterator<Item = &InteractionTarget> {
        self.hosts.iter().map(|entry| &entry.target)
    }

    pub fn step_activity(
        &mut self,
        target: &InteractionTarget,
        input_events: &[InputEvent],
        host_events: &[HostEvent],
    ) -> Result<ActivityStepOutput, ActivityHostStepError> {
        let Some(entry) = self.hosts.iter_mut().find(|entry| &entry.target == target) else {
            return Err(ActivityHostStepError::UnknownTarget(target.clone()));
        };
        let input = ActivityStepInputRef::new(&entry.target, input_events, host_events);
        let mut output = ActivityStepOutput::default();
        let mut sink = ActivityStepOutputSink {
            output: &mut output,
        };
        entry
            .host
            .step(input, &mut sink)
            .map_err(|source| ActivityHostStepError::Host {
                target: target.clone(),
                source,
            })?;
        Ok(output)
    }
}

impl ActivityHostError {
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self::Rejected(reason.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::input::{
        ActionTarget, InputEpoch, InputEventKind, KeyPhase, PointerPhase,
    };

    fn public_id(name: &str) -> PublicId {
        PublicId::try_new(name).unwrap()
    }

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(public_id(&format!("target.{name}")))
    }

    #[test]
    fn activity_step_input_selects_only_routed_events_for_target() {
        let truck = target("activity.truck");
        let menu = target("activity.menu");
        let input_events = vec![
            InputEvent::new(
                InputEpoch(1),
                truck.clone(),
                InputEventKind::Pointer {
                    phase: PointerPhase::Down,
                },
            ),
            InputEvent::new(
                InputEpoch(2),
                menu.clone(),
                InputEventKind::Key {
                    key: "Escape".to_owned(),
                    phase: KeyPhase::Down,
                },
            ),
        ];
        let host_events = vec![
            HostEvent::new(
                HostEventSource::Activity(truck.clone()),
                public_id("host.activity.loaded"),
            ),
            HostEvent::new(
                HostEventSource::Activity(menu),
                public_id("host.activity.paused"),
            ),
        ];

        let input = ActivityStepInputRef::new(&truck, &input_events, &host_events);

        assert_eq!(input.target(), &truck);
        assert_eq!(input.input_events().len(), 1);
        assert_eq!(input.input_events()[0].raw_epoch(), InputEpoch(1));
        assert_eq!(input.host_events().len(), 1);
        assert_eq!(
            input.host_events()[0].kind().as_str(),
            "host.activity.loaded"
        );
    }

    #[test]
    fn registry_steps_activity_with_selected_input_and_collects_output() {
        let truck = target("activity.truck");
        let menu = target("activity.menu");
        let mut registry = ActivityHostRegistry::default();
        registry.register(truck.clone(), RecordingActivity).unwrap();
        let input_events = vec![
            InputEvent::new(InputEpoch(1), truck.clone(), InputEventKind::Activate),
            InputEvent::new(InputEpoch(2), menu, InputEventKind::Activate),
        ];
        let host_events = vec![HostEvent::new(
            HostEventSource::Activity(truck.clone()),
            public_id("host.activity.ready"),
        )];

        let output = registry
            .step_activity(&truck, &input_events, &host_events)
            .unwrap();

        assert_eq!(output.actions().as_slice().len(), 1);
        assert_eq!(
            output.actions().as_slice()[0].target(),
            &ActionTarget::Activity(truck.clone())
        );
        assert_eq!(output.host_events().as_slice().len(), 1);
        assert_eq!(
            output.host_events().as_slice()[0].source(),
            &HostEventSource::Activity(truck)
        );
    }

    #[test]
    fn registry_rejects_duplicate_or_unknown_activity_targets() {
        let truck = target("activity.truck");
        let mut registry = ActivityHostRegistry::default();
        registry.register(truck.clone(), RecordingActivity).unwrap();

        assert_eq!(
            registry.register(truck.clone(), RecordingActivity),
            Err(ActivityHostRegistrationError::DuplicateTarget(
                truck.clone()
            ))
        );
        assert_eq!(
            registry.step_activity(&target("activity.missing"), &[], &[]),
            Err(ActivityHostStepError::UnknownTarget(target(
                "activity.missing"
            )))
        );
    }

    #[derive(Default)]
    struct RecordingActivity;

    impl ActivityHost for RecordingActivity {
        fn step(
            &mut self,
            input: ActivityStepInputRef<'_>,
            output: &mut ActivityStepOutputSink<'_>,
        ) -> Result<(), ActivityHostError> {
            if !input.input_events().is_empty() {
                output.push_action(Action::new(
                    ActionTarget::Activity(input.target().clone()),
                    public_id("action.activity.input_handled"),
                ));
            }
            if !input.host_events().is_empty() {
                output.push_host_event(HostEvent::new(
                    HostEventSource::Activity(input.target().clone()),
                    public_id("host.activity.step_complete"),
                ));
            }
            Ok(())
        }
    }
}
