//! One-way final-HIR dialogue line-plan projection.
//!
//! This module owns the construction-only draft needed to correlate scheduled
//! operations, callback nodes, and handle sites before the core builder seals
//! their dense runtime identities. It is not retained in `RuntimePlan`.

use std::collections::BTreeSet;

use arcweft_core::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineCleanupPolicy, ParallelPolicy, RuntimeLineHandleScope,
    RuntimeLineHandleSiteKind,
};
use arcweft_core::plan::{
    RuntimeDialogueContentPlanSeedId, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed,
    RuntimeLineHandleSiteSeed, RuntimeLineOperationSeed, RuntimeLineTaskCancelRuleSeed,
    RuntimeLineTaskGroupSeed, RuntimeLineTaskNodeSeed, RuntimeLineTaskNodeSeedId,
    RuntimeLineTaskTriggerSeed, RuntimeLocalSeedId, RuntimePatternSeed,
    RuntimeScheduledCaptureSeed,
};
use arcweft_core::runtime_id::RuntimeLineHandleSiteId;
use arcweft_core::value::RuntimeValue;
use arcweft_lang_hir::dialogue_application::HirLinePlanItem;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::identity::{ExprId, LocalId, StmtId};
use arcweft_lang_hir::stmt::{HirStmtKind, HirTriggerPattern};
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;

use crate::assertion_identity::RuntimeAssertionSite;
use crate::errors::RuntimePlanLowerError;
use crate::final_pattern::FinalPatternLowerer;
use crate::semantic_facts::{
    RuntimeDialogueEffectTrigger, RuntimeLineCallable, RuntimeNormalizedType, RuntimeResolvedCall,
    RuntimeResolvedCallDispatch, RuntimeResolvedStaticCallTarget, RuntimeTypeShape,
};

use super::{FinalFlowLowerer, FinalLoweringContext, RuntimeAssertionOwner, module_by_id};

#[derive(Clone, Copy)]
struct SiteDraftId(usize);

struct SiteDraft {
    kind: RuntimeLineHandleSiteKind,
    result_type: arcweft_core::pattern::RuntimeSemanticTypeId,
    character: Option<arcweft_character::id::CharacterId>,
    scheduled_child: Option<ChildDraftId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ChildDraftId(usize);

#[derive(Clone)]
enum LineOperationDraft {
    AcquireActor {
        site: SiteDraftId,
        character: arcweft_character::id::CharacterId,
    },
    Schedule {
        site: SiteDraftId,
        delay: RuntimeExprSeed,
        child: ChildDraftId,
        captures: Box<[RuntimeScheduledCaptureSeed]>,
    },
    ActorLook {
        site: SiteDraftId,
        character: arcweft_character::id::CharacterId,
        actor: RuntimeExprSeed,
        look: RuntimeExprSeed,
        crossfade: RuntimeExprSeed,
    },
    VoiceHandle {
        site: SiteDraftId,
    },
}

#[derive(Clone)]
enum FlowDraft {
    Flow(RuntimeFlowOpSeed),
    LineOperation {
        binding: Option<RuntimePatternSeed>,
        operation: LineOperationDraft,
    },
}

enum TriggerDraft {
    Immediate,
    Mark(arcweft_core::plan::RuntimeDialogueMarkSeedId),
    ContentEffect(arcweft_core::plan::RuntimeDialogueEffectSiteSeedId),
    Scheduled(SiteDraftId),
}

enum NodeDraft {
    Sequence(Vec<Self>),
    Start(Vec<Self>),
    Parallel {
        policy: ParallelPolicy,
        children: Vec<Self>,
    },
    Child {
        id: ChildDraftId,
        trigger: TriggerDraft,
        join_policy: ChildJoinPolicy,
        cancel_policy: ChildCancelPolicy,
        scope: Box<Self>,
    },
    Action(Vec<FlowDraft>),
}

struct CancelRuleDraft {
    trigger: arcweft_core::plan::RuntimeDialogueMarkSeedId,
    action: Vec<FlowDraft>,
}

struct LinePlanLowerer<'a, 'project, 'data> {
    context: &'a FinalLoweringContext<'project, 'data>,
    module: &'project arcweft_lang_hir::module::HirModule,
    flow: FinalFlowLowerer<'a>,
    content: &'a RuntimeDialogueContentPlanSeedId,
    owner: ExprId,
    sites: Vec<SiteDraft>,
    activation_ops: Vec<FlowDraft>,
    root_children: Vec<NodeDraft>,
    cancel_rules: Vec<CancelRuleDraft>,
    cleanup_completed: Vec<FlowDraft>,
    cleanup_cancelled: Vec<FlowDraft>,
    cleanup_failed: Vec<FlowDraft>,
    next_child: usize,
    committed_result: bool,
}

