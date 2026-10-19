//! Original namespace admission and actual Line seed reconstruction. Content
//! Source alone binds the resulting genuine group handles to content plans.

use super::super::RuntimePlanSourceConstruction;
use super::super::flow::RuntimeFlowSourceHandles;
use super::reading::{self, Group, Node, Trigger};
use super::{Error, PreparedRuntimeLineGroupSources, limit};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeFunctionDefinitionIdentity, RuntimeLineHandleSiteSeed, RuntimeLineTaskCancelRuleSeed,
    RuntimeLineTaskGroupSeed, RuntimeLineTaskNodeSeed, RuntimeLineTaskNodeSeedId,
    RuntimeLineTaskTriggerSeed,
};
use crate::runtime_id::{RuntimeLineHandleSiteId, RuntimePlanTypeId};

pub(super) fn authenticate<'a>(
    prepared: &PreparedRuntimeLineGroupSources<'a>,
    source: &mut RuntimePlanSourceConstruction,
    handles: &RuntimeFlowSourceHandles<'_>,
    resources: &ValueResourceAdmission,
) -> Result<Vec<Group<'a>>, Error> {
    // The same original Flow handle owner authenticates the entire namespace.
    handles.validate(source, prepared.limits.flow)?;
    let actual = source.builder.line_task_groups.len();
    if actual != 0 {
        return Err(Error::ExistingGroups { actual });
    }
    let groups = reading::retain(
        prepared.bytes,
        prepared.limits,
        resources,
        &mut source.meter,
    )?;
    for group in &groups {
        for (index, site) in group.sites.iter().enumerate() {
            let invalid = || crate::plan::RuntimePlanBuildError::InvalidLineHandleType {
                site: RuntimeLineHandleSiteId::from_zero_based(
                    u32::try_from(index).expect("admitted dense site"),
                ),
            };
            let id = RuntimePlanTypeId::from_accepted_ordinal(
                std::num::NonZeroU32::new(site.result).expect("preflighted positive site type"),
            );
            let declaration = source.builder.types.get(id).ok_or_else(invalid)?;
            let crate::plan::RuntimePlanTypeProjection::Opaque {
                producer,
                admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class,
                persistence,
                ..
            } = declaration.projection()
            else {
                return Err(invalid().into());
            };
            limit(
                "opaque producer bytes",
                producer.as_str().len(),
                prepared.limits.opaque_producer_bytes,
            )?;
            let expected = site
                .kind
                .handle_kind()
                .try_producer()
                .map_err(|_| invalid())?;
            limit(
                "opaque producer bytes",
                expected.as_str().len(),
                prepared.limits.opaque_producer_bytes,
            )?;
            let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact_with(
                producer.clone(),
                declaration.semantic_identity(),
                *value_class,
                *persistence,
            );
            crate::line_task::RuntimeLineHandleSite::validate_metadata(
                site.kind,
                site.character.is_some(),
                site.scheduled_child.is_some(),
                &owner,
            )
            .map_err(crate::plan::RuntimePlanBuildError::from)?;
        }
        for node in &group.nodes {
            if let Node::Child {
                trigger: Trigger::Mark { content, mark },
                ..
            } = node
            {
                let id = handles.contents[*content as usize - 1]
                    .resolve(&source.builder.issuer)
                    .ok_or(Error::Handle {
                        kind: "mark content",
                    })?;
                let count = source
                    .builder
                    .dialogue_content
                    .get(id)
                    .ok_or(Error::Handle {
                        kind: "mark content",
                    })?
                    .marks()
                    .len();
                if *mark as usize > count {
                    return Err(Error::Reference {
                        kind: "mark",
                        actual: *mark,
                        maximum: count,
                    });
                }
            }
        }
    }
    Ok(groups)
}

fn semantic(source: &RuntimePlanSourceConstruction, raw: u32) -> RuntimeSemanticTypeId {
    let id = RuntimePlanTypeId::from_accepted_ordinal(
        std::num::NonZeroU32::new(raw).expect("preflighted type"),
    );
    source
        .builder
        .types
        .get(id)
        .expect("authenticated original type namespace")
        .semantic_identity()
}

