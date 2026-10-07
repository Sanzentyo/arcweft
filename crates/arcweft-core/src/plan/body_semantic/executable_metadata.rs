//! Plan-resolved executable leaf metadata; body traversal stays with its owners.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan;
use crate::task::semantic::TaskSemanticEncoder;

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn write_iterator(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        evidence: &plan::RuntimeIteratorEvidence,
    ) -> Result<(), RuntimeBodySemanticError> {
        match evidence {
            plan::RuntimeIteratorEvidence::Builtin(value) => {
                encoder.tag(0);
                encoder.tag(match value.family {
                    plan::RuntimeBuiltinIteratorFamily::Range => 0,
                    plan::RuntimeBuiltinIteratorFamily::Seq => 1,
                    plan::RuntimeBuiltinIteratorFamily::Stream => 2,
                    plan::RuntimeBuiltinIteratorFamily::Vec => 3,
                    plan::RuntimeBuiltinIteratorFamily::Array => 4,
                    plan::RuntimeBuiltinIteratorFamily::Slice => 5,
                    plan::RuntimeBuiltinIteratorFamily::TupleHomogeneous => 6,
                });
                self.write_type(encoder, value.item)?;
                self.write_type(encoder, value.iterator)?;
                self.write_type(encoder, value.next_value)?;
                self.write_type(encoder, value.step)?;
            }
            plan::RuntimeIteratorEvidence::Witness(value) => {
                encoder.tag(1);
                self.write_type(encoder, value.item)?;
                self.write_type(encoder, value.iterator)?;
                match value.executable {
                    plan::RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next } => {
                        encoder.tag(0);
                        self.write_method(encoder, into_iter)?;
                        self.write_method(encoder, next)?;
                    }
                    plan::RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next } => {
                        encoder.tag(1);
                        self.write_method(encoder, next)?;
                    }
                }
            }
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_line_operation(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        operation: &plan::RuntimeLineOperation,
    ) -> Result<(), RuntimeBodySemanticError> {
        use plan::RuntimeLineOperation as Op;
        // A site is scoped to its containing line group; its coordinate is
        // a source role, never a program-wide semantic identity.
        encoder.ordinal(operation.site().get());
        match operation {
            Op::AcquireActor {
                character, scope, ..
            } => {
                encoder.tag(0);
                encoder.string(character.as_str());
                encoder.tag(match scope {
                    crate::line_task::RuntimeLineHandleScope::Line => 0,
                });
            }
            Op::Schedule {
                child, captures, ..
            } => {
                encoder.tag(1);
                encoder.count(child.index());
                encoder.count(captures.len());
                for capture in captures {
                    encoder.enter_element();
                    self.write_local(encoder, capture.local())?;
                }
            }
            Op::ActorLook {
                character, actor, ..
            } => {
                encoder.tag(2);
                encoder.string(character.as_str());
                self.write_local(encoder, actor.local())?;
            }
            Op::VoiceHandle { .. } => encoder.tag(3),
        }
        encoder.status().map_err(Into::into)
    }

    pub(crate) fn write_project_call(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        site: crate::runtime_id::RuntimeProjectCallSiteId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.project_call_sites().get(site).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "project calls",
                ordinal: site.index(),
            }
        })?;
        let call = row.plan();
        self.write_expression(encoder, call.callee())?;
        self.write_callable_state(encoder, call.state())?;
        encoder.ordinal(call.completed_group());
        encoder.count(call.operands().len());
        for operand in call.operands() {
            encoder.enter_element();
            encoder.tag(operand.mode().semantic_tag());
            self.write_expression(encoder, operand.value())?;
        }
        encoder.count(call.ordinary().len());
        for parameter in call.ordinary() {
            encoder.enter_element();
            encoder.ordinal(parameter.parameter());
            self.write_type(encoder, parameter.abi_ty())?;
            self.write_type(encoder, parameter.binding_ty())?;
            match parameter {
                plan::RuntimeProjectCallOrdinaryMaterialization::Fixed(value) => {
                    encoder.tag(0);
                    encoder.ordinal(value.source_index());
                }
                plan::RuntimeProjectCallOrdinaryMaterialization::Rest(value) => {
                    encoder.tag(1);
                    encoder.count(value.source_indices().len());
                    for source in value.source_indices() {
                        encoder.enter_element();
                        encoder.ordinal(*source);
                    }
                }
            }
        }
        encoder.tag(u8::from(call.attached().is_some()));
        if let Some(attached) = call.attached() {
            self.write_type(encoder, attached.abi_ty())?;
            self.write_type(encoder, attached.binding_ty())?;
            encoder.tag(match attached.presence() {
                plan::RuntimeProjectCallAttachedPresence::RequiredPresent => 0,
                plan::RuntimeProjectCallAttachedPresence::OptionalPresent => 1,
                plan::RuntimeProjectCallAttachedPresence::OptionalOmitted => 2,
                plan::RuntimeProjectCallAttachedPresence::DefaultedPresent => 3,
                plan::RuntimeProjectCallAttachedPresence::DefaultedOmitted => 4,
            });
            encoder.tag(u8::from(attached.source_index().is_some()));
            if let Some(source) = attached.source_index() {
                encoder.ordinal(source);
            }
        }
        self.write_node(
            encoder,
            crate::value::RuntimeExpressionNode::Pattern(row.result()),
        )?;
        encoder.status().map_err(Into::into)
    }

    /// Full input signature metadata resolves declaration identities before
    /// physical frame coordinates. The body's semantic root is encoded by
    /// the same context's expression/Flow owner, not by this reference.
    pub(crate) fn write_function_signature(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        function: crate::runtime_id::RuntimeFunctionSiteId,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::plan::{
            RuntimeFunctionCaptureMode as Capture, RuntimeFunctionInputOrigin as Origin,
            RuntimeFunctionInputOwnershipRequirement as Ownership,
            RuntimeFunctionInputSource as Source, RuntimeFunctionInputTransfer as Transfer,
        };
        self.write_function_reference(encoder, function)?;
        let site = self.plan.function_sites().get(function).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "function signatures",
                ordinal: function.get().get() as usize - 1,
            }
        })?;
        encoder.tag(u8::from(site.function_type().is_some()));
        if let Some(ty) = site.function_type() {
            self.write_type(encoder, ty)?;
        }
        encoder.count(site.inputs().len());
        for input in site.inputs() {
            encoder.enter_element();
            match input.source() {
                Source::Capture { position } => {
                    encoder.tag(0);
                    encoder.ordinal(position);
                }
                Source::CapturedParameter { position, passing } => {
                    encoder.tag(1);
                    encoder.ordinal(position);
                    encoder.tag(passing.semantic_tag());
                }
                Source::Parameter { position, passing } => {
                    encoder.tag(2);
                    encoder.ordinal(position);
                    encoder.tag(passing.semantic_tag());
                }
            }
            match input.origin() {
                Origin::Binding(identity) => {
                    encoder.tag(0);
                    encoder.digest(&identity);
                }
                Origin::Parameter(identity) => {
                    encoder.tag(1);
                    encoder.digest(identity.as_bytes());
                }
                Origin::EvaluatedResult(identity) => {
                    encoder.tag(2);
                    encoder.digest(identity.as_bytes());
                }
            }
            match input.transfer() {
                Transfer::Transferred(mode) => {
                    encoder.tag(0);
                    encoder.tag(match mode {
                        Capture::Copy => 0,
                        Capture::SnapshotClone => 1,
                        Capture::Move => 2,
                    });
                }
                Transfer::ExternalBinding => encoder.tag(1),
                Transfer::Formal => encoder.tag(2),
            }
            encoder.tag(match input.ownership() {
                Ownership::Owned => 0,
                Ownership::Unrestricted => 1,
            });
            self.write_local(encoder, input.input_local())?;
            self.write_node(
                encoder,
                crate::value::RuntimeExpressionNode::Pattern(input.pattern()),
            )?;
            encoder.count(input.unrestricted_bindings().len());
            for local in input.unrestricted_bindings() {
                encoder.enter_element();
                self.write_local(encoder, *local)?;
            }
        }
        encoder.tag(match site.body() {
            crate::plan::RuntimeFunctionSiteBody::Expression(_) => 0,
            crate::plan::RuntimeFunctionSiteBody::Executable(_) => 1,
        });
        encoder.status().map_err(Into::into)
    }
    pub(crate) fn write_dialogue_content(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        content: crate::runtime_id::RuntimeDialogueContentPlanId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let row = self.plan.dialogue_content().get(content).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "dialogue content",
                ordinal: content.get().get() as usize - 1,
            }
        })?;
        self.write_content_template(encoder, row.template())?;
        encoder.count(row.values().len());
        for value in row.values() {
            encoder.enter_element();
            encoder.ordinal(value.slot().get().get() - 1);
            encoder.tag(value.role().encoded());
            self.write_function_reference(encoder, value.function())?;
            encoder.count(value.captures().len());
            for capture in value.captures() {
                encoder.enter_element();
                self.write_expression(encoder, capture)?;
            }
        }
        encoder.count(row.effect_sites().len());
        for effect in row.effect_sites() {
            encoder.enter_element();
            encoder.ordinal(effect.site().get().get() - 1);
            self.write_callable_state(encoder, effect.state())?;
            encoder.count(effect.captures().len());
            for capture in effect.captures() {
                encoder.enter_element();
                self.write_expression(encoder, capture)?;
            }
        }
        encoder.count(row.marks().len());
        for mark in row.marks() {
            encoder.enter_element();
            encoder.count(mark.id().index());
        }
        encoder.count(row.effect_site_count().get() as usize);
        encoder.tag(u8::from(row.line_task_group().is_some()));
        if let Some(group) = row.line_task_group() {
            let group = self
                .plan
                .line_task_groups()
                .get(group.index())
                .ok_or_else(|| {
                    encoder.reject_owner();
                    RuntimeBodySemanticError::MissingRow {
                        table: "line task groups",
                        ordinal: group.index(),
                    }
                })?;
            encoder.digest(group.definition().as_bytes());
        }
        encoder.status().map_err(Into::into)
    }
    /// Encodes the actual signature and owned body under one row owner.
    /// Code references remain accepted-definition leaves, allowing ordinary
    /// recursive calls without recursively re-expanding function definitions.
    pub(crate) fn function_row_digest(
        &self,
        meter: &mut crate::task::semantic::TaskSemanticMeter,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        task_owner: &crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner,
        task_reference: &mut impl FnMut(
            super::flow::RuntimeBodyTaskSource<'_>,
        ) -> Result<
            crate::plan::construction::task_coordinates::RuntimeTaskPlanBuildCoordinate,
            RuntimeBodySemanticError,
        >,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.function-row.v1\0", meter);
        self.write_function_signature(&mut encoder, function)?;
        let site = self.plan.function_sites().get(function).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "function body",
                ordinal: function.get().get() as usize - 1,
            }
        })?;
        match site.body() {
            plan::RuntimeFunctionSiteBody::Expression(body) => {
                self.write_expression(&mut encoder, body)?;
            }
            plan::RuntimeFunctionSiteBody::Executable(body) => {
                encoder.count(body.effects().len());
                for effect in body.effects().iter() {
                    encoder.enter_element();
                    encoder.string(effect.as_str());
                }
                self.write_flow(&mut encoder, body.ops(), task_owner, task_reference)?;
            }
        }
        encoder.finish().map_err(Into::into)
    }
}
