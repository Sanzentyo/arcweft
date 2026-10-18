//! Counted source emission. Both passes borrow the admitted expression graph;
//! nested frames use their owners and the same original semantic bank.

use super::{
    Array, Counts, DOMAIN, Error, Footprint, RuntimeExpressionSourceLimits, charge, limit,
    reference,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::*;

pub(in crate::plan) struct EncodedRuntimeExpressionSource {
    bytes: Vec<u8>,
    footprint: Footprint,
    nodes: usize,
}
impl EncodedRuntimeExpressionSource {
    pub(in crate::plan) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.nodes
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
impl RuntimeExpr {
    pub(in crate::plan) fn encode_sources(
        &self,
        limits: RuntimeExpressionSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeExpressionSource, Error> {
        meter.status()?;
        let result = (|| {
            let rows = gather(self, limits, meter)?;
            let mut counts = Counts::new(limits, rows.len())?;
            let mut sink = Sink {
                output: None,
                length: 0,
                limits,
                counts: &mut counts,
                resources,
                meter,
            };
            sink.header(rows.len())?;
            for row in &rows {
                sink.row(row)?;
            }
            let length = sink.length;
            let footprint = counts.footprint;
            charge(meter, length)?;
            let mut bytes = Vec::with_capacity(length);
            let mut emitted = Counts::new(limits, rows.len())?;
            let mut sink = Sink {
                output: Some(&mut bytes),
                length: 0,
                limits,
                counts: &mut emitted,
                resources,
                meter,
            };
            sink.header(rows.len())?;
            for row in &rows {
                sink.row(row)?;
            }
            debug_assert_eq!(emitted.footprint, footprint);
            debug_assert_eq!(bytes.len(), length);
            Ok(EncodedRuntimeExpressionSource {
                bytes,
                footprint,
                nodes: rows.len(),
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
fn gather<'a>(
    root: &'a RuntimeExpr,
    limits: RuntimeExpressionSourceLimits,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<&'a RuntimeExpr>, Error> {
    Counts::new(limits, 1)?;
    let mut rows = vec![root];
    let mut stack = vec![root.owned_children()];
    meter.charge_work(1)?;
    while let Some(children) = stack.last_mut() {
        if let Some((_, RuntimeExpressionNode::Expression(child))) =
            children.find(|(_, node)| matches!(node, RuntimeExpressionNode::Expression(_)))
        {
            let next = rows.len().checked_add(1).ok_or(Error::Arithmetic)?;
            Counts::new(limits, next)?;
            limit("depth", stack.len(), limits.depth as usize)?;
            meter.charge_work(1)?;
            rows.reserve_exact(1);
            rows.push(child);
            stack.reserve_exact(1);
            stack.push(child.owned_children());
        } else {
            stack.pop();
        }
    }
    Ok(rows)
}
struct Sink<'a, 'b> {
    output: Option<&'a mut Vec<u8>>,
    length: usize,
    limits: RuntimeExpressionSourceLimits,
    counts: &'a mut Counts,
    resources: &'b ValueResourceAdmission,
    meter: &'a mut TaskSemanticMeter,
}
impl Sink<'_, '_> {
    fn advance(&mut self, n: usize) -> Result<(), Error> {
        self.length = self.length.checked_add(n).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.limits.encoded_bytes)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.advance(bytes.len())?;
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<(), Error> {
        self.raw(&[value])
    }
    fn boolean(&mut self, value: bool) -> Result<(), Error> {
        self.byte(u8::from(value))
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.u32(u32::try_from(value).map_err(|_| Error::Arithmetic)?)
    }
    fn array(&mut self, n: usize, array: Array) -> Result<(), Error> {
        self.counts.array(n, array.width())?;
        self.count(n)
    }
    fn id(&mut self, kind: &'static str, value: u32, max: usize) -> Result<(), Error> {
        reference(kind, value, max)?;
        self.u32(value)
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.counts
            .storage(value.len().checked_mul(2).ok_or(Error::Arithmetic)?)?;
        self.count(value.len())?;
        self.raw(value.as_bytes())
    }
    fn header(&mut self, nodes: usize) -> Result<(), Error> {
        self.raw(DOMAIN)?;
        self.byte(1)?;
        self.count(nodes)
    }
    fn literal(&mut self, value: &RuntimeValue) -> Result<(), Error> {
        let frame = value.prepare_literal_source_encoding(
            self.counts.literal_limits(),
            self.resources,
            self.meter,
        )?;
        self.counts.nested(
            frame
                .storage_bytes()
                .checked_mul(2)
                .ok_or(Error::Arithmetic)?,
            frame.elements(),
        )?;
        self.count(frame.encoded_bytes())?;
        self.advance(frame.encoded_bytes())?;
        if let Some(output) = self.output.as_mut() {
            frame.append_to(output)?;
        }
        Ok(())
    }
    fn entity(&mut self, value: &RuntimeEntityReference) -> Result<(), Error> {
        let frame = value.prepare_literal_source_encoding(
            self.counts.literal_limits(),
            self.resources,
            self.meter,
        )?;
        self.counts.nested(
            frame
                .storage_bytes()
                .checked_mul(2)
                .ok_or(Error::Arithmetic)?,
            frame.elements(),
        )?;
        self.count(frame.encoded_bytes())?;
        self.advance(frame.encoded_bytes())?;
        if let Some(output) = self.output.as_mut() {
            frame.append_to(output)?;
        }
        Ok(())
    }
    fn pattern(&mut self, value: &RuntimePattern) -> Result<(), Error> {
        let frame = value.prepare_sources_encoding(
            self.counts.pattern_limits(),
            self.resources,
            self.meter,
        )?;
        self.counts
            .nested(frame.storage_bytes(), frame.elements())?;
        self.count(frame.encoded_bytes())?;
        self.advance(frame.encoded_bytes())?;
        if let Some(output) = self.output.as_mut() {
            frame.append_to(output)?;
        }
        Ok(())
    }
    fn fields(&mut self, fields: &[RuntimeRecordFieldId]) -> Result<(), Error> {
        self.array(fields.len(), Array::Fields)?;
        for field in fields {
            self.u32(field.get().get())?;
        }
        Ok(())
    }
    fn place(&mut self, place: &RuntimeMutablePlace) -> Result<(), Error> {
        self.id("local", place.local().get().get(), self.limits.local_count)?;
        self.fields(place.fields())
    }
    fn arguments(&mut self, args: &[RuntimeCallArgument]) -> Result<(), Error> {
        self.array(args.len(), Array::Arguments)?;
        for arg in args {
            self.byte(arg.mode().semantic_tag())?;
            self.u32(arg.abi_position())?;
        }
        Ok(())
    }
    fn initialization(&mut self, value: RuntimePlaceInitialization) -> Result<(), Error> {
        self.byte(match value {
            RuntimePlaceInitialization::Initialized => 0,
            RuntimePlaceInitialization::Uninitialized => 1,
            RuntimePlaceInitialization::Conditional => 2,
        })
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive source projection of the admitted expression algebra"
    )]
    fn row(&mut self, expr: &RuntimeExpr) -> Result<(), Error> {
        use RuntimeExprKind as K;
        self.byte(expr.kind().semantic_tag())?;
        self.id("type", expr.ty().get().get(), self.limits.type_count)?;
        self.array(expr.guard_copy_locals().len(), Array::Guards)?;
        for local in expr.guard_copy_locals() {
            self.id("local", local.get().get(), self.limits.local_count)?;
        }
        match expr.kind() {
            K::Value(value) => self.literal(value)?,
            K::EntityRef(value) => self.entity(value)?,
            K::Agent(agent) => {
                self.byte(agent.constructor().semantic_tag())?;
                if let RuntimeAgentExpr::ChoiceAction { choice } = agent {
                    self.text(choice.as_str())?;
                }
                self.array(expr.owned_children().count(), Array::Children)?;
            }
            K::Local(read) => {
                self.id("local", read.local().get().get(), self.limits.local_count)?;
                self.byte(read.mode().semantic_tag())?;
                self.fields(read.fields())?;
            }
            K::SequencePopFront { place }
            | K::SequencePopBack { place }
            | K::SequencePush { place, .. } => self.place(place)?,
            K::Let { binding, .. } => {
                self.id("local", binding.get().get(), self.limits.local_count)?
            }
            K::Scope { identity, .. } => {
                self.boolean(identity.name().is_some())?;
                if let Some(name) = identity.name() {
                    self.text(name.as_str())?;
                }
            }
            K::Tuple(items) | K::BracketSeq(items) => self.array(items.len(), Array::Children)?,
            K::DialogueContent {
                template,
                values,
                effects,
            } => {
                self.id("template", template.get().get(), self.limits.template_count)?;
                self.array(values.len(), Array::Children)?;
                self.array(effects.len(), Array::Effects)?;
                for effect in effects {
                    self.u32(effect.site.get().get())?;
                    self.id("state", effect.state.get().get(), self.limits.state_count)?;
                    self.array(effect.captures.len(), Array::Children)?;
                }
            }
            K::FormatContent {
                template,
                attempt,
                operands,
                project_method,
                project_option,
            } => {
                self.id("template", template.get().get(), self.limits.template_count)?;
                self.boolean(attempt.is_some())?;
                if let Some(attempt) = attempt {
                    self.id("attempt", attempt.get().get(), self.limits.attempt_count)?;
                }
                self.boolean(project_method.is_some())?;
                if let Some(method) = project_method {
                    self.id(
                        "method",
                        u32::try_from(method.0)
                            .map_err(|_| Error::Arithmetic)?
                            .checked_add(1)
                            .ok_or(Error::Arithmetic)?,
                        self.limits.method_count,
                    )?;
                }
                self.boolean(*project_option)?;
                self.array(operands.len(), Array::Format)?;
                for operand in operands {
                    self.raw(operand.definition().as_bytes())?;
                    self.byte(operand.parameter().index() as u8)?;
                }
            }
            K::CharacterDialogue {
                operation, fields, ..
            } => {
                use arcweft_interaction_model::dialogue::*;
                self.boolean(*operation == CharacterDialogueOperation::Reconfigure)?;
                self.array(fields.len(), Array::Dialogue)?;
                for field in fields {
                    self.byte(field.coordinate.semantic_tag())?;
                    if let CharacterDialogueFieldCoordinate::Custom(id) = &field.coordinate {
                        self.text(id.as_str())?;
                    }
                    self.boolean(matches!(
                        field.operation,
                        CharacterDialoguePatchOperation::Set(_)
                    ))?;
                }
            }
            K::RepeatSeq { len, .. } => self.count(*len)?,
            K::Range {
                start,
                end,
                inclusive,
            } => {
                self.boolean(start.is_some())?;
                self.boolean(end.is_some())?;
                self.boolean(*inclusive)?;
            }
            K::NominalRecord(record) => {
                self.array(record.initializers().len(), Array::Record)?;
                for field in record.initializers() {
                    self.u32(field.field().get().get())?;
                }
            }
            K::Variant { ordinal, payload } => {
                self.u32(*ordinal)?;
                self.boolean(payload.is_some())?;
            }
            K::Field { target, field } => {
                self.boolean(matches!(target, RuntimeFieldTarget::Inspect { .. }))?;
                if let RuntimeFieldTarget::Inspect { place, .. } = target {
                    self.place(place)?;
                }
                match field {
                    RuntimeFieldProjection::Nominal(field) => {
                        self.byte(0)?;
                        self.u32(target.ty().get().get())?;
                        self.u32(field.get().get())?;
                    }
                    RuntimeFieldProjection::OpaqueRecord { owner, field } => {
                        self.byte(1)?;
                        self.u32(target.ty().get().get())?;
                        self.text(owner.producer().as_str())?;
                        self.u32(field.get().get())?;
                    }
                    RuntimeFieldProjection::Agent(field) => {
                        self.byte(2)?;
                        self.u32(u32::from(field.semantic_tag()))?;
                    }
                    RuntimeFieldProjection::EntityReference(field) => {
                        self.byte(3)?;
                        self.byte(match field {
                            RuntimeEntityReferenceField::Id => 0,
                            RuntimeEntityReferenceField::Family => 1,
                            RuntimeEntityReferenceField::Name => 2,
                        })?;
                    }
                    RuntimeFieldProjection::Progress(field) => {
                        self.byte(4)?;
                        self.byte(match field {
                            RuntimeProgressField::Ratio => 0,
                            RuntimeProgressField::Label => 1,
                        })?;
                    }
                }
            }
            K::ProjectTuple { ordinal, .. } | K::ProjectRecord { ordinal, .. } => {
                self.count(*ordinal)?
            }
            K::Assign { place, .. } => {
                self.place(place.place())?;
                self.boolean(matches!(
                    place.displacement(),
                    RuntimePlaceDisplacement::Reachable { .. }
                ))?;
                if let RuntimePlaceDisplacement::Reachable {
                    initialization,
                    fields,
                } = place.displacement()
                {
                    self.initialization(*initialization)?;
                    self.array(fields.len(), Array::Displacement)?;
                    for field in fields {
                        self.fields(&field.fields)?;
                        self.initialization(field.initialization)?;
                    }
                }
            }
            K::Call { callee, args } => {
                self.call_target(callee)?;
                self.arguments(args)?;
            }
            K::MakeCallable { state, captures } => {
                self.id("state", state.get().get(), self.limits.state_count)?;
                self.array(captures.len(), Array::Children)?;
            }
            K::SpecializeCallable { specialization, .. } => self.id(
                "specialization",
                specialization.get().get(),
                self.limits.specialization_count,
            )?,
            K::ApplyGroup { args, .. } => self.arguments(args)?,
            K::TraitCall {
                callable,
                receiver_mode,
                args,
                ..
            } => {
                self.id(
                    "method",
                    u32::try_from(callable.0)
                        .map_err(|_| Error::Arithmetic)?
                        .checked_add(1)
                        .ok_or(Error::Arithmetic)?,
                    self.limits.method_count,
                )?;
                self.byte(match receiver_mode {
                    crate::plan::RuntimeReceiverMode::Owned => 0,
                    crate::plan::RuntimeReceiverMode::SharedRef => 1,
                    crate::plan::RuntimeReceiverMode::MutRef => 2,
                })?;
                self.arguments(args)?;
            }
            K::PureCall { helper, args } => {
                self.id(
                    "helper",
                    u32::try_from(helper.0)
                        .map_err(|_| Error::Arithmetic)?
                        .checked_add(1)
                        .ok_or(Error::Arithmetic)?,
                    self.limits.helper_count,
                )?;
                self.arguments(args)?;
            }
            K::Sum { .. } | K::If { .. } | K::ReductionUnchanged { .. } => {}
            K::Unary { op, .. } => self.byte(op.semantic_tag())?,
            K::Binary { op, .. } => self.byte(op.semantic_tag())?,
            K::IfLet { pattern, guard, .. } => {
                self.pattern(pattern)?;
                self.boolean(guard.is_some())?;
            }
            K::Match { arms, .. } => {
                self.array(arms.len(), Array::Arms)?;
                for arm in arms {
                    self.pattern(arm.pattern())?;
                    self.boolean(arm.guard().is_some())?;
                }
            }
        }
        Ok(())
    }
    fn call_target(&mut self, target: &RuntimeCallTarget) -> Result<(), Error> {
        match target {
            RuntimeCallTarget::Intrinsic(intrinsic) => {
                self.byte(0)?;
                self.byte(intrinsic.semantic_tag())?;
            }
            RuntimeCallTarget::Callable(callable) => {
                self.byte(1)?;
                self.text(callable.as_str())?;
            }
            RuntimeCallTarget::External(callable) => {
                self.byte(2)?;
                self.text(callable.identity().as_str())?;
                let declaration = callable.declaration();
                match declaration.provider() {
                    RuntimeExternalCallableProvider::Standard(provider) => {
                        self.byte(0)?;
                        self.byte(provider.canonical_tag())?;
                    }
                    RuntimeExternalCallableProvider::Adapter(package) => {
                        self.byte(1)?;
                        self.text(package)?;
                    }
                }
                match declaration.export() {
                    RuntimeExternalCallableExport::Function(path)
                    | RuntimeExternalCallableExport::RustFunction(path) => {
                        self.byte(
                            if matches!(
                                declaration.export(),
                                RuntimeExternalCallableExport::Function(_)
                            ) {
                                0
                            } else {
                                2
                            },
                        )?;
                        self.counts.array(path.len(), 2 * size_of::<String>())?;
                        self.count(path.len())?;
                        for segment in path {
                            self.text(segment)?;
                        }
                    }
                    RuntimeExternalCallableExport::Method { receiver, name } => {
                        self.byte(1)?;
                        self.raw(receiver)?;
                        self.text(name)?;
                    }
                }
                self.raw(&declaration.overload().to_le_bytes())?;
            }
        }
        Ok(())
    }
}
