//! Authoritative argument mapping and checking for registered calls.

use arcweft_lang_syntax::expr::{
    ArgumentListSyntax, CallArg, CallArgumentRecoverySyntax, CallExpr, Expr,
};
use arcweft_source::SourceSpan;

use super::super::support::{FixedLiteralSpreadSlot, fixed_literal_spread_slots, spread_item_type};
use super::facts::{ArgumentFactBuilder, RegisteredArgumentSlot};
use crate::{
    callable::{
        CallPoison, CallableDiagnosticCode, CallableDiagnosticSubject, CallableGroupIndex,
        CallableParameter, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallableSignatureSchema,
        CheckedCallArgumentFact, CheckedCallArgumentSlotFact, SignatureQueryStep,
        SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    checker::{
        CallableDiagnosticDraft, CandidateExpectedType, PhysicalArgumentEvaluationKind,
        TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    },
    types::TypeParameterSubstitutions,
};

#[derive(Clone)]
pub(super) struct RegisteredArgumentCheck {
    pub(super) facts: Vec<CheckedCallArgumentFact>,
    pub(super) poison: CallPoison,
    pub(super) diagnostics: Vec<CallableDiagnosticDraft>,
    pub(super) substitutions: TypeParameterSubstitutions,
}

impl RegisteredArgumentCheck {
    pub(super) fn new(facts: Vec<CheckedCallArgumentFact>, poison: CallPoison) -> Self {
        let poison = facts
            .iter()
            .fold(poison, |combined, fact| combined.merge(fact.poison()));
        Self {
            facts,
            poison,
            diagnostics: Vec::new(),
            substitutions: TypeParameterSubstitutions::default(),
        }
    }

    #[must_use]
    fn with_diagnostics(mut self, diagnostics: Vec<CallableDiagnosticDraft>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    #[must_use]
    pub(super) fn with_substitutions(mut self, substitutions: TypeParameterSubstitutions) -> Self {
        self.substitutions = substitutions;
        self
    }
}

struct RegisteredSpreadResult {
    poison: CallPoison,
    shape_rejected: bool,
}

struct RegisteredArgumentMappingState {
    provided: Vec<bool>,
    first_bindings: Vec<Option<SourceSpan>>,
    positional: usize,
    fact_builders: Option<Vec<ArgumentFactBuilder>>,
    diagnostics: Vec<CallableDiagnosticDraft>,
    poison: CallPoison,
    spread_shape_rejected: bool,
    positional_mapping_stopped: bool,
    substitutions: TypeParameterSubstitutions,
}

impl RegisteredArgumentMappingState {
    fn new(
        parameter_count: usize,
        implicit: Option<CallableParameterIndex>,
        fact_builders: Option<Vec<ArgumentFactBuilder>>,
    ) -> Self {
        let mut provided = vec![false; parameter_count];
        if let Some(implicit) = implicit
            && let Some(slot) = provided.get_mut(implicit.get())
        {
            *slot = true;
        }
        Self {
            provided,
            first_bindings: vec![None; parameter_count],
            positional: 0,
            fact_builders,
            diagnostics: Vec::new(),
            poison: CallPoison::Clean,
            spread_shape_rejected: false,
            positional_mapping_stopped: false,
            substitutions: TypeParameterSubstitutions::default(),
        }
    }

    fn inference(&mut self) -> RegisteredArgumentInference<'_> {
        RegisteredArgumentInference::new(&mut self.fact_builders, &mut self.substitutions)
    }
}

pub(super) struct RegisteredSlotCheck {
    pub(super) poison: CallPoison,
    pub(super) inferred: Option<TypeKind>,
    pub(super) expression: Option<TypeExpressionId>,
    pub(super) source: Option<SourceSpan>,
}

#[derive(Clone, Copy)]
pub(super) struct RegisteredArgumentEvaluation {
    kind: PhysicalArgumentEvaluationKind,
    poison: CallPoison,
}

impl RegisteredArgumentEvaluation {
    pub(super) const fn new(kind: PhysicalArgumentEvaluationKind, poison: CallPoison) -> Self {
        Self { kind, poison }
    }
}

pub(super) struct RegisteredArgumentInference<'a> {
    fact_builders: &'a mut Option<Vec<ArgumentFactBuilder>>,
    substitutions: &'a mut TypeParameterSubstitutions,
}