pub(super) fn group(
    group: Group<'_>,
    source: &mut RuntimePlanSourceConstruction,
    handles: &RuntimeFlowSourceHandles<'_>,
    resources: &mut ValueResourceAdmission,
) -> Result<RuntimeLineTaskGroupSeed, Error> {
    let result_type = semantic(source, group.result);
    let handle_sites = group
        .sites
        .into_iter()
        .enumerate()
        .map(|(index, site)| {
            Ok(RuntimeLineHandleSiteSeed {
                id: RuntimeLineHandleSiteId::from_zero_based(
                    u32::try_from(index).map_err(|_| Error::Arithmetic)?,
                ),
                source_ordinal: site.source_ordinal,
                kind: site.kind,
                result_type: semantic(source, site.result),
                character: site
                    .character
                    .map(arcweft_character::id::CharacterId::try_new)
                    .transpose()?,
                scheduled_child: site
                    .scheduled_child
                    .map(|raw| {
                        RuntimeLineTaskNodeSeedId::from_zero_based(raw as usize)
                            .ok_or(Error::Arithmetic)
                    })
                    .transpose()?,
            })
        })
        .collect::<Result<Box<[_]>, Error>>()?;
    let activation_ops = group.activation.materialize(source, handles, resources)?;
    let mut nodes = Vec::with_capacity(group.nodes.len());
    for (index, node) in group.nodes.into_iter().enumerate().rev() {
        let mut children = |count: usize| -> Result<Vec<RuntimeLineTaskNodeSeed>, Error> {
            if nodes.len() < count {
                return Err(Error::Prefix);
            }
            Ok((0..count)
                .map(|_| nodes.pop().expect("complete original node prefix"))
                .collect())
        };
        let node = match node {
            Node::Sequence(count) => RuntimeLineTaskNodeSeed::Sequence(children(count)?),
            Node::Start(count) => RuntimeLineTaskNodeSeed::Start(children(count)?),
            Node::Parallel(policy, count) => RuntimeLineTaskNodeSeed::Parallel {
                policy,
                children: children(count)?,
            },
            Node::Child {
                trigger,
                join,
                cancel,
            } => {
                let scope = children(1)?.pop().expect("complete child scope");
                let trigger = match trigger {
                    Trigger::Immediate => RuntimeLineTaskTriggerSeed::Immediate,
                    Trigger::Scheduled(site) => RuntimeLineTaskTriggerSeed::Scheduled(
                        RuntimeLineHandleSiteId::from_zero_based(site),
                    ),
                    Trigger::Mark { content, mark } => RuntimeLineTaskTriggerSeed::Mark(
                        handles.contents[content as usize - 1]
                            .mark(mark as usize - 1)
                            .ok_or(Error::Reference {
                                kind: "mark",
                                actual: mark,
                                maximum: u32::MAX as usize,
                            })?,
                    ),
                };
                RuntimeLineTaskNodeSeed::Child {
                    node: RuntimeLineTaskNodeSeedId::from_zero_based(index)
                        .ok_or(Error::Arithmetic)?,
                    trigger,
                    join_policy: join,
                    cancel_policy: cancel,
                    scope: Box::new(scope),
                }
            }
            Node::Action(flow) => {
                RuntimeLineTaskNodeSeed::Action(flow.materialize(source, handles, resources)?)
            }
        };
        nodes.push(node);
    }
    if nodes.len() != 1 {
        return Err(Error::Prefix);
    }
    let root = nodes.pop().expect("one complete Line root");
    let mut cancel_rules = Vec::with_capacity(group.cancel.len());
    for cancel in group.cancel {
        cancel_rules.push(RuntimeLineTaskCancelRuleSeed {
            trigger: arcweft_interaction_model::input::InputActionId::new(cancel.trigger)?,
            action: cancel.action.materialize(source, handles, resources)?,
        });
    }
    let [completed, cancelled, failed] = group.cleanup;
    Ok(RuntimeLineTaskGroupSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(group.definition),
        activation_ops,
        result_type,
        handle_sites,
        root,
        cancel_rules: cancel_rules.into_boxed_slice(),
        cleanup_completed: completed.materialize(source, handles, resources)?,
        cleanup_cancelled: cancelled.materialize(source, handles, resources)?,
        cleanup_failed: failed.materialize(source, handles, resources)?,
        cleanup_policy: group.policy,
    })
}
