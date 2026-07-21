//! Allocation-on-demand argument facts for focused registered calls.

use arcweft_lang_syntax::expr::{CallArgumentRecoverySyntax, CallExpr};

use super::{ArgumentFactBuilder, RegisteredArgumentSlot};
use crate::{
    callable::{
        CallPoison, CallableArgumentIndex, CallableArgumentSlotIndex, CallableGroupIndex,
        CallableName, CallableParameterCoordinate, CallableParameterType, CheckedCallArgumentFact,
        CheckedCallArgumentSlotFact, CheckedCallArgumentSlotInput,
    },
    checker::{TypeCheckError, TypeChecker, TypeExpressionId},
};

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

    pub(super) fn check_unmapped_registered_arguments(
        &mut self,
        call: &CallExpr,
        poison: CallPoison,
        records_facts: bool,
    ) -> Vec<CheckedCallArgumentFact> {
        let mut builders = self.registered_argument_fact_builders(call, records_facts);
        let focused = self.is_focused_registered_call(call);
        let syntax = call
            .syntax()
            .argument_list()
            .map(arcweft_lang_syntax::expr::ArgumentListSyntax::arguments);
        for (argument_index, argument) in call.args().iter().enumerate() {
            if !self.begin_registered_candidate_argument_probe(call, focused) {
                break;
            }
            let expression = TypeExpressionId::from_index(self.stats.expressions);
            let source = syntax
                .and_then(|arguments| arguments.get(argument_index))
                .and_then(|argument| self.source_span_for_current_range(argument.value_range()))
                .or_else(|| self.source_span_for_expr(argument.value()));
            if !self.charge_callable_work(
                call,
                focused,
                crate::checker::call_target_facts::CallableWorkOperation::TypeCheck,
            ) {
                break;
            }
            let inferred = self.check_expr(argument.value());
            Self::push_registered_argument_slot(
                RegisteredArgumentSlot {
                    argument_index,
                    expression,
                    source,
                    group: CallableGroupIndex::ZERO,
                    parameter: None,
                    inferred,
                    poison,
                },
                &mut builders,
            );
        }
        builders.map_or_else(Vec::new, |builders| {
            builders
                .into_iter()
                .map(ArgumentFactBuilder::finish)
                .collect()
        })
    }
}

impl ArgumentFactBuilder {
    pub(super) fn finish(self) -> CheckedCallArgumentFact {
        CheckedCallArgumentFact::new(
            self.index,
            self.source,
            self.authored_name,
            self.spread,
            self.slots,
            self.poison,
        )
    }
}
