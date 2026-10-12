//! Iterator witness signatures borrowed from the selected executable authority.

use super::RuntimeProgramTypes;
use crate::{
    awbc::schema::{AwbcFrameSlotRole, AwbcTraitReceiverMode},
    pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeSemanticTypeId},
    plan::{RuntimePlanTypeProjection, RuntimeReceiverMode, RuntimeTraitMethodId},
};

impl RuntimeProgramTypes<'_> {
    /// Reads `(receiver state, item)` from one original `MutRef` method whose
    /// only parameter is its receiver and whose result is the builtin `Option`.
    /// Diagnostic method names and self-type strings are not type authority.
    pub(crate) fn iterator_witness_types(
        self,
        next: RuntimeTraitMethodId,
    ) -> Option<(RuntimeSemanticTypeId, RuntimeSemanticTypeId)> {
        match self {
            Self::Plan(plan) => {
                let method = plan.trait_methods().get(next.0)?;
                if method.id != next || method.receiver != RuntimeReceiverMode::MutRef {
                    return None;
                }
                let [receiver] = method.inputs.as_ref() else {
                    return None;
                };
                let state = plan.local_declarations().get(receiver.local())?.ty();
                let RuntimePlanTypeProjection::Option { item, .. } =
                    plan.type_table().get(method.body.ty())?.projection()
                else {
                    return None;
                };
                Some((
                    plan.type_table().get(state)?.semantic_identity(),
                    plan.type_table().get(*item)?.semantic_identity(),
                ))
            }
            Self::Awbc(program) => {
                let method = program.trait_methods.get(next.0)?;
                if method.receiver != AwbcTraitReceiverMode::MutRef {
                    return None;
                }
                let function = program.functions.get(method.function.index())?;
                if function.signature != method.signature {
                    return None;
                }
                let signature = program.signatures.get(method.signature.index())?;
                let [state] = signature.params.as_slice() else {
                    return None;
                };
                let layout = program.frame_layouts.get(function.frame_layout.index())?;
                let slot = layout.slots.get(method.receiver_state_slot?.index())?;
                if slot.role != AwbcFrameSlotRole::Parameter || slot.ty != *state {
                    return None;
                }
                let item = program.builtin_variant_payload_item(
                    signature.result?,
                    RuntimeBuiltinVariantCaseIdentity::OptionSome,
                )?;
                Some((
                    program
                        .runtime_types
                        .get(state.index())?
                        .semantic_identity(),
                    program.runtime_types.get(item.index())?.semantic_identity(),
                ))
            }
        }
    }
}