impl<'a> RegisteredArgumentInference<'a> {
    pub(super) fn new(
        fact_builders: &'a mut Option<Vec<ArgumentFactBuilder>>,
        substitutions: &'a mut TypeParameterSubstitutions,
    ) -> Self {
        Self {
            fact_builders,
            substitutions,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct RegisteredArgumentContext<'a> {
    pub(super) label: &'a str,
    pub(super) schema: &'a CallableSignatureSchema,
    pub(super) group: CallableGroupIndex,
    pub(super) parameters: &'a [CallableParameter],
    pub(super) call: &'a CallExpr,
    pub(super) focused: bool,
}

struct RegisteredPositionalCheck<'a> {
    context: RegisteredArgumentContext<'a>,
    value: FixedLiteralSpreadSlot<'a>,
    mapping: &'a mut RegisteredArgumentMappingState,
    argument_index: usize,
    evaluation_kind: PhysicalArgumentEvaluationKind,
}

struct RegisteredNamedCheck<'a> {
    context: RegisteredArgumentContext<'a>,
    name: &'a str,
    value: &'a Expr,
    mapping: &'a mut RegisteredArgumentMappingState,
    argument_index: usize,
}

struct RegisteredSpreadCheck<'a> {
    context: RegisteredArgumentContext<'a>,
    value: &'a Expr,
    mapping: &'a mut RegisteredArgumentMappingState,
    argument_index: usize,
}

