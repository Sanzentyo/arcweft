//! Private typed context for actual body semantic transcripts.
//! No public seal proof is minted by this preparation owner.

use super::RuntimePlanInventory;

mod callable;
mod entries;
#[cfg(test)]
mod entry_fixtures;
mod executable_metadata;
mod executable_roles;
mod executable_rows;
pub(crate) mod flow;
pub(crate) mod function;
mod local_rows;
mod nominal;
mod pure_rows;
mod request;
mod task_image;
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};
use crate::value::{
    RuntimeAssignment, RuntimeMutablePlace, RuntimePlaceDisplacement, RuntimePlaceInitialization,
    RuntimeRecordFieldId,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeBodySemanticError {
    #[error("body references unknown type {ty}")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("body references unknown local {local}")]
    UnknownLocal { local: RuntimeLocalDeclarationId },
    #[error("body semantic transcript rejected: {0:?}")]
    Encoding(TaskSemanticEncodingError),
    #[error("body contains a runtime value which is not a static literal")]
    InvalidStaticLiteral,
    #[error("body contains an engine-only Flow continuation")]
    RuntimeFlowContinuation,
    #[error("body task coordinate belongs to another candidate inventory")]
    ForeignTaskCoordinate,
    #[error("producer function has invalid input evidence at input {input}")]
    InvalidFunctionInput { input: usize },
    #[error("producer function roles {actual} exceed limit {maximum}")]
    FunctionRoles { actual: usize, maximum: u32 },
    #[error("request endpoint does not belong to this plan or is not Host-backed")]
    InvalidHostRequestEndpoint,
    #[error("request template roles {actual} exceed limit {maximum}")]
    RequestRoles { actual: usize, maximum: u32 },
    #[error("request template endpoint {actual} does not match completed F endpoint {expected}")]
    InvalidRequestEndpoint { expected: u32, actual: u32 },

    #[error("Line row {ordinal} children {actual} exceed limit {maximum}")]
    LineChildren {
        ordinal: usize,
        actual: usize,
        maximum: u32,
    },

    #[error("body references missing {table} row {ordinal}")]
    MissingRow { table: &'static str, ordinal: usize },
    #[error("body references unknown pure program {program:?}")]
    UnknownPureProgram {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    },
    #[error("body references ambiguous pure program {program:?}")]
    AmbiguousPureProgram {
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    },
    #[error("callable semantic graph has a cycle at state {state}")]
    CallableCycle { state: usize },
    #[error("nominal domain does not belong to this candidate inventory")]
    ForeignNominalDomain,
    #[error("nominal domain owner {owner} has no accepted nominal declaration")]
    InvalidNominalDomainOwner { owner: RuntimePlanTypeId },
    #[error("function transcript does not belong to this candidate inventory")]
    ForeignFunctionTranscript,
    #[error("pure helper row does not belong to this candidate inventory")]
    ForeignPureHelperRow,
    #[error("trait method row does not belong to this candidate inventory")]
    ForeignTraitMethodRow,
    #[error("Flow row {ordinal} does not belong to this producer function")]
    InvalidFlowProducer { ordinal: usize },
    #[error("Line transcript does not belong to this candidate inventory")]
    ForeignLineTranscript,
    #[error("callable executable row {ordinal} has no matching completed code transcript")]
    InvalidCallableExecutableProducer { ordinal: usize },
    #[error("Flow executable row {ordinal} has no matching completed producer transcript")]
    InvalidFlowExecutableProducer { ordinal: usize },
    #[error("executable rows {actual} exceed limit {maximum}")]
    ExecutableRows { actual: usize, maximum: u32 },
    #[error("executable semantic graph has a cycle at table {table}, row {ordinal}")]
    ExecutableCycle { table: u8, ordinal: usize },
}

impl From<TaskSemanticEncodingError> for RuntimeBodySemanticError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Encoding(error)
    }
}

/// Borrows the sole admitted tables; arena IDs are resolved before writing.
pub(crate) struct RuntimeBodySemanticContext<'a> {
    plan: &'a RuntimePlanInventory,
}

impl<'a> RuntimeBodySemanticContext<'a> {
    pub(crate) const fn new(plan: &'a RuntimePlanInventory) -> Self {
        Self { plan }
    }

    pub(crate) fn write_expression(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        expression: &crate::value::RuntimeExpr,
    ) -> Result<(), RuntimeBodySemanticError> {
        self.write_node(
            encoder,
            crate::value::RuntimeExpressionNode::Expression(expression),
        )
    }

