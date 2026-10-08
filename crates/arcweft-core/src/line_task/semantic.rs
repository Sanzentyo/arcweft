//! Actual Line graph transcript, independent of completed task-plan keys.
//! The group owns finite dense nodes; structural admission owns graph validity.
//! Flow payloads use the shared body visitor and the same seal meter.

use super::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineCancelRule,
    LineTaskGroup, LineTaskNode, LineTaskTrigger, ParallelPolicy, PresentationCleanup,
    RuntimeLineHandleSiteKind,
};
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LinePlanSemanticDigest([u8; 32]);
impl LinePlanSemanticDigest {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl LineTaskGroup {
    pub(crate) fn semantic_child_count(
        &self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<usize, RuntimeBodySemanticError> {
        let mut actual = 0;
        for count in [
            self.captures.len(),
            self.activation_exports.len(),
            self.handle_sites.len(),
            self.nodes.len(),
            self.cancel_rules.len(),
        ] {
            actual = meter.checked_count_sum(actual, count)?;
        }
        for node in &self.nodes {
            let edges = match node {
                LineTaskNode::Sequence(children)
                | LineTaskNode::Start(children)
                | LineTaskNode::Parallel { children, .. } => children.len(),
                LineTaskNode::Child { .. } => 1,
                LineTaskNode::Action(_) => 0,
            };
            actual = meter.checked_count_sum(actual, edges)?;
        }
        let action_ops = self.nodes.iter().filter_map(|node| match node {
            LineTaskNode::Action(ops) => Some(ops.as_ref()),
            _ => None,
        });
        for ops in std::iter::once(self.activation_ops.as_ref())
            .chain(action_ops)
            .chain(self.cancel_rules.iter().map(LineCancelRule::action))
            .chain([
                self.cleanup.completed.as_ref(),
                self.cleanup.cancelled.as_ref(),
                self.cleanup.failed.as_ref(),
            ])
        {
            crate::plan::try_visit_ops_events(ops, &mut |event| {
                if let crate::plan::RuntimeFlowTreeEvent::EnterOperation { .. } = event {
                    actual = meter.checked_count_sum(actual, 1)?;
                }
                Ok::<(), RuntimeBodySemanticError>(())
            })?;
        }
        Ok(actual)
    }

    pub(crate) fn semantic_digest(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        meter: &mut TaskSemanticMeter,
        group: crate::runtime_id::RuntimeLineTaskGroupId,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<LinePlanSemanticDigest, RuntimeBodySemanticError> {
        meter.status()?;
        let actual = self.semantic_child_count(meter)?;
        if actual > limits.max_children_per_row as usize {
            meter.reject_owner();
            return Err(RuntimeBodySemanticError::LineChildren {
                ordinal: group.index(),
                actual,
                maximum: limits.max_children_per_row,
            });
        }
        let mut encoder = TaskSemanticEncoder::new(b"arcweft.runtime-plan.line-plan.v1\0", meter);
        self.write_semantic(context, &mut encoder, task_owner, task_reference)?;
        Ok(LinePlanSemanticDigest(*encoder.finish()?.as_bytes()))
    }

    fn write_semantic(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        encoder: &mut TaskSemanticEncoder<'_>,
        task_owner: &RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            RuntimeBodyTaskSource<'_>,
        ) -> Result<
            RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.digest(self.definition.as_bytes());
        context.write_type(encoder, self.result_type)?;
        for locals in [&self.captures, &self.activation_exports] {
            encoder.count(locals.len());
            for (ordinal, local) in locals.iter().enumerate() {
                encoder.enter_element();
                encoder.count(ordinal);
                context.write_local(encoder, *local)?;
            }
        }
        context.write_flow(encoder, &self.activation_ops, task_owner, task_reference)?;
        encoder.count(self.handle_sites.len());
        for (ordinal, site) in self.handle_sites.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.ordinal(site.id().get());
            encoder.ordinal(site.source_ordinal());
            encoder.tag(site.site_kind().semantic_tag());
            context.write_type(encoder, site.result_type())?;
            encoder.tag(u8::from(site.character().is_some()));
            if let Some(character) = site.character() {
                encoder.string(character.as_str());
            }
            encoder.tag(u8::from(site.scheduled_child().is_some()));
            if let Some(child) = site.scheduled_child() {
                encoder.count(child.index());
            }
        }
        encoder.count(self.root.index());
        encoder.count(self.nodes.len());
        for (ordinal, node) in self.nodes.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.tag(node.semantic_tag());
            match node {
                LineTaskNode::Sequence(children) | LineTaskNode::Start(children) => {
                    encoder.count(children.len());
                    for child in children {
                        encoder.enter_element();
                        encoder.count(child.index());
                    }
                }
                LineTaskNode::Parallel { policy, children } => {
                    encoder.tag(policy.semantic_tag());
                    encoder.count(children.len());
                    for child in children {
                        encoder.enter_element();
                        encoder.count(child.index());
                    }
                }
                LineTaskNode::Child {
                    trigger,
                    join_policy,
                    cancel_policy,
                    scope,
                } => {
                    encoder.tag(trigger.semantic_tag());
                    match trigger {
                        LineTaskTrigger::Immediate => {}
                        LineTaskTrigger::Mark(mark) => encoder.count(mark.index()),
                        LineTaskTrigger::Scheduled(site) => encoder.ordinal(site.get()),
                    }
                    encoder.tag(join_policy.semantic_tag());
                    encoder.tag(cancel_policy.semantic_tag());
                    encoder.count(scope.index());
                }
                LineTaskNode::Action(ops) => {
                    context.write_flow(encoder, ops, task_owner, task_reference)?;
                }
            }
            encoder.status()?;
        }
        encoder.count(self.cancel_rules.len());
        for (ordinal, rule) in self.cancel_rules.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.string(rule.trigger.as_str());
            context.write_flow(encoder, &rule.action, task_owner, task_reference)?;
        }
        for (ordinal, ops) in [
            &self.cleanup.completed,
            &self.cleanup.cancelled,
            &self.cleanup.failed,
        ]
        .into_iter()
        .enumerate()
        {
            encoder.enter_element();
            encoder.count(ordinal);
            context.write_flow(encoder, ops, task_owner, task_reference)?;
        }
        encoder.tag(self.cleanup.policy.child_tasks.semantic_tag());
        encoder.tag(self.cleanup.policy.presentation.semantic_tag());
        encoder.tag(self.cleanup.policy.audio.semantic_tag());
        encoder.status().map_err(Into::into)
    }
}

