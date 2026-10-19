//! Borrowed original Line metadata and prefix framing. Retained rows contain
//! borrowed text and already admitted Flow tokens, never captures or proofs.

use super::super::flow::PreparedRuntimeFlowSource;
use super::{Counts, DOMAIN, Error, Footprint, RuntimeLineGroupSourceLimits, limit};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::line_task::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineCleanupPolicy,
    ParallelPolicy, PresentationCleanup, RuntimeLineHandleSiteKind,
};
use crate::plan::{
    RuntimeLineHandleSiteSeed, RuntimeLineTaskCancelRuleSeed, RuntimeLineTaskNodeSeed,
};
use crate::task::semantic::TaskSemanticMeter;

pub(super) struct Site<'a> {
    pub source_ordinal: u32,
    pub kind: RuntimeLineHandleSiteKind,
    pub result: u32,
    pub character: Option<&'a str>,
    pub scheduled_child: Option<u32>,
}
pub(super) enum Trigger {
    Immediate,
    Mark { content: u32, mark: u32 },
    Scheduled(u32),
}
pub(super) enum Node<'a> {
    Sequence(usize),
    Start(usize),
    Parallel(ParallelPolicy, usize),
    Child {
        trigger: Trigger,
        join: ChildJoinPolicy,
        cancel: ChildCancelPolicy,
    },
    Action(PreparedRuntimeFlowSource<'a>),
}
impl Node<'_> {
    fn children(&self) -> usize {
        match self {
            Self::Sequence(count) | Self::Start(count) | Self::Parallel(_, count) => *count,
            Self::Child { .. } => 1,
            Self::Action(_) => 0,
        }
    }
}
pub(super) struct Cancel<'a> {
    pub trigger: &'a str,
    pub action: PreparedRuntimeFlowSource<'a>,
}
pub(super) struct Group<'a> {
    pub definition: [u8; 32],
    pub result: u32,
    pub policy: LineCleanupPolicy,
    pub sites: Vec<Site<'a>>,
    pub activation: PreparedRuntimeFlowSource<'a>,
    pub nodes: Vec<Node<'a>>,
    pub cancel: Vec<Cancel<'a>>,
    pub cleanup: [PreparedRuntimeFlowSource<'a>; 3],
}