    pub(crate) fn write_node(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        node: crate::value::RuntimeExpressionNode<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::value::expression_tree::RuntimeExpressionTreeEvent as Event;
        node.try_visit_owned_events(&mut |event| {
            encoder.status()?;
            match event {
                Event::Enter { role, node } => {
                    encoder.enter_element();
                    encoder.tag(0);
                    role.encode_semantic_path(encoder);
                    match node {
                        crate::value::RuntimeExpressionNode::Expression(expression) => {
                            encoder.tag(0);
                            expression.encode_body_metadata(self, encoder)?;
                        }
                        crate::value::RuntimeExpressionNode::Pattern(pattern) => {
                            encoder.tag(1);
                            self.write_pattern_metadata(encoder, pattern)?;
                        }
                    }
                }
                Event::Exit { node } => {
                    encoder.tag(1);
                    encoder.tag(match node {
                        crate::value::RuntimeExpressionNode::Expression(_) => 0,
                        crate::value::RuntimeExpressionNode::Pattern(_) => 1,
                    });
                }
            }
            encoder.status().map_err(Into::into)
        })
    }

    /// Stream bodies use the same typed expression/pattern context. An explicit
    /// work stack retains empty-body boundaries and source arm/child positions.
    pub(crate) fn write_stream(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        stream: &crate::stream::StreamPlan,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::stream::StreamOp as Op;
        use crate::value::RuntimeExpressionNode as Node;
        enum Work<'a> {
            Body(u8, usize, &'a [Op]),
            Ops(std::iter::Enumerate<std::slice::Iter<'a, Op>>),
            EndBody,
            EndOperation,
            Arms(std::iter::Enumerate<std::slice::Iter<'a, crate::stream::StreamMatchArm>>),
        }
        encoder.count(stream.id().path().segments().len());
        for segment in stream.id().path().segments() {
            encoder.enter_element();
            encoder.string(segment.as_str());
        }
        self.write_type(encoder, stream.item_ty())?;
        self.write_type(encoder, stream.error_ty())?;
        let mut work = vec![Work::Body(0, 0, stream.ops())];
        while let Some(next) = work.pop() {
            encoder.status()?;
            match next {
                Work::Body(role, ordinal, ops) => {
                    encoder.enter_element();
                    encoder.tag(0);
                    encoder.tag(role);
                    encoder.count(ordinal);
                    encoder.count(ops.len());
                    encoder.status()?;
                    work.push(Work::EndBody);
                    work.push(Work::Ops(ops.iter().enumerate()));
                }
                Work::Arms(mut arms) => {
                    if let Some((ordinal, arm)) = arms.next() {
                        work.push(Work::Arms(arms));
                        work.push(Work::Body(4, ordinal, &arm.ops));
                    }
                }
                Work::EndBody => encoder.tag(1),
                Work::EndOperation => encoder.tag(3),
                Work::Ops(mut ops) => {
                    let Some((ordinal, op)) = ops.next() else {
                        continue;
                    };
                    encoder.enter_element();
                    encoder.tag(2);
                    encoder.count(ordinal);
                    encoder.tag(op.semantic_tag());
                    encoder.status()?;
                    work.push(Work::Ops(ops));
                    work.push(Work::EndOperation);
                    match op {
                        Op::Let { pattern, expr } => {
                            self.write_node(encoder, Node::Pattern(pattern))?;
                            self.write_expression(encoder, expr)?;
                        }
                        Op::ForNext {
                            pattern,
                            source,
                            body,
                        } => {
                            self.write_node(encoder, Node::Pattern(pattern))?;
                            self.write_expression(encoder, source)?;
                            work.push(Work::Body(1, 0, body));
                        }
                        Op::Yield { expr } => self.write_expression(encoder, expr)?,
                        Op::If {
                            condition,
                            then_ops,
                            else_ops,
                        } => {
                            self.write_expression(encoder, condition)?;
                            work.push(Work::Body(3, 0, else_ops));
                            work.push(Work::Body(2, 0, then_ops));
                        }
                        Op::Match { scrutinee, arms } => {
                            self.write_expression(encoder, scrutinee)?;
                            encoder.count(arms.len());
                            for arm in arms {
                                encoder.enter_element();
                                self.write_node(encoder, Node::Pattern(&arm.pattern))?;
                                encoder.tag(u8::from(arm.guard.is_some()));
                                if let Some(guard) = &arm.guard {
                                    self.write_expression(encoder, guard)?;
                                }
                            }
                            encoder.status()?;
                            work.push(Work::Arms(arms.iter().enumerate()));
                        }
                        Op::Close { source } => self.write_expression(encoder, source)?,
                        Op::Return => {}
                    }
                }
            }
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_type(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.type_table().get(ty).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::UnknownType { ty }
        })?;
        encoder.digest(row.semantic_identity().as_bytes());
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_local(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        local: RuntimeLocalDeclarationId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.local_declarations().get(local).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::UnknownLocal { local }
        })?;
        row.source().encode_semantic_source(encoder);
        self.write_type(encoder, row.ty())?;
        self.write_local_placement(encoder, row.placement())
    }

    pub(crate) fn write_fields(
        encoder: &mut TaskSemanticEncoder<'_>,
        fields: &[RuntimeRecordFieldId],
    ) {
        encoder.count(fields.len());
        for field in fields {
            encoder.enter_element();
            encoder.ordinal(field.zero_based());
        }
    }

    pub(crate) fn write_place(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        place: &RuntimeMutablePlace,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.tag(match place {
            RuntimeMutablePlace::Local(_) => 0,
            RuntimeMutablePlace::Fields { .. } => 1,
        });
        self.write_local(encoder, place.local())?;
        Self::write_fields(encoder, place.fields());
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_assignment(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        assignment: &RuntimeAssignment,
    ) -> Result<(), RuntimeBodySemanticError> {
        self.write_place(encoder, assignment.place())?;
        match assignment.displacement() {
            RuntimePlaceDisplacement::Unreachable => encoder.tag(0),
            RuntimePlaceDisplacement::Reachable {
                initialization,
                fields,
            } => {
                encoder.tag(1);
                Self::write_initialization(encoder, *initialization);
                encoder.count(fields.len());
                for field in fields {
                    encoder.enter_element();
                    Self::write_fields(encoder, &field.fields);
                    Self::write_initialization(encoder, field.initialization);
                }
            }
        }
        encoder.status().map_err(Into::into)
    }

    fn write_pattern_metadata(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        pattern: &crate::pattern::RuntimePattern,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::pattern::RuntimePatternKind as Kind;
        self.write_type(encoder, pattern.ty())?;
        encoder.tag(pattern.kind().semantic_tag());
        match pattern.kind() {
            Kind::Bind { mutable, binding } => {
                encoder.tag(u8::from(*mutable));
                self.write_pattern_binding(encoder, binding)?;
            }
            Kind::Typed { binding } | Kind::Whole { binding, .. } => {
                self.write_pattern_binding(encoder, binding)?;
            }
            Kind::Discard => {}
            Kind::Literal(value) => value.encode_static_literal(encoder)?,
            Kind::Entity(value) => value.encode_body_identity(encoder),
            Kind::Tuple(items) | Kind::Or(items) => encoder.count(items.len()),
            Kind::Sequence { items, rest } => {
                encoder.count(items.len());
                self.write_pattern_rest(encoder, rest)?;
            }
            Kind::Record { fields, rest } => {
                encoder.count(fields.len());
                for field in fields {
                    encoder.enter_element();
                    encoder.ordinal(field.field().zero_based());
                }
                self.write_pattern_rest(encoder, rest)?;
            }
            Kind::Variant { ordinal, payload } => {
                encoder.ordinal(*ordinal);
                encoder.tag(u8::from(payload.is_some()));
            }
        }
        encoder.status().map_err(Into::into)
    }

    fn write_pattern_rest(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        rest: &crate::pattern::RuntimePatternRest,
    ) -> Result<(), RuntimeBodySemanticError> {
        match rest {
            crate::pattern::RuntimePatternRest::Exact => encoder.tag(0),
            crate::pattern::RuntimePatternRest::Ignore => encoder.tag(1),
            crate::pattern::RuntimePatternRest::Bind(binding) => {
                encoder.tag(2);
                self.write_pattern_binding(encoder, binding)?;
            }
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_pattern_binding(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        binding: &crate::pattern::RuntimePatternBindingCoordinate,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::pattern::RuntimePatternBindingStep as Step;
        self.write_local(encoder, binding.local())?;
        encoder.count(binding.path().steps().len());
        for step in binding.path().steps() {
            encoder.enter_element();
            match step {
                Step::Whole => encoder.tag(0),
                Step::TupleElement(ordinal) => {
                    encoder.tag(1);
                    encoder.ordinal(*ordinal);
                }
                Step::RecordField(ordinal) => {
                    encoder.tag(2);
                    encoder.ordinal(*ordinal);
                }
                Step::SequenceElement(ordinal) => {
                    encoder.tag(3);
                    encoder.ordinal(*ordinal);
                }
                Step::SequenceRest => encoder.tag(4),
                Step::VariantPayload => encoder.tag(5),
                Step::RecordRest => encoder.tag(6),
            }
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_call_arguments(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        args: &[crate::value::RuntimeCallArgument],
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.count(args.len());
        for arg in args {
            encoder.enter_element();
            encoder.tag(arg.mode().semantic_tag());
            encoder.ordinal(arg.abi_position());
            self.write_type(encoder, arg.value().ty())?;
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_function_reference(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        function: crate::runtime_id::RuntimeFunctionSiteId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let site = self.plan.function_sites().get(function).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "function sites",
                ordinal: function.get().get() as usize - 1,
            }
        })?;
        encoder.digest(site.definition().as_bytes());
        encoder.tag(site.role().semantic_tag());
        self.write_type(encoder, site.result())?;
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_specialization(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        specialization: crate::runtime_id::RuntimeCallableSpecializationId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self
            .plan
            .callable_specializations()
            .get(specialization.index())
            .ok_or_else(|| {
                encoder.reject_owner();
                RuntimeBodySemanticError::MissingRow {
                    table: "callable specializations",
                    ordinal: specialization.index(),
                }
            })?;
        self.write_type(encoder, row.source_type)?;
        self.write_type(encoder, row.target_type)?;
        encoder.count(row.arguments.types.len());
        for ty in &row.arguments.types {
            encoder.enter_element();
            self.write_type(encoder, *ty)?;
        }
        encoder.count(row.arguments.const_lengths.len());
        for length in &row.arguments.const_lengths {
            encoder.enter_element();
            match length {
                super::RuntimeArrayLength::Constant(value) => {
                    encoder.tag(0);
                    encoder.scalar_u64(*value);
                }
                super::RuntimeArrayLength::Bound(reference) => {
                    encoder.tag(1);
                    encoder.ordinal(reference.depth());
                    encoder.ordinal(u32::from(reference.slot()));
                }
            }
        }
        encoder.count(row.arguments.effects.len());
        for effect in &row.arguments.effects {
            encoder.enter_element();
            effect.encode(encoder)?;
        }
        encoder.count(row.states.len());
        for state in &row.states {
            encoder.enter_element();
            self.write_callable_state(encoder, state.source)?;
            self.write_callable_state(encoder, state.target)?;
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_content_template(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let manifest = self
            .plan
            .dialogue_content_templates()
            .get(template)
            .ok_or_else(|| {
                encoder.reject_owner();
                RuntimeBodySemanticError::MissingRow {
                    table: "content templates",
                    ordinal: template.index(),
                }
            })?;
        encoder.digest(manifest.digest().as_bytes());
        encoder.count(manifest.slots().len());
        for slot in manifest.slots() {
            encoder.enter_element();
            encoder.ordinal(slot.slot().get().get() - 1);
            encoder.tag(slot.role().encoded());
            encoder.digest(slot.semantic_type().as_bytes());
        }
        encoder.count(manifest.effects().len());
        for effect in manifest.effects() {
            encoder.enter_element();
            encoder.ordinal(effect.site().get().get() - 1);
            effect.trigger().encode_body_metadata(encoder);
            encoder.count(effect.capture_types().len());
            for ty in effect.capture_types() {
                encoder.enter_element();
                self.write_type(encoder, *ty)?;
            }
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_format_attempt(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.format_attempt(attempt).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "format attempts",
                ordinal: attempt.index(),
            }
        })?;
        self.write_content_template(encoder, row.template())?;
        encoder.count(row.operands().len());
        for operand in row.operands() {
            encoder.enter_element();
            encoder.count(operand.parameter().index());
            self.write_type(encoder, operand.ty())?;
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_method(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        method: super::RuntimeTraitMethodId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.trait_methods().get(method.0).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "trait methods",
                ordinal: method.0,
            }
        })?;
        encoder.digest(row.definition.as_bytes());
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_helper(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        helper: super::RuntimePureHelperId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.pure_helpers().get(helper.0).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "pure helpers",
                ordinal: helper.0,
            }
        })?;
        encoder.digest(row.definition.as_bytes());
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_pure_program_reference(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let binding = self.plan.resolve_pure_program(program).map_err(|reason| {
            encoder.reject_owner();
            match reason {
                super::RuntimePureProgramLookupError::Missing => {
                    RuntimeBodySemanticError::UnknownPureProgram { program }
                }
                super::RuntimePureProgramLookupError::Ambiguous => {
                    RuntimeBodySemanticError::AmbiguousPureProgram { program }
                }
            }
        })?;
        self.write_function_reference(encoder, binding.site())
    }

    fn write_initialization(
        encoder: &mut TaskSemanticEncoder<'_>,
        state: RuntimePlaceInitialization,
    ) {
        encoder.tag(match state {
            RuntimePlaceInitialization::Initialized => 0,
            RuntimePlaceInitialization::Uninitialized => 1,
            RuntimePlaceInitialization::Conditional => 2,
        });
    }
}

#[cfg(test)]
mod tests;
