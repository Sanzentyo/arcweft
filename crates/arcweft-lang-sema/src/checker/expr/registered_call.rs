//! Registered-catalog call checking.

use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    expr::{CallArg, CallExpr, Expr},
    reference::BorrowKind,
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use super::{TypeCheckError, TypeChecker, TypeExpressionId, TypeKind};
use crate::{
    callable::{
        CallCallee, CallPoison, CallResolverRequest, CallSourceContext, CallableArgumentIndex,
        CallableDiagnosticCode, CallableDiagnosticSubject, CallableGroupIndex, CallableName,
        CallableParameter, CallableParameterPassing, CallableParameterPresence,
        CallableParameterType, CallablePath, CallableSignatureSchema, CallableValidator,
        CheckedCallArgumentFact, CheckedCallArgumentSlotFact, CheckedCallTarget,
        DataLastCallableId, DialogueCallableId, DialogueCalleeIdentity, FunctionValueOrdinal,
        FunctionValueSignatureId, LexicalBindingIndex, LexicalCallBinding, LexicalCallableScope,
        LocalCallableId, PRODUCTION_CALLABLE_LIMITS, ProjectNominalTypeId, ResolveCallOutcome,
        ResolvedCallTarget, ResolvedCallable, ResolvedEnumSeed, ResolvedFunctionValueSeed,
        SignatureQueryStep, SpeakerCallableId, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
        data_last_unsupported_spread_reason,
    },
    checker::{
        CurriedSignatureCallValue, DataLastMethodFallbackArg, TypedLoweringEvidence,
        TypedLoweringEvidenceKind,
    },
    effect_model::EffectSite,
    effect_row::EffectRow,
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

#[derive(Clone)]
struct RegisteredArgumentCheck {
    facts: Vec<CheckedCallArgumentFact>,
    poison: CallPoison,
}

impl RegisteredArgumentCheck {
    fn new(facts: Vec<CheckedCallArgumentFact>, poison: CallPoison) -> Self {
        let poison = facts
            .iter()
            .fold(poison, |combined, fact| combined.merge(fact.poison()));
        Self { facts, poison }
    }
}

struct RegisteredSpreadResult {
    poison: CallPoison,
    shape_rejected: bool,
}

struct RegisteredSlotCheck {
    poison: CallPoison,
    inferred: Option<TypeKind>,
}

#[derive(Clone)]
struct RegisteredCandidateCheck {
    arguments: RegisteredArgumentCheck,
    result: TypeKind,
}

#[derive(Clone, Copy)]
enum RegisteredSumConstructor {
    Result(crate::callable::ResultConstructorKind),
    Option(crate::callable::OptionConstructorKind),
}

impl RegisteredSumConstructor {
    fn has_instantiated_result(self, result: &TypeKind) -> bool {
        match (self, result) {
            (Self::Result(_), TypeKind::Result { ok, error }) => {
                !is_placeholder(ok) || !is_placeholder(error)
            }
            (Self::Option(_), TypeKind::Option(item)) => !is_placeholder(item),
            _ => false,
        }
    }

    fn result_with_payload(self, payload: TypeKind) -> TypeKind {
        match self {
            Self::Result(crate::callable::ResultConstructorKind::Ok) => TypeKind::Result {
                ok: Box::new(payload),
                error: Box::new(TypeKind::Named("_".to_owned())),
            },
            Self::Result(crate::callable::ResultConstructorKind::Err) => TypeKind::Result {
                ok: Box::new(TypeKind::Named("_".to_owned())),
                error: Box::new(payload),
            },
            Self::Option(crate::callable::OptionConstructorKind::Some) => {
                TypeKind::Option(Box::new(payload))
            }
        }
    }
}

fn is_placeholder(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if name == "_")
}

#[derive(Clone)]
struct RegisteredCallSite<'a> {
    label: &'a str,
    call: &'a CallExpr,
    call_span: Option<arcweft_source::SourceSpan>,
    callee_range: Option<arcweft_lang_syntax::ast::common::TextRange>,
    expression: TypeExpressionId,
    document: &'a arcweft_source::SourceDocumentIdentity,
    group: CallableGroupIndex,
    receiver: Option<(&'a Expr, &'a TypeKind)>,
    function_value_type: Option<TypeKind>,
}

mod selection;

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
    group: CallableGroupIndex,
    parameters: &'a [CallableParameter],
    call: &'a CallExpr,
    focused: bool,
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
    group: CallableGroupIndex,
    parameter: Option<&'a CallableParameter>,
    inferred: Option<TypeKind>,
    poison: CallPoison,
}

