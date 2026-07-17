//! Registered-catalog free-call checking.

use std::sync::atomic::AtomicBool;

use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    expr::{CallArg, Expr},
};

use super::{TypeCheckError, TypeChecker, TypeExpressionId, TypeKind};
use crate::{
    callable::{
        CallCallee, CallResolverRequest, CallSourceContext, CallableGroupIndex, CallableName,
        CallableParameter, CallableParameterPassing, CallableParameterPresence,
        CallableParameterType, CallablePath, CallableSignatureSchema, CallableValidator,
        LexicalCallableScope, NonEmptyResolvedCandidates, PRODUCTION_CALLABLE_LIMITS,
        ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable, ResolverWork,
        SpreadArgumentPolicy,
    },
    effect_model::EffectSite,
};

use super::support::{FixedLiteralSpreadSlot, fixed_literal_spread_slots, spread_item_type};

pub(super) enum RegisteredFreeCallOutcome {
    NotHandled,
    Checked(Option<TypeKind>),
}

impl TypeChecker<'_> {
    pub(super) fn check_registered_catalog_free_call(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
        expected: Option<&TypeKind>,
        expression: TypeExpressionId,
    ) -> RegisteredFreeCallOutcome {
        let (Some(world), Some(symbols)) = (self.registered_world, self.project_symbols) else {
            return RegisteredFreeCallOutcome::NotHandled;
        };
        let Some(path) = callable_path(callee) else {
            return RegisteredFreeCallOutcome::NotHandled;
        };

        if self.registered_free_path_has_value_receiver(callee, &path) {
            return RegisteredFreeCallOutcome::NotHandled;
        }

        if let [name] = path.segments()
            && self
                .symbol_type(name.as_str())
                .is_some_and(is_deferred_callable_value)
        {
            return RegisteredFreeCallOutcome::NotHandled;
        }

        let lexical = LexicalCallableScope::from_non_callable_bindings(
            self.locals
                .iter()
                .chain(self.global_symbols.iter())
                .filter_map(|(name, ty)| {
                    (!is_deferred_callable_value(ty))
                        .then(|| {
                            CallableName::try_new(name.as_str())
                                .ok()
                                .map(|name| (name, ty.clone()))
                        })
                        .flatten()
                }),
        );
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
        let cancellation = AtomicBool::new(false);
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let request = match CallResolverRequest::try_new(
            CallCallee::Free { path: &path },
            &lexical,
            expected,
            &module,
            symbols,
            world,
            &self.trait_catalog,
            CallSourceContext::new(document, None, None),
            CallableGroupIndex::ZERO,
            expression,
            &cancellation,
            &mut work,
            &PRODUCTION_CALLABLE_LIMITS,
        ) {
            Ok(request) => request,
            Err(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                for arg in args {
                    self.check_expr(arg.value());
                }
                return RegisteredFreeCallOutcome::Checked(None);
            }
        };
        match crate::callable::resolve_call_target(request) {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                RegisteredFreeCallOutcome::Checked(Some(self.check_registered_candidates(
                    path.leaf().as_str(),
                    &candidates,
                    args,
                )))
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(target)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`{}` resolves to non-callable type {:?}",
                    path.leaf().as_str(),
                    target.ty()
                )));
                for arg in args {
                    self.check_expr(arg.value());
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::FunctionValue(_))
            | ResolveCallOutcome::Missing(_) => RegisteredFreeCallOutcome::NotHandled,
            ResolveCallOutcome::Rejected(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                for arg in args {
                    self.check_expr(arg.value());
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
        }
    }

    fn registered_free_path_has_value_receiver(&self, callee: &Expr, path: &CallablePath) -> bool {
        matches!(callee, Expr::Select(_))
            && path.len() > 1
            && path
                .segments()
                .first()
                .is_some_and(|root| self.symbol_type(root.as_str()).is_some())
    }

    fn check_registered_candidates(
        &mut self,
        label: &str,
        candidates: &NonEmptyResolvedCandidates,
        args: &[CallArg],
    ) -> TypeKind {
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
                    "call `{label}` is ambiguous between {:?}",
                    multiple
                        .iter()
                        .map(|candidate| candidate.id())
                        .collect::<Vec<_>>()
                )));
                for arg in args {
                    self.check_expr(arg.value());
                }
                return TypeKind::Named("_".to_owned());
            }
        };
        self.check_registered_candidate(label, selected, args)
    }

    fn check_registered_candidate(
        &mut self,
        label: &str,
        candidate: &ResolvedCallable,
        args: &[CallArg],
    ) -> TypeKind {
        let schema = candidate.schema();
        self.check_virtual_path_call(label, args);
        match schema.validator() {
            CallableValidator::Ordinary => self.check_registered_schema_args(label, schema, args),
            CallableValidator::Untyped => {
                for arg in args {
                    self.check_expr(arg.value());
                }
            }
            validator => {
                self.errors.push(TypeCheckError::new(format!(
                    "registered free callable `{label}` has unsupported validator {validator:?}"
                )));
                for arg in args {
                    self.check_expr(arg.value());
                }
            }
        }
        let effects = schema.effects().declared();
        self.effect_collector.record_named_call(
            label,
            Some(effects.concrete().clone()),
            EffectSite::new(format!("call `{label}`")),
        );
        schema_result_type(schema, CallableGroupIndex::ZERO)
    }

    fn check_registered_schema_args(
        &mut self,
        label: &str,
        schema: &CallableSignatureSchema,
        args: &[CallArg],
    ) {
        let Some(group) = schema.group(CallableGroupIndex::ZERO) else {
            self.errors.push(TypeCheckError::new(format!(
                "registered callable `{label}` has no initial parameter group"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return;
        };
        let parameters = group.parameters();
        let mut provided = vec![false; parameters.len()];
        let mut positional = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    self.check_registered_positional(
                        label,
                        FixedLiteralSpreadSlot::Expr(value),
                        parameters,
                        &mut provided,
                        &mut positional,
                    );
                }
                CallArg::Named { name, value } => {
                    self.check_registered_named(label, name, value, parameters, &mut provided);
                }
                CallArg::Spread { value } => {
                    if let Some(slots) = fixed_literal_spread_slots(value) {
                        self.reserve_fixed_literal_spread_container_expr(value);
                        for slot in slots {
                            self.check_registered_positional(
                                label,
                                slot,
                                parameters,
                                &mut provided,
                                &mut positional,
                            );
                        }
                    } else {
                        self.check_registered_spread(label, value, schema, parameters, &provided);
                    }
                }
            }
        }
        for parameter in parameters {
            if !provided[parameter.index().get()]
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
            }
        }
    }

    fn check_registered_positional(
        &mut self,
        label: &str,
        value: FixedLiteralSpreadSlot<'_>,
        parameters: &[CallableParameter],
        provided: &mut [bool],
        positional: &mut usize,
    ) {
        while let Some(parameter) = parameters.get(*positional) {
            if provided[*positional] || parameter.passing() == CallableParameterPassing::NamedOnly {
                *positional += 1;
            } else {
                break;
            }
        }
        let Some(parameter) = parameters.get(*positional).or_else(|| {
            parameters
                .iter()
                .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
        }) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` received too many positional arguments"
            )));
            self.check_fixed_literal_spread_slot(value, None);
            return;
        };
        let index = parameter.index().get();
        if parameter.passing() != CallableParameterPassing::RestPositional {
            provided[index] = true;
            *positional = index + 1;
        }
        self.check_registered_argument_slot(label, parameter, value);
    }

    fn check_registered_named(
        &mut self,
        label: &str,
        name: &str,
        value: &Expr,
        parameters: &[CallableParameter],
        provided: &mut [bool],
    ) {
        let Some(parameter) = parameters.iter().find(|parameter| {
            parameter
                .name()
                .is_some_and(|candidate| candidate.as_str() == name)
                && matches!(
                    parameter.passing(),
                    CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::NamedOnly
                        | CallableParameterPassing::RestNamed
                )
        }) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` has no named parameter `{name}`"
            )));
            self.check_expr(value);
            return;
        };
        let index = parameter.index().get();
        if parameter.passing() != CallableParameterPassing::RestNamed && provided[index] {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` argument `{name}` was provided more than once"
            )));
        }
        provided[index] = true;
        self.check_registered_argument_slot(label, parameter, FixedLiteralSpreadSlot::Expr(value));
    }

    fn check_registered_spread(
        &mut self,
        label: &str,
        value: &Expr,
        schema: &CallableSignatureSchema,
        parameters: &[CallableParameter],
        provided: &[bool],
    ) {
        if schema.argument_policy().spread() != SpreadArgumentPolicy::TypedRest {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` does not accept non-literal spread arguments"
            )));
            self.check_expr(value);
            return;
        }
        let Some(rest) = parameters
            .iter()
            .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
        else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` has no positional rest parameter"
            )));
            self.check_expr(value);
            return;
        };
        if parameters.iter().any(|parameter| {
            parameter.presence() == CallableParameterPresence::Required
                && parameter.passing() != CallableParameterPassing::RestPositional
                && !provided[parameter.index().get()]
        }) {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` spread argument must follow required fixed arguments"
            )));
            self.check_expr(value);
            return;
        }
        let actual = self.check_expr(value);
        let Some(item) = actual.as_ref().and_then(spread_item_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{label}` spread argument must have a sequence type"
            )));
            return;
        };
        if let CallableParameterType::Exact(expected) = rest.ty()
            && !self.types_compatible(expected, item)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                label,
                parameter_label(rest),
                expected.clone(),
                item.clone(),
            ));
        }
    }

    fn check_registered_argument_slot(
        &mut self,
        label: &str,
        parameter: &CallableParameter,
        value: FixedLiteralSpreadSlot<'_>,
    ) {
        let expected = match parameter.ty() {
            CallableParameterType::Exact(expected) => Some(expected),
            CallableParameterType::Unchecked => None,
        };
        let actual = self.check_fixed_literal_spread_slot(value, expected);
        if let (Some(expected), Some(actual)) = (expected, actual.as_ref())
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                label,
                parameter_label(parameter),
                expected.clone(),
                actual.clone(),
            ));
        }
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
    let required = parameters
        .iter()
        .filter(|parameter| {
            parameter.presence() == CallableParameterPresence::Required
                && !matches!(
                    parameter.passing(),
                    CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
                )
        })
        .count();
    let has_rest = parameters.iter().any(|parameter| {
        matches!(
            parameter.passing(),
            CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
        )
    });
    let supplied = args
        .iter()
        .map(|argument| match argument {
            CallArg::Spread { value } => {
                fixed_literal_spread_slots(value).map_or(1, |slots| slots.len())
            }
            CallArg::Named { .. } | CallArg::Positional(_) => 1,
        })
        .sum::<usize>();
    supplied >= required && (has_rest || supplied <= parameters.len())
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