pub(super) fn lower_dialogue_line_plan<'a, 'project, 'data>(
    context: &'a FinalLoweringContext<'project, 'data>,
    owner: ExprId,
    content: &'a RuntimeDialogueContentPlanSeedId,
) -> Result<(RuntimeLineTaskGroupSeed, Vec<RuntimeAssertionSite>), RuntimePlanLowerError> {
    let module = module_by_id(context.project, owner.module()).ok_or_else(|| {
        RuntimePlanLowerError::new(format!("dialogue application {owner:?} module is absent"))
    })?;
    let expression = module.resolve_expr(owner).map_err(|error| {
        RuntimePlanLowerError::new(format!(
            "cannot resolve dialogue application {owner:?}: {error}"
        ))
    })?;
    let HirExprKind::DialogueContentApplication(dialogue) = expression.kind() else {
        return Err(RuntimePlanLowerError::new(format!(
            "dialogue fact owner {owner:?} is not a final-HIR dialogue application"
        )));
    };
    let application = context.facts.dialogue_application(owner).ok_or_else(|| {
        RuntimePlanLowerError::new(format!(
            "dialogue application {owner:?} has no checked runtime projection"
        ))
    })?;
    let mut lowerer = LinePlanLowerer {
        context,
        module,
        flow: FinalFlowLowerer::new(
            module,
            context,
            RuntimeAssertionOwner::Line(application.content().line().clone()),
        ),
        content,
        owner,
        sites: Vec::new(),
        activation_ops: Vec::new(),
        root_children: Vec::new(),
        cancel_rules: Vec::new(),
        cleanup_completed: Vec::new(),
        cleanup_cancelled: Vec::new(),
        cleanup_failed: Vec::new(),
        next_child: 0,
        committed_result: false,
    };
    lowerer.lower_dialogue_effects(application.effects())?;
    if let Some(plan) = dialogue.plan() {
        lowerer.lower_items(plan.items())?;
    }
    if !lowerer.committed_result {
        if !matches!(application.line_result().shape(), RuntimeTypeShape::Unit) {
            return Err(RuntimePlanLowerError::new(format!(
                "dialogue application {owner:?} has a non-Unit line result without an out commit"
            )));
        }
        lowerer
            .activation_ops
            .push(FlowDraft::Flow(RuntimeFlowOpSeed::CommitDialogueResult {
                value: RuntimeExprSeed::new(
                    application.line_result().identity(),
                    RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                ),
            }));
    }
    lowerer.finish(application.line_result())
}

