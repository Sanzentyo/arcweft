//! Original seed reconstruction after complete borrowed forest admission.
//! Each row reuses its admitted aggregate prefix; only the actual construction
//! issuer supplies declaration handles. No executable proof enters this reader.

use super::super::RuntimePlanSourceConstruction;
use super::super::audio_commands::PreparedRuntimeAudioCommandSource;
use super::super::evaluated_effects::PreparedRuntimeEvaluatedEffectSource;
use super::super::host_calls::PreparedRuntimeHostCallSource;
use super::super::project_calls::PreparedRuntimeProjectCallSource;
use super::super::selected_producers::PreparedRuntimeSelectedProducerSource;
use super::super::static_effects::PreparedRuntimeStaticEffectSource;
use super::metadata::{Cursor, Row};
use super::{Counts, Error, RuntimeFlowSourceHandles, RuntimeFlowSourceLimits, charge};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::{RuntimePattern, RuntimeSemanticTypeId};
use crate::plan::{
    FlowRuntimeId, RuntimeAssignmentSeed, RuntimeAwaitManyTargetSeed,
    RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed, RuntimeBorrowedLocalSeed,
    RuntimeBuiltinIteratorEvidenceSeed, RuntimeBuiltinIteratorFamily, RuntimeCallArgumentSeed,
    RuntimeChoiceOptionSeed, RuntimeDeferOwner, RuntimeDialogueResultTargetSeed, RuntimeExprSeed,
    RuntimeFlowMatchArmSeed, RuntimeFlowMatchGuardSeed, RuntimeFlowOpSeed,
    RuntimeFunctionDefinitionIdentity, RuntimeHostArgumentSeed, RuntimeHostTaskRequestTemplateSeed,
    RuntimeIteratorEvidenceSeed, RuntimeIteratorWitnessEvidenceSeed,
    RuntimeIteratorWitnessExecutableSeed, RuntimeLineEffectSeed, RuntimeLineOperationSeed,
    RuntimeLineTaskNodeSeedId, RuntimeLocalSeedId, RuntimeMutablePlaceSeed,
    RuntimeNeedProducerTemplateSeed, RuntimePatternSeed, RuntimePlanBuildError,
    RuntimeRecordFieldSeedId, RuntimeScheduledCaptureSeed, RuntimeStandardMapControlSeed,
    RuntimeTraitMethodSeedId,
};
use crate::runtime_id::{
    RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment, RuntimeLineHandleSiteId,
    RuntimeLocalDeclarationId, RuntimePlanTypeId,
};
use crate::task::{
    CancelScopeId, HostCapabilityId, NeedProducerContractDigest, NeedProducerFamily,
    NeedProducerSiteDigest, RuntimeRequestRoleIdentity, RuntimeTypeSemanticDigest, TaskClass,
    TaskPolicy, TaskPriority,
};
use crate::value::{
    RuntimeCallArgumentMode, RuntimeDisplacedField, RuntimeExpr, RuntimeFmtParameterId,
    RuntimePlaceDisplacement, RuntimePlaceInitialization, RuntimeStandardMapFamily,
};

impl RuntimeFlowSourceHandles<'_> {
    pub(in crate::plan::construction::source) fn validate(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        limits: RuntimeFlowSourceLimits,
    ) -> Result<(), Error> {
        self.expression.validate(source, limits.expression)?;
        let tasks = source
            .builder
            .task_plan_definitions
            .lock()
            .map_err(|_| RuntimePlanBuildError::Poisoned)?
            .as_ref()
            .ok_or(Error::Handle { kind: "task owner" })?
            .len();
        for (kind, expected, actual) in [
            (
                "content",
                limits.content_count,
                source.builder.dialogue_content.len(),
            ),
            ("content handles", limits.content_count, self.contents.len()),
            (
                "defer",
                limits.defer_count,
                source.builder.defer_sites.len(),
            ),
            ("defer handles", limits.defer_count, self.defers.len()),
            ("task", limits.task_count, tasks),
            ("task handles", limits.task_count, self.tasks.len()),
            (
                "project",
                limits.project_count,
                source.builder.project_call_sites.borrow().rows.len(),
            ),
            (
                "project handles",
                limits.project_count,
                source.projects.len(),
            ),
        ] {
            if expected != actual {
                return Err(Error::Namespace {
                    kind,
                    expected,
                    actual,
                });
            }
        }
        let handles = self
            .contents
            .len()
            .checked_add(self.defers.len())
            .and_then(|n| n.checked_add(self.tasks.len()))
            .and_then(|n| n.checked_add(source.projects.len()))
            .ok_or(Error::Arithmetic)?;
        charge(&mut source.meter, handles)?;
        for (index, project) in source.projects.iter().enumerate() {
            if project
                .resolve(&source.builder.issuer)
                .is_none_or(|id| id.index() != index)
            {
                return Err(Error::Handle { kind: "project" });
            }
        }
        for (index, handle) in self.contents.iter().enumerate() {
            if handle
                .resolve(&source.builder.issuer)
                .is_none_or(|id| id.get().get() as usize - 1 != index)
            {
                return Err(Error::Handle { kind: "content" });
            }
        }
        for (index, handle) in self.defers.iter().enumerate() {
            if handle.index() != index {
                return Err(Error::Handle { kind: "defer" });
            }
        }
        let owner = source
            .builder
            .task_coordinate_owner(u32::try_from(tasks).map_err(|_| Error::Arithmetic)?);
        for (index, handle) in self.tasks.iter().enumerate() {
            if handle.ordinal() as usize != index || !owner.contains(handle) {
                return Err(Error::Handle { kind: "task" });
            }
        }
        Ok(())
    }
}

