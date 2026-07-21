//! Allocation-on-demand argument facts for whole-module and focused registered calls.

use arcweft_lang_syntax::expr::{CallArgumentRecoverySyntax, CallExpr};
use arcweft_source::SourceSpan;

use crate::{
    callable::{
        CallPoison, CallableArgumentIndex, CallableArgumentSlotIndex, CallableGroupIndex,
        CallableName, CallableParameter, CallableParameterCoordinate, CallableParameterType,
        CheckedCallArgumentFact, CheckedCallArgumentSlotFact, CheckedCallArgumentSlotInput,
    },
    checker::{TypeCheckError, TypeChecker, TypeExpressionId},
};

pub(super) struct ArgumentFactBuilder {
    index: CallableArgumentIndex,
    pub(super) source: Option<SourceSpan>,
    authored_name: Option<CallableName>,
    pub(super) authored_name_source: Option<SourceSpan>,
    spread: bool,
    pub(super) slots: Vec<CheckedCallArgumentSlotFact>,
    authored_poison: CallPoison,
    poison: CallPoison,
}

pub(super) struct RegisteredArgumentSlot<'a> {
    pub(super) argument_index: usize,
    pub(super) expression: TypeExpressionId,
    pub(super) source: Option<SourceSpan>,
    pub(super) group: CallableGroupIndex,
    pub(super) parameter: Option<&'a CallableParameter>,
    pub(super) inferred: Option<crate::checker::TypeKind>,
    pub(super) poison: CallPoison,
}

impl TypeChecker<'_> {
    pub(super) fn registered_argument_fact_builders(
        &mut self,
        call: &CallExpr,
        records_facts: bool,
    ) -> Option<Vec<ArgumentFactBuilder>> {
        if !records_facts {
            return None;
        }
        let syntax = call
            .syntax()
            .argument_list()
            .map(arcweft_lang_syntax::expr::ArgumentListSyntax::arguments);
        let mut builders = Vec::with_capacity(call.args().len());
        for (raw_index, argument) in call.args().iter().enumerate() {
            let Ok(index) = CallableArgumentIndex::try_from_usize(raw_index) else {
                self.errors.push(TypeCheckError::new(format!(
                    "call argument index {raw_index} cannot be retained"
                )));
                return None;
            };
            let argument_syntax = syntax.and_then(|arguments| arguments.get(index.get()));
            let source = argument_syntax
                .and_then(|argument| self.source_span_for_current_range(argument.range()))
                .or_else(|| self.source_span_for_expr(argument.value()));
            let authored_name_source = argument_syntax.and_then(|argument| match argument.form() {
                arcweft_lang_syntax::expr::CallArgumentFormSyntax::Named { name, .. } => {
                    self.source_span_for_current_range(*name)
                }
                arcweft_lang_syntax::expr::CallArgumentFormSyntax::Positional
                | arcweft_lang_syntax::expr::CallArgumentFormSyntax::Spread { .. } => None,
            });
            let authored_poison = if argument_syntax.is_some_and(|argument| {
                matches!(
                    argument.recovery(),
                    CallArgumentRecoverySyntax::Recovered { .. }
                )
            }) {
                CallPoison::Recovered
            } else {
                CallPoison::Clean
            };
            builders.push(ArgumentFactBuilder {
                index,
                source,
                authored_name: argument
                    .name()
                    .and_then(|name| CallableName::try_new(name).ok()),
                authored_name_source,
                spread: argument.is_spread(),
                slots: Vec::new(),
                authored_poison,
                poison: authored_poison,
            });
        }
        Some(builders)
    }

    pub(super) fn push_registered_argument_slot(
        slot: RegisteredArgumentSlot<'_>,
        fact_builders: &mut Option<Vec<ArgumentFactBuilder>>,
    ) {
        let Some(builder) = fact_builders
            .as_mut()
            .and_then(|builders| builders.get_mut(slot.argument_index))
        else {
            return;
        };
        let Ok(slot_index) = CallableArgumentSlotIndex::try_from_usize(builder.slots.len()) else {
            builder.poison = CallPoison::Rejected;
            return;
        };
        let mapped = slot
            .parameter
            .map(|parameter| CallableParameterCoordinate::new(slot.group, parameter.index()));
        let expected = slot.parameter.and_then(|parameter| match parameter.ty() {
            CallableParameterType::Exact(expected) => Some(expected.clone()),
            CallableParameterType::Unchecked => None,
        });
        let slot_poison = builder.authored_poison.merge(slot.poison);
        builder.poison = builder.poison.merge(slot_poison);
        builder.slots.push(CheckedCallArgumentSlotFact::new(
            CheckedCallArgumentSlotInput {
                slot: slot_index,
                expression: slot.expression,
                source: slot.source,
                mapped,
                inferred: slot.inferred,
                expected,
                poison: slot_poison,
            },
        ));
    }
}

impl ArgumentFactBuilder {
    pub(super) fn finish(self) -> CheckedCallArgumentFact {
        CheckedCallArgumentFact::new(
            self.index,
            self.source,
            self.authored_name,
            self.authored_name_source,
            self.spread,
            self.slots,
            self.poison,
        )
    }
}
