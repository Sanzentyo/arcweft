//! Registered-catalog call checking.

use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    expr::{CallArg, CallExpr, Expr},
};

use super::{TypeCheckError, TypeChecker, TypeExpressionId, TypeKind};
use crate::{
    callable::{
        CallCallee, CallPoison, CallResolverRequest, CallSourceContext, CallableArgumentIndex,
        CallableCandidateId, CallableGroupIndex, CallableName, CallableParameter,
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallablePath,
        CallableSignatureSchema, CallableValidator, CheckedCallArgumentFact,
        CheckedCallArgumentSlotFact, CheckedCallTarget, LexicalCallableScope,
        NonEmptyResolvedCandidates, PRODUCTION_CALLABLE_LIMITS, ResolveCallOutcome,
        ResolvedCallTarget, ResolvedCallable, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    effect_model::EffectSite,
};

use super::support::{FixedLiteralSpreadSlot, fixed_literal_spread_slots, spread_item_type};

mod facts;

pub(super) enum RegisteredFreeCallOutcome {
    NotHandled,
    Checked(Option<TypeKind>),
}

pub(super) enum RegisteredMethodCallOutcome {
    NotHandled,
    Checked(Option<TypeKind>),
}

struct ArgumentFactBuilder {
    index: CallableArgumentIndex,
    source: Option<arcweft_source::SourceSpan>,
    authored_name: Option<CallableName>,
    spread: bool,
    slots: Vec<CheckedCallArgumentSlotFact>,
    authored_poison: CallPoison,
    poison: CallPoison,
}

struct RegisteredArgumentCheck {
    facts: Vec<CheckedCallArgumentFact>,
    poison: CallPoison,
}

struct RegisteredSpreadResult {
    poison: CallPoison,
    shape_rejected: bool,
}

struct RegisteredCallSite<'a> {
    label: &'a str,
    call: &'a CallExpr,
    call_span: Option<arcweft_source::SourceSpan>,
    callee_range: Option<arcweft_lang_syntax::ast::common::TextRange>,
    expression: TypeExpressionId,
    document: &'a arcweft_source::SourceDocumentIdentity,
}

struct RegisteredFreeResolutionSite<'a> {
    path: &'a CallablePath,
    call: &'a CallExpr,
    call_span: Option<arcweft_source::SourceSpan>,
    expression: TypeExpressionId,
    document: &'a arcweft_source::SourceDocumentIdentity,
}

#[derive(Clone, Copy)]
struct RegisteredArgumentContext<'a> {
    label: &'a str,
    schema: &'a CallableSignatureSchema,
    parameters: &'a [CallableParameter],
}

struct RegisteredPositionalCheck<'a> {
    context: RegisteredArgumentContext<'a>,
    value: FixedLiteralSpreadSlot<'a>,
    provided: &'a mut [bool],
    positional: &'a mut usize,
    argument_index: usize,
    fact_builders: &'a mut Option<Vec<ArgumentFactBuilder>>,
}

struct RegisteredNamedCheck<'a> {
    context: RegisteredArgumentContext<'a>,
    name: &'a str,
    value: &'a Expr,
    provided: &'a mut [bool],
    argument_index: usize,
    fact_builders: &'a mut Option<Vec<ArgumentFactBuilder>>,
}

struct RegisteredSpreadCheck<'a> {
    context: RegisteredArgumentContext<'a>,
    value: &'a Expr,
    provided: &'a mut [bool],
    positional: &'a mut usize,
    argument_index: usize,
    fact_builders: &'a mut Option<Vec<ArgumentFactBuilder>>,
}

struct RegisteredArgumentSlot<'a> {
    argument_index: usize,
    expression: TypeExpressionId,
    source: Option<arcweft_source::SourceSpan>,
    parameter: Option<&'a CallableParameter>,
    inferred: Option<TypeKind>,
    poison: CallPoison,
}