pub(super) struct Decoder<'a, 'h> {
    pub source: &'a mut RuntimePlanSourceConstruction,
    pub handles: &'a RuntimeFlowSourceHandles<'h>,
    pub resources: &'a mut ValueResourceAdmission,
    pub limits: RuntimeFlowSourceLimits,
    pub seeds: &'a mut Vec<RuntimeFlowOpSeed>,
}
impl Decoder<'_, '_> {
    pub fn operation(self, row: &Row<'_>) -> Result<RuntimeFlowOpSeed, Error> {
        if self.seeds.len() < row.children {
            return Err(Error::Prefix);
        }
        let before = self.seeds.len();
        let mut reader = Reader {
            metadata: Cursor::new(row.metadata),
            values: Cursor::new(row.values),
            bodies: Cursor::new(row.bodies),
            counts: Counts {
                limits: self.limits,
                footprint: row.prefix,
            },
            source: self.source,
            handles: self.handles,
            resources: self.resources,
            seeds: self.seeds,
        };
        let seed = reader.operation(row.tag)?;
        reader.metadata.finish()?;
        reader.values.finish()?;
        reader.bodies.finish()?;
        debug_assert_eq!(reader.counts.footprint, row.end);
        debug_assert_eq!(reader.seeds.len(), before - row.children);
        Ok(seed)
    }
}