pub(super) fn inspect(
    bytes: &[u8],
    limits: RuntimeLineGroupSourceLimits,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(usize, Footprint), Error> {
    let (groups, footprint, _) = read(bytes, limits, resources, meter, false)?;
    Ok((groups, footprint))
}
pub(super) fn retain<'a>(
    bytes: &'a [u8],
    limits: RuntimeLineGroupSourceLimits,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<Group<'a>>, Error> {
    Ok(read(bytes, limits, resources, meter, true)?.2)
}
fn read<'a>(
    bytes: &'a [u8],
    limits: RuntimeLineGroupSourceLimits,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
    retain: bool,
) -> Result<(usize, Footprint, Vec<Group<'a>>), Error> {
    let mut cursor = Cursor::new(bytes);
    let groups = cursor.header()?;
    let mut counts = Counts::new(limits, groups)?;
    let mut rows = if retain {
        Vec::with_capacity(groups)
    } else {
        Vec::new()
    };
    for _ in 0..groups {
        let group = cursor.group(&mut counts, resources, meter, retain)?;
        if retain {
            rows.push(group);
        }
    }
    cursor.finish()?;
    Ok((groups, counts.footprint, rows))
}

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn tag(&mut self, kind: &'static str, count: u8) -> Result<u8, Error> {
        let tag = self.byte()?;
        if tag < count {
            Ok(tag)
        } else {
            Err(Error::Tag { kind, tag })
        }
    }
    fn flag(&mut self) -> Result<bool, Error> {
        Ok(self.tag("optional field", 2)? == 1)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("Source u32 width"),
        ))
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let count = self.u32()? as usize;
        self.raw(count)
    }
    fn text(&mut self, counts: &mut Counts) -> Result<&'a str, Error> {
        let text = std::str::from_utf8(self.frame()?)?;
        counts.text(text)?;
        Ok(text)
    }
    fn positive(&mut self, kind: &'static str, maximum: usize) -> Result<u32, Error> {
        let actual = self.u32()?;
        if actual == 0 || actual as usize > maximum {
            return Err(Error::Reference {
                kind,
                actual,
                maximum,
            });
        }
        Ok(actual)
    }
    fn zero_based(&mut self, kind: &'static str, maximum: usize) -> Result<u32, Error> {
        let actual = self.u32()?;
        if actual as usize >= maximum {
            return Err(Error::Reference {
                kind,
                actual,
                maximum,
            });
        }
        Ok(actual)
    }
    pub fn header(&mut self) -> Result<usize, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        Ok(self.u32()? as usize)
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Trailing {
                offset: self.offset,
            })
        }
    }
    fn flow(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeFlowSource<'a>, Error> {
        let bytes = self.frame()?;
        let child = PreparedRuntimeFlowSource::preflight(
            bytes,
            counts.flow(bytes.len()),
            resources,
            meter,
        )?;
        counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        counts.footprint.value_nodes = counts
            .footprint
            .value_nodes
            .checked_add(
                child
                    .node_count()
                    .checked_sub(child.operation_count())
                    .ok_or(Error::Arithmetic)?,
            )
            .ok_or(Error::Arithmetic)?;
        counts.footprint.owned_bodies = counts
            .footprint
            .owned_bodies
            .checked_add(child.owned_body_count())
            .ok_or(Error::Arithmetic)?;
        counts.footprint.lexical_scopes = counts
            .footprint
            .lexical_scopes
            .checked_add(child.lexical_scope_count())
            .ok_or(Error::Arithmetic)?;
        Ok(child)
    }
    pub fn group(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
        retain: bool,
    ) -> Result<Group<'a>, Error> {
        let before = counts.footprint;
        let definition = self.raw(32)?.try_into().expect("Source definition width");
        let result = self.positive("result type", counts.limits.flow.expression.type_count)?;
        let child_tasks = match self.tag("child cleanup", 3)? {
            0 => ChildTaskCleanup::CancelAndJoin,
            1 => ChildTaskCleanup::Detach,
            2 => ChildTaskCleanup::Finish,
            _ => unreachable!(),
        };
        let presentation = match self.tag("presentation cleanup", 2)? {
            0 => PresentationCleanup::DropRegistered,
            1 => PresentationCleanup::KeepRegistered,
            _ => unreachable!(),
        };
        let audio = match self.tag("audio cleanup", 3)? {
            0 => AudioCleanup::StopRegistered,
            1 => AudioCleanup::FadeRegistered,
            2 => AudioCleanup::KeepRegistered,
            _ => unreachable!(),
        };
        let policy = LineCleanupPolicy {
            child_tasks,
            presentation,
            audio,
        };
        crate::plan::RuntimeLineTaskGroupSeed::validate_cleanup_policy(&policy)?;
        let nodes = self.u32()? as usize;
        if nodes == 0 {
            return Err(Error::Prefix);
        }
        counts.nodes(nodes)?;
        counts.storage(
            nodes
                .checked_mul(
                    size_of::<Node<'_>>()
                        + 2 * size_of::<RuntimeLineTaskNodeSeed>()
                        + size_of::<crate::line_task::LineTaskNode>()
                        + 2 * size_of::<usize>(),
                )
                .ok_or(Error::Arithmetic)?,
        )?;
        let site_count = self.u32()? as usize;
        crate::plan::RuntimeLineTaskGroupSeed::validate_handle_site_count(site_count)?;
        counts.array::<Site<'_>>(site_count)?;
        counts.storage(
            site_count
                .checked_mul(
                    size_of::<RuntimeLineHandleSiteSeed>()
                        + size_of::<crate::line_task::RuntimeLineHandleSite>(),
                )
                .ok_or(Error::Arithmetic)?,
        )?;
        let mut sites = if retain {
            Vec::with_capacity(site_count)
        } else {
            Vec::new()
        };
        let mut previous = None;
        for _ in 0..site_count {
            let source_ordinal = self.u32()?;
            if previous.is_some_and(|previous| previous >= source_ordinal) {
                return Err(Error::Handle {
                    kind: "handle source ordinal",
                });
            }
            previous = Some(source_ordinal);
            let kind = match self.tag("handle site", 4)? {
                0 => RuntimeLineHandleSiteKind::StageActor,
                1 => RuntimeLineHandleSiteKind::ScheduledCue,
                2 => RuntimeLineHandleSiteKind::StageLookCue,
                3 => RuntimeLineHandleSiteKind::Voice,
                _ => unreachable!(),
            };
            let producer_bytes = kind.handle_kind().producer_label().len();
            limit(
                "opaque producer bytes",
                producer_bytes,
                counts.limits.opaque_producer_bytes,
            )?;
            counts.storage(producer_bytes.checked_mul(2).ok_or(Error::Arithmetic)?)?;
            let result = self.positive("handle type", counts.limits.flow.expression.type_count)?;
            let character = if self.flag()? {
                let character = self.text(counts)?;
                arcweft_character::id::CharacterId::validate_source(character)?;
                Some(character)
            } else {
                None
            };
            let scheduled_child = if self.flag()? {
                Some(self.zero_based("scheduled child", nodes)?)
            } else {
                None
            };
            if retain {
                sites.push(Site {
                    source_ordinal,
                    kind,
                    result,
                    character,
                    scheduled_child,
                });
            }
        }
        let activation = self.flow(counts, resources, meter)?;
        let mut rows = if retain {
            Vec::with_capacity(nodes)
        } else {
            Vec::new()
        };
        let mut pending = vec![1usize];
        let mut remaining = 1usize;
        for index in 0..nodes {
            while pending.last() == Some(&0) {
                pending.pop();
            }
            limit("depth", pending.len(), counts.limits.depth as usize)?;
            let current = pending.last_mut().ok_or(Error::Prefix)?;
            *current -= 1;
            remaining = remaining.checked_sub(1).ok_or(Error::Prefix)?;
            let row = self.node(counts, resources, meter, site_count)?;
            let children = row.children();
            counts.array::<crate::runtime_id::RuntimeLineTaskNodeId>(children)?;
            remaining = remaining.checked_add(children).ok_or(Error::Arithmetic)?;
            if remaining > nodes - index - 1 {
                return Err(Error::Prefix);
            }
            if children != 0 {
                pending.push(children);
            }
            if retain {
                rows.push(row);
            }
        }
        if remaining != 0 {
            return Err(Error::Prefix);
        }
        let cancel_count = self.u32()? as usize;
        counts.array::<Cancel<'_>>(cancel_count)?;
        counts.storage(
            cancel_count
                .checked_mul(
                    size_of::<RuntimeLineTaskCancelRuleSeed>()
                        + size_of::<crate::line_task::LineCancelRule>(),
                )
                .ok_or(Error::Arithmetic)?,
        )?;
        let mut cancel = if retain {
            Vec::with_capacity(cancel_count)
        } else {
            Vec::new()
        };
        for _ in 0..cancel_count {
            let trigger = self.text(counts)?;
            // The original interaction identity owner supplies borrowed grammar
            // and normalization; do not copy its trim/empty rule into this codec.
            arcweft_interaction_model::id::Identifier::validate_source(trigger)?;
            counts.text(trigger)?; // constructor input scratch plus retained text
            let action = self.flow(counts, resources, meter)?;
            if retain {
                cancel.push(Cancel { trigger, action });
            }
        }
        let cleanup = [
            self.flow(counts, resources, meter)?,
            self.flow(counts, resources, meter)?,
            self.flow(counts, resources, meter)?,
        ];
        let after = counts.footprint;
        let atoms = (after.nodes - before.nodes)
            .checked_add(after.elements - before.elements)
            .ok_or(Error::Arithmetic)?;
        let (storage, work) = crate::plan::construction::lower::RuntimePlanBodyConstruction::line_group_source_scope_storage(
            atoms,
            after.value_nodes - before.value_nodes,
            after.owned_bodies - before.owned_bodies,
            after.lexical_scopes - before.lexical_scopes,
            counts.limits.flow.expression.local_count,
        )
        .ok_or(Error::Arithmetic)?;
        counts.storage(storage)?;
        super::charge(meter, work)?;
        Ok(Group {
            definition,
            result,
            policy,
            sites,
            activation,
            nodes: rows,
            cancel,
            cleanup,
        })
    }
    fn node(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
        sites: usize,
    ) -> Result<Node<'a>, Error> {
        Ok(match self.tag("node", 5)? {
            0 => Node::Sequence(self.u32()? as usize),
            1 => Node::Start(self.u32()? as usize),
            2 => {
                self.tag("parallel policy", 1)?;
                Node::Parallel(ParallelPolicy::JoinAll, self.u32()? as usize)
            }
            3 => {
                let trigger = match self.tag("child trigger", 3)? {
                    0 => Trigger::Immediate,
                    1 => Trigger::Mark {
                        content: self.positive("mark content", counts.limits.flow.content_count)?,
                        mark: self.positive("mark", u32::MAX as usize)?,
                    },
                    2 => Trigger::Scheduled(self.zero_based("scheduled site", sites)?),
                    _ => unreachable!(),
                };
                let join = match self.tag("child join", 2)? {
                    0 => ChildJoinPolicy::Join,
                    1 => ChildJoinPolicy::Detached,
                    _ => unreachable!(),
                };
                let cancel = match self.tag("child cancel", 3)? {
                    0 => ChildCancelPolicy::CancelAndJoin,
                    1 => ChildCancelPolicy::Finish,
                    2 => ChildCancelPolicy::Detach,
                    _ => unreachable!(),
                };
                RuntimeLineTaskNodeSeed::validate_cancel_policy(cancel)?;
                Node::Child {
                    trigger,
                    join,
                    cancel,
                }
            }
            4 => Node::Action(self.flow(counts, resources, meter)?),
            _ => unreachable!(),
        })
    }
}