impl LinePlanLowerer<'_, '_, '_> {
    fn lower_dialogue_effects(
        &mut self,
        effects: &[crate::semantic_facts::RuntimeDialogueEffectExpression],
    ) -> Result<(), RuntimePlanLowerError> {
        for effect in effects {
            let operation = FlowDraft::Flow(RuntimeFlowOpSeed::EvaluatedEffect(
                super::lower_evaluated_effect(
                    &self.context.expr_lowerer(self.module),
                    effect.operation().effect(),
                )?,
            ));
            match effect.trigger() {
                RuntimeDialogueEffectTrigger::Content => {
                    let trigger = self.content.effect_site(effect.site().index()).ok_or_else(
                        || {
                            RuntimePlanLowerError::new(
                                "dialogue effect-site ordinal exceeds the runtime identity domain",
                            )
                        },
                    )?;
                    let child = self.allocate_child()?;
                    self.root_children.push(NodeDraft::Child {
                        id: child,
                        trigger: TriggerDraft::ContentEffect(trigger),
                        join_policy: ChildJoinPolicy::Join,
                        cancel_policy: ChildCancelPolicy::CancelAndJoin,
                        scope: Box::new(NodeDraft::Action(vec![operation])),
                    });
                }
                RuntimeDialogueEffectTrigger::Delay {
                    duration,
                    schedule_handle_type,
                } => {
                    let child = self.allocate_child()?;
                    let site = self.push_site(
                        RuntimeLineHandleSiteKind::ScheduledCue,
                        schedule_handle_type,
                        None,
                        Some(child),
                    );
                    let actions = vec![operation];
                    let captures = self.flow_captures(&actions)?;
                    self.root_children.push(NodeDraft::Child {
                        id: child,
                        trigger: TriggerDraft::Scheduled(site),
                        join_policy: ChildJoinPolicy::Join,
                        cancel_policy: ChildCancelPolicy::CancelAndJoin,
                        scope: Box::new(NodeDraft::Action(actions)),
                    });
                    self.activation_ops.push(FlowDraft::LineOperation {
                        binding: None,
                        operation: LineOperationDraft::Schedule {
                            site,
                            delay: RuntimeExprSeed::new(
                                arcweft_core::pattern::RuntimeCheckedType::Duration
                                    .semantic_identity_digest(),
                                RuntimeExprSeedKind::Value(RuntimeValue::Duration(*duration)),
                            ),
                            child,
                            captures,
                        },
                    });
                }
            }
        }
        Ok(())
    }

    fn lower_items(&mut self, items: &[HirLinePlanItem]) -> Result<(), RuntimePlanLowerError> {
        for item in items {
            match item {
                HirLinePlanItem::Let { pattern, value, .. } => {
                    let binding = FinalPatternLowerer::new(
                        self.module,
                        self.context.facts,
                        self.context.locals,
                    )
                    .lower(*pattern)
                    .map_err(RuntimePlanLowerError::new)?;
                    if self.line_call(*value)?.is_some() {
                        let operation = self.lower_line_call(*value, Some(binding))?;
                        self.activation_ops.push(operation);
                    } else {
                        self.activation_ops
                            .push(FlowDraft::Flow(RuntimeFlowOpSeed::Let {
                                pattern: binding,
                                expr: self
                                    .context
                                    .expr_lowerer(self.module)
                                    .lower(*value)
                                    .map_err(RuntimePlanLowerError::new)?,
                            }));
                    }
                }
                HirLinePlanItem::Out { value, .. } => {
                    if self.committed_result {
                        return Err(RuntimePlanLowerError::new(format!(
                            "dialogue application {:?} commits its line result more than once",
                            self.owner
                        )));
                    }
                    self.activation_ops.push(FlowDraft::Flow(
                        RuntimeFlowOpSeed::CommitDialogueResult {
                            value: self
                                .context
                                .expr_lowerer(self.module)
                                .lower(*value)
                                .map_err(RuntimePlanLowerError::new)?,
                        },
                    ));
                    self.committed_result = true;
                }
                HirLinePlanItem::Expression(expression) => {
                    if self.line_call(*expression)?.is_some() {
                        let operation = self.lower_line_call(*expression, None)?;
                        self.activation_ops.push(operation);
                    } else if let Some(child) = self.lower_thread_expression(*expression)? {
                        self.root_children.push(child);
                    } else {
                        return Err(RuntimePlanLowerError::new(format!(
                            "line-plan expression {expression:?} has no typed operation disposition"
                        )));
                    }
                }
                HirLinePlanItem::Statement(statement) => {
                    self.lower_top_level_statement(*statement)?;
                }
                HirLinePlanItem::StartGroup(items) => {
                    let children = self.lower_node_items(items)?;
                    self.root_children.push(NodeDraft::Start(children));
                }
                HirLinePlanItem::TogetherGroup(items) => {
                    let children = self.lower_node_items(items)?;
                    self.root_children.push(NodeDraft::Parallel {
                        policy: ParallelPolicy::JoinAll,
                        children,
                    });
                }
                HirLinePlanItem::Init(_)
                | HirLinePlanItem::Thread(_)
                | HirLinePlanItem::On(_)
                | HirLinePlanItem::Option { .. }
                | HirLinePlanItem::CancelRule(_)
                | HirLinePlanItem::TimedCue { .. }
                | HirLinePlanItem::TimelineAssert { .. }
                | HirLinePlanItem::Error(_) => {
                    return Err(RuntimePlanLowerError::new(format!(
                        "line-plan item {item:?} has no complete typed runtime projection"
                    )));
                }
            }
        }
        Ok(())
    }

    fn lower_top_level_statement(
        &mut self,
        statement: StmtId,
    ) -> Result<(), RuntimePlanLowerError> {
        let kind = self.resolve_statement(statement)?.kind().clone();
        match kind {
            HirStmtKind::On { trigger, body, .. } => {
                let trigger = self.mark_trigger(statement, &trigger)?;
                let actions = self.lower_statement_actions(&body)?;
                let id = self.allocate_child()?;
                self.root_children.push(NodeDraft::Child {
                    id,
                    trigger: TriggerDraft::Mark(trigger),
                    join_policy: ChildJoinPolicy::Join,
                    cancel_policy: ChildCancelPolicy::CancelAndJoin,
                    scope: Box::new(NodeDraft::Action(actions)),
                });
            }
            HirStmtKind::DeferBlock { outcome, body, .. } => {
                let actions = self.lower_statement_actions(&body)?;
                self.register_cleanup(outcome, actions);
            }
            HirStmtKind::Defer { .. } => {
                return Err(RuntimePlanLowerError::new(format!(
                    "line-plan defer expression {statement:?} has no expression-owned checked effect disposition"
                )));
            }
            HirStmtKind::Expression { expression } => {
                if let Some(child) = self.lower_thread_expression(expression)? {
                    self.root_children.push(child);
                } else {
                    let actions = self.lower_statement_action(statement, &kind)?;
                    self.activation_ops.extend(actions);
                }
            }
            _ => {
                let actions = self.lower_statement_action(statement, &kind)?;
                self.activation_ops.extend(actions);
            }
        }
        Ok(())
    }

    fn lower_node_items(
        &mut self,
        items: &[HirLinePlanItem],
    ) -> Result<Vec<NodeDraft>, RuntimePlanLowerError> {
        let mut nodes = Vec::new();
        for item in items {
            match item {
                HirLinePlanItem::Let { pattern, value, .. } => {
                    let binding = FinalPatternLowerer::new(
                        self.module,
                        self.context.facts,
                        self.context.locals,
                    )
                    .lower(*pattern)
                    .map_err(RuntimePlanLowerError::new)?;
                    let action = if self.line_call(*value)?.is_some() {
                        self.lower_line_call(*value, Some(binding))?
                    } else {
                        FlowDraft::Flow(RuntimeFlowOpSeed::Let {
                            pattern: binding,
                            expr: self
                                .context
                                .expr_lowerer(self.module)
                                .lower(*value)
                                .map_err(RuntimePlanLowerError::new)?,
                        })
                    };
                    nodes.push(NodeDraft::Action(vec![action]));
                }
                HirLinePlanItem::Expression(expression) => {
                    if self.line_call(*expression)?.is_some() {
                        nodes.push(NodeDraft::Action(vec![
                            self.lower_line_call(*expression, None)?,
                        ]));
                    } else if let Some(child) = self.lower_thread_expression(*expression)? {
                        nodes.push(child);
                    } else {
                        return Err(RuntimePlanLowerError::new(format!(
                            "line-plan grouped expression {expression:?} has no typed operation disposition"
                        )));
                    }
                }
                HirLinePlanItem::Statement(statement) => {
                    let kind = self.resolve_statement(*statement)?.kind().clone();
                    match kind {
                        HirStmtKind::On { trigger, body, .. } => {
                            let trigger = self.mark_trigger(*statement, &trigger)?;
                            let actions = self.lower_statement_actions(&body)?;
                            let id = self.allocate_child()?;
                            nodes.push(NodeDraft::Child {
                                id,
                                trigger: TriggerDraft::Mark(trigger),
                                join_policy: ChildJoinPolicy::Join,
                                cancel_policy: ChildCancelPolicy::CancelAndJoin,
                                scope: Box::new(NodeDraft::Action(actions)),
                            });
                        }
                        HirStmtKind::Expression { expression } => {
                            if let Some(child) = self.lower_thread_expression(expression)? {
                                nodes.push(child);
                            } else {
                                nodes.push(NodeDraft::Action(
                                    self.lower_statement_action(*statement, &kind)?,
                                ));
                            }
                        }
                        HirStmtKind::DeferBlock { .. } | HirStmtKind::Defer { .. } => {
                            return Err(RuntimePlanLowerError::new(format!(
                                "nested line-plan cleanup {statement:?} requires scope-owned cleanup topology"
                            )));
                        }
                        _ => nodes.push(NodeDraft::Action(
                            self.lower_statement_action(*statement, &kind)?,
                        )),
                    }
                }
                HirLinePlanItem::StartGroup(children) => {
                    nodes.push(NodeDraft::Start(self.lower_node_items(children)?));
                }
                HirLinePlanItem::TogetherGroup(children) => {
                    nodes.push(NodeDraft::Parallel {
                        policy: ParallelPolicy::JoinAll,
                        children: self.lower_node_items(children)?,
                    });
                }
                HirLinePlanItem::Out { .. } => {
                    return Err(RuntimePlanLowerError::new(
                        "a child line-task scope cannot commit the dialogue result",
                    ));
                }
                HirLinePlanItem::Error(statement) => {
                    return Err(RuntimePlanLowerError::new(format!(
                        "recovered line-plan statement {statement:?} cannot enter runtime lowering"
                    )));
                }
                HirLinePlanItem::Init(_)
                | HirLinePlanItem::Thread(_)
                | HirLinePlanItem::On(_)
                | HirLinePlanItem::Option { .. }
                | HirLinePlanItem::CancelRule(_)
                | HirLinePlanItem::TimedCue { .. }
                | HirLinePlanItem::TimelineAssert { .. } => {
                    return Err(RuntimePlanLowerError::new(format!(
                        "shadow line-plan item {item:?} reached final runtime lowering"
                    )));
                }
            }
        }
        Ok(nodes)
    }

    fn resolve_statement(
        &self,
        statement: StmtId,
    ) -> Result<&arcweft_lang_hir::stmt::HirStmt, RuntimePlanLowerError> {
        self.module.resolve_stmt(statement).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve line-plan statement {statement:?}: {error}"
            ))
        })
    }

    fn lower_statement_actions(
        &mut self,
        statements: &[StmtId],
    ) -> Result<Vec<FlowDraft>, RuntimePlanLowerError> {
        let mut actions = Vec::new();
        for statement in statements {
            let kind = self.resolve_statement(*statement)?.kind().clone();
            actions.extend(self.lower_statement_action(*statement, &kind)?);
        }
        Ok(actions)
    }

    fn lower_statement_action(
        &mut self,
        statement: StmtId,
        kind: &HirStmtKind,
    ) -> Result<Vec<FlowDraft>, RuntimePlanLowerError> {
        match kind {
            HirStmtKind::Let {
                pattern,
                initializer,
                ..
            } if self.line_call(*initializer)?.is_some() => {
                let binding =
                    FinalPatternLowerer::new(self.module, self.context.facts, self.context.locals)
                        .lower(*pattern)
                        .map_err(RuntimePlanLowerError::new)?;
                Ok(vec![self.lower_line_call(*initializer, Some(binding))?])
            }
            HirStmtKind::Expression { expression } if self.line_call(*expression)?.is_some() => {
                Ok(vec![self.lower_line_call(*expression, None)?])
            }
            HirStmtKind::On { .. }
            | HirStmtKind::Defer { .. }
            | HirStmtKind::DeferBlock { .. }
            | HirStmtKind::Out { .. } => Err(RuntimePlanLowerError::new(format!(
                "line-plan statement {statement:?} requires a scope-owned disposition"
            ))),
            _ => self
                .flow
                .lower_statement(statement, kind)
                .map(|ops| ops.into_iter().map(FlowDraft::Flow).collect()),
        }
    }

    fn lower_thread_expression(
        &mut self,
        expression: ExprId,
    ) -> Result<Option<NodeDraft>, RuntimePlanLowerError> {
        let expression = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve line-plan thread expression {expression:?}: {error}"
            ))
        })?;
        let HirExprKind::Thread(thread) = expression.kind() else {
            return Ok(None);
        };
        if thread.mode() == arcweft_lang_hir::expr::HirThreadMode::Detached {
            return Err(RuntimePlanLowerError::new(
                "detached line-plan child requires checked detached capture ownership",
            ));
        }
        let actions = self
            .flow
            .lower_body(thread.body())
            .map_err(|errors| {
                RuntimePlanLowerError::new(
                    errors
                        .into_iter()
                        .map(|error| error.to_string())
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            })?
            .into_iter()
            .map(FlowDraft::Flow)
            .collect();
        let id = self.allocate_child()?;
        Ok(Some(NodeDraft::Child {
            id,
            trigger: TriggerDraft::Immediate,
            join_policy: ChildJoinPolicy::Join,
            cancel_policy: ChildCancelPolicy::CancelAndJoin,
            scope: Box::new(NodeDraft::Action(actions)),
        }))
    }

    fn mark_trigger(
        &self,
        statement: StmtId,
        trigger: &HirTriggerPattern,
    ) -> Result<arcweft_core::plan::RuntimeDialogueMarkSeedId, RuntimePlanLowerError> {
        let HirTriggerPattern::Mark(_) = trigger else {
            return Err(RuntimePlanLowerError::new(
                "line-task event handlers currently require one checked dialogue mark trigger",
            ));
        };
        let application = self
            .context
            .facts
            .dialogue_application(self.owner)
            .ok_or_else(|| {
                RuntimePlanLowerError::new("dialogue application fact disappeared during lowering")
            })?;
        let mut matches = application
            .mark_handlers()
            .iter()
            .filter(|handler| handler.statement() == statement);
        let handler = matches.next().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "line-plan mark handler {statement:?} has no checked content coordinate"
            ))
        })?;
        if matches.next().is_some() {
            return Err(RuntimePlanLowerError::new(format!(
                "line-plan mark handler {statement:?} repeats its checked content coordinate"
            )));
        }
        let mark_index = usize::try_from(handler.ordinal()).map_err(|_| {
            RuntimePlanLowerError::new("checked dialogue mark ordinal does not fit usize")
        })?;
        self.content.mark(mark_index).ok_or_else(|| {
            RuntimePlanLowerError::new("dialogue mark count exceeds the runtime identity domain")
        })
    }

    fn register_cleanup(&mut self, outcome: DeferOutcome, actions: Vec<FlowDraft>) {
        match outcome {
            DeferOutcome::Always => {
                self.cleanup_completed.extend(actions.iter().cloned());
                self.cleanup_cancelled.extend(actions.iter().cloned());
                self.cleanup_failed.extend(actions);
            }
            DeferOutcome::Completed => self.cleanup_completed.extend(actions),
            DeferOutcome::Cancelled => self.cleanup_cancelled.extend(actions),
            DeferOutcome::Failed => self.cleanup_failed.extend(actions),
        }
    }

    fn allocate_child(&mut self) -> Result<ChildDraftId, RuntimePlanLowerError> {
        let id = ChildDraftId(self.next_child);
        self.next_child = self
            .next_child
            .checked_add(1)
            .ok_or_else(|| RuntimePlanLowerError::new("line-task child draft count overflow"))?;
        Ok(id)
    }

    fn line_call(
        &self,
        expression: ExprId,
    ) -> Result<Option<(&RuntimeResolvedCall, &RuntimeLineCallable)>, RuntimePlanLowerError> {
        let Some(call) = self.context.facts.call(expression) else {
            return Ok(None);
        };
        match call.dispatch() {
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Line(line)) => {
                Ok(Some((call, line)))
            }
            _ => Ok(None),
        }
    }

    fn lower_line_call(
        &mut self,
        expression: ExprId,
        binding: Option<RuntimePatternSeed>,
    ) -> Result<FlowDraft, RuntimePlanLowerError> {
        let (_call, line) = self.line_call(expression)?.ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "line-plan expression {expression:?} is not a checked line operation"
            ))
        })?;
        let line = line.clone();
        let result = self
            .context
            .facts
            .expression_type(expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "line operation {expression:?} has no accepted result type"
                ))
            })?;
        let operation = match line {
            RuntimeLineCallable::AcquireActor { character } => {
                let site = self.push_site(
                    RuntimeLineHandleSiteKind::StageActor,
                    result,
                    Some(character.clone()),
                    None,
                );
                LineOperationDraft::AcquireActor { site, character }
            }
            RuntimeLineCallable::VoiceHandle => {
                let site = self.push_site(RuntimeLineHandleSiteKind::Voice, result, None, None);
                LineOperationDraft::VoiceHandle { site }
            }
            RuntimeLineCallable::ActorLook {
                character,
                actor,
                look,
                crossfade,
            } => {
                let actor = self
                    .context
                    .expr_lowerer(self.module)
                    .lower(actor)
                    .map_err(RuntimePlanLowerError::new)?;
                let look = self
                    .context
                    .expr_lowerer(self.module)
                    .lower(look)
                    .map_err(RuntimePlanLowerError::new)?;
                let crossfade = self
                    .context
                    .expr_lowerer(self.module)
                    .lower(crossfade)
                    .map_err(RuntimePlanLowerError::new)?;
                let site = self.push_site(
                    RuntimeLineHandleSiteKind::StageLookCue,
                    result,
                    Some(character.clone()),
                    None,
                );
                LineOperationDraft::ActorLook {
                    site,
                    character,
                    actor,
                    look,
                    crossfade,
                }
            }
            RuntimeLineCallable::Schedule { anchor, callback } => {
                let delay = self
                    .context
                    .expr_lowerer(self.module)
                    .lower(anchor)
                    .map_err(RuntimePlanLowerError::new)?;
                let accepted_anchor =
                    self.context.facts.expression_type(anchor).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "schedule anchor {anchor:?} has no accepted runtime type"
                        ))
                    })?;
                if !matches!(accepted_anchor.shape(), RuntimeTypeShape::Duration)
                    || delay.ty() != accepted_anchor.identity()
                {
                    return Err(RuntimePlanLowerError::new(format!(
                        "schedule anchor {anchor:?} lost its accepted Duration type"
                    )));
                }
                let accepted_callback =
                    self.context
                        .facts
                        .expression_type(callback)
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "schedule callback {callback:?} has no accepted runtime type"
                            ))
                        })?;
                if !matches!(accepted_callback.shape(), RuntimeTypeShape::Function { .. }) {
                    return Err(RuntimePlanLowerError::new(format!(
                        "schedule callback {callback:?} lost its accepted function type"
                    )));
                }
                let child = self.allocate_child()?;
                let site = self.push_site(
                    RuntimeLineHandleSiteKind::ScheduledCue,
                    result,
                    None,
                    Some(child),
                );
                let actions = self.lower_callback(callback)?;
                let captures = self.callback_captures(callback, &actions)?;
                self.root_children.push(NodeDraft::Child {
                    id: child,
                    trigger: TriggerDraft::Scheduled(site),
                    join_policy: ChildJoinPolicy::Join,
                    cancel_policy: ChildCancelPolicy::CancelAndJoin,
                    scope: Box::new(NodeDraft::Action(actions)),
                });
                LineOperationDraft::Schedule {
                    site,
                    delay,
                    child,
                    captures,
                }
            }
        };
        Ok(FlowDraft::LineOperation { binding, operation })
    }

    fn lower_callback(
        &mut self,
        expression: ExprId,
    ) -> Result<Vec<FlowDraft>, RuntimePlanLowerError> {
        let hir = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve scheduled callback {expression:?}: {error}"
            ))
        })?;
        match hir.kind() {
            HirExprKind::Call(_) if self.line_call(expression)?.is_some() => {
                Ok(vec![self.lower_line_call(expression, None)?])
            }
            HirExprKind::Block(block) => self.lower_callback_body(block.statements(), block.tail()),
            HirExprKind::NamedBlock(block) => {
                self.lower_callback_body(block.statements(), block.tail())
            }
            HirExprKind::ComputationBlock(block) => {
                self.lower_callback_body(block.statements(), block.tail())
            }
            HirExprKind::Closure(closure) => self.lower_callback_body(&[], closure.body()),
            _ => Err(RuntimePlanLowerError::new(format!(
                "scheduled callback {expression:?} has no typed line action body"
            ))),
        }
    }

    fn lower_callback_body(
        &mut self,
        statements: &[arcweft_lang_hir::identity::StmtId],
        tail: ExprId,
    ) -> Result<Vec<FlowDraft>, RuntimePlanLowerError> {
        let mut actions = Vec::new();
        for statement in statements {
            let statement = self.module.resolve_stmt(*statement).map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "cannot resolve scheduled callback statement {statement:?}: {error}"
                ))
            })?;
            let HirStmtKind::Expression { expression } = statement.kind() else {
                return Err(RuntimePlanLowerError::new(format!(
                    "scheduled callback statement {statement:?} has no typed line action projection"
                )));
            };
            actions.push(self.lower_line_call(*expression, None)?);
        }
        if !matches!(
            self.module.resolve_expr(tail).map(|expr| expr.kind()),
            Ok(HirExprKind::Unit)
        ) {
            actions.push(self.lower_line_call(tail, None)?);
        }
        Ok(actions)
    }

    fn callback_captures(
        &self,
        callback: ExprId,
        actions: &[FlowDraft],
    ) -> Result<Box<[RuntimeScheduledCaptureSeed]>, RuntimePlanLowerError> {
        let mut locals = Vec::new();
        if let Some(callable) = self.context.facts.implicit_callable(callback) {
            locals.extend_from_slice(callable.captures());
        } else {
            return self.flow_captures(actions);
        }
        let mut seen = BTreeSet::new();
        locals
            .into_iter()
            .filter(|local| seen.insert(*local))
            .map(|local| self.scheduled_capture(local))
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn flow_captures(
        &self,
        actions: &[FlowDraft],
    ) -> Result<Box<[RuntimeScheduledCaptureSeed]>, RuntimePlanLowerError> {
        let mut seeds = Vec::<RuntimeLocalSeedId>::new();
        for action in actions {
            action.collect_free_locals(&mut seeds);
        }
        let mut seen = BTreeSet::new();
        seeds
            .into_iter()
            .map(|seed| {
                self.context
                    .locals
                    .iter()
                    .find_map(|(local, candidate)| (candidate == &seed).then_some(*local))
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(
                            "scheduled action capture has no accepted HIR local owner",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|local| seen.insert(*local))
            .map(|local| self.scheduled_capture(local))
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn scheduled_capture(
        &self,
        local: LocalId,
    ) -> Result<RuntimeScheduledCaptureSeed, RuntimePlanLowerError> {
        let seed = self.context.locals.get(&local).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "scheduled callback capture {local:?} has no admitted runtime local"
            ))
        })?;
        let ty = self.context.facts.local_type(local).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "scheduled callback capture {local:?} has no accepted type"
            ))
        })?;
        Ok(RuntimeScheduledCaptureSeed {
            local: seed.clone(),
            value: RuntimeExprSeed::new(ty.identity(), RuntimeExprSeedKind::Local(seed)),
        })
    }

    fn push_site(
        &mut self,
        kind: RuntimeLineHandleSiteKind,
        result: &RuntimeNormalizedType,
        character: Option<arcweft_character::id::CharacterId>,
        scheduled_child: Option<ChildDraftId>,
    ) -> SiteDraftId {
        let id = SiteDraftId(self.sites.len());
        self.sites.push(SiteDraft {
            kind,
            result_type: result.identity(),
            character,
            scheduled_child,
        });
        id
    }

    fn finish(
        self,
        result: &RuntimeNormalizedType,
    ) -> Result<(RuntimeLineTaskGroupSeed, Vec<RuntimeAssertionSite>), RuntimePlanLowerError> {
        let root = NodeDraft::Sequence(vec![NodeDraft::Start(self.root_children)]);
        let mut next_node = 0_usize;
        let mut child_nodes = std::collections::BTreeMap::new();
        assign_node_ids(&root, &mut next_node, &mut child_nodes)?;
        let resolve_site =
            |site: SiteDraftId| -> Result<RuntimeLineHandleSiteId, RuntimePlanLowerError> {
                let index = u32::try_from(site.0)
                    .map_err(|_| RuntimePlanLowerError::new("line handle site ordinal overflow"))?;
                Ok(RuntimeLineHandleSiteId::from_zero_based(index))
            };
        let resolve_flow = |draft: FlowDraft| -> Result<RuntimeFlowOpSeed, RuntimePlanLowerError> {
            resolve_flow_draft(draft, &resolve_site, &child_nodes)
        };
        let activation_ops = self
            .activation_ops
            .into_iter()
            .map(&resolve_flow)
            .collect::<Result<Vec<_>, _>>()?;
        let root = resolve_node_draft(root, &resolve_site, &child_nodes)?;
        let handle_sites = self
            .sites
            .into_iter()
            .enumerate()
            .map(|(index, site)| {
                let source_ordinal = u32::try_from(index).map_err(|_| {
                    RuntimePlanLowerError::new("line handle source ordinal overflow")
                })?;
                let id = RuntimeLineHandleSiteId::from_zero_based(source_ordinal);
                Ok(RuntimeLineHandleSiteSeed {
                    id,
                    source_ordinal,
                    kind: site.kind,
                    result_type: site.result_type,
                    character: site.character,
                    scheduled_child: site
                        .scheduled_child
                        .map(|child| {
                            child_nodes.get(&child).copied().ok_or_else(|| {
                                RuntimePlanLowerError::new(
                                    "scheduled handle site references an unknown child draft",
                                )
                            })
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?
            .into_boxed_slice();
        let cancel_rules = self
            .cancel_rules
            .into_iter()
            .map(|rule| {
                Ok(RuntimeLineTaskCancelRuleSeed {
                    trigger: rule.trigger,
                    action: rule
                        .action
                        .into_iter()
                        .map(&resolve_flow)
                        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?
            .into_boxed_slice();
        let cleanup_completed = self
            .cleanup_completed
            .into_iter()
            .map(&resolve_flow)
            .collect::<Result<Vec<_>, _>>()?;
        let cleanup_cancelled = self
            .cleanup_cancelled
            .into_iter()
            .map(&resolve_flow)
            .collect::<Result<Vec<_>, _>>()?;
        let cleanup_failed = self
            .cleanup_failed
            .into_iter()
            .map(&resolve_flow)
            .collect::<Result<Vec<_>, _>>()?;
        let group = RuntimeLineTaskGroupSeed {
            activation_ops,
            result_type: result.identity(),
            handle_sites,
            root,
            cancel_rules,
            cleanup_completed,
            cleanup_cancelled,
            cleanup_failed,
            cleanup_policy: LineCleanupPolicy::default(),
        };
        Ok((group, self.flow.into_assertion_sites()))
    }
}

impl FlowDraft {
    fn collect_free_locals(&self, locals: &mut Vec<RuntimeLocalSeedId>) {
        let expressions: Vec<&RuntimeExprSeed> = match self {
            Self::Flow(flow) => {
                for local in flow.free_locals() {
                    if !locals.contains(&local) {
                        locals.push(local);
                    }
                }
                Vec::new()
            }
            Self::LineOperation { operation, .. } => match operation {
                LineOperationDraft::AcquireActor { .. }
                | LineOperationDraft::VoiceHandle { .. } => Vec::new(),
                LineOperationDraft::Schedule {
                    delay, captures, ..
                } => std::iter::once(delay)
                    .chain(captures.iter().map(|capture| &capture.value))
                    .collect(),
                LineOperationDraft::ActorLook {
                    actor,
                    look,
                    crossfade,
                    ..
                } => vec![actor, look, crossfade],
            },
        };
        for expression in expressions {
            for local in expression.free_locals() {
                if !locals.contains(&local) {
                    locals.push(local);
                }
            }
        }
    }
}

fn assign_node_ids(
    node: &NodeDraft,
    next: &mut usize,
    children: &mut std::collections::BTreeMap<ChildDraftId, RuntimeLineTaskNodeSeedId>,
) -> Result<(), RuntimePlanLowerError> {
    let id = RuntimeLineTaskNodeSeedId::from_zero_based(*next)
        .ok_or_else(|| RuntimePlanLowerError::new("line-task node ordinal overflow"))?;
    *next = next
        .checked_add(1)
        .ok_or_else(|| RuntimePlanLowerError::new("line-task node count overflow"))?;
    match node {
        NodeDraft::Sequence(nodes) | NodeDraft::Start(nodes) => {
            for node in nodes {
                assign_node_ids(node, next, children)?;
            }
        }
        NodeDraft::Parallel {
            children: nodes, ..
        } => {
            for node in nodes {
                assign_node_ids(node, next, children)?;
            }
        }
        NodeDraft::Child {
            id: child, scope, ..
        } => {
            if children.insert(*child, id).is_some() {
                return Err(RuntimePlanLowerError::new(
                    "line-task child draft identity is duplicated",
                ));
            }
            assign_node_ids(scope, next, children)?;
        }
        NodeDraft::Action(_) => {}
    }
    Ok(())
}

fn resolve_node_draft(
    node: NodeDraft,
    resolve_site: &impl Fn(SiteDraftId) -> Result<RuntimeLineHandleSiteId, RuntimePlanLowerError>,
    child_nodes: &std::collections::BTreeMap<ChildDraftId, RuntimeLineTaskNodeSeedId>,
) -> Result<RuntimeLineTaskNodeSeed, RuntimePlanLowerError> {
    Ok(match node {
        NodeDraft::Sequence(nodes) => RuntimeLineTaskNodeSeed::Sequence(
            nodes
                .into_iter()
                .map(|node| resolve_node_draft(node, resolve_site, child_nodes))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        NodeDraft::Start(nodes) => RuntimeLineTaskNodeSeed::Start(
            nodes
                .into_iter()
                .map(|node| resolve_node_draft(node, resolve_site, child_nodes))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        NodeDraft::Parallel { policy, children } => RuntimeLineTaskNodeSeed::Parallel {
            policy,
            children: children
                .into_iter()
                .map(|node| resolve_node_draft(node, resolve_site, child_nodes))
                .collect::<Result<Vec<_>, _>>()?,
        },
        NodeDraft::Child {
            id,
            trigger,
            join_policy,
            cancel_policy,
            scope,
        } => RuntimeLineTaskNodeSeed::Child {
            node: child_nodes.get(&id).copied().ok_or_else(|| {
                RuntimePlanLowerError::new("line-task child draft has no preorder identity")
            })?,
            trigger: match trigger {
                TriggerDraft::Immediate => RuntimeLineTaskTriggerSeed::Immediate,
                TriggerDraft::Mark(mark) => RuntimeLineTaskTriggerSeed::Mark(mark),
                TriggerDraft::ContentEffect(effect) => {
                    RuntimeLineTaskTriggerSeed::ContentEffect(effect)
                }
                TriggerDraft::Scheduled(site) => {
                    RuntimeLineTaskTriggerSeed::Scheduled(resolve_site(site)?)
                }
            },
            join_policy,
            cancel_policy,
            scope: Box::new(resolve_node_draft(*scope, resolve_site, child_nodes)?),
        },
        NodeDraft::Action(actions) => RuntimeLineTaskNodeSeed::Action(
            actions
                .into_iter()
                .map(|action| resolve_flow_draft(action, resolve_site, child_nodes))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    })
}

fn resolve_flow_draft(
    draft: FlowDraft,
    resolve_site: &impl Fn(SiteDraftId) -> Result<RuntimeLineHandleSiteId, RuntimePlanLowerError>,
    child_nodes: &std::collections::BTreeMap<ChildDraftId, RuntimeLineTaskNodeSeedId>,
) -> Result<RuntimeFlowOpSeed, RuntimePlanLowerError> {
    Ok(match draft {
        FlowDraft::Flow(flow) => flow,
        FlowDraft::LineOperation { binding, operation } => RuntimeFlowOpSeed::LineOperation {
            binding,
            operation: match operation {
                LineOperationDraft::AcquireActor { site, character } => {
                    RuntimeLineOperationSeed::AcquireActor {
                        site: resolve_site(site)?,
                        character,
                        scope: RuntimeLineHandleScope::Line,
                    }
                }
                LineOperationDraft::Schedule {
                    site,
                    delay,
                    child,
                    captures,
                } => RuntimeLineOperationSeed::Schedule {
                    site: resolve_site(site)?,
                    delay,
                    child: child_nodes.get(&child).copied().ok_or_else(|| {
                        RuntimePlanLowerError::new(
                            "schedule operation references an unknown child draft",
                        )
                    })?,
                    captures,
                },
                LineOperationDraft::ActorLook {
                    site,
                    character,
                    actor,
                    look,
                    crossfade,
                } => RuntimeLineOperationSeed::ActorLook {
                    site: resolve_site(site)?,
                    character,
                    actor,
                    look,
                    crossfade,
                },
                LineOperationDraft::VoiceHandle { site } => RuntimeLineOperationSeed::VoiceHandle {
                    site: resolve_site(site)?,
                },
            },
        },
    })
}