struct Reader<'p, 'a, 'h> {
    metadata: Cursor<'p>,
    values: Cursor<'p>,
    bodies: Cursor<'p>,
    counts: Counts,
    source: &'a mut RuntimePlanSourceConstruction,
    handles: &'a RuntimeFlowSourceHandles<'h>,
    resources: &'a mut ValueResourceAdmission,
    seeds: &'a mut Vec<RuntimeFlowOpSeed>,
}
impl Reader<'_, '_, '_> {
    fn text(&mut self) -> Result<String, Error> {
        let value = self.metadata.text()?;
        self.counts.text(value)?;
        Ok(value.to_owned())
    }
    fn optional_text(&mut self) -> Result<Option<String>, Error> {
        if self.metadata.flag("optional text")? {
            self.text().map(Some)
        } else {
            Ok(None)
        }
    }
    fn local(&mut self) -> Result<RuntimeLocalSeedId, Error> {
        let raw = self
            .metadata
            .positive("local", self.counts.limits.expression.local_count)?;
        Ok(self.source.locals[raw as usize - 1].clone())
    }
    fn ty(&mut self) -> Result<RuntimeSemanticTypeId, Error> {
        let raw = self
            .metadata
            .positive("type", self.counts.limits.expression.type_count)?;
        let id = RuntimePlanTypeId::from_accepted_ordinal(
            std::num::NonZeroU32::new(raw).expect("admitted positive type"),
        );
        Ok(self
            .source
            .builder
            .types
            .get(id)
            .expect("authenticated original type namespace")
            .semantic_identity())
    }
    fn locals(&mut self) -> Result<Vec<RuntimeLocalSeedId>, Error> {
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeLocalSeedId>(count)?;
        (0..count).map(|_| self.local()).collect()
    }
    fn body(&mut self) -> Result<Vec<RuntimeFlowOpSeed>, Error> {
        let count = self.bodies.u32()? as usize;
        if count > self.seeds.len() {
            return Err(Error::Prefix);
        }
        Ok((0..count)
            .map(|_| self.seeds.pop().expect("complete admitted body prefix"))
            .collect())
    }
    fn expression(&mut self) -> Result<RuntimeExprSeed, Error> {
        let (kind, bytes) = self.values.value()?;
        if kind != 0 {
            return Err(Error::Tag {
                kind: "expression root",
                tag: kind,
            });
        }
        let child = RuntimeExpr::preflight_sources(
            bytes,
            self.counts.expression(bytes.len()),
            self.resources,
            &mut self.source.meter,
        )?;
        self.counts
            .child(child.node_count(), child.elements(), child.storage_bytes())?;
        Ok(child.materialize(self.source, self.handles.expression, self.resources)?)
    }
    fn pattern(&mut self) -> Result<RuntimePatternSeed, Error> {
        let (kind, bytes) = self.values.value()?;
        if kind != 1 {
            return Err(Error::Tag {
                kind: "pattern root",
                tag: kind,
            });
        }
        let child = RuntimePattern::preflight_sources(
            bytes,
            self.counts.pattern(bytes.len()),
            self.resources,
            &mut self.source.meter,
        )?;
        self.counts
            .child(child.node_count(), child.elements(), child.storage_bytes())?;
        Ok(child.materialize(self.source, self.resources)?)
    }
    fn optional_pattern(&mut self) -> Result<Option<RuntimePatternSeed>, Error> {
        if self.metadata.flag("optional pattern")? {
            self.pattern().map(Some)
        } else {
            Ok(None)
        }
    }
    fn optional_expression(&mut self) -> Result<Option<RuntimeExprSeed>, Error> {
        if self.metadata.flag("optional expression")? {
            self.expression().map(Some)
        } else {
            Ok(None)
        }
    }
    fn effect(&mut self) -> Result<RuntimeLineEffectSeed, Error> {
        let tag = self.metadata.byte()?;
        let bytes = self.metadata.frame()?;
        match tag {
            0 => {
                let child = PreparedRuntimeStaticEffectSource::preflight(
                    bytes,
                    self.counts.static_effect(bytes.len()),
                    self.resources,
                    &mut self.source.meter,
                )?;
                self.counts
                    .child(child.node_count(), child.elements(), child.storage_bytes())?;
                Ok(child.materialize(self.source, self.resources)?)
            }
            1 => {
                let child = PreparedRuntimeAudioCommandSource::preflight(
                    bytes,
                    self.counts.audio(bytes.len()),
                    self.resources,
                    &mut self.source.meter,
                )?;
                self.counts
                    .child(child.node_count(), child.elements(), child.storage_bytes())?;
                Ok(RuntimeLineEffectSeed::Audio(Box::new(child.materialize(
                    self.source,
                    self.handles.expression,
                    self.resources,
                )?)))
            }
            tag => Err(Error::Tag {
                kind: "effect Source",
                tag,
            }),
        }
    }
    fn path(&mut self) -> Result<FlowRuntimeId, Error> {
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeIdSegment>(count)?;
        let mut segments = Vec::with_capacity(count);
        for index in 0..count {
            let text = self.metadata.text()?;
            self.counts.text(text)?;
            segments.push(
                RuntimeIdPath::preflight_stored_source_segment(
                    RuntimeIdFamily::Flow,
                    index,
                    count,
                    text,
                )?
                .materialize(),
            );
        }
        let path = RuntimeIdPath::from_source_segment_storage(RuntimeIdFamily::Flow, segments)?;
        Ok(FlowRuntimeId::from_source_path(path, self.text()?))
    }
    fn scope(&mut self) -> Result<crate::scope::RuntimeScopeIdentity, Error> {
        Ok(match self.optional_text()? {
            Some(name) => crate::scope::RuntimeScopeIdentity::Named(
                arcweft_id::DeclarationName::try_new(name)?,
            ),
            None => crate::scope::RuntimeScopeIdentity::Anonymous,
        })
    }
    fn arguments(&mut self) -> Result<Vec<RuntimeHostArgumentSeed>, Error> {
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeHostArgumentSeed>(count)?;
        let start = self.metadata.offset;
        for _ in 0..count {
            let tag = self.metadata.byte()?;
            self.metadata.digest()?;
            if tag == 1 {
                let name = self.metadata.text()?;
                self.counts.text(name)?;
            }
        }
        // All retained names were charged before the first value, in the same
        // order as borrowed inspection. Roles need no scratch allocation.
        let mut roles = Cursor::new(&self.metadata.bytes[start..self.metadata.offset]);
        let mut args = Vec::with_capacity(count);
        for _ in 0..count {
            let tag = roles.byte()?;
            let identity = RuntimeRequestRoleIdentity::from_accepted_identity(roles.digest()?);
            let name = if tag == 1 { Some(roles.text()?) } else { None };
            let value = self.expression()?;
            args.push(match tag {
                0 => crate::task::RuntimeHostArgumentTemplate::positional(identity, value),
                1 => crate::task::RuntimeHostArgumentTemplate::named(
                    identity,
                    name.expect("named role"),
                    value,
                ),
                2 => crate::task::RuntimeHostArgumentTemplate::spread(identity, value),
                tag => {
                    return Err(Error::Tag {
                        kind: "argument",
                        tag,
                    });
                }
            });
        }
        Ok(args)
    }
    fn producer_definition(&mut self) -> Result<crate::task::NeedProducerDefinition, Error> {
        self.counts
            .storage(2 * size_of::<crate::task::NeedProducerDefinition>())?;
        let function =
            RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.metadata.digest()?);
        let tag = self.metadata.byte()?;
        let family = NeedProducerFamily::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "producer family",
            tag,
        })?;
        let contract = NeedProducerContractDigest::from_bytes(self.metadata.digest()?);
        let coordinate = self
            .metadata
            .zero_based("task", self.counts.limits.task_count)?;
        let plan = self.handles.tasks[coordinate as usize].clone();
        let producer_site = NeedProducerSiteDigest::from_bytes(self.metadata.digest()?);
        let handle_tag = self.metadata.byte()?;
        let payload = self.ty()?;
        let handle_type = crate::task::TaskHandleType::from_semantic_tag(
            handle_tag,
            RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
        )
        .ok_or(Error::Tag {
            kind: "task handle type",
            tag: handle_tag,
        })?;
        let tag = self.metadata.byte()?;
        let class = TaskClass::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "task class",
            tag,
        })?;
        let priority = TaskPriority(self.metadata.i32()?);
        let cancel_scope = CancelScopeId(self.text()?);
        let tag = self.metadata.byte()?;
        let policy = TaskPolicy::from_semantic_tag(tag).map_err(|_| Error::Tag {
            kind: "task policy",
            tag,
        })?;
        let debug_label = self.text()?;
        Ok(crate::task::NeedProducerDefinition {
            function,
            family,
            contract,
            plan,
            producer_site,
            handle_type,
            class,
            priority,
            cancel_scope,
            policy,
            debug_label,
        })
    }
    fn template(&mut self) -> Result<RuntimeNeedProducerTemplateSeed, Error> {
        let definition = self.producer_definition()?;
        self.counts.storage(
            size_of::<RuntimeHostTaskRequestTemplateSeed>()
                + size_of::<crate::task::HostTaskRequestTemplate>(),
        )?;
        let capability = HostCapabilityId(self.text()?);
        let operation = self.text()?;
        let before = self.counts.footprint;
        let args = self.arguments()?;
        let units = (self.counts.footprint.nodes - before.nodes)
            .checked_add(self.counts.footprint.elements - before.elements)
            .ok_or(Error::Arithmetic)?;
        self.counts.storage(
            units
                .checked_mul(
                    2 * size_of::<RuntimeLocalSeedId>()
                        + 2 * size_of::<RuntimeLocalDeclarationId>(),
                )
                .ok_or(Error::Arithmetic)?,
        )?;
        Ok(RuntimeNeedProducerTemplateSeed {
            definition,
            request: RuntimeHostTaskRequestTemplateSeed {
                capability,
                operation,
                args,
            },
        })
    }
    fn fields(&mut self) -> Result<Box<[RuntimeRecordFieldSeedId]>, Error> {
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeRecordFieldSeedId>(count)?;
        (0..count)
            .map(|_| {
                Ok(RuntimeRecordFieldSeedId::from_zero_based(
                    self.metadata.u32()?,
                ))
            })
            .collect()
    }
    fn initialization(&mut self) -> Result<RuntimePlaceInitialization, Error> {
        Ok(match self.metadata.byte()? {
            0 => RuntimePlaceInitialization::Initialized,
            1 => RuntimePlaceInitialization::Uninitialized,
            2 => RuntimePlaceInitialization::Conditional,
            tag => {
                return Err(Error::Tag {
                    kind: "place initialization",
                    tag,
                });
            }
        })
    }
    fn assignment(&mut self) -> Result<RuntimeAssignmentSeed, Error> {
        let place = match self.metadata.byte()? {
            0 => RuntimeMutablePlaceSeed::Local(self.local()?),
            1 => RuntimeMutablePlaceSeed::Fields {
                base: self.local()?,
                fields: self.fields()?,
            },
            tag => {
                return Err(Error::Tag {
                    kind: "mutable place",
                    tag,
                });
            }
        };
        let displacement = match self.metadata.byte()? {
            0 => RuntimePlaceDisplacement::Unreachable,
            1 => {
                let initialization = self.initialization()?;
                let count = self.metadata.u32()? as usize;
                self.counts
                    .array::<RuntimeDisplacedField<RuntimeRecordFieldSeedId>>(count)?;
                let mut fields = Vec::with_capacity(count);
                for _ in 0..count {
                    fields.push(RuntimeDisplacedField {
                        fields: self.fields()?,
                        initialization: self.initialization()?,
                    });
                }
                RuntimePlaceDisplacement::Reachable {
                    initialization,
                    fields: fields.into_boxed_slice(),
                }
            }
            tag => {
                return Err(Error::Tag {
                    kind: "place displacement",
                    tag,
                });
            }
        };
        Ok(RuntimeAssignmentSeed {
            place,
            displacement,
        })
    }
    fn iterator(&mut self) -> Result<RuntimeIteratorEvidenceSeed, Error> {
        Ok(match self.metadata.byte()? {
            0 => {
                let family = match self.metadata.byte()? {
                    0 => RuntimeBuiltinIteratorFamily::Range,
                    1 => RuntimeBuiltinIteratorFamily::Seq,
                    2 => RuntimeBuiltinIteratorFamily::Stream,
                    3 => RuntimeBuiltinIteratorFamily::Vec,
                    4 => RuntimeBuiltinIteratorFamily::Array,
                    5 => RuntimeBuiltinIteratorFamily::Slice,
                    6 => RuntimeBuiltinIteratorFamily::TupleHomogeneous,
                    tag => {
                        return Err(Error::Tag {
                            kind: "builtin iterator",
                            tag,
                        });
                    }
                };
                RuntimeIteratorEvidenceSeed::Builtin(RuntimeBuiltinIteratorEvidenceSeed {
                    family,
                    item: self.ty()?,
                    iterator: self.ty()?,
                    next_value: self.ty()?,
                    step: self.ty()?,
                })
            }
            1 => {
                let item = self.ty()?;
                let iterator = self.ty()?;
                let tag = self.metadata.byte()?;
                let mut method = || -> Result<RuntimeTraitMethodSeedId, Error> {
                    let id = self.metadata.positive(
                        "iterator method",
                        self.counts.limits.expression.method_count,
                    )?;
                    Ok(self.handles.expression.methods[id as usize - 1].clone())
                };
                let executable = match tag {
                    0 => RuntimeIteratorWitnessExecutableSeed::TraitCalls {
                        into_iter: method()?,
                        next: method()?,
                    },
                    1 => RuntimeIteratorWitnessExecutableSeed::IdentityIntoIterator {
                        next: method()?,
                    },
                    tag => {
                        return Err(Error::Tag {
                            kind: "iterator witness",
                            tag,
                        });
                    }
                };
                RuntimeIteratorEvidenceSeed::Witness(RuntimeIteratorWitnessEvidenceSeed {
                    item,
                    iterator,
                    executable,
                })
            }
            tag => {
                return Err(Error::Tag {
                    kind: "iterator evidence",
                    tag,
                });
            }
        })
    }
    fn line_operation(&mut self) -> Result<RuntimeFlowOpSeed, Error> {
        let binding = self.optional_pattern()?;
        let tag = self.metadata.byte()?;
        let site = RuntimeLineHandleSiteId::from_zero_based(self.metadata.u32()?);
        let operation = match tag {
            0 => {
                let character = arcweft_character::id::CharacterId::try_new(self.text()?)?;
                let tag = self.metadata.byte()?;
                if tag != 0 {
                    return Err(Error::Tag {
                        kind: "Line handle scope",
                        tag,
                    });
                }
                RuntimeLineOperationSeed::AcquireActor {
                    site,
                    character,
                    scope: crate::line_task::RuntimeLineHandleScope::Line,
                }
            }
            1 => {
                let raw = self.metadata.u32()?;
                let child = RuntimeLineTaskNodeSeedId::from_zero_based(raw as usize)
                    .ok_or(Error::Arithmetic)?;
                let delay = self.expression()?;
                let count = self.metadata.u32()? as usize;
                self.counts.array::<RuntimeScheduledCaptureSeed>(count)?;
                let mut captures = Vec::with_capacity(count);
                for _ in 0..count {
                    captures.push(RuntimeScheduledCaptureSeed {
                        local: self.local()?,
                        value: self.expression()?,
                    });
                }
                RuntimeLineOperationSeed::Schedule {
                    site,
                    delay,
                    child,
                    captures: captures.into_boxed_slice(),
                }
            }
            2 => RuntimeLineOperationSeed::ActorLook {
                site,
                character: arcweft_character::id::CharacterId::try_new(self.text()?)?,
                actor: RuntimeBorrowedLocalSeed::new(self.local()?),
                look: self.expression()?,
                crossfade: self.expression()?,
            },
            3 => RuntimeLineOperationSeed::VoiceHandle { site },
            tag => {
                return Err(Error::Tag {
                    kind: "Line operation",
                    tag,
                });
            }
        };
        Ok(RuntimeFlowOpSeed::LineOperation { binding, operation })
    }
    fn choice(&mut self) -> Result<RuntimeFlowOpSeed, Error> {
        let id = self.optional_text()?;
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeChoiceOptionSeed>(count)?;
        let mut options = Vec::with_capacity(count);
        for _ in 0..count {
            let id = self.optional_text()?;
            let label = self.text()?;
            let target = if self.metadata.flag("choice target")? {
                Some(self.path()?)
            } else {
                None
            };
            let out = if self.metadata.flag("choice output")? {
                Some(crate::line_task::LineOutRequest {
                    label: self.optional_text()?,
                    value: self.text()?,
                })
            } else {
                None
            };
            let count = self.metadata.u32()? as usize;
            self.counts.array::<RuntimeLineEffectSeed>(count)?;
            let effects = (0..count)
                .map(|_| self.effect())
                .collect::<Result<_, Error>>()?;
            options.push(RuntimeChoiceOptionSeed {
                id,
                label,
                target,
                out,
                effects,
            });
        }
        Ok(RuntimeFlowOpSeed::Choice { id, options })
    }
    fn match_arms(&mut self) -> Result<RuntimeFlowOpSeed, Error> {
        let scrutinee = self.expression()?;
        let count = self.metadata.u32()? as usize;
        self.counts.array::<RuntimeFlowMatchArmSeed>(count)?;
        let mut arms = Vec::with_capacity(count);
        for _ in 0..count {
            let pattern = self.pattern()?;
            let guard = if self.metadata.flag("Match guard")? {
                self.counts
                    .storage(size_of::<RuntimeFlowMatchGuardSeed>())?;
                Some(RuntimeFlowMatchGuardSeed {
                    candidate: self.local()?,
                    result: self.ty()?,
                    condition: self.optional_expression()?,
                    copy_locals: self.locals()?.into_boxed_slice(),
                    ops: self.body()?,
                })
            } else {
                None
            };
            arms.push(RuntimeFlowMatchArmSeed {
                pattern,
                guard,
                ops: self.body()?,
            });
        }
        Ok(RuntimeFlowOpSeed::Match { scrutinee, arms })
    }
    fn operation(&mut self, tag: u8) -> Result<RuntimeFlowOpSeed, Error> {
        use RuntimeFlowOpSeed as Op;
        Ok(match tag {
            1 => Op::Let {
                pattern: self.pattern()?,
                expr: self.expression()?,
            },
            2 => {
                let attempt = self.metadata.zero_based(
                    "format attempt",
                    self.counts.limits.expression.attempt_count,
                )?;
                let parameter = self.metadata.u32()?;
                Op::FormatOperandAttempt {
                    attempt: self.handles.expression.attempts[attempt as usize].clone(),
                    parameter: RuntimeFmtParameterId::from_index(parameter as usize).ok_or(
                        Error::Reference {
                            kind: "format parameter",
                            actual: parameter,
                            maximum: 9,
                        },
                    )?,
                    value: self.expression()?,
                    body: self.body()?,
                }
            }
            4 => Op::LetElse {
                pattern: self.pattern()?,
                expr: self.expression()?,
                else_ops: self.body()?,
            },
            5 => Op::Assign {
                place: self.assignment()?,
                value: self.expression()?,
            },
            6 => self.line_operation()?,
            7 => Op::CommitDialogueResult {
                value: self.expression()?,
            },
            8 => Op::SelectDialogueResult {
                value: self.expression()?,
            },
            9 => {
                let content = self
                    .metadata
                    .positive("content plan", self.counts.limits.content_count)?;
                let ty = self.ty()?;
                let target = self.expression()?;
                let result = RuntimeDialogueResultTargetSeed::try_new(ty, self.pattern()?)?;
                Op::Dialogue {
                    target,
                    content: self.handles.contents[content as usize - 1].clone(),
                    result,
                }
            }
            10 => self.choice()?,
            11 => {
                let binding = self.optional_pattern()?;
                let target = RuntimeAwaitTargetSeed {
                    source: self.expression()?,
                };
                let count = self.metadata.u32()? as usize;
                self.counts
                    .array::<RuntimeAwaitPendingObserverSeed>(count)?;
                let mut observers = Vec::with_capacity(count);
                for _ in 0..count {
                    observers.push(RuntimeAwaitPendingObserverSeed {
                        pattern: self.pattern()?,
                        ops: self.body()?,
                    });
                }
                Op::Await {
                    binding,
                    target,
                    observers,
                }
            }
            12 => {
                let bytes = self.metadata.frame()?;
                let child = PreparedRuntimeSelectedProducerSource::preflight(
                    bytes,
                    self.counts.selected(bytes.len()),
                    self.resources,
                    &mut self.source.meter,
                )?;
                self.counts
                    .child(child.node_count(), child.elements(), child.storage_bytes())?;
                child.materialize(
                    self.source,
                    &self.handles.tasks[child.task_ordinal() as usize],
                    self.handles.expression,
                    self.resources,
                )?
            }
            13 => {
                let binding = self.optional_pattern()?;
                let target = RuntimeAwaitManyTargetSeed {
                    source: self.expression()?,
                    item_binding: self.local()?,
                    limit: self.metadata.u32()?,
                    base: self.template()?,
                    child: self.template()?,
                };
                let count = self.metadata.u32()? as usize;
                self.counts.array::<RuntimeLineEffectSeed>(count)?;
                let pending = (0..count)
                    .map(|_| self.effect())
                    .collect::<Result<_, Error>>()?;
                Op::AwaitMany {
                    binding,
                    target,
                    pending,
                }
            }
            14 => {
                let bytes = self.metadata.frame()?;
                let child = PreparedRuntimeHostCallSource::preflight(
                    bytes,
                    self.counts.host(bytes.len()),
                    self.resources,
                    &mut self.source.meter,
                )?;
                self.counts
                    .child(child.node_count(), child.elements(), child.storage_bytes())?;
                child.materialize(
                    self.source,
                    &self.handles.tasks[child.task_ordinal() as usize],
                    self.handles.expression,
                    self.resources,
                )?
            }
            15 => {
                let ordinal = self.metadata.u32()?;
                let site = self
                    .source
                    .project(ordinal)
                    .ok_or(Error::Reference {
                        kind: "project",
                        actual: ordinal,
                        maximum: self.counts.limits.project_count,
                    })?
                    .clone();
                self.source
                    .builder
                    .project_call_sites
                    .borrow()
                    .vacant(&self.source.builder.issuer, &site)
                    .map_err(RuntimePlanBuildError::from)?;
                let bytes = self.metadata.frame()?;
                let child = PreparedRuntimeProjectCallSource::preflight(
                    bytes,
                    self.counts.project(bytes.len()),
                    self.resources,
                    &mut self.source.meter,
                )?;
                self.counts
                    .child(child.node_count(), child.elements(), child.storage_bytes())?;
                let mut operation =
                    child.materialize(self.source, self.handles.expression, self.resources)?;
                let RuntimeFlowOpSeed::ProjectCall { plan, .. } = &mut operation else {
                    unreachable!("project Source owns its operation")
                };
                plan.placement = crate::plan::RuntimeProjectCallSitePlacementSeed::Reserved(site);
                operation
            }
            16 => {
                let callee = self.expression()?;
                let count = self.metadata.u32()? as usize;
                self.counts.array::<RuntimeCallArgumentSeed>(count)?;
                let mut args = Vec::with_capacity(count);
                for _ in 0..count {
                    let mode = match self.metadata.byte()? {
                        0 => RuntimeCallArgumentMode::Value,
                        1 => RuntimeCallArgumentMode::Spread,
                        tag => {
                            return Err(Error::Tag {
                                kind: "call argument mode",
                                tag,
                            });
                        }
                    };
                    let position = self.metadata.u32()?;
                    args.push(RuntimeCallArgumentSeed::new(
                        self.expression()?,
                        mode,
                        position,
                    ));
                }
                Op::ApplyFunction {
                    callee,
                    args: args.into_boxed_slice(),
                    result: self.pattern()?,
                }
            }
            17 => Op::If {
                condition: self.expression()?,
                then_ops: self.body()?,
                else_ops: self.body()?,
            },
            18 => Op::IfLet {
                pattern: self.pattern()?,
                expr: self.expression()?,
                guard: self.optional_expression()?,
                then_ops: self.body()?,
                else_ops: self.body()?,
            },
            19 => self.match_arms()?,
            20 => Op::Loop {
                result: self.optional_pattern()?,
                body: self.body()?,
            },
            22 => Op::While {
                condition: self.expression()?,
                body: self.body()?,
            },
            24 => Op::WhileLet {
                pattern: self.pattern()?,
                expr: self.expression()?,
                guard: self.optional_expression()?,
                body: self.body()?,
            },
            26 => Op::For {
                pattern: self.pattern()?,
                source: self.expression()?,
                evidence: self.iterator()?,
                body: self.body()?,
            },
            28 => Op::ProgramTask {
                name: self.optional_text()?,
                binding: self.pattern()?,
                producer: self.producer_definition()?,
                captures: self.locals()?,
                body: self.body()?,
            },
            29 => Op::Scope {
                identity: self.scope()?,
                body: self.body()?,
            },
            31 => Op::Break(self.optional_expression()?),
            32 => Op::Continue,
            33 => Op::Goto(self.path()?),
            34 => Op::GotoExpr(self.expression()?),
            35 => Op::Return(self.text()?),
            36 => Op::ReturnExpr(self.expression()?),
            37 => Op::Effect(self.effect()?),
            38 => {
                let bytes = self.metadata.frame()?;
                let child = PreparedRuntimeEvaluatedEffectSource::preflight(
                    bytes,
                    self.counts.evaluated(bytes.len()),
                    self.resources,
                    &mut self.source.meter,
                )?;
                self.counts
                    .child(child.node_count(), child.elements(), child.storage_bytes())?;
                Op::EvaluatedEffect(child.materialize(
                    self.source,
                    self.handles.expression,
                    self.resources,
                )?)
            }
            39 => {
                let site = self
                    .metadata
                    .zero_based("defer site", self.counts.limits.defer_count)?;
                let outcome = match self.metadata.byte()? {
                    0 => crate::line_task::RuntimeDeferOutcomeFilter::Always,
                    1 => crate::line_task::RuntimeDeferOutcomeFilter::Completed,
                    2 => crate::line_task::RuntimeDeferOutcomeFilter::Cancelled,
                    3 => crate::line_task::RuntimeDeferOutcomeFilter::Failed,
                    tag => {
                        return Err(Error::Tag {
                            kind: "defer outcome",
                            tag,
                        });
                    }
                };
                let owner = match self.metadata.byte()? {
                    0 => RuntimeDeferOwner::CurrentScope,
                    1 => RuntimeDeferOwner::LineRoot,
                    tag => {
                        return Err(Error::Tag {
                            kind: "defer owner",
                            tag,
                        });
                    }
                };
                let count = self.metadata.u32()? as usize;
                self.counts.array::<RuntimeExprSeed>(count)?;
                let captures = (0..count)
                    .map(|_| self.expression())
                    .collect::<Result<_, Error>>()?;
                Op::RegisterDefer {
                    site: self.handles.defers[site as usize],
                    outcome,
                    owner,
                    captures,
                }
            }
            40 => Op::RegisterCleanup {
                key: self.text()?,
                effect: self.effect()?,
            },
            41 => Op::CancelCleanup { key: self.text()? },
            42 => Op::EnterScope {
                identity: self.scope()?,
            },
            43 => Op::ExitScope,
            45 => Op::ExitScopeBind {
                pattern: self.pattern()?,
                expr: self.expression()?,
            },
            46 => Op::Noop,
            49 => {
                let tag = self.metadata.byte()?;
                let family =
                    RuntimeStandardMapFamily::from_semantic_tag(tag).ok_or(Error::Tag {
                        kind: "Map family",
                        tag,
                    })?;
                let source = self.local()?;
                let mapping = self.local()?;
                let result = self.local()?;
                let completion = self.metadata.byte()?;
                let boundary = if completion == 1 {
                    Some(self.metadata.u32()? as usize)
                } else {
                    None
                };
                let mut body = self.body()?;
                let control = if let Some(boundary) = boundary {
                    if boundary > body.len() {
                        return Err(Error::Prefix);
                    }
                    let consumption = body.split_off(boundary);
                    RuntimeStandardMapControlSeed::sum(
                        family,
                        source,
                        mapping,
                        result,
                        body,
                        consumption,
                    )
                } else {
                    RuntimeStandardMapControlSeed::collection(family, source, mapping, result, body)
                };
                Op::StandardMap(control)
            }
            tag => {
                return Err(Error::Tag {
                    kind: "operation",
                    tag,
                });
            }
        })
    }
}
