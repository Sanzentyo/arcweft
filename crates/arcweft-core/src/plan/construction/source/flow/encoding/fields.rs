//! Borrowed operation metadata. Expressions/patterns come from `FlowOp`'s
//! ordered value-root projection; referenced definitions remain real edges.

use super::super::{Counts, Error, limit};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    FlowOp, FlowRuntimeId, RuntimeIteratorEvidence, RuntimeIteratorWitnessExecutable, RuntimePlan,
    RuntimeStandardMapCompletion,
};
use crate::runtime_id::RuntimeIdPath;
use crate::task::semantic::TaskSemanticMeter;
use crate::task::{NeedProducerTemplate, RuntimeHostArgumentTemplate};
use crate::value::{
    RuntimeCallArgumentMode, RuntimeExpressionNode, RuntimeMutablePlace, RuntimePlaceDisplacement,
    RuntimePlaceInitialization,
};

pub(super) fn encode(
    op: &FlowOp,
    plan: &RuntimePlan,
    counts: &Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<u8>, Error> {
    let maximum = counts.limits.encoded_bytes;
    let mut metadata = Writer::new(maximum);
    metadata.operation(op, plan, counts, resources, meter)?;
    let mut values = Writer::new(maximum);
    let mut value_count = 0usize;
    if !matches!(
        op,
        FlowOp::Choice { .. }
            | FlowOp::HostCall { .. }
            | FlowOp::StartNeedProducer { .. }
            | FlowOp::ProjectCall { .. }
            | FlowOp::Effect(_)
            | FlowOp::EvaluatedEffect(_)
            | FlowOp::RegisterCleanup { .. }
    ) {
        op.try_visit_value_roots(&mut |_, node| {
            value_count = value_count.checked_add(1).ok_or(Error::Arithmetic)?;
            match node {
                RuntimeExpressionNode::Expression(expr) => {
                    let prepared =
                        expr.encode_sources(counts.expression(maximum), resources, meter)?;
                    values.byte(0)?;
                    values.frame(prepared.bytes())?;
                }
                RuntimeExpressionNode::Pattern(pattern) => {
                    let prepared = pattern.prepare_sources_encoding(
                        counts.pattern(maximum),
                        resources,
                        meter,
                    )?;
                    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
                    prepared.append_to(&mut bytes)?;
                    values.byte(1)?;
                    values.frame(&bytes)?;
                }
            }
            Ok::<_, Error>(())
        })?;
    }
    let mut bodies = Writer::new(maximum);
    let mut body_count = 0usize;
    for (_, body) in op.owned_bodies() {
        body_count = body_count.checked_add(1).ok_or(Error::Arithmetic)?;
        bodies.count(body.len())?;
    }
    let mut output = Writer::new(maximum);
    output.byte(op.semantic_tag())?;
    output.frame(&metadata.bytes)?;
    output.count(value_count)?;
    output.raw(&values.bytes)?;
    output.count(body_count)?;
    output.raw(&bodies.bytes)?;
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
    fn byte(&mut self, value: u8) -> Result<(), Error> {
        self.raw(&[value])
    }
    fn flag(&mut self, value: bool) -> Result<(), Error> {
        self.byte(u8::from(value))
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.u32(u32::try_from(value).map_err(|_| Error::Arithmetic)?)
    }
    fn frame(&mut self, value: &[u8]) -> Result<(), Error> {
        self.count(value.len())?;
        self.raw(value)
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.frame(value.as_bytes())
    }
    fn optional_text(&mut self, value: Option<&str>) -> Result<(), Error> {
        self.flag(value.is_some())?;
        if let Some(value) = value {
            self.text(value)?;
        }
        Ok(())
    }
    fn local(&mut self, value: crate::runtime_id::RuntimeLocalDeclarationId) -> Result<(), Error> {
        self.u32(value.get().get())
    }
    fn ty(&mut self, value: crate::runtime_id::RuntimePlanTypeId) -> Result<(), Error> {
        self.u32(value.get().get())
    }
    fn path(&mut self, flow: &FlowRuntimeId) -> Result<(), Error> {
        self.path_segments(flow.path())?;
        self.text(flow.public_label_ref().as_str())
    }
    fn path_segments(&mut self, path: &RuntimeIdPath) -> Result<(), Error> {
        self.count(path.segments().len())?;
        for segment in path.segments() {
            self.text(segment.as_str())?;
        }
        Ok(())
    }
    fn scope(&mut self, scope: &crate::scope::RuntimeScopeIdentity) -> Result<(), Error> {
        self.optional_text(scope.name().map(arcweft_id::DeclarationName::as_str))
    }
    fn locals(
        &mut self,
        locals: &[crate::runtime_id::RuntimeLocalDeclarationId],
    ) -> Result<(), Error> {
        self.count(locals.len())?;
        for local in locals {
            self.local(*local)?;
        }
        Ok(())
    }
    fn argument_roles(&mut self, args: &[RuntimeHostArgumentTemplate]) -> Result<(), Error> {
        self.count(args.len())?;
        for argument in args {
            match argument {
                RuntimeHostArgumentTemplate::Positional(identity, _) => {
                    self.byte(0)?;
                    self.raw(identity.as_bytes())?;
                }
                RuntimeHostArgumentTemplate::Named(identity, named) => {
                    self.byte(1)?;
                    self.raw(identity.as_bytes())?;
                    self.text(&named.name)?;
                }
                RuntimeHostArgumentTemplate::Spread(identity, _) => {
                    self.byte(2)?;
                    self.raw(identity.as_bytes())?;
                }
            }
        }
        Ok(())
    }
    fn template(
        &mut self,
        template: &NeedProducerTemplate,
        plan: &RuntimePlan,
    ) -> Result<(), Error> {
        self.producer_definition(&template.definition, plan)?;
        self.text(&template.request.capability.0)?;
        self.text(&template.request.operation)?;
        self.argument_roles(&template.request.args)
    }
    fn producer_definition(
        &mut self,
        definition: &crate::task::NeedProducerDefinition,
        plan: &RuntimePlan,
    ) -> Result<(), Error> {
        if plan.task_plan(&definition.plan).is_none() {
            return Err(Error::Handle {
                kind: "producer task",
            });
        }
        self.raw(definition.function.as_bytes())?;
        self.byte(definition.family.semantic_tag())?;
        self.raw(definition.contract.as_bytes())?;
        self.u32(definition.plan.ordinal())?;
        self.raw(definition.producer_site.as_bytes())?;
        let semantic =
            crate::pattern::RuntimeSemanticTypeId::from_bytes(*definition.payload_type.as_bytes());
        let payload = plan
            .type_table()
            .id_for_semantic(semantic)
            .ok_or(Error::Handle {
                kind: "producer payload type",
            })?;
        self.ty(payload)?;
        self.byte(definition.class.semantic_tag())?;
        self.raw(&definition.priority.0.to_le_bytes())?;
        self.text(&definition.cancel_scope.0)?;
        self.byte(definition.policy.semantic_tag())?;
        self.text(&definition.debug_label)
    }
    fn effect(
        &mut self,
        effect: &crate::effect::LineEffectRequest,
        counts: &Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        if let crate::effect::LineEffectRequest::Audio(command) = effect {
            self.byte(1)?;
            let child = command.encode_source(counts.audio(self.maximum), resources, meter)?;
            self.frame(child.bytes())
        } else {
            self.byte(0)?;
            let child = effect.encode_static_source(
                counts.static_effect(self.maximum),
                resources,
                meter,
            )?;
            self.frame(child.bytes())
        }
    }
    fn fields(&mut self, fields: &[crate::value::RuntimeRecordFieldId]) -> Result<(), Error> {
        self.count(fields.len())?;
        for field in fields {
            self.u32(field.zero_based())?;
        }
        Ok(())
    }
    fn initialization(&mut self, initialization: RuntimePlaceInitialization) -> Result<(), Error> {
        self.byte(match initialization {
            RuntimePlaceInitialization::Initialized => 0,
            RuntimePlaceInitialization::Uninitialized => 1,
            RuntimePlaceInitialization::Conditional => 2,
        })
    }
    fn assignment(&mut self, assignment: &crate::value::RuntimeAssignment) -> Result<(), Error> {
        match assignment.place() {
            RuntimeMutablePlace::Local(local) => {
                self.byte(0)?;
                self.local(*local)?;
            }
            RuntimeMutablePlace::Fields { base, fields } => {
                self.byte(1)?;
                self.local(*base)?;
                self.fields(fields)?;
            }
        }
        match assignment.displacement() {
            RuntimePlaceDisplacement::Unreachable => self.byte(0)?,
            RuntimePlaceDisplacement::Reachable {
                initialization,
                fields,
            } => {
                self.byte(1)?;
                self.initialization(*initialization)?;
                self.count(fields.len())?;
                for field in fields {
                    self.fields(&field.fields)?;
                    self.initialization(field.initialization)?;
                }
            }
        }
        Ok(())
    }
    fn iterator(&mut self, evidence: &RuntimeIteratorEvidence) -> Result<(), Error> {
        match evidence {
            RuntimeIteratorEvidence::Builtin(evidence) => {
                self.byte(0)?;
                self.byte(match evidence.family {
                    crate::plan::RuntimeBuiltinIteratorFamily::Range => 0,
                    crate::plan::RuntimeBuiltinIteratorFamily::Seq => 1,
                    crate::plan::RuntimeBuiltinIteratorFamily::Stream => 2,
                    crate::plan::RuntimeBuiltinIteratorFamily::Vec => 3,
                    crate::plan::RuntimeBuiltinIteratorFamily::Array => 4,
                    crate::plan::RuntimeBuiltinIteratorFamily::Slice => 5,
                    crate::plan::RuntimeBuiltinIteratorFamily::TupleHomogeneous => 6,
                })?;
                for ty in [
                    evidence.item,
                    evidence.iterator,
                    evidence.next_value,
                    evidence.step,
                ] {
                    self.ty(ty)?;
                }
            }
            RuntimeIteratorEvidence::Witness(evidence) => {
                self.byte(1)?;
                self.ty(evidence.item)?;
                self.ty(evidence.iterator)?;
                match evidence.executable {
                    RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next } => {
                        self.byte(0)?;
                        self.count(into_iter.0.checked_add(1).ok_or(Error::Arithmetic)?)?;
                        self.count(next.0.checked_add(1).ok_or(Error::Arithmetic)?)?;
                    }
                    RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next } => {
                        self.byte(1)?;
                        self.count(next.0.checked_add(1).ok_or(Error::Arithmetic)?)?;
                    }
                }
            }
        }
        Ok(())
    }
    fn operation(
        &mut self,
        op: &FlowOp,
        plan: &RuntimePlan,
        counts: &Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        match op {
            FlowOp::Let { .. }
            | FlowOp::LetElse { .. }
            | FlowOp::CommitDialogueResult { .. }
            | FlowOp::SelectDialogueResult { .. }
            | FlowOp::If { .. }
            | FlowOp::While { .. }
            | FlowOp::Continue
            | FlowOp::GotoExpr(_)
            | FlowOp::ReturnExpr(_)
            | FlowOp::ExitScope
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::Noop => (),
            FlowOp::StandardMap(control) => {
                self.byte(control.family().semantic_tag())?;
                self.local(control.source())?;
                self.local(control.mapping())?;
                self.local(control.result())?;
                match control.completion() {
                    RuntimeStandardMapCompletion::Collection => self.byte(0)?,
                    RuntimeStandardMapCompletion::Sum { map_operations } => {
                        self.byte(1)?;
                        self.count(map_operations)?;
                    }
                }
            }
            FlowOp::FormatOperandAttempt {
                attempt, parameter, ..
            } => {
                self.count(attempt.index())?;
                self.count(parameter.index())?;
            }
            FlowOp::Assign { place, .. } => self.assignment(place)?,
            FlowOp::LineOperation { binding, operation } => {
                self.flag(binding.is_some())?;
                match operation {
                    crate::plan::RuntimeLineOperation::AcquireActor {
                        site,
                        character,
                        scope,
                    } => {
                        self.byte(0)?;
                        self.u32(site.get())?;
                        self.text(character.as_str())?;
                        self.byte(match scope {
                            crate::line_task::RuntimeLineHandleScope::Line => 0,
                        })?;
                    }
                    crate::plan::RuntimeLineOperation::Schedule {
                        site,
                        child,
                        captures,
                        ..
                    } => {
                        self.byte(1)?;
                        self.u32(site.get())?;
                        self.count(child.index())?;
                        self.count(captures.len())?;
                        for capture in captures {
                            self.local(capture.local)?;
                        }
                    }
                    crate::plan::RuntimeLineOperation::ActorLook {
                        site,
                        character,
                        actor,
                        ..
                    } => {
                        self.byte(2)?;
                        self.u32(site.get())?;
                        self.text(character.as_str())?;
                        self.local(actor.local())?;
                    }
                    crate::plan::RuntimeLineOperation::VoiceHandle { site } => {
                        self.byte(3)?;
                        self.u32(site.get())?;
                    }
                }
            }
            FlowOp::Dialogue {
                content, result, ..
            } => {
                self.u32(content.get().get())?;
                self.ty(result.ty())?;
            }
            FlowOp::Choice { id, options } => {
                self.optional_text(id.as_deref())?;
                self.count(options.len())?;
                for option in options {
                    self.optional_text(option.id.as_deref())?;
                    self.text(&option.label)?;
                    self.flag(option.target.is_some())?;
                    if let Some(target) = &option.target {
                        self.path(target)?;
                    }
                    self.flag(option.out.is_some())?;
                    if let Some(out) = &option.out {
                        self.optional_text(out.label.as_deref())?;
                        self.text(&out.value)?;
                    }
                    self.count(option.effects.len())?;
                    for effect in &option.effects {
                        self.effect(effect, counts, resources, meter)?;
                    }
                }
            }
            FlowOp::Await {
                binding, observers, ..
            } => {
                self.flag(binding.is_some())?;
                self.count(observers.len())?;
            }
            FlowOp::StartNeedProducer { .. } => {
                let child = op.encode_selected_producer_source(
                    plan,
                    counts.selected(self.maximum),
                    resources,
                    meter,
                )?;
                self.frame(child.bytes())?;
            }
            FlowOp::AwaitMany {
                binding,
                target,
                pending,
            } => {
                self.flag(binding.is_some())?;
                self.local(target.item_binding)?;
                self.u32(target.limit)?;
                self.template(&target.base, plan)?;
                self.template(&target.child, plan)?;
                self.count(pending.len())?;
                for effect in pending {
                    self.effect(effect, counts, resources, meter)?;
                }
            }
            FlowOp::HostCall { .. } => {
                let child =
                    op.encode_host_call_source(plan, counts.host(self.maximum), resources, meter)?;
                self.frame(child.bytes())?;
            }
            FlowOp::ProjectCall { site } => {
                if site.get().get() as usize > counts.limits.project_count {
                    return Err(Error::Reference {
                        kind: "project",
                        actual: site.get().get(),
                        maximum: counts.limits.project_count,
                    });
                }
                self.u32(site.get().get())?;
                let site = plan.project_call_sites().get(*site).ok_or(Error::Handle {
                    kind: "project call",
                })?;
                let child = site.encode_source(counts.project(self.maximum), resources, meter)?;
                self.frame(child.bytes())?;
            }
            FlowOp::ApplyGroup { args, .. } => {
                self.count(args.len())?;
                for arg in args {
                    self.byte(match arg.mode() {
                        RuntimeCallArgumentMode::Value => 0,
                        RuntimeCallArgumentMode::Spread => 1,
                    })?;
                    self.u32(arg.abi_position())?;
                }
            }
            FlowOp::IfLet { guard, .. } | FlowOp::WhileLet { guard, .. } => {
                self.flag(guard.is_some())?;
            }
            FlowOp::Match { arms, .. } => {
                self.count(arms.len())?;
                for arm in arms {
                    self.flag(arm.guard.is_some())?;
                    if let Some(guard) = &arm.guard {
                        self.local(guard.candidate)?;
                        self.ty(guard.result)?;
                        self.flag(guard.condition.is_some())?;
                        self.locals(&guard.copy_locals)?;
                    }
                }
            }
            FlowOp::Loop { result, .. } => self.flag(result.is_some())?,
            FlowOp::For { evidence, .. } => self.iterator(evidence)?,
            FlowOp::Thread {
                name,
                producer,
                captures,
                ..
            } => {
                self.optional_text(name.as_deref())?;
                self.producer_definition(producer, plan)?;
                self.locals(captures)?;
            }
            FlowOp::Scope { identity, .. } | FlowOp::EnterScope { identity } => {
                self.scope(identity)?;
            }
            FlowOp::Break(value) => self.flag(value.is_some())?,
            FlowOp::Goto(flow) => self.path(flow)?,
            FlowOp::Return(text) | FlowOp::CancelCleanup { key: text } => self.text(text)?,
            FlowOp::Effect(effect) => self.effect(effect, counts, resources, meter)?,
            FlowOp::EvaluatedEffect(effect) => {
                let child =
                    effect.encode_source(counts.evaluated(self.maximum), resources, meter)?;
                self.frame(child.bytes())?;
            }
            FlowOp::RegisterDefer {
                site,
                outcome,
                captures,
                owner,
            } => {
                self.count(site.index())?;
                self.byte(match outcome {
                    crate::line_task::RuntimeDeferOutcomeFilter::Always => 0,
                    crate::line_task::RuntimeDeferOutcomeFilter::Completed => 1,
                    crate::line_task::RuntimeDeferOutcomeFilter::Cancelled => 2,
                    crate::line_task::RuntimeDeferOutcomeFilter::Failed => 3,
                })?;
                self.byte(match owner {
                    crate::plan::RuntimeDeferOwner::CurrentScope => 0,
                    crate::plan::RuntimeDeferOwner::LineRoot => 1,
                })?;
                self.count(captures.len())?;
            }
            FlowOp::RegisterCleanup { key, effect } => {
                self.text(key)?;
                self.effect(effect, counts, resources, meter)?;
            }
            FlowOp::Bind(_)
            | FlowOp::CompleteFormatOperand { .. }
            | FlowOp::LoopNext { .. }
            | FlowOp::WhileNext { .. }
            | FlowOp::WhileLetNext { .. }
            | FlowOp::ForNext { .. }
            | FlowOp::LetScope { .. }
            | FlowOp::CompleteAwaitObserver
            | FlowOp::EnterScheduledScope { .. }
            | FlowOp::ExitScheduledScope { .. } => return Err(Error::Continuation),
        }
        Ok(())
    }
}