impl TypeChecker<'_> {
    pub(super) fn check_registered_schema_args(
        &mut self,
        label: &str,
        schema: &CallableSignatureSchema,
        current_group: CallableGroupIndex,
        call: &CallExpr,
        records_facts: bool,
    ) -> RegisteredArgumentCheck {
        self.check_registered_schema_args_with_implicit(
            label,
            schema,
            current_group,
            call,
            records_facts,
            None,
        )
    }

    pub(super) fn begin_registered_candidate_argument_probe(
        &mut self,
        call: &CallExpr,
        focused: bool,
    ) -> bool {
        if focused
            && let Err(error) = self
                .call_resolver_control
                .check_signature_query_step(SignatureQueryStep::CandidateArgumentProbe)
        {
            self.errors.push(TypeCheckError::new(error.to_string()));
            let call_span = self.source_span_for_current_range(call.range());
            self.call_target_fact_recorder
                .record_resolve_error(call_span.as_ref(), error);
            return false;
        }
        if !self.charge_callable_work(
            call,
            focused,
            crate::checker::call_target_facts::CallableWorkOperation::ArgumentMapping,
        ) {
            return false;
        }
        !self.signature_work_charge.candidate_work
            || self.charge_signature_work(crate::callable::SignatureWorkKind::SpecificityChecks, 1)
    }

    pub(super) fn is_focused_registered_call(&self, call: &CallExpr) -> bool {
        let call_span = self.source_span_for_current_range(call.range());
        self.uses_focused_callable_work(call_span.as_ref())
    }

    pub(super) fn check_registered_schema_args_with_implicit(
        &mut self,
        label: &str,
        schema: &CallableSignatureSchema,
        current_group: CallableGroupIndex,
        call: &CallExpr,
        records_facts: bool,
        implicit: Option<CallableParameterIndex>,
    ) -> RegisteredArgumentCheck {
        let args = call.args();
        let focused = self.is_focused_registered_call(call);
        let Some(group) = schema.group(current_group) else {
            self.errors.push(TypeCheckError::new(format!(
                "registered callable `{label}` has no parameter group {}",
                current_group.get()
            )));
            return RegisteredArgumentCheck::new(
                self.check_unmapped_registered_arguments(call, CallPoison::Rejected, records_facts),
                CallPoison::Rejected,
            );
        };
        let context = RegisteredArgumentContext {
            label,
            schema,
            group: current_group,
            parameters: group.parameters(),
            call,
            focused,
        };
        let mut mapping = RegisteredArgumentMappingState::new(
            context.parameters.len(),
            implicit,
            self.registered_argument_fact_builders(call, records_facts),
        );
        self.check_registered_authored_arguments(context, args, &mut mapping);
        let missing_insertion = call.syntax().argument_list().and_then(|arguments| {
            let offset = arguments.content_range().end();
            self.source_span_for_current_range(arcweft_lang_syntax::ast::common::TextRange::new(
                offset, offset,
            ))
        });
        for parameter in context.parameters {
            if !mapping.spread_shape_rejected
                && !mapping.provided[parameter.index().get()]
                && parameter.presence() == CallableParameterPresence::Required
                && !matches!(
                    parameter.passing(),
                    CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
                )
            {
                self.errors.push(TypeCheckError::new(format!(
                    "function `{label}` missing required argument `{}`",
                    parameter_label(parameter)
                )));
                mapping.diagnostics.push(CallableDiagnosticDraft::error(
                    CallableDiagnosticCode::MissingArgument,
                    missing_insertion.clone(),
                    CallableDiagnosticSubject::Parameter(
                        crate::callable::CallableParameterCoordinate::new(
                            current_group,
                            parameter.index(),
                        ),
                    ),
                ));
                mapping.poison = CallPoison::Rejected;
            }
        }
        let facts = mapping.fact_builders.map_or_else(Vec::new, |builders| {
            builders
                .into_iter()
                .map(ArgumentFactBuilder::finish)
                .collect::<Vec<_>>()
        });
        RegisteredArgumentCheck::new(facts, mapping.poison)
            .with_diagnostics(mapping.diagnostics)
            .with_substitutions(mapping.substitutions)
    }

    pub(in crate::checker::expr) fn check_unmapped_registered_arguments(
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
            .map(ArgumentListSyntax::arguments);
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
            self.record_physical_candidate_argument_evaluation(
                call,
                argument_index,
                PhysicalArgumentEvaluationKind::Unmapped,
                CandidateExpectedType::Unmapped,
            );
            #[cfg(test)]
            {
                self.stats.registered_argument_expression_checks += 1;
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
                    expected: None,
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

    fn check_registered_authored_arguments(
        &mut self,
        context: RegisteredArgumentContext<'_>,
        args: &[CallArg],
        mapping: &mut RegisteredArgumentMappingState,
    ) {
        for (argument_index, arg) in args.iter().enumerate() {
            if !self.begin_registered_candidate_argument_probe(context.call, context.focused) {
                mapping.poison = CallPoison::Rejected;
                break;
            }
            let argument_poison = match arg {
                CallArg::Positional(value) if mapping.positional_mapping_stopped => self
                    .check_registered_argument_slot(
                        context,
                        None,
                        FixedLiteralSpreadSlot::Expr(value),
                        argument_index,
                        mapping,
                        RegisteredArgumentEvaluation::new(
                            authored_argument_evaluation_kind(context.call, argument_index),
                            CallPoison::Rejected,
                        ),
                    ),
                CallArg::Positional(value) => {
                    self.check_registered_positional(RegisteredPositionalCheck {
                        context,
                        value: FixedLiteralSpreadSlot::Expr(value),
                        mapping,
                        argument_index,
                        evaluation_kind: authored_argument_evaluation_kind(
                            context.call,
                            argument_index,
                        ),
                    })
                }
                CallArg::Named { name, value } => {
                    self.check_registered_named(RegisteredNamedCheck {
                        context,
                        name,
                        value,
                        mapping,
                        argument_index,
                    })
                }
                CallArg::Spread { value } => {
                    let spread = if mapping.positional_mapping_stopped {
                        RegisteredSpreadResult {
                            poison: self.check_registered_argument_slot(
                                context,
                                None,
                                FixedLiteralSpreadSlot::Expr(value),
                                argument_index,
                                mapping,
                                RegisteredArgumentEvaluation::new(
                                    authored_argument_evaluation_kind(context.call, argument_index),
                                    CallPoison::Rejected,
                                ),
                            ),
                            shape_rejected: false,
                        }
                    } else {
                        self.check_registered_spread(RegisteredSpreadCheck {
                            context,
                            value,
                            mapping,
                            argument_index,
                        })
                    };
                    mapping.spread_shape_rejected |= spread.shape_rejected;
                    mapping.positional_mapping_stopped |= spread.shape_rejected;
                    if spread.shape_rejected {
                        let source = mapping
                            .fact_builders
                            .as_ref()
                            .and_then(|builders| builders.get(argument_index))
                            .and_then(|builder| builder.source.clone());
                        let subject = mapping
                            .fact_builders
                            .as_ref()
                            .and_then(|builders| builders.get(argument_index))
                            .and_then(|builder| builder.slots.first())
                            .map(CheckedCallArgumentSlotFact::expression)
                            .map_or(
                                CallableDiagnosticSubject::None,
                                CallableDiagnosticSubject::Argument,
                            );
                        mapping.diagnostics.push(CallableDiagnosticDraft::error(
                            CallableDiagnosticCode::UnsupportedSpread,
                            source,
                            subject,
                        ));
                    }
                    spread.poison
                }
            };
            mapping.poison = mapping.poison.merge(argument_poison);
        }
    }

    fn check_registered_positional(&mut self, input: RegisteredPositionalCheck<'_>) -> CallPoison {
        let RegisteredPositionalCheck {
            context,
            value,
            mapping,
            argument_index,
            evaluation_kind,
        } = input;
        let prior_positional = mapping.positional;
        let Some(parameter) = next_registered_positional_parameter(
            context.parameters,
            &mapping.provided,
            &mut mapping.positional,
        ) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` received too many positional arguments",
                context.label
            )));
            let checked = self.check_registered_argument_slot_with_inferred(
                context,
                None,
                value,
                argument_index,
                mapping.inference(),
                RegisteredArgumentEvaluation::new(evaluation_kind, CallPoison::Rejected),
            );
            mapping.diagnostics.push(CallableDiagnosticDraft::error(
                CallableDiagnosticCode::TooManyPositionalArguments,
                checked.source.clone(),
                checked.expression.map_or(
                    CallableDiagnosticSubject::None,
                    CallableDiagnosticSubject::Argument,
                ),
            ));
            return checked.poison;
        };
        let index = parameter.index().get();
        let skipped_bindings = (prior_positional..index)
            .filter_map(|skipped| {
                let parameter = context.parameters.get(skipped)?;
                matches!(
                    parameter.passing(),
                    CallableParameterPassing::PositionalOnly
                        | CallableParameterPassing::PositionalOrNamed
                )
                .then(|| {
                    mapping.first_bindings[skipped].clone().map(|source| {
                        (
                            crate::callable::CallableParameterCoordinate::new(
                                context.group,
                                CallableParameterIndex::try_from_usize(skipped)
                                    .expect("schema parameter indices are validated"),
                            ),
                            source,
                        )
                    })
                })
                .flatten()
            })
            .collect::<Vec<_>>();
        if parameter.passing() != CallableParameterPassing::RestPositional {
            mapping.provided[index] = true;
            mapping.positional = index + 1;
        }
        let checked = self.check_registered_argument_slot_with_inferred(
            context,
            Some(parameter),
            value,
            argument_index,
            mapping.inference(),
            RegisteredArgumentEvaluation::new(evaluation_kind, CallPoison::Clean),
        );
        for (coordinate, first_source) in skipped_bindings {
            mapping.diagnostics.push(
                CallableDiagnosticDraft::error(
                    CallableDiagnosticCode::ParameterAlreadyBound,
                    checked.source.clone(),
                    CallableDiagnosticSubject::Parameter(coordinate),
                )
                .with_related(
                    CallableDiagnosticSubject::Parameter(coordinate),
                    Some(first_source),
                ),
            );
        }
        if parameter.passing() != CallableParameterPassing::RestPositional
            && mapping.first_bindings[index].is_none()
        {
            mapping.first_bindings[index].clone_from(&checked.source);
        }
        checked.poison
    }

    fn check_registered_named(&mut self, input: RegisteredNamedCheck<'_>) -> CallPoison {
        let RegisteredNamedCheck {
            context,
            name,
            value,
            mapping,
            argument_index,
        } = input;
        let parameter = registered_named_parameter(context.parameters, name);
        let Some(parameter) = parameter else {
            let poison = if context.schema.argument_policy().unknown_named()
                == UnknownNamedArgumentPolicy::Reject
            {
                self.errors.push(TypeCheckError::new(format!(
                    "function `{}` has no named parameter `{name}`",
                    context.label
                )));
                CallPoison::Rejected
            } else {
                CallPoison::Clean
            };
            let checked = self.check_registered_argument_slot_with_inferred(
                context,
                None,
                FixedLiteralSpreadSlot::Expr(value),
                argument_index,
                mapping.inference(),
                RegisteredArgumentEvaluation::new(
                    authored_argument_evaluation_kind(context.call, argument_index),
                    poison,
                ),
            );
            if poison == CallPoison::Rejected {
                let name_source = mapping
                    .fact_builders
                    .as_ref()
                    .and_then(|builders| builders.get(argument_index))
                    .and_then(|builder| builder.authored_name_source.clone());
                mapping.diagnostics.push(CallableDiagnosticDraft::error(
                    CallableDiagnosticCode::UnknownNamedArgument,
                    name_source,
                    checked.expression.map_or(
                        CallableDiagnosticSubject::None,
                        CallableDiagnosticSubject::Argument,
                    ),
                ));
            }
            return checked.poison;
        };
        let index = parameter.index().get();
        let mut poison = CallPoison::Clean;
        let duplicate =
            parameter.passing() != CallableParameterPassing::RestNamed && mapping.provided[index];
        if duplicate {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` argument `{name}` was provided more than once",
                context.label
            )));
            poison = CallPoison::Rejected;
        }
        mapping.provided[index] = true;
        let checked = self.check_registered_argument_slot_with_inferred(
            context,
            Some(parameter),
            FixedLiteralSpreadSlot::Expr(value),
            argument_index,
            mapping.inference(),
            RegisteredArgumentEvaluation::new(
                authored_argument_evaluation_kind(context.call, argument_index),
                poison,
            ),
        );
        let name_source = mapping
            .fact_builders
            .as_ref()
            .and_then(|builders| builders.get(argument_index))
            .and_then(|builder| builder.authored_name_source.clone());
        if duplicate {
            let coordinate =
                crate::callable::CallableParameterCoordinate::new(context.group, parameter.index());
            let diagnostic = CallableDiagnosticDraft::error(
                CallableDiagnosticCode::DuplicateArgument,
                name_source.clone(),
                CallableDiagnosticSubject::Parameter(coordinate),
            );
            mapping
                .diagnostics
                .push(if let Some(first) = mapping.first_bindings[index].clone() {
                    diagnostic.with_related(
                        CallableDiagnosticSubject::Parameter(coordinate),
                        Some(first),
                    )
                } else {
                    diagnostic
                });
        } else if parameter.passing() != CallableParameterPassing::RestNamed {
            mapping.first_bindings[index] = name_source.or(checked.source.clone());
        }
        checked.poison
    }

    fn check_registered_spread(
        &mut self,
        input: RegisteredSpreadCheck<'_>,
    ) -> RegisteredSpreadResult {
        let RegisteredSpreadCheck {
            context,
            value,
            mapping,
            argument_index,
        } = input;
        match context.schema.argument_policy().spread() {
            SpreadArgumentPolicy::Unchecked => RegisteredSpreadResult {
                poison: self.check_registered_argument_slot(
                    context,
                    None,
                    FixedLiteralSpreadSlot::Expr(value),
                    argument_index,
                    mapping,
                    RegisteredArgumentEvaluation::new(
                        authored_argument_evaluation_kind(context.call, argument_index),
                        CallPoison::Clean,
                    ),
                ),
                shape_rejected: false,
            },
            SpreadArgumentPolicy::Reject => {
                self.errors.push(TypeCheckError::new(format!(
                    "function `{}` does not accept spread arguments",
                    context.label
                )));
                RegisteredSpreadResult {
                    poison: self.check_registered_argument_slot(
                        context,
                        None,
                        FixedLiteralSpreadSlot::Expr(value),
                        argument_index,
                        mapping,
                        RegisteredArgumentEvaluation::new(
                            authored_argument_evaluation_kind(context.call, argument_index),
                            CallPoison::Rejected,
                        ),
                    ),
                    shape_rejected: true,
                }
            }
            SpreadArgumentPolicy::FixedLiteralOnly => {
                let Some(slots) = fixed_literal_spread_slots(value) else {
                    self.errors.push(TypeCheckError::new(format!(
                        "function `{}` does not accept non-literal spread arguments",
                        context.label
                    )));
                    return RegisteredSpreadResult {
                        poison: self.check_registered_argument_slot(
                            context,
                            None,
                            FixedLiteralSpreadSlot::Expr(value),
                            argument_index,
                            mapping,
                            RegisteredArgumentEvaluation::new(
                                authored_argument_evaluation_kind(context.call, argument_index),
                                CallPoison::Rejected,
                            ),
                        ),
                        shape_rejected: true,
                    };
                };
                RegisteredSpreadResult {
                    poison: self.check_registered_fixed_spread(
                        RegisteredSpreadCheck {
                            context,
                            value,
                            mapping,
                            argument_index,
                        },
                        slots,
                    ),
                    shape_rejected: false,
                }
            }
            SpreadArgumentPolicy::TypedRest => {
                if let Some(slots) = fixed_literal_spread_slots(value) {
                    RegisteredSpreadResult {
                        poison: self.check_registered_fixed_spread(
                            RegisteredSpreadCheck {
                                context,
                                value,
                                mapping,
                                argument_index,
                            },
                            slots,
                        ),
                        shape_rejected: false,
                    }
                } else {
                    self.check_registered_typed_rest_spread(context, value, mapping, argument_index)
                }
            }
        }
    }

    fn check_registered_fixed_spread(
        &mut self,
        input: RegisteredSpreadCheck<'_>,
        slots: Vec<FixedLiteralSpreadSlot<'_>>,
    ) -> CallPoison {
        let RegisteredSpreadCheck {
            context,
            value,
            mapping,
            argument_index,
        } = input;
        self.reserve_fixed_literal_spread_container_expr(value);
        let mut poison = CallPoison::Clean;
        for (slot_index, slot) in slots.into_iter().enumerate() {
            if slot_index != 0
                && !self.begin_registered_candidate_argument_probe(context.call, context.focused)
            {
                poison = CallPoison::Rejected;
                break;
            }
            poison = poison.merge(self.check_registered_positional(RegisteredPositionalCheck {
                context,
                value: slot,
                mapping,
                argument_index,
                evaluation_kind: PhysicalArgumentEvaluationKind::FixedLiteralSpread,
            }));
        }
        poison
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the typed-rest rule atomically validates placement, sequence item type, and checked facts"
    )]
    fn check_registered_typed_rest_spread(
        &mut self,
        context: RegisteredArgumentContext<'_>,
        value: &Expr,
        mapping: &mut RegisteredArgumentMappingState,
        argument_index: usize,
    ) -> RegisteredSpreadResult {
        let Some(rest) = context
            .parameters
            .iter()
            .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
        else {
            return self.reject_registered_typed_rest_spread(
                format!(
                    "function `{}` has no positional rest parameter",
                    context.label
                ),
                context,
                value,
                argument_index,
                mapping,
            );
        };
        if context.parameters.iter().any(|parameter| {
            parameter.presence() == CallableParameterPresence::Required
                && parameter.passing() != CallableParameterPassing::RestPositional
                && !mapping.provided[parameter.index().get()]
        }) {
            return self.reject_registered_typed_rest_spread(
                format!(
                    "function `{}` spread argument must follow required fixed arguments",
                    context.label
                ),
                context,
                value,
                argument_index,
                mapping,
            );
        }
        if self.signature_work_charge.candidate_work
            && !self.charge_signature_work(crate::callable::SignatureWorkKind::ArgumentBindings, 1)
        {
            return RegisteredSpreadResult {
                poison: CallPoison::Rejected,
                shape_rejected: false,
            };
        }
        let expression = TypeExpressionId::from_index(self.stats.expressions);
        let source = self.source_span_for_expr(value);
        if !self.charge_callable_work(
            context.call,
            context.focused,
            crate::checker::call_target_facts::CallableWorkOperation::TypeCheck,
        ) {
            return RegisteredSpreadResult {
                poison: CallPoison::Rejected,
                shape_rejected: false,
            };
        }
        self.record_physical_candidate_argument_evaluation(
            context.call,
            argument_index,
            PhysicalArgumentEvaluationKind::TypedRestSpread,
            CandidateExpectedType::Unchecked,
        );
        #[cfg(test)]
        {
            self.stats.registered_argument_expression_checks += 1;
        }
        let actual = self.check_expr(value);
        let Some(item) = actual.as_ref().and_then(spread_item_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` spread argument must have a sequence type",
                context.label
            )));
            Self::push_registered_argument_slot(
                RegisteredArgumentSlot {
                    argument_index,
                    expression,
                    source,
                    group: context.group,
                    parameter: Some(rest),
                    inferred: actual,
                    expected: match rest.ty() {
                        CallableParameterType::Exact(expected) => {
                            Some(mapping.substitutions.apply(expected))
                        }
                        CallableParameterType::Unchecked => None,
                    },
                    poison: CallPoison::Rejected,
                },
                &mut mapping.fact_builders,
            );
            return RegisteredSpreadResult {
                poison: CallPoison::Rejected,
                shape_rejected: false,
            };
        };
        let mut poison = CallPoison::Clean;
        let mut retained_expected = None;
        if let CallableParameterType::Exact(expected) = rest.ty() {
            let inferred = mapping.substitutions.observe(expected, item);
            let specialized_expected = mapping.substitutions.apply(expected);
            retained_expected = Some(specialized_expected.clone());
            if !inferred || !self.types_compatible(&specialized_expected, item) {
                self.errors.push(TypeCheckError::argument_type_mismatch(
                    context.label,
                    parameter_label(rest),
                    specialized_expected,
                    item.clone(),
                ));
                poison = CallPoison::Rejected;
            }
        }
        Self::push_registered_argument_slot(
            RegisteredArgumentSlot {
                argument_index,
                expression,
                source: source.clone(),
                group: context.group,
                parameter: Some(rest),
                inferred: actual,
                expected: retained_expected,
                poison,
            },
            &mut mapping.fact_builders,
        );
        RegisteredSpreadResult {
            poison,
            shape_rejected: false,
        }
    }

    fn reject_registered_typed_rest_spread(
        &mut self,
        message: String,
        context: RegisteredArgumentContext<'_>,
        value: &Expr,
        argument_index: usize,
        mapping: &mut RegisteredArgumentMappingState,
    ) -> RegisteredSpreadResult {
        self.errors.push(TypeCheckError::new(message));
        RegisteredSpreadResult {
            poison: self.check_registered_argument_slot(
                context,
                None,
                FixedLiteralSpreadSlot::Expr(value),
                argument_index,
                mapping,
                RegisteredArgumentEvaluation::new(
                    authored_argument_evaluation_kind(context.call, argument_index),
                    CallPoison::Rejected,
                ),
            ),
            shape_rejected: true,
        }
    }

    fn check_registered_argument_slot(
        &mut self,
        context: RegisteredArgumentContext<'_>,
        parameter: Option<&CallableParameter>,
        value: FixedLiteralSpreadSlot<'_>,
        argument_index: usize,
        mapping: &mut RegisteredArgumentMappingState,
        evaluation: RegisteredArgumentEvaluation,
    ) -> CallPoison {
        self.check_registered_argument_slot_with_inferred(
            context,
            parameter,
            value,
            argument_index,
            mapping.inference(),
            evaluation,
        )
        .poison
    }

    pub(super) fn check_registered_argument_slot_with_inferred(
        &mut self,
        context: RegisteredArgumentContext<'_>,
        parameter: Option<&CallableParameter>,
        value: FixedLiteralSpreadSlot<'_>,
        argument_index: usize,
        RegisteredArgumentInference {
            fact_builders,
            substitutions,
        }: RegisteredArgumentInference<'_>,
        RegisteredArgumentEvaluation {
            kind: evaluation_kind,
            mut poison,
        }: RegisteredArgumentEvaluation,
    ) -> RegisteredSlotCheck {
        if self.signature_work_charge.candidate_work
            && !self.charge_signature_work(crate::callable::SignatureWorkKind::ArgumentBindings, 1)
        {
            return RegisteredSlotCheck {
                poison: CallPoison::Rejected,
                inferred: None,
                expression: None,
                source: None,
            };
        }
        let expected = match parameter.map(CallableParameter::ty) {
            Some(CallableParameterType::Exact(expected)) => Some(expected),
            Some(CallableParameterType::Unchecked) | None => None,
        };
        let expression = TypeExpressionId::from_index(self.stats.expressions);
        let source = value
            .source_expr()
            .and_then(|expr| self.source_span_for_expr(expr))
            .or_else(|| {
                value
                    .literal_source_range()
                    .and_then(|range| self.source_span_for_current_range(range))
            });
        if value.source_expr().is_none() {
            self.stats.expressions += 1;
        }
        if !self.charge_callable_work(
            context.call,
            context.focused,
            crate::checker::call_target_facts::CallableWorkOperation::TypeCheck,
        ) {
            return RegisteredSlotCheck {
                poison: CallPoison::Rejected,
                inferred: None,
                expression: Some(expression),
                source,
            };
        }
        let expected_for_check = expected.map(|expected| substitutions.apply(expected));
        let expected_evidence = match parameter.map(CallableParameter::ty) {
            Some(CallableParameterType::Exact(expected)) => {
                CandidateExpectedType::Exact(substitutions.apply(expected))
            }
            Some(CallableParameterType::Unchecked) => CandidateExpectedType::Unchecked,
            None => CandidateExpectedType::Unmapped,
        };
        self.record_physical_candidate_argument_evaluation(
            context.call,
            argument_index,
            evaluation_kind,
            expected_evidence,
        );
        #[cfg(test)]
        {
            self.stats.registered_argument_expression_checks += 1;
        }
        let actual = self.check_fixed_literal_spread_slot(value, expected_for_check.as_ref());
        if let (Some(expected), Some(actual)) = (expected, actual.as_ref()) {
            let inferred = substitutions.observe(expected, actual);
            let specialized_expected = substitutions.apply(expected);
            if !inferred || !self.types_compatible(&specialized_expected, actual) {
                self.errors.push(TypeCheckError::argument_type_mismatch(
                    context.label,
                    parameter.map_or_else(|| "<unmapped>".to_owned(), parameter_label),
                    specialized_expected,
                    actual.clone(),
                ));
                poison = CallPoison::Rejected;
            }
        }
        Self::push_registered_argument_slot(
            RegisteredArgumentSlot {
                argument_index,
                expression,
                source: source.clone(),
                group: context.group,
                parameter,
                inferred: actual.clone(),
                expected: expected_for_check,
                poison,
            },
            fact_builders,
        );
        RegisteredSlotCheck {
            poison,
            inferred: actual,
            expression: Some(expression),
            source,
        }
    }
}

pub(super) fn authored_argument_evaluation_kind(
    call: &CallExpr,
    argument_index: usize,
) -> PhysicalArgumentEvaluationKind {
    let recovered = call
        .syntax()
        .argument_list()
        .map(ArgumentListSyntax::arguments)
        .and_then(|arguments| arguments.get(argument_index))
        .is_some_and(|argument| {
            matches!(
                argument.recovery(),
                CallArgumentRecoverySyntax::Recovered { .. }
            )
        });
    if recovered {
        PhysicalArgumentEvaluationKind::Recovered
    } else {
        PhysicalArgumentEvaluationKind::Authored
    }
}

fn next_registered_positional_parameter<'a>(
    parameters: &'a [CallableParameter],
    provided: &[bool],
    positional: &mut usize,
) -> Option<&'a CallableParameter> {
    while let Some(parameter) = parameters.get(*positional) {
        if provided[*positional]
            || matches!(
                parameter.passing(),
                CallableParameterPassing::NamedOnly | CallableParameterPassing::RestNamed
            )
        {
            *positional += 1;
        } else {
            break;
        }
    }
    parameters.get(*positional).or_else(|| {
        parameters
            .iter()
            .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
    })
}

fn registered_named_parameter<'a>(
    parameters: &'a [CallableParameter],
    name: &str,
) -> Option<&'a CallableParameter> {
    parameters
        .iter()
        .find(|parameter| {
            parameter
                .name()
                .is_some_and(|candidate| candidate.as_str() == name)
                && matches!(
                    parameter.passing(),
                    CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::NamedOnly
                )
        })
        .or_else(|| {
            parameters
                .iter()
                .find(|parameter| parameter.passing() == CallableParameterPassing::RestNamed)
        })
}

fn parameter_label(parameter: &CallableParameter) -> String {
    parameter.name().map_or_else(
        || format!("#{}", parameter.index().get()),
        |name| name.as_str().to_owned(),
    )
}
