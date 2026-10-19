//! Counted emission of the original Line graph and its Flow Source bodies.
//! Original node coordinates must describe the same dense preorder prefix.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeLineGroupSourceLimits, charge, limit, reading,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::line_task::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineTaskGroup,
    LineTaskNode, LineTaskTrigger, ParallelPolicy, PresentationCleanup, RuntimeLineHandleSiteKind,
    ScopeExit,
};
use crate::plan::{FlowOp, RuntimePlan};
use crate::runtime_id::RuntimeLineTaskNodeId;
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct EncodedRuntimeLineGroupSources {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeLineGroupSources {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes.len() {
            return Err(Error::OutputCapacity {
                required: self.bytes.len(),
                remaining,
            });
        }
        output.extend_from_slice(&self.bytes);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn encode_line_group_sources(
        &self,
        limits: RuntimeLineGroupSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeLineGroupSources, Error> {
        meter.status()?;
        let result = (|| {
            let groups = self.line_task_groups().len();
            let mut counts = Counts::new(limits, groups)?;
            let mut length = DOMAIN.len().checked_add(5).ok_or(Error::Arithmetic)?;
            limit("encoded bytes", length, limits.encoded_bytes)?;
            for (index, group) in self.line_task_groups().iter().enumerate() {
                let bytes = encode_group(self, index, group, &counts, resources, meter)?;
                length = length.checked_add(bytes.len()).ok_or(Error::Arithmetic)?;
                limit("encoded bytes", length, limits.encoded_bytes)?;
                let mut cursor = reading::Cursor::new(&bytes);
                cursor.group(&mut counts, resources, meter, false)?;
                cursor.finish()?;
            }
            charge(meter, length)?;
            let mut output = Writer::with_capacity(length);
            output.raw(DOMAIN)?;
            output.byte(1)?;
            output.count(groups)?;
            let mut emitted = Counts::new(limits, groups)?;
            for (index, group) in self.line_task_groups().iter().enumerate() {
                let bytes = encode_group(self, index, group, &emitted, resources, meter)?;
                let mut cursor = reading::Cursor::new(&bytes);
                cursor.group(&mut emitted, resources, meter, false)?;
                cursor.finish()?;
                output.raw(&bytes)?;
            }
            debug_assert_eq!(output.bytes.len(), length);
            debug_assert_eq!(emitted.footprint, counts.footprint);
            Ok(EncodedRuntimeLineGroupSources {
                bytes: output.bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}

fn canonical_preorder(group: &LineTaskGroup, counts: &Counts) -> Result<(), Error> {
    limit(
        "nodes",
        group.nodes().len(),
        (counts.limits.nodes as usize).saturating_sub(counts.footprint.nodes),
    )?;
    // The same prefix-counter storage is counted by the borrowed group reader.
    let scratch = group
        .nodes()
        .len()
        .checked_mul(size_of::<RuntimeLineTaskNodeId>())
        .ok_or(Error::Arithmetic)?;
    limit(
        "decoded bytes",
        counts
            .footprint
            .storage
            .checked_add(scratch)
            .ok_or(Error::Arithmetic)?,
        counts.limits.decoded_bytes,
    )?;
    let mut pending = Vec::with_capacity(group.nodes().len());
    pending.push(group.root());
    let mut expected = 0usize;
    while let Some(id) = pending.pop() {
        if id.index() != expected {
            return Err(Error::Prefix);
        }
        expected = expected.checked_add(1).ok_or(Error::Arithmetic)?;
        let node = group.node(id).ok_or(Error::Prefix)?;
        match node {
            LineTaskNode::Sequence(children)
            | LineTaskNode::Start(children)
            | LineTaskNode::Parallel { children, .. } => {
                pending.extend(children.iter().rev().copied())
            }
            LineTaskNode::Child { scope, .. } => pending.push(*scope),
            LineTaskNode::Action(_) => (),
        }
        if pending.len() > group.nodes().len().saturating_sub(expected) {
            return Err(Error::Prefix);
        }
    }
    if expected != group.nodes().len() {
        return Err(Error::Prefix);
    }
    Ok(())
}
fn encode_group(
    plan: &RuntimePlan,
    index: usize,
    group: &LineTaskGroup,
    counts: &Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<u8>, Error> {
    canonical_preorder(group, counts)?;
    let mut output = Writer::new(counts.limits.encoded_bytes);
    output.raw(group.definition().as_bytes())?;
    output.u32(group.result_type().get().get())?;
    let policy = group.cleanup().policy();
    output.byte(match policy.child_tasks {
        ChildTaskCleanup::CancelAndJoin => 0,
        ChildTaskCleanup::Detach => 1,
        ChildTaskCleanup::Finish => 2,
    })?;
    output.byte(match policy.presentation {
        PresentationCleanup::DropRegistered => 0,
        PresentationCleanup::KeepRegistered => 1,
    })?;
    output.byte(match policy.audio {
        AudioCleanup::StopRegistered => 0,
        AudioCleanup::FadeRegistered => 1,
        AudioCleanup::KeepRegistered => 2,
    })?;
    output.count(group.nodes().len())?;
    output.count(group.handle_sites().len())?;
    for (ordinal, site) in group.handle_sites().iter().enumerate() {
        if site.id().index() != ordinal {
            return Err(Error::Handle {
                kind: "handle site",
            });
        }
        output.u32(site.source_ordinal())?;
        output.byte(match site.site_kind() {
            RuntimeLineHandleSiteKind::StageActor => 0,
            RuntimeLineHandleSiteKind::ScheduledCue => 1,
            RuntimeLineHandleSiteKind::StageLookCue => 2,
            RuntimeLineHandleSiteKind::Voice => 3,
        })?;
        output.u32(site.result_type().get().get())?;
        output.byte(u8::from(site.character().is_some()))?;
        if let Some(character) = site.character() {
            output.text(character.as_str())?;
        }
        output.byte(u8::from(site.scheduled_child().is_some()))?;
        if let Some(child) = site.scheduled_child() {
            output.count(child.index())?;
        }
    }
    output.flow(plan, group.activation_ops(), counts, resources, meter)?;
    for node in group.nodes() {
        match node {
            LineTaskNode::Sequence(children) => {
                output.byte(0)?;
                output.count(children.len())?;
            }
            LineTaskNode::Start(children) => {
                output.byte(1)?;
                output.count(children.len())?;
            }
            LineTaskNode::Parallel { policy, children } => {
                output.byte(2)?;
                output.byte(match policy {
                    ParallelPolicy::JoinAll => 0,
                })?;
                output.count(children.len())?;
            }
            LineTaskNode::Child {
                trigger,
                join_policy,
                cancel_policy,
                ..
            } => {
                output.byte(3)?;
                match trigger {
                    LineTaskTrigger::Immediate => output.byte(0)?,
                    LineTaskTrigger::Mark(mark) => {
                        let mut owners = plan.dialogue_content().rows().iter().enumerate().filter(
                            |(_, content)| {
                                content
                                    .line_task_group()
                                    .is_some_and(|group| group.index() == index)
                            },
                        );
                        let (owner, content) = owners.next().ok_or(Error::Handle {
                            kind: "mark content owner",
                        })?;
                        if owners.next().is_some() || content.marks().get(mark.index()).is_none() {
                            return Err(Error::Handle {
                                kind: "mark content owner",
                            });
                        }
                        output.byte(1)?;
                        output.count(owner.checked_add(1).ok_or(Error::Arithmetic)?)?;
                        output.count(mark.index().checked_add(1).ok_or(Error::Arithmetic)?)?;
                    }
                    LineTaskTrigger::Scheduled(site) => {
                        output.byte(2)?;
                        output.u32(site.get())?;
                    }
                }
                output.byte(match join_policy {
                    ChildJoinPolicy::Join => 0,
                    ChildJoinPolicy::Detached => 1,
                })?;
                output.byte(match cancel_policy {
                    ChildCancelPolicy::CancelAndJoin => 0,
                    ChildCancelPolicy::Finish => 1,
                    ChildCancelPolicy::Detach => 2,
                })?;
            }
            LineTaskNode::Action(ops) => {
                output.byte(4)?;
                output.flow(plan, ops, counts, resources, meter)?;
            }
        }
    }
    output.count(group.cancel_rules().len())?;
    for rule in group.cancel_rules() {
        output.text(rule.trigger().as_str())?;
        output.flow(plan, rule.action(), counts, resources, meter)?;
    }
    for exit in [
        ScopeExit::Completed,
        ScopeExit::Cancelled,
        ScopeExit::Failed,
    ] {
        output.flow(
            plan,
            group.cleanup().actions(exit),
            counts,
            resources,
            meter,
        )?;
    }
    Ok(output.bytes)
}

struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Writer {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }
    fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            maximum: capacity,
        }
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", length, self.maximum)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.raw(&[byte])
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn count(&mut self, count: usize) -> Result<(), Error> {
        self.u32(u32::try_from(count).map_err(|_| Error::Arithmetic)?)
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.count(bytes.len())?;
        self.raw(bytes)
    }
    fn text(&mut self, text: &str) -> Result<(), Error> {
        self.frame(text.as_bytes())
    }
    fn flow(
        &mut self,
        plan: &RuntimePlan,
        ops: &[FlowOp],
        counts: &Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let child = plan.encode_flow_source(ops, counts.flow(self.maximum), resources, meter)?;
        self.frame(child.bytes())
    }
}