impl TypeChecker<'_> {
    pub(super) fn check_registered_catalog_free_call(
        &mut self,
        call: &CallExpr,
        expected: Option<&TypeKind>,
        expression: TypeExpressionId,
    ) -> RegisteredFreeCallOutcome {
        let callee = call.callee();
        let args = call.args();
        let (Some(world), Some(symbols)) = (self.registered_world, self.project_symbols) else {
            return RegisteredFreeCallOutcome::NotHandled;
        };
        let Some(path) = callable_path(callee) else {
            return RegisteredFreeCallOutcome::NotHandled;
        };

        if self.registered_free_path_has_value_receiver(callee, &path) {
            return RegisteredFreeCallOutcome::NotHandled;
        }

        if self.registered_free_path_is_deferred(&path) {
            return RegisteredFreeCallOutcome::NotHandled;
        }

        let lexical = self.registered_free_lexical_scope();
        let module = self
            .current_module
            .clone()
            .unwrap_or_else(CanonicalModulePath::crate_root);
        let Some(document) = symbols.source_identity(&module) else {
            self.errors.push(TypeCheckError::new(format!(
                "call `{}` has no accepted source identity",
                path.leaf().as_str()
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return RegisteredFreeCallOutcome::Checked(None);
        };
        let call_span = self.source_span_for_current_range(call.range());
        let callee_span = self.source_span_for_current_range(call.callee_range());
        let (cancellation, work) = self.call_resolver_control.parts();
        let request = match CallResolverRequest::try_new(
            CallCallee::Free { path: &path },
            &lexical,
            expected,
            &module,
            symbols,
            world,
            &self.trait_catalog,
            CallSourceContext::new(document, call_span.as_ref(), callee_span.as_ref()),
            CallableGroupIndex::ZERO,
            expression,
            cancellation,
            work,
            &PRODUCTION_CALLABLE_LIMITS,
        ) {
            Ok(request) => request,
            Err(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
                for arg in args {
                    self.check_expr(arg.value());
                }
                return RegisteredFreeCallOutcome::Checked(None);
            }
        };
        let resolved = crate::callable::resolve_call_target(request);
        self.finish_registered_free_resolution(
            RegisteredFreeResolutionSite {
                path: &path,
                call,
                call_span,
                expression,
                document,
            },
            resolved,
        )
    }

    fn finish_registered_free_resolution(
        &mut self,
        site: RegisteredFreeResolutionSite<'_>,
        resolved: ResolveCallOutcome,
    ) -> RegisteredFreeCallOutcome {
        let args = site.call.args();
        match resolved {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                let label = site.path.dotted_name();
                let callee_range =
                    self.source_range_for_expr(site.call.callee())
                        .and_then(|range| {
                            let end = range.end();
                            let start = end.checked_sub(site.path.leaf().as_str().len())?;
                            (start >= range.start()).then_some(
                                arcweft_lang_syntax::ast::common::TextRange::new(start, end),
                            )
                        });
                RegisteredFreeCallOutcome::Checked(Some(self.check_registered_candidates(
                    RegisteredCallSite {
                        label: &label,
                        call: site.call,
                        call_span: site.call_span,
                        callee_range,
                        expression: site.expression,
                        document: site.document,
                    },
                    &candidates,
                )))
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(target)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`{}` resolves to non-callable type {:?}",
                    site.path.leaf().as_str(),
                    target.ty()
                )));
                let records_facts = self.records_call_target_facts(site.call_span.as_ref());
                let arguments = self.check_unmapped_registered_arguments(
                    site.call,
                    CallPoison::Rejected,
                    records_facts,
                );
                if records_facts && let Some(call_span) = site.call_span {
                    self.record_call_target_facts(
                        site.expression,
                        site.document,
                        &call_span,
                        CheckedCallTarget::non_callable(
                            target.source().clone(),
                            target.ty().clone(),
                            arguments,
                            CallableGroupIndex::ZERO,
                        ),
                    );
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::FunctionValue(_))
            | ResolveCallOutcome::Missing(_) => RegisteredFreeCallOutcome::NotHandled,
            ResolveCallOutcome::Rejected(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(site.call_span.as_ref(), error);
                for arg in args {
                    self.check_expr(arg.value());
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
        }
    }

    fn registered_free_path_is_deferred(&self, path: &CallablePath) -> bool {
        let [name] = path.segments() else {
            return false;
        };
        let name = name.as_str();
        self.locals
            .get(name)
            .is_some_and(is_deferred_callable_value)
            || (self
                .global_symbols
                .get(name)
                .is_some_and(is_deferred_callable_value)
                && self.resolve_project_callable(name).is_none())
    }

    fn registered_free_lexical_scope(&self) -> LexicalCallableScope {
        LexicalCallableScope::from_non_callable_bindings(
            self.locals
                .iter()
                .chain(self.global_symbols.iter())
                .filter_map(|(name, ty)| {
                    if is_deferred_callable_value(ty) {
                        return None;
                    }
                    CallableName::try_new(name.as_str())
                        .ok()
                        .map(|name| (name, ty.clone()))
                }),
        )
    }

    fn registered_free_path_has_value_receiver(&self, callee: &Expr, path: &CallablePath) -> bool {
        matches!(callee, Expr::Select(_))
            && path.len() > 1
            && path
                .segments()
                .first()
                .is_some_and(|root| self.symbol_type(root.as_str()).is_some())
    }

    pub(super) fn check_registered_catalog_method_call(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        call: &CallExpr,
        receiver_expression: TypeExpressionId,
        expression: TypeExpressionId,
    ) -> RegisteredMethodCallOutcome {
        let args = call.args();
        let (Some(world), Some(symbols)) = (self.registered_world, self.project_symbols) else {
            return RegisteredMethodCallOutcome::NotHandled;
        };
        let Ok(method) = CallableName::try_new(method_name) else {
            return RegisteredMethodCallOutcome::NotHandled;
        };
        let module = self
            .current_module
            .clone()
            .unwrap_or_else(CanonicalModulePath::crate_root);
        let Some(document) = symbols.source_identity(&module) else {
            self.errors.push(TypeCheckError::new(format!(
                "method call `{method_name}` has no accepted source identity"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return RegisteredMethodCallOutcome::Checked(None);
        };
        let lexical = LexicalCallableScope::default();
        let call_span = self.source_span_for_current_range(call.range());
        let callee_span = self.source_span_for_current_range(call.callee_range());
        let (cancellation, work) = self.call_resolver_control.parts();
        let request = match CallResolverRequest::try_new(
            CallCallee::Selected {
                receiver_expression,
                receiver_type,
                method: &method,
            },
            &lexical,
            None,
            &module,
            symbols,
            world,
            &self.trait_catalog,
            CallSourceContext::new(document, call_span.as_ref(), callee_span.as_ref()),
            CallableGroupIndex::ZERO,
            expression,
            cancellation,
            work,
            &PRODUCTION_CALLABLE_LIMITS,
        ) {
            Ok(request) => request,
            Err(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
                for arg in args {
                    self.check_expr(arg.value());
                }
                return RegisteredMethodCallOutcome::Checked(None);
            }
        };
        match crate::callable::resolve_call_target(request) {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                RegisteredMethodCallOutcome::Checked(Some(self.check_registered_candidates(
                    RegisteredCallSite {
                        label: method_name,
                        call,
                        call_span,
                        callee_range: None,
                        expression,
                        document,
                    },
                    &candidates,
                )))
            }
            ResolveCallOutcome::Missing(_) => RegisteredMethodCallOutcome::NotHandled,
            ResolveCallOutcome::Rejected(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
                for arg in args {
                    self.check_expr(arg.value());
                }
                RegisteredMethodCallOutcome::Checked(None)
            }
            ResolveCallOutcome::Resolved(
                ResolvedCallTarget::FunctionValue(_) | ResolvedCallTarget::NonCallable(_),
            ) => {
                self.errors.push(TypeCheckError::new(format!(
                    "registered method `{method_name}` resolved to a non-method target"
                )));
                for arg in args {
                    self.check_expr(arg.value());
                }
                RegisteredMethodCallOutcome::Checked(None)
            }
        }
    }

    fn check_registered_candidates(
        &mut self,
        site: RegisteredCallSite<'_>,
        candidates: &NonEmptyResolvedCandidates,
    ) -> TypeKind {
        let args = site.call.args();
        let viable = candidates
            .as_slice()
            .iter()
            .filter(|candidate| call_shape_is_viable(candidate.schema(), args))
            .collect::<Vec<_>>();
        let selected = match viable.as_slice() {
            [selected] => *selected,
            [] => candidates.first(),
            multiple => {
                self.errors.push(TypeCheckError::new(format!(
                    "call `{}` is ambiguous between {:?}",
                    site.label,
                    multiple
                        .iter()
                        .map(|candidate| candidate.id())
                        .collect::<Vec<_>>()
                )));
                let records_facts = self.records_call_target_facts(site.call_span.as_ref());
                let arguments = self.check_unmapped_registered_arguments(
                    site.call,
                    CallPoison::Rejected,
                    records_facts,
                );
                if records_facts && let Some(call_span) = site.call_span {
                    let candidate_products = multiple
                        .iter()
                        .map(|candidate| (*candidate).clone())
                        .collect::<Vec<_>>();
                    self.record_call_target_facts(
                        site.expression,
                        site.document,
                        &call_span,
                        CheckedCallTarget::ambiguous(
                            &candidate_products,
                            arguments,
                            CallableGroupIndex::ZERO,
                        ),
                    );
                }
                return TypeKind::Named("_".to_owned());
            }
        };
        self.check_registered_candidate(site, selected, candidates.as_slice())
    }

    fn check_registered_candidate(
        &mut self,
        site: RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
        considered: &[ResolvedCallable],
    ) -> TypeKind {
        let RegisteredCallSite {
            label,
            call,
            call_span,
            callee_range,
            expression,
            document,
        } = site;
        let args = call.args();
        let schema = candidate.schema();
        self.check_virtual_path_call(label, args);
        let records_facts = self.records_call_target_facts(call_span.as_ref());
        let argument_check = match schema.validator() {
            CallableValidator::Ordinary | CallableValidator::Untyped => {
                self.check_registered_schema_args(label, schema, call, records_facts)
            }
            validator => {
                self.errors.push(TypeCheckError::new(format!(
                    "registered callable `{label}` has unsupported validator {validator:?}"
                )));
                RegisteredArgumentCheck {
                    facts: self.check_unmapped_registered_arguments(
                        call,
                        CallPoison::Rejected,
                        records_facts,
                    ),
                    poison: CallPoison::Rejected,
                }
            }
        };
        let site = EffectSite::new(format!("call `{label}`"));
        if let Some(declaration) = schema.effects().project_declaration() {
            self.effect_collector.record_local_call(
                crate::effect_model::CallableId::project_function(declaration),
                site,
            );
        } else {
            self.effect_collector.record_named_call(
                label,
                Some(schema.effects().declared().concrete().clone()),
                site,
            );
        }
        if let CallableCandidateId::Project(declaration) = candidate.id()
            && let (Some(module), Some(range)) = (&self.current_module, callee_range)
        {
            self.project_callable_references
                .push(super::super::ProjectCallableReference {
                    module: module.clone(),
                    declaration: declaration.clone(),
                    range,
                });
        }
        let result = schema_result_type(schema, CallableGroupIndex::ZERO);
        if records_facts && let Some(call_span) = call_span {
            self.record_call_target_facts(
                expression,
                document,
                &call_span,
                CheckedCallTarget::selected(
                    candidate,
                    considered,
                    argument_check.facts,
                    result.clone(),
                    CallableGroupIndex::ZERO,
                    argument_check.poison,
                ),
            );
        }
        result
    }

    fn check_registered_schema_args(
        &mut self,
        label: &str,
        schema: &CallableSignatureSchema,
        call: &CallExpr,
        records_facts: bool,
    ) -> RegisteredArgumentCheck {
        let args = call.args();
        let Some(group) = schema.group(CallableGroupIndex::ZERO) else {
            self.errors.push(TypeCheckError::new(format!(
                "registered callable `{label}` has no initial parameter group"
            )));
            return RegisteredArgumentCheck {
                facts: self.check_unmapped_registered_arguments(
                    call,
                    CallPoison::Rejected,
                    records_facts,
                ),
                poison: CallPoison::Rejected,
            };
        };
        let context = RegisteredArgumentContext {
            label,
            schema,
            parameters: group.parameters(),
        };
        let mut fact_builders = self.registered_argument_fact_builders(call, records_facts);
        let mut provided = vec![false; context.parameters.len()];
        let mut positional = 0usize;
        let mut poison = CallPoison::Clean;
        let mut spread_shape_rejected = false;
        for (argument_index, arg) in args.iter().enumerate() {
            match arg {
                CallArg::Positional(value) => {
                    poison =
                        poison.merge(self.check_registered_positional(RegisteredPositionalCheck {
                            context,
                            value: FixedLiteralSpreadSlot::Expr(value),
                            provided: &mut provided,
                            positional: &mut positional,
                            argument_index,
                            fact_builders: &mut fact_builders,
                        }));
                }
                CallArg::Named { name, value } => {
                    poison = poison.merge(self.check_registered_named(RegisteredNamedCheck {
                        context,
                        name,
                        value,
                        provided: &mut provided,
                        argument_index,
                        fact_builders: &mut fact_builders,
                    }));
                }
                CallArg::Spread { value } => {
                    let spread = self.check_registered_spread(RegisteredSpreadCheck {
                        context,
                        value,
                        provided: &mut provided,
                        positional: &mut positional,
                        argument_index,
                        fact_builders: &mut fact_builders,
                    });
                    spread_shape_rejected |= spread.shape_rejected;
                    poison = poison.merge(spread.poison);
                }
            }
        }
        for parameter in context.parameters {
            if !spread_shape_rejected
                && !provided[parameter.index().get()]
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
                poison = CallPoison::Rejected;
            }
        }
        RegisteredArgumentCheck {
            facts: fact_builders.map_or_else(Vec::new, |builders| {
                builders
                    .into_iter()
                    .map(ArgumentFactBuilder::finish)
                    .collect()
            }),
            poison,
        }
    }

    fn check_registered_positional(&mut self, input: RegisteredPositionalCheck<'_>) -> CallPoison {
        let RegisteredPositionalCheck {
            context,
            value,
            provided,
            positional,
            argument_index,
            fact_builders,
        } = input;
        let Some(parameter) =
            next_registered_positional_parameter(context.parameters, provided, positional)
        else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` received too many positional arguments",
                context.label
            )));
            return self.check_registered_argument_slot(
                context.label,
                None,
                value,
                argument_index,
                fact_builders,
                CallPoison::Rejected,
            );
        };
        let index = parameter.index().get();
        if parameter.passing() != CallableParameterPassing::RestPositional {
            provided[index] = true;
            *positional = index + 1;
        }
        self.check_registered_argument_slot(
            context.label,
            Some(parameter),
            value,
            argument_index,
            fact_builders,
            CallPoison::Clean,
        )
    }

    fn check_registered_named(&mut self, input: RegisteredNamedCheck<'_>) -> CallPoison {
        let RegisteredNamedCheck {
            context,
            name,
            value,
            provided,
            argument_index,
            fact_builders,
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
            return self.check_registered_argument_slot(
                context.label,
                None,
                FixedLiteralSpreadSlot::Expr(value),
                argument_index,
                fact_builders,
                poison,
            );
        };
        let index = parameter.index().get();
        let mut poison = CallPoison::Clean;
        if parameter.passing() != CallableParameterPassing::RestNamed && provided[index] {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` argument `{name}` was provided more than once",
                context.label
            )));
            poison = CallPoison::Rejected;
        }
        provided[index] = true;
        self.check_registered_argument_slot(
            context.label,
            Some(parameter),
            FixedLiteralSpreadSlot::Expr(value),
            argument_index,
            fact_builders,
            poison,
        )
    }

    fn check_registered_spread(
        &mut self,
        input: RegisteredSpreadCheck<'_>,
    ) -> RegisteredSpreadResult {
        let RegisteredSpreadCheck {
            context,
            value,
            provided,
            positional,
            argument_index,
            fact_builders,
        } = input;
        match context.schema.argument_policy().spread() {
            SpreadArgumentPolicy::Unchecked => RegisteredSpreadResult {
                poison: self.check_registered_argument_slot(
                    context.label,
                    None,
                    FixedLiteralSpreadSlot::Expr(value),
                    argument_index,
                    fact_builders,
                    CallPoison::Clean,
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
                        context.label,
                        None,
                        FixedLiteralSpreadSlot::Expr(value),
                        argument_index,
                        fact_builders,
                        CallPoison::Rejected,
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
                            context.label,
                            None,
                            FixedLiteralSpreadSlot::Expr(value),
                            argument_index,
                            fact_builders,
                            CallPoison::Rejected,
                        ),
                        shape_rejected: true,
                    };
                };
                RegisteredSpreadResult {
                    poison: self.check_registered_fixed_spread(
                        RegisteredSpreadCheck {
                            context,
                            value,
                            provided,
                            positional,
                            argument_index,
                            fact_builders,
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
                                provided,
                                positional,
                                argument_index,
                                fact_builders,
                            },
                            slots,
                        ),
                        shape_rejected: false,
                    }
                } else {
                    self.check_registered_typed_rest_spread(
                        context,
                        value,
                        provided,
                        argument_index,
                        fact_builders,
                    )
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
            provided,
            positional,
            argument_index,
            fact_builders,
        } = input;
        self.reserve_fixed_literal_spread_container_expr(value);
        let mut poison = CallPoison::Clean;
        for slot in slots {
            poison = poison.merge(self.check_registered_positional(RegisteredPositionalCheck {
                context,
                value: slot,
                provided,
                positional,
                argument_index,
                fact_builders,
            }));
        }
        poison
    }

    fn check_registered_typed_rest_spread(
        &mut self,
        context: RegisteredArgumentContext<'_>,
        value: &Expr,
        provided: &[bool],
        argument_index: usize,
        fact_builders: &mut Option<Vec<ArgumentFactBuilder>>,
    ) -> RegisteredSpreadResult {
        let Some(rest) = context
            .parameters
            .iter()
            .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
        else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` has no positional rest parameter",
                context.label
            )));
            return RegisteredSpreadResult {
                poison: self.check_registered_argument_slot(
                    context.label,
                    None,
                    FixedLiteralSpreadSlot::Expr(value),
                    argument_index,
                    fact_builders,
                    CallPoison::Rejected,
                ),
                shape_rejected: true,
            };
        };
        if context.parameters.iter().any(|parameter| {
            parameter.presence() == CallableParameterPresence::Required
                && parameter.passing() != CallableParameterPassing::RestPositional
                && !provided[parameter.index().get()]
        }) {
            self.errors.push(TypeCheckError::new(format!(
                "function `{}` spread argument must follow required fixed arguments",
                context.label
            )));
            return RegisteredSpreadResult {
                poison: self.check_registered_argument_slot(
                    context.label,
                    None,
                    FixedLiteralSpreadSlot::Expr(value),
                    argument_index,
                    fact_builders,
                    CallPoison::Rejected,
                ),
                shape_rejected: true,
            };
        }
        let expression = TypeExpressionId::from_index(self.stats.expressions);
        let source = self.source_span_for_expr(value);
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
                    parameter: Some(rest),
                    inferred: actual,
                    poison: CallPoison::Rejected,
                },
                fact_builders,
            );
            return RegisteredSpreadResult {
                poison: CallPoison::Rejected,
                shape_rejected: false,
            };
        };
        let mut poison = CallPoison::Clean;
        if let CallableParameterType::Exact(expected) = rest.ty()
            && !self.types_compatible(expected, item)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                context.label,
                parameter_label(rest),
                expected.clone(),
                item.clone(),
            ));
            poison = CallPoison::Rejected;
        }
        Self::push_registered_argument_slot(
            RegisteredArgumentSlot {
                argument_index,
                expression,
                source,
                parameter: Some(rest),
                inferred: actual,
                poison,
            },
            fact_builders,
        );
        RegisteredSpreadResult {
            poison,
            shape_rejected: false,
        }
    }

    fn check_registered_argument_slot(
        &mut self,
        label: &str,
        parameter: Option<&CallableParameter>,
        value: FixedLiteralSpreadSlot<'_>,
        argument_index: usize,
        fact_builders: &mut Option<Vec<ArgumentFactBuilder>>,
        mut poison: CallPoison,
    ) -> CallPoison {
        let expected = match parameter.map(CallableParameter::ty) {
            Some(CallableParameterType::Exact(expected)) => Some(expected),
            Some(CallableParameterType::Unchecked) | None => None,
        };
        let expression = TypeExpressionId::from_index(self.stats.expressions);
        let source = value
            .source_expr()
            .and_then(|expr| self.source_span_for_expr(expr));
        if value.source_expr().is_none() {
            self.stats.expressions += 1;
        }
        let actual = self.check_fixed_literal_spread_slot(value, expected);
        if let (Some(expected), Some(actual)) = (expected, actual.as_ref())
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                label,
                parameter.map_or_else(|| "<unmapped>".to_owned(), parameter_label),
                expected.clone(),
                actual.clone(),
            ));
            poison = CallPoison::Rejected;
        }
        Self::push_registered_argument_slot(
            RegisteredArgumentSlot {
                argument_index,
                expression,
                source,
                parameter,
                inferred: actual,
                poison,
            },
            fact_builders,
        );
        poison
    }
}

fn callable_path(callee: &Expr) -> Option<CallablePath> {
    let mut segments = Vec::new();
    collect_callable_path_segments(callee, &mut segments)?;
    CallablePath::try_new(segments).ok()
}

fn collect_callable_path_segments(callee: &Expr, segments: &mut Vec<CallableName>) -> Option<()> {
    match callee {
        Expr::Path(path) => {
            segments.extend(
                path.segments()
                    .iter()
                    .map(|segment| CallableName::try_new(segment.as_str()))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?,
            );
            Some(())
        }
        Expr::Select(select) => {
            collect_callable_path_segments(select.target(), segments)?;
            segments.push(CallableName::try_new(select.member().as_str()).ok()?);
            Some(())
        }
        _ => None,
    }
}

fn is_deferred_callable_value(ty: &TypeKind) -> bool {
    match ty {
        TypeKind::Function { .. } | TypeKind::Speaker(_) | TypeKind::SpeakerPreset(_) => true,
        TypeKind::Ref(entity) => entity.kind() == &crate::types::EntityKind::Character,
        _ => false,
    }
}

fn call_shape_is_viable(schema: &CallableSignatureSchema, args: &[CallArg]) -> bool {
    let Some(group) = schema.group(CallableGroupIndex::ZERO) else {
        return false;
    };
    let parameters = group.parameters();
    let mut provided = vec![false; parameters.len()];
    let mut positional = 0usize;
    for argument in args {
        match argument {
            CallArg::Positional(_) => {
                if !mark_viable_positional(parameters, &mut provided, &mut positional) {
                    return false;
                }
            }
            CallArg::Named { name, .. } => {
                let parameter = registered_named_parameter(parameters, name);
                let Some(parameter) = parameter else {
                    if schema.argument_policy().unknown_named()
                        == UnknownNamedArgumentPolicy::Reject
                    {
                        return false;
                    }
                    continue;
                };
                if parameter.passing() != CallableParameterPassing::RestNamed {
                    let index = parameter.index().get();
                    if provided[index] {
                        return false;
                    }
                    provided[index] = true;
                }
            }
            CallArg::Spread { value } => match schema.argument_policy().spread() {
                SpreadArgumentPolicy::Reject => return false,
                SpreadArgumentPolicy::Unchecked => {}
                SpreadArgumentPolicy::FixedLiteralOnly => {
                    let Some(slots) = fixed_literal_spread_slots(value) else {
                        return false;
                    };
                    if slots.into_iter().any(|_| {
                        !mark_viable_positional(parameters, &mut provided, &mut positional)
                    }) {
                        return false;
                    }
                }
                SpreadArgumentPolicy::TypedRest => {
                    if let Some(slots) = fixed_literal_spread_slots(value) {
                        if slots.into_iter().any(|_| {
                            !mark_viable_positional(parameters, &mut provided, &mut positional)
                        }) {
                            return false;
                        }
                    } else if !parameters.iter().any(|parameter| {
                        parameter.passing() == CallableParameterPassing::RestPositional
                    }) || required_fixed_parameter_is_missing(parameters, &provided)
                    {
                        return false;
                    }
                }
            },
        }
    }
    !required_fixed_parameter_is_missing(parameters, &provided)
}

fn mark_viable_positional(
    parameters: &[CallableParameter],
    provided: &mut [bool],
    positional: &mut usize,
) -> bool {
    let Some(parameter) = next_registered_positional_parameter(parameters, provided, positional)
    else {
        return false;
    };
    if parameter.passing() != CallableParameterPassing::RestPositional {
        let index = parameter.index().get();
        provided[index] = true;
        *positional = index + 1;
    }
    true
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

fn required_fixed_parameter_is_missing(
    parameters: &[CallableParameter],
    provided: &[bool],
) -> bool {
    parameters.iter().any(|parameter| {
        parameter.presence() == CallableParameterPresence::Required
            && !matches!(
                parameter.passing(),
                CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
            )
            && !provided[parameter.index().get()]
    })
}

fn parameter_label(parameter: &CallableParameter) -> String {
    parameter.name().map_or_else(
        || format!("#{}", parameter.index().get()),
        |name| name.as_str().to_owned(),
    )
}

fn schema_result_type(
    schema: &CallableSignatureSchema,
    current_group: CallableGroupIndex,
) -> TypeKind {
    schema
        .groups()
        .iter()
        .skip(current_group.get() + 1)
        .rev()
        .fold(schema.result().clone(), |result, group| {
            TypeKind::function_with_effects(
                group
                    .parameters()
                    .iter()
                    .map(|parameter| match parameter.ty() {
                        CallableParameterType::Exact(ty) => ty.clone(),
                        CallableParameterType::Unchecked => TypeKind::Named("_".to_owned()),
                    }),
                result,
                schema.effects().declared().clone(),
            )
        })
}