impl TypeChecker<'_> {
    pub(super) fn check_standalone_language_call(
        &mut self,
        label: &str,
        schema: &CallableSignatureSchema,
        call: &CallExpr,
    ) -> TypeKind {
        let argument_check =
            self.check_registered_schema_args(label, schema, CallableGroupIndex::ZERO, call, false);
        debug_assert!(
            argument_check.facts.is_empty(),
            "standalone calls do not retain focused registered facts"
        );
        self.effect_collector.record_named_call(
            label,
            Some(schema.effects().declared().concrete().clone()),
            EffectSite::new(format!("call `{label}`")),
        );
        schema_result_type(schema, CallableGroupIndex::ZERO)
    }

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

        let lexical = self.registered_free_lexical_scope(expression);
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
        let focused_work = self.uses_focused_callable_work(call_span.as_ref());
        let enum_variant = self.registered_enum_seed(expected, &path, &module, symbols);
        let trait_catalog = &self.trait_catalog;
        let resolved = match self.call_resolver_control.with_parts(
            focused_work,
            |cancellation, work, signature_work, signature_control| {
                CallResolverRequest::try_new(
                    CallCallee::Free {
                        path: &path,
                        enum_variant: enum_variant.as_ref(),
                    },
                    &lexical,
                    expected,
                    &module,
                    symbols,
                    world,
                    trait_catalog,
                    &[],
                    CallSourceContext::new(document, call_span.as_ref(), callee_span.as_ref()),
                    CallableGroupIndex::ZERO,
                    expression,
                    cancellation,
                    work,
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .map(|request| {
                    request
                        .with_signature_work(signature_work)
                        .with_signature_control(signature_control)
                })
                .map(crate::callable::resolve_call_target)
            },
        ) {
            Ok(resolved) => resolved,
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

    fn registered_enum_seed(
        &self,
        expected: Option<&TypeKind>,
        path: &CallablePath,
        module: &CanonicalModulePath,
        symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    ) -> Option<ResolvedEnumSeed> {
        let expected = expected?;
        let payload = self.enum_variant_payload_for_path(expected, &path.dotted_name())?;
        let owner = super::enum_variant::nominal_type_name(expected)?;
        let owner = ProjectNominalTypeId::new(
            symbols.world().package().clone(),
            module.clone(),
            CallableName::try_new(owner).ok()?,
        );
        let id = crate::callable::EnumVariantSignatureId::new(owner, path.leaf().clone());
        let schema = id.signature_schema(&payload, expected.clone());
        Some(ResolvedEnumSeed::new(id, expected.clone(), schema))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one typed resolver outcome boundary retains free-call facts and ordinary fallback semantics"
    )]
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
                    &RegisteredCallSite {
                        label: &label,
                        call: site.call,
                        call_span: site.call_span,
                        callee_range,
                        expression: site.expression,
                        document: site.document,
                        group: CallableGroupIndex::ZERO,
                        receiver: None,
                        function_value_type: None,
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
                        Some((
                            CallableDiagnosticCode::NonCallableTarget,
                            CallableDiagnosticSubject::None,
                        )),
                    );
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
            ResolveCallOutcome::Resolved(ResolvedCallTarget::FunctionValue(value)) => {
                let label = site.path.dotted_name();
                let candidate = value.callable();
                RegisteredFreeCallOutcome::Checked(Some(self.check_registered_candidate(
                    &RegisteredCallSite {
                        label: &label,
                        call: site.call,
                        call_span: site.call_span,
                        callee_range: None,
                        expression: site.expression,
                        document: site.document,
                        group: value.current_group(),
                        receiver: None,
                        function_value_type: Some(value.function_type().clone()),
                    },
                    candidate,
                    std::slice::from_ref(candidate),
                )))
            }
            ResolveCallOutcome::Missing(missing) => {
                let records_facts = self.records_call_target_facts(site.call_span.as_ref());
                if !records_facts {
                    return RegisteredFreeCallOutcome::NotHandled;
                }
                let arguments =
                    self.check_unmapped_registered_arguments(site.call, CallPoison::Rejected, true);
                if let Some(call_span) = site.call_span {
                    self.record_call_target_facts(
                        site.expression,
                        site.document,
                        &call_span,
                        CheckedCallTarget::missing(
                            missing.kind(),
                            arguments,
                            CallableGroupIndex::ZERO,
                        ),
                        None,
                    );
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
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

    #[allow(
        clippy::too_many_lines,
        reason = "one scope builder preserves precedence across every lexical callable carrier"
    )]
    fn registered_free_lexical_scope(&self, expression: TypeExpressionId) -> LexicalCallableScope {
        let mut scope = LexicalCallableScope::default();
        let names = self
            .global_symbols
            .keys()
            .chain(self.locals.keys())
            .collect::<std::collections::BTreeSet<_>>();
        let mut function_value_ordinal = 0usize;
        for name in names {
            let Some(ty) = self
                .locals
                .get(name)
                .or_else(|| self.global_symbols.get(name))
            else {
                continue;
            };
            let Ok(callable_name) = CallableName::try_new(name.as_str()) else {
                continue;
            };
            if let Some(id) = SpeakerCallableId::resolve_value(ty, None) {
                scope.insert(
                    callable_name,
                    LexicalCallBinding::Speaker {
                        schema: std::sync::Arc::new(id.signature_schema()),
                        id,
                    },
                );
            } else if let Some(signature) = self.local_callable_signatures.get(name)
                && let Some(id) = self.registered_local_callable_id(name)
            {
                let effects = function_value_effects(ty);
                let Ok(schema) = signature.signature.callable_schema(
                    effects.clone(),
                    CallableValidator::Ordinary,
                    &PRODUCTION_CALLABLE_LIMITS,
                ) else {
                    continue;
                };
                scope.insert(
                    callable_name,
                    LexicalCallBinding::Callable {
                        id,
                        schema: std::sync::Arc::new(schema),
                        effects,
                    },
                );
            } else if matches!(ty, TypeKind::Function { .. }) {
                let ordinal = FunctionValueOrdinal::try_from_usize(function_value_ordinal)
                    .unwrap_or_else(|_| {
                        FunctionValueOrdinal::try_from_usize(0)
                            .expect("zero function-value ordinal is representable")
                    });
                function_value_ordinal = function_value_ordinal.saturating_add(1);
                let id_expression = self
                    .local_symbol_identities
                    .get(name)
                    .and_then(local_binding_expression)
                    .unwrap_or(expression);
                let id = FunctionValueSignatureId::new(id_expression, ordinal);
                let curried = self.local_curried_signature_calls.get(name);
                let schema = curried
                    .and_then(|value| value.resolved.as_ref())
                    .map(|candidate| candidate.schema().clone())
                    .or_else(|| {
                        self.local_callable_signatures.get(name).and_then(|source| {
                            source
                                .signature
                                .callable_schema(
                                    function_value_effects(ty),
                                    CallableValidator::Ordinary,
                                    &PRODUCTION_CALLABLE_LIMITS,
                                )
                                .ok()
                        })
                    })
                    .or_else(|| {
                        CallableSignatureSchema::for_function_value(ty, &PRODUCTION_CALLABLE_LIMITS)
                            .ok()
                    });
                let Some(schema) = schema else {
                    continue;
                };
                let next_group = curried
                    .and_then(|value| {
                        CallableGroupIndex::try_from_usize(value.remaining_group_index).ok()
                    })
                    .unwrap_or(CallableGroupIndex::ZERO);
                let seed = ResolvedFunctionValueSeed::new(
                    id,
                    ty.clone(),
                    schema,
                    self.local_function_effects.get(name).cloned(),
                    curried
                        .and_then(|value| value.resolved.as_ref())
                        .map(|candidate| candidate.id().clone()),
                    next_group,
                );
                scope.insert(callable_name, LexicalCallBinding::FunctionValue(seed));
            } else {
                scope.insert(
                    callable_name,
                    LexicalCallBinding::NonCallable { ty: ty.clone() },
                );
            }
        }
        scope
    }

    fn registered_local_callable_id(&self, name: &str) -> Option<LocalCallableId> {
        let crate::canonicalization::SemanticSymbolIdentity::Local { scope, binding, .. } =
            self.local_symbol_identities.get(name)?
        else {
            return None;
        };
        let binding = LexicalBindingIndex::try_from_usize(binding.0 as usize).ok()?;
        Some(LocalCallableId::new(*scope, binding))
    }

    fn registered_free_path_has_value_receiver(&self, callee: &Expr, path: &CallablePath) -> bool {
        matches!(callee, Expr::Select(_))
            && path.len() > 1
            && path
                .segments()
                .first()
                .is_some_and(|root| self.symbol_type(root.as_str()).is_some())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the method entry point owns one resolver request and every typed terminal outcome"
    )]
    pub(super) fn check_registered_catalog_method_call(
        &mut self,
        receiver: &Expr,
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
        let lexical = self.registered_free_lexical_scope(expression);
        let call_span = self.source_span_for_current_range(call.range());
        let callee_span = self.source_span_for_current_range(call.callee_range());
        let focused_work = self.uses_focused_callable_work(call_span.as_ref());
        let active_trait_predicates = self.active_trait_predicates();
        let trait_catalog = &self.trait_catalog;
        let resolved = match self.call_resolver_control.with_parts(
            focused_work,
            |cancellation, work, signature_work, signature_control| {
                CallResolverRequest::try_new(
                    CallCallee::Selected {
                        receiver_expression,
                        receiver_type,
                        method: &method,
                        arguments: args,
                    },
                    &lexical,
                    None,
                    &module,
                    symbols,
                    world,
                    trait_catalog,
                    &active_trait_predicates,
                    CallSourceContext::new(document, call_span.as_ref(), callee_span.as_ref()),
                    CallableGroupIndex::ZERO,
                    expression,
                    cancellation,
                    work,
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .map(|request| {
                    request
                        .with_signature_work(signature_work)
                        .with_signature_control(signature_control)
                })
                .map(crate::callable::resolve_call_target)
            },
        ) {
            Ok(resolved) => resolved,
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
        match resolved {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                RegisteredMethodCallOutcome::Checked(Some(self.check_registered_candidates(
                    &RegisteredCallSite {
                        label: method_name,
                        call,
                        call_span,
                        callee_range: None,
                        expression,
                        document,
                        group: CallableGroupIndex::ZERO,
                        receiver: Some((receiver, receiver_type)),
                        function_value_type: None,
                    },
                    &candidates,
                )))
            }
            ResolveCallOutcome::Missing(missing) => {
                let records_facts = self.records_call_target_facts(call_span.as_ref());
                if !records_facts {
                    return RegisteredMethodCallOutcome::NotHandled;
                }
                let arguments =
                    self.check_unmapped_registered_arguments(call, CallPoison::Rejected, true);
                if let Some(call_span) = call_span {
                    self.record_call_target_facts(
                        expression,
                        document,
                        &call_span,
                        CheckedCallTarget::missing(
                            missing.kind(),
                            arguments,
                            CallableGroupIndex::ZERO,
                        ),
                        None,
                    );
                }
                RegisteredMethodCallOutcome::Checked(None)
            }
            ResolveCallOutcome::Rejected(error) => {
                match &error {
                    crate::callable::ResolveCallError::AmbiguousTraitMethod { candidates } => {
                        let traits = candidates
                            .iter()
                            .map(|candidate| candidate.trait_name().dotted_name())
                            .collect::<Vec<_>>();
                        self.errors.push(TypeCheckError::trait_diagnostic(
                            crate::diagnostics::TraitDiagnostic::ambiguous_method(
                                method_name,
                                traits.iter().map(String::as_str),
                            ),
                        ));
                    }
                    crate::callable::ResolveCallError::DataLastAmbiguity { candidates } => {
                        self.errors
                            .push(TypeCheckError::ambiguous_data_last_method_fallback(
                                method_name,
                                receiver_type.clone(),
                                candidates
                                    .iter()
                                    .map(|candidate| format!("{:?}", candidate.callable())),
                            ));
                    }
                    _ => self.errors.push(TypeCheckError::new(error.to_string())),
                }
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

    #[allow(
        clippy::too_many_lines,
        reason = "function-value checking retains one resolver product, facts, and curried state"
    )]
    pub(super) fn check_registered_function_value_call(
        &mut self,
        call: &CallExpr,
        expression: TypeExpressionId,
        callee: Option<&str>,
        callee_ty: &TypeKind,
        effect_callable: Option<crate::effect_model::CallableId>,
        curried: Option<&CurriedSignatureCallValue>,
    ) -> RegisteredFreeCallOutcome {
        let (Some(world), Some(symbols)) = (self.registered_world, self.project_symbols) else {
            return RegisteredFreeCallOutcome::NotHandled;
        };
        let module = self
            .current_module
            .clone()
            .unwrap_or_else(CanonicalModulePath::crate_root);
        let Some(document) = symbols.source_identity(&module) else {
            return RegisteredFreeCallOutcome::NotHandled;
        };
        let schema = curried
            .and_then(|value| value.resolved.as_ref())
            .map(|candidate| candidate.schema().clone())
            .or_else(|| {
                CallableSignatureSchema::for_function_value(callee_ty, &PRODUCTION_CALLABLE_LIMITS)
                    .ok()
            });
        let Some(schema) = schema else {
            self.errors.push(TypeCheckError::new(
                "function value has no canonical callable schema".to_owned(),
            ));
            for argument in call.args() {
                self.check_expr(argument.value());
            }
            return RegisteredFreeCallOutcome::Checked(None);
        };
        let current_group = curried
            .and_then(|value| CallableGroupIndex::try_from_usize(value.remaining_group_index).ok())
            .unwrap_or(CallableGroupIndex::ZERO);
        let seed = ResolvedFunctionValueSeed::new(
            FunctionValueSignatureId::new(
                expression,
                FunctionValueOrdinal::try_from_usize(0)
                    .expect("zero function-value ordinal is representable"),
            ),
            callee_ty.clone(),
            schema,
            effect_callable,
            curried
                .and_then(|value| value.resolved.as_ref())
                .map(|candidate| candidate.id().clone()),
            current_group,
        );
        let lexical = LexicalCallableScope::default();
        let call_span = self.source_span_for_current_range(call.range());
        let callee_span = self.source_span_for_current_range(call.callee_range());
        let focused_work = self.uses_focused_callable_work(call_span.as_ref());
        let trait_catalog = &self.trait_catalog;
        let resolved = match self.call_resolver_control.with_parts(
            focused_work,
            |cancellation, work, signature_work, signature_control| {
                CallResolverRequest::try_new(
                    CallCallee::FunctionValue { value: &seed },
                    &lexical,
                    None,
                    &module,
                    symbols,
                    world,
                    trait_catalog,
                    &[],
                    CallSourceContext::new(document, call_span.as_ref(), callee_span.as_ref()),
                    current_group,
                    expression,
                    cancellation,
                    work,
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .map(|request| {
                    request
                        .with_signature_work(signature_work)
                        .with_signature_control(signature_control)
                })
                .map(crate::callable::resolve_call_target)
            },
        ) {
            Ok(resolved) => resolved,
            Err(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
                for argument in call.args() {
                    self.check_expr(argument.value());
                }
                return RegisteredFreeCallOutcome::Checked(None);
            }
        };
        match resolved {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::FunctionValue(value)) => {
                let label = callee.unwrap_or("<function value>");
                let candidate = value.callable();
                RegisteredFreeCallOutcome::Checked(Some(self.check_registered_candidate(
                    &RegisteredCallSite {
                        label,
                        call,
                        call_span,
                        callee_range: None,
                        expression,
                        document,
                        group: value.current_group(),
                        receiver: None,
                        function_value_type: Some(value.function_type().clone()),
                    },
                    candidate,
                    std::slice::from_ref(candidate),
                )))
            }
            ResolveCallOutcome::Rejected(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
                for argument in call.args() {
                    self.check_expr(argument.value());
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
            ResolveCallOutcome::Missing(_)
            | ResolveCallOutcome::Resolved(
                ResolvedCallTarget::Candidates(_) | ResolvedCallTarget::NonCallable(_),
            ) => {
                self.errors.push(TypeCheckError::new(
                    "function-value resolver returned a non-function target".to_owned(),
                ));
                for argument in call.args() {
                    self.check_expr(argument.value());
                }
                RegisteredFreeCallOutcome::Checked(None)
            }
        }
    }

    pub(super) fn resolve_registered_dialogue_call(
        &mut self,
        dialogue: &Expr,
        callee: &Expr,
        callee_ty: Option<&TypeKind>,
        expression: TypeExpressionId,
    ) {
        let (Some(world), Some(symbols), Some(module)) = (
            self.registered_world,
            self.project_symbols,
            self.current_module.clone(),
        ) else {
            return;
        };
        let Some(path) = callable_path(callee) else {
            return;
        };
        let callee_identity = self
            .resolve_project_character_in(&module, &path.dotted_name())
            .map_or_else(
                || DialogueCalleeIdentity::Content { path: path.clone() },
                |character| {
                    if matches!(callee_ty, Some(TypeKind::SpeakerPreset(_))) {
                        DialogueCalleeIdentity::SpeakerPreset { character }
                    } else {
                        DialogueCalleeIdentity::Speaker { character }
                    }
                },
            );
        let id = DialogueCallableId::resolve(&callee_identity);
        let Some(document) = symbols.source_identity(&module) else {
            return;
        };
        let lexical = LexicalCallableScope::default();
        let call_span = self.source_span_for_expr(dialogue);
        let callee_span = self.source_span_for_expr(callee);
        let records_facts = self.call_target_fact_recorder.wants(call_span.as_ref());
        let focused_work = self.uses_focused_callable_work(call_span.as_ref());
        let trait_catalog = &self.trait_catalog;
        let resolved = match self.call_resolver_control.with_parts(
            focused_work,
            |cancellation, work, signature_work, signature_control| {
                CallResolverRequest::try_new(
                    CallCallee::Dialogue {
                        id,
                        callee: &callee_identity,
                    },
                    &lexical,
                    None,
                    &module,
                    symbols,
                    world,
                    trait_catalog,
                    &[],
                    CallSourceContext::new(document, call_span.as_ref(), callee_span.as_ref()),
                    CallableGroupIndex::ZERO,
                    expression,
                    cancellation,
                    work,
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .map(|request| {
                    request
                        .with_signature_work(signature_work)
                        .with_signature_control(signature_control)
                })
                .map(crate::callable::resolve_call_target)
            },
        ) {
            Ok(resolved) => resolved,
            Err(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
                return;
            }
        };
        self.finish_registered_dialogue_resolution(
            resolved,
            expression,
            document,
            call_span,
            records_facts,
        );
    }

    fn finish_registered_dialogue_resolution(
        &mut self,
        resolved: ResolveCallOutcome,
        expression: TypeExpressionId,
        document: &SourceDocumentIdentity,
        call_span: Option<SourceSpan>,
        records_facts: bool,
    ) {
        match resolved {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                let selected = candidates.first();
                if records_facts && let Some(call_span) = call_span {
                    self.record_call_target_facts(
                        expression,
                        document,
                        &call_span,
                        CheckedCallTarget::selected(
                            selected,
                            candidates.as_slice(),
                            Vec::new(),
                            selected.schema().result().clone(),
                            CallableGroupIndex::ZERO,
                            CallPoison::Clean,
                        ),
                        None,
                    );
                }
            }
            ResolveCallOutcome::Rejected(error) => {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(call_span.as_ref(), error);
            }
            ResolveCallOutcome::Missing(_)
            | ResolveCallOutcome::Resolved(
                ResolvedCallTarget::FunctionValue(_) | ResolvedCallTarget::NonCallable(_),
            ) => self.errors.push(TypeCheckError::new(
                "dialogue resolver returned a non-dialogue target".to_owned(),
            )),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the reduction validator atomically checks shape, borrow semantics, facts, and inferred result"
    )]
    fn check_registered_reduction_constructor(
        &mut self,
        kind: crate::callable::ReductionConstructorKind,
        label: &str,
        schema: &CallableSignatureSchema,
        call: &CallExpr,
        records_facts: bool,
    ) -> RegisteredCandidateCheck {
        let focused = self.is_focused_registered_call(call);
        match kind {
            crate::callable::ReductionConstructorKind::Unchanged => {
                let expected_state = kind.state_type(schema.result());
                let mut fact_builders = self.registered_argument_fact_builders(call, records_facts);
                let mut poison = CallPoison::Clean;
                let mut inferred_state = None;
                if call.args().len() != 1 {
                    self.errors.push(TypeCheckError::new(format!(
                        "`Reduction.unchanged` requires exactly one positional state borrow, got {}",
                        call.args().len()
                    )));
                    poison = CallPoison::Rejected;
                }
                let parameter = schema
                    .group(CallableGroupIndex::ZERO)
                    .and_then(|group| group.parameters().first());
                let context = RegisteredArgumentContext {
                    label,
                    schema,
                    group: CallableGroupIndex::ZERO,
                    parameters: schema
                        .group(CallableGroupIndex::ZERO)
                        .map_or(&[], |group| group.parameters()),
                    call,
                    focused,
                };
                for (argument_index, arg) in call.args().iter().enumerate() {
                    if !self.begin_registered_candidate_argument_probe(call, focused) {
                        poison = CallPoison::Rejected;
                        break;
                    }
                    let (value, mapped, shape_poison) = match arg {
                        CallArg::Positional(value) => (value, parameter, CallPoison::Clean),
                        CallArg::Named { name, value } => {
                            self.errors.push(TypeCheckError::new(format!(
                                "`Reduction.unchanged` state must be positional, got named `{name}`"
                            )));
                            (value.as_ref(), None, CallPoison::Rejected)
                        }
                        CallArg::Spread { value } => {
                            self.errors.push(TypeCheckError::new(
                                "`Reduction.unchanged` state cannot be spread".to_owned(),
                            ));
                            (value.as_ref(), None, CallPoison::Rejected)
                        }
                    };
                    let checked = self.check_registered_argument_slot_with_inferred(
                        context,
                        mapped,
                        FixedLiteralSpreadSlot::Expr(value),
                        argument_index,
                        &mut fact_builders,
                        shape_poison,
                    );
                    poison = poison.merge(checked.poison);
                    if mapped.is_none() {
                        continue;
                    }
                    let Some(TypeKind::BorrowRef { kind, inner, .. }) = checked.inferred else {
                        self.errors.push(TypeCheckError::new(
                            "`Reduction.unchanged` state must be a shared borrow".to_owned(),
                        ));
                        poison = CallPoison::Rejected;
                        continue;
                    };
                    if kind != BorrowKind::Shared {
                        self.errors.push(TypeCheckError::new(
                            "`Reduction.unchanged` state must be a shared borrow".to_owned(),
                        ));
                        poison = CallPoison::Rejected;
                        continue;
                    }
                    if let Some(expected_state) = expected_state.as_ref()
                        && !self.types_compatible(expected_state, &inner)
                    {
                        self.errors.push(TypeCheckError::new(format!(
                            "`Reduction.unchanged` state must borrow {}, found {}",
                            super::super::helpers::type_kind_label(expected_state),
                            super::super::helpers::type_kind_label(&inner)
                        )));
                        poison = CallPoison::Rejected;
                    }
                    inferred_state.get_or_insert(*inner);
                }
                let facts = fact_builders.map_or_else(Vec::new, |builders| {
                    builders
                        .into_iter()
                        .map(ArgumentFactBuilder::finish)
                        .collect()
                });
                RegisteredCandidateCheck {
                    arguments: RegisteredArgumentCheck::new(facts, poison),
                    result: expected_state.map_or_else(
                        || {
                            inferred_state.map_or_else(
                                || TypeKind::Named("Reduction<_>".to_owned()),
                                |state| {
                                    TypeKind::Named(format!("Reduction<{}>", state.source_label()))
                                },
                            )
                        },
                        |_| schema.result().clone(),
                    ),
                }
            }
        }
    }

    fn check_registered_sum_constructor(
        &mut self,
        constructor: RegisteredSumConstructor,
        label: &str,
        schema: &CallableSignatureSchema,
        call: &CallExpr,
        records_facts: bool,
    ) -> RegisteredCandidateCheck {
        let focused = self.is_focused_registered_call(call);
        let mut fact_builders = self.registered_argument_fact_builders(call, records_facts);
        let mut poison = CallPoison::Clean;
        let mut inferred_payload = None;
        if call.args().len() != 1 {
            self.errors.push(TypeCheckError::new(format!(
                "`{label}` requires exactly one positional payload"
            )));
            poison = CallPoison::Rejected;
        }
        let parameters: &[CallableParameter] = schema
            .group(CallableGroupIndex::ZERO)
            .map_or(&[], |group| group.parameters());
        let parameter = parameters.first();
        let context = RegisteredArgumentContext {
            label,
            schema,
            group: CallableGroupIndex::ZERO,
            parameters,
            call,
            focused,
        };
        for (argument_index, arg) in call.args().iter().enumerate() {
            if !self.begin_registered_candidate_argument_probe(call, focused) {
                poison = CallPoison::Rejected;
                break;
            }
            let (value, mapped, shape_poison) = match arg {
                CallArg::Positional(value) => (value, parameter, CallPoison::Clean),
                CallArg::Named { name, value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`{label}` payload must be positional, got named `{name}`"
                    )));
                    (value.as_ref(), None, CallPoison::Rejected)
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`{label}` payload cannot be spread"
                    )));
                    (value.as_ref(), None, CallPoison::Rejected)
                }
            };
            let checked = self.check_registered_argument_slot_with_inferred(
                context,
                mapped,
                FixedLiteralSpreadSlot::Expr(value),
                argument_index,
                &mut fact_builders,
                shape_poison,
            );
            poison = poison.merge(checked.poison);
            if mapped.is_some() {
                inferred_payload = inferred_payload.or(checked.inferred);
            }
        }
        let facts = fact_builders.map_or_else(Vec::new, |builders| {
            builders
                .into_iter()
                .map(ArgumentFactBuilder::finish)
                .collect()
        });
        let result = if constructor.has_instantiated_result(schema.result()) {
            schema.result().clone()
        } else {
            constructor.result_with_payload(inferred_payload.unwrap_or(TypeKind::Unit))
        };
        RegisteredCandidateCheck {
            arguments: RegisteredArgumentCheck::new(facts, poison),
            result,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_registered_data_last_candidate(
        &mut self,
        label: &str,
        id: &DataLastCallableId,
        schema: &CallableSignatureSchema,
        call: &CallExpr,
        _receiver: &Expr,
        _receiver_type: &TypeKind,
        current_group: CallableGroupIndex,
        expression: TypeExpressionId,
        records_facts: bool,
    ) -> RegisteredCandidateCheck {
        let receiver_group = id.receiver_group();
        let result = schema_result_type(schema, receiver_group);
        if let Some(reason) = data_last_unsupported_spread_reason(call.args()) {
            self.errors
                .push(TypeCheckError::unsupported_data_last_method_fallback(
                    label, reason,
                ));
            return RegisteredCandidateCheck {
                arguments: RegisteredArgumentCheck::new(
                    self.check_unmapped_registered_arguments(
                        call,
                        CallPoison::Rejected,
                        records_facts,
                    ),
                    CallPoison::Rejected,
                ),
                result,
            };
        }
        let implicit = (receiver_group == current_group).then_some(id.receiver_parameter());
        let arguments = self.check_registered_schema_args_with_implicit(
            label,
            schema,
            current_group,
            call,
            true,
            implicit,
        );
        let mut mapped = arguments
            .facts
            .iter()
            .filter_map(|argument| {
                let parameter = argument
                    .slots()
                    .iter()
                    .filter_map(CheckedCallArgumentSlotFact::mapped)
                    .filter(|coordinate| coordinate.group() == current_group)
                    .map(|coordinate| coordinate.parameter().get())
                    .min()?;
                Some((parameter, argument.index().get()))
            })
            .collect::<Vec<_>>();
        mapped.sort_unstable();
        mapped.dedup_by_key(|(_, argument)| *argument);
        let arg_order = mapped
            .into_iter()
            .map(|(_, index)| DataLastMethodFallbackArg::CallArg { index })
            .chain(std::iter::once(DataLastMethodFallbackArg::Receiver))
            .collect();
        self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
            expression,
            TypedLoweringEvidenceKind::DataLastMethodFallback {
                method: label.to_owned(),
                arg_count: call.args().len(),
                arg_order,
            },
        ));
        RegisteredCandidateCheck {
            arguments: RegisteredArgumentCheck::new(
                if records_facts {
                    arguments.facts
                } else {
                    Vec::new()
                },
                arguments.poison,
            ),
            result,
        }
    }

    pub(super) fn check_registered_curried_candidate(
        &mut self,
        call: &CallExpr,
        expression: TypeExpressionId,
        value: &CurriedSignatureCallValue,
    ) -> TypeKind {
        let candidate = value
            .resolved
            .as_ref()
            .expect("registered curried calls retain their typed resolver product");
        let current_group = CallableGroupIndex::try_from_usize(value.remaining_group_index)
            .expect("registered curried group was validated by its candidate");
        let Some(module) = self.current_module.as_ref() else {
            return TypeKind::Named("_".to_owned());
        };
        let Some(document) = self
            .project_symbols
            .and_then(|symbols| symbols.source_identity(module))
        else {
            return TypeKind::Named("_".to_owned());
        };
        let call_span = self.source_span_for_current_range(call.range());
        let records_facts = self.records_call_target_facts(call_span.as_ref());
        let argument_check = self.check_registered_schema_args(
            &value.function_name,
            candidate.schema(),
            current_group,
            call,
            records_facts,
        );
        let result = schema_result_type(candidate.schema(), current_group);
        if records_facts && let Some(call_span) = call_span {
            self.record_call_target_facts(
                expression,
                document,
                &call_span,
                CheckedCallTarget::selected(
                    candidate,
                    std::slice::from_ref(candidate),
                    argument_check.facts,
                    result.clone(),
                    current_group,
                    argument_check.poison,
                ),
                None,
            );
        }
        self.retain_registered_curried_result(
            &value.function_name,
            candidate,
            current_group,
            &result,
        );
        result
    }

    fn retain_registered_curried_result(
        &mut self,
        label: &str,
        candidate: &ResolvedCallable,
        current_group: CallableGroupIndex,
        result: &TypeKind,
    ) {
        let next = CallableGroupIndex::try_from_usize(current_group.get() + 1)
            .ok()
            .filter(|next| candidate.schema().group(*next).is_some());
        self.last_checked_curried_signature_call = match (next, result) {
            (Some(next), TypeKind::Function { .. }) => {
                match candidate.try_curried(next, &PRODUCTION_CALLABLE_LIMITS) {
                    Ok(resolved) => Some(CurriedSignatureCallValue {
                        function_name: label.to_owned(),
                        remaining_group_index: next.get(),
                        group_arg_offset: 0,
                        current_group_params: None,
                        pending_higher_order_args: Vec::new(),
                        resolved: Some(resolved),
                    }),
                    Err(error) => {
                        self.errors.push(TypeCheckError::new(error.to_string()));
                        None
                    }
                }
            }
            _ => None,
        };
    }

    fn check_registered_schema_args(
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

    fn begin_registered_candidate_argument_probe(
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

    fn is_focused_registered_call(&self, call: &CallExpr) -> bool {
        let call_span = self.source_span_for_current_range(call.range());
        self.uses_focused_callable_work(call_span.as_ref())
    }

    fn check_registered_schema_args_with_implicit(
        &mut self,
        label: &str,
        schema: &CallableSignatureSchema,
        current_group: CallableGroupIndex,
        call: &CallExpr,
        records_facts: bool,
        implicit: Option<crate::callable::CallableParameterIndex>,
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
        let mut fact_builders = self.registered_argument_fact_builders(call, records_facts);
        let mut provided = vec![false; context.parameters.len()];
        if let Some(implicit) = implicit
            && let Some(slot) = provided.get_mut(implicit.get())
        {
            *slot = true;
        }
        let mut positional = 0usize;
        let mut poison = CallPoison::Clean;
        let mut spread_shape_rejected = false;
        for (argument_index, arg) in args.iter().enumerate() {
            if !self.begin_registered_candidate_argument_probe(call, focused) {
                poison = CallPoison::Rejected;
                break;
            }
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
        RegisteredArgumentCheck::new(
            fact_builders.map_or_else(Vec::new, |builders| {
                builders
                    .into_iter()
                    .map(ArgumentFactBuilder::finish)
                    .collect()
            }),
            poison,
        )
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
                context,
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
            context,
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
                context,
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
            context,
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
                    context,
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
                        context,
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
                            context,
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

    #[allow(
        clippy::too_many_lines,
        reason = "the typed-rest rule atomically validates placement, sequence item type, and checked facts"
    )]
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
            return self.reject_registered_typed_rest_spread(
                format!(
                    "function `{}` has no positional rest parameter",
                    context.label
                ),
                context,
                value,
                argument_index,
                fact_builders,
            );
        };
        if context.parameters.iter().any(|parameter| {
            parameter.presence() == CallableParameterPresence::Required
                && parameter.passing() != CallableParameterPassing::RestPositional
                && !provided[parameter.index().get()]
        }) {
            return self.reject_registered_typed_rest_spread(
                format!(
                    "function `{}` spread argument must follow required fixed arguments",
                    context.label
                ),
                context,
                value,
                argument_index,
                fact_builders,
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
                group: context.group,
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

    fn reject_registered_typed_rest_spread(
        &mut self,
        message: String,
        context: RegisteredArgumentContext<'_>,
        value: &Expr,
        argument_index: usize,
        fact_builders: &mut Option<Vec<ArgumentFactBuilder>>,
    ) -> RegisteredSpreadResult {
        self.errors.push(TypeCheckError::new(message));
        RegisteredSpreadResult {
            poison: self.check_registered_argument_slot(
                context,
                None,
                FixedLiteralSpreadSlot::Expr(value),
                argument_index,
                fact_builders,
                CallPoison::Rejected,
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
        fact_builders: &mut Option<Vec<ArgumentFactBuilder>>,
        poison: CallPoison,
    ) -> CallPoison {
        self.check_registered_argument_slot_with_inferred(
            context,
            parameter,
            value,
            argument_index,
            fact_builders,
            poison,
        )
        .poison
    }

    fn check_registered_argument_slot_with_inferred(
        &mut self,
        context: RegisteredArgumentContext<'_>,
        parameter: Option<&CallableParameter>,
        value: FixedLiteralSpreadSlot<'_>,
        argument_index: usize,
        fact_builders: &mut Option<Vec<ArgumentFactBuilder>>,
        mut poison: CallPoison,
    ) -> RegisteredSlotCheck {
        if self.signature_work_charge.candidate_work
            && !self.charge_signature_work(crate::callable::SignatureWorkKind::ArgumentBindings, 1)
        {
            return RegisteredSlotCheck {
                poison: CallPoison::Rejected,
                inferred: None,
            };
        }
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
        if !self.charge_callable_work(
            context.call,
            context.focused,
            crate::checker::call_target_facts::CallableWorkOperation::TypeCheck,
        ) {
            return RegisteredSlotCheck {
                poison: CallPoison::Rejected,
                inferred: None,
            };
        }
        let actual = self.check_fixed_literal_spread_slot(value, expected);
        if let (Some(expected), Some(actual)) = (expected, actual.as_ref())
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                context.label,
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
                group: context.group,
                parameter,
                inferred: actual.clone(),
                poison,
            },
            fact_builders,
        );
        RegisteredSlotCheck {
            poison,
            inferred: actual,
        }
    }
}

pub(super) fn callable_path(callee: &Expr) -> Option<CallablePath> {
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

fn local_binding_expression(
    identity: &crate::canonicalization::SemanticSymbolIdentity,
) -> Option<TypeExpressionId> {
    let crate::canonicalization::SemanticSymbolIdentity::Local { binding, .. } = identity else {
        return None;
    };
    Some(TypeExpressionId::from_index(
        usize::try_from(binding.0).ok()?,
    ))
}

fn function_value_effects(ty: &TypeKind) -> EffectRow {
    match ty {
        TypeKind::Function { effects, .. } => effects.clone(),
        _ => EffectRow::unknown(),
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