impl LineTaskNode {
    pub(crate) const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Sequence(_) => 0,
            Self::Start(_) => 1,
            Self::Parallel { .. } => 2,
            Self::Child { .. } => 3,
            Self::Action(_) => 4,
        }
    }
}
impl LineTaskTrigger {
    const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Immediate => 0,
            Self::Mark(_) => 1,
            Self::Scheduled(_) => 2,
        }
    }
}
impl ParallelPolicy {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::JoinAll => 0,
        }
    }
}
impl ChildJoinPolicy {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::Join => 0,
            Self::Detached => 1,
        }
    }
}
impl ChildCancelPolicy {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::CancelAndJoin => 0,
            Self::Finish => 1,
            Self::Detach => 2,
        }
    }
}
impl ChildTaskCleanup {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::CancelAndJoin => 0,
            Self::Detach => 1,
            Self::Finish => 2,
        }
    }
}
impl PresentationCleanup {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::DropRegistered => 0,
            Self::KeepRegistered => 1,
        }
    }
}
impl AudioCleanup {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::StopRegistered => 0,
            Self::FadeRegistered => 1,
            Self::KeepRegistered => 2,
        }
    }
}
impl RuntimeLineHandleSiteKind {
    const fn semantic_tag(self) -> u8 {
        match self {
            Self::StageActor => 0,
            Self::ScheduledCue => 1,
            Self::StageLookCue => 2,
            Self::Voice => 3,
        }
    }
}
