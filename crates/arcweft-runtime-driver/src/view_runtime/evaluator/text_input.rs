//! Projection of a retained String field into an adapter text-control session.

use super::{
    BundleViewDiagnosticCode, EvaluationFailure, MountRenderBuilder, MountedView,
    ViewDefinitionResource, ViewOccurrenceKey, ViewProgramInstruction,
};
use crate::view_runtime::{MountedViewTextInputSeal, mount_scoped_id};
use arcweft_bundle::resource_codec::view::{ViewInputKind, ViewRuntimeTextControlBounds};
use arcweft_core::value::RuntimeValue;
use arcweft_view::{ViewExecutionInput, ViewExecutionInputSource, ViewExpressionProgram};

impl<B: super::RuntimeCallBackend> super::ViewEvaluator<'_, B> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_text_input(
        &mut self,
        key: &ViewOccurrenceKey,
        definition: &ViewDefinitionResource,
        mounted: &mut MountedView,
        instruction: usize,
        input: ViewExecutionInput,
        label: Option<&ViewExpressionProgram>,
        builder: &mut MountRenderBuilder,
    ) -> Result<(), EvaluationFailure> {
        let invalid = || {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidControlFlow,
                Some(instruction),
                "text input has no exact retained field/element owner",
            )
        };
        let (path, ordinal, target) = builder.last_node.as_ref().ok_or_else(invalid)?;
        let ViewProgramInstruction::OpenElement { element, .. } = self
            .program
            .instructions
            .get(*ordinal as usize)
            .ok_or_else(invalid)?
        else {
            return Err(invalid());
        };
        let kind = ViewInputKind::from_element(*element).ok_or_else(invalid)?;
        let target = target.as_deref().ok_or_else(invalid)?;
        let ViewExecutionInputSource::Local(coordinate) = input.source else {
            return Err(invalid());
        };
        let local = mounted
            .execution_locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(&coordinate))
            .ok_or_else(invalid)?;
        let address = local.retained.clone().ok_or_else(invalid)?;
        let field = mounted.local_state.get(&address).ok_or_else(invalid)?;
        if field.value_type != input.value_type() {
            return Err(invalid());
        }
        let RuntimeValue::String(value) = &field.value else {
            return Err(invalid());
        };
        let value = value.clone();
        let seal_key = (path.clone(), *ordinal);
        let scoped_target = mount_scoped_id(builder.mount, target);
        let session = if let Some(seal) = mounted.text_input_seals.get(&seal_key) {
            if seal.address != address
                || seal.target != scoped_target
                || seal.value_type != input.value_type()
                || seal.secure != kind.is_secure()
            {
                return Err(invalid());
            }
            seal.session
        } else {
            let session = mounted.next_handler_seal_revision;
            mounted.next_handler_seal_revision = session.checked_add(1).ok_or_else(invalid)?;
            mounted.text_input_seals.insert(seal_key.clone(), MountedViewTextInputSeal {
                target: scoped_target.clone(), session, address, value_type: input.value_type(), secure: kind.is_secure(),
                selection: arcweft_bundle::resource_codec::view::ViewRuntimeTextSelection::collapsed_at_end(&value), revision: 0,
            });
            session
        };
        let label = label
            .map(|label| {
                match self.evaluate_expression_program(
                    &key.handle,
                    definition,
                    mounted,
                    instruction,
                    label,
                )? {
                    RuntimeValue::String(value) => Ok(value),
                    _ => Err(invalid()),
                }
            })
            .transpose()?;
        let bounds = self
            .program
            .text_control_bounds_for(target)
            .unwrap_or_else(|| {
                ViewRuntimeTextControlBounds::new(
                    48_000,
                    48_000,
                    420_000,
                    kind.default_text_control_height_milli(),
                )
            });
        let mut control = kind.retained_control(
            scoped_target,
            mount_scoped_id(builder.mount, definition.public_id.as_str()),
            session,
            value,
            label,
            bounds,
        );
        control.selection = mounted
            .text_input_seals
            .get(&seal_key)
            .ok_or_else(invalid)?
            .selection
            .clamped_to_text(&control.value);
        builder.text_inputs.push(control);
        if !builder.text_input_seals.insert(seal_key) {
            return Err(invalid());
        }
        Ok(())
    }
}
