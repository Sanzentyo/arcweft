//! Member selection, path-member classification, and typed method-call entry.

use super::registered_call;
use super::support::{
    TraitMethodCallOutcome, agent_action_result_field_type, agent_action_target_field_type,
    agent_bbox_field_type, agent_capture_ref_field_type, agent_entity_ref_field_type,
    agent_observation_field_type, agent_observed_object_field_type, agent_resource_body_field_type,
    agent_resource_field_type, inline_failure_builtin_variant_type, std_float_constant_type,
};
use super::{
    Expr, InherentMethodCallOutcome, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
};
use crate::callable::CallResolverAuthority;
use crate::checker::helpers::{
    builtin_path_type, expr_path_label, is_drop_name, type_kind_label, well_known_field_type,
};
use crate::diagnostics::TraitDiagnostic;
use crate::nominal::{
    NominalTypeDiagnosticKind, ResolvedAssociatedTypeReceiver, TypeResolutionReport,
};
use crate::traits::TraitMethodResolution;
use arcweft_lang_hir::symbol::{CallableDeclarationId, ProjectValueLookup};
use arcweft_lang_syntax::ast::{module_path::CanonicalModulePath, symbol_path::SymbolPath};
use arcweft_lang_syntax::expr::{CallExpr, PathMemberCalleeSyntax, SelectExpr};
use arcweft_lang_syntax::types::TypeRef;

enum PathMemberReceiverClassification {
    Value(PathMemberValueBinding),
    OrdinaryFree,
    AssociatedType {
        report: TypeResolutionReport,
        unresolved_value: bool,
    },
    Failed,
}

pub(super) enum PathMemberCallOutcome {
    NotHandled,
    Checked(Option<TypeKind>),
}

enum PathMemberValueLookup {
    Present(PathMemberValueBinding),
    FunctionNamespace,
    Absent,
    Failed,
}

pub(super) enum PathMemberValueBinding {
    Expression,
    ProjectCallable(CallableDeclarationId),
}

impl TypeChecker<'_> {
    pub(super) fn check_selected_callee_call(
        &mut self,
        call: &CallExpr,
        select: &SelectExpr,
        expression_id: TypeExpressionId,
        binding: PathMemberValueBinding,
    ) -> Option<TypeKind> {
        let args = call.args();
        let method_name = select.member().as_str();
        let receiver_expression = TypeExpressionId::from_index(self.stats.expressions);
        let receiver_type = match binding {
            PathMemberValueBinding::Expression => self.check_expr(select.target()),
            PathMemberValueBinding::ProjectCallable(declaration) => {
                self.check_expr_with_project_callable(select.target(), &declaration)
            }
        };
        if self.registered_world.is_none() && is_drop_name(method_name) {
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(TypeKind::Unit);
        }
        let Some(receiver_type) = receiver_type else {
            self.check_untyped_method_args(args);
            return None;
        };
        self.check_typed_method_call(
            select.target(),
            &receiver_type,
            method_name,
            call,
            receiver_expression,
            expression_id,
        )
    }

    pub(super) fn check_path_member_callee_call(
        &mut self,
        call: &CallExpr,
        syntax: &PathMemberCalleeSyntax,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> PathMemberCallOutcome {
        let Expr::Select(select) = call.callee() else {
            self.errors.push(TypeCheckError::new(
                "typed path-member surface lost its semantic selector".to_owned(),
            ));
            self.check_untyped_function_args(call.args());
            return PathMemberCallOutcome::Checked(None);
        };
        match self.classify_path_member_receiver(call, syntax, expected) {
            PathMemberReceiverClassification::Value(binding) => PathMemberCallOutcome::Checked(
                self.check_selected_callee_call(call, select, expression_id, binding),
            ),
            PathMemberReceiverClassification::OrdinaryFree => PathMemberCallOutcome::NotHandled,
            PathMemberReceiverClassification::AssociatedType {
                report,
                unresolved_value,
            } => {
                let Ok(receiver) = ResolvedAssociatedTypeReceiver::try_from_report(&report) else {
                    if unresolved_value || report.diagnostics().is_empty() {
                        if let Some(path) = select.target().dotted_path() {
                            self.errors.push(TypeCheckError::new(format!(
                                "unknown symbol `{}`",
                                path.as_label()
                            )));
                        } else {
                            self.errors.push(TypeCheckError::new(
                                "unknown associated-call receiver".to_owned(),
                            ));
                        }
                    }
                    self.recover_missing_call(
                        call,
                        expression_id,
                        if unresolved_value {
                            crate::callable::UnknownCallKind::Method
                        } else {
                            crate::callable::UnknownCallKind::AssociatedType
                        },
                        crate::callable::CallPoison::Rejected,
                    );
                    return PathMemberCallOutcome::Checked(None);
                };
                PathMemberCallOutcome::Checked(self.check_associated_type_call(
                    call,
                    receiver,
                    syntax.member().as_str(),
                    expected,
                    expression_id,
                ))
            }
            PathMemberReceiverClassification::Failed => {
                self.check_untyped_function_args(call.args());
                PathMemberCallOutcome::Checked(None)
            }
        }
    }

    pub(super) fn recover_missing_call(
        &mut self,
        call: &CallExpr,
        expression: TypeExpressionId,
        kind: crate::callable::UnknownCallKind,
        poison: crate::callable::CallPoison,
    ) {
        let call_span = self.source_span_for_current_range(call.range());
        let document = self
            .source_document_for_current_module()
            .map(|source| source.identity().clone());
        let records_facts =
            document.is_some() && self.records_call_target_facts(call_span.as_ref());
        let arguments = self.check_unmapped_registered_arguments(call, poison, records_facts);
        if records_facts
            && let (Some(document), Some(call_span)) = (document.as_ref(), call_span.as_ref())
        {
            self.record_call_target_facts(
                expression,
                document,
                call_span,
                crate::callable::CheckedCallTarget::missing(
                    kind,
                    arguments,
                    crate::callable::CallableGroupIndex::ZERO,
                ),
                Vec::new(),
            );
        }
    }

    fn classify_path_member_receiver(
        &mut self,
        call: &CallExpr,
        syntax: &PathMemberCalleeSyntax,
        expected: Option<&TypeKind>,
    ) -> PathMemberReceiverClassification {
        if !syntax.separator().is_explicit_path() {
            let Expr::Select(select) = call.callee() else {
                self.errors.push(TypeCheckError::new(
                    "typed path-member surface lost its semantic selector".to_owned(),
                ));
                return PathMemberReceiverClassification::Failed;
            };
            match self.lookup_path_member_value(select.target(), syntax) {
                PathMemberValueLookup::Present(binding) => {
                    return PathMemberReceiverClassification::Value(binding);
                }
                PathMemberValueLookup::FunctionNamespace => {
                    return match self.qualified_free_path_is_present(call, expected) {
                        Ok(true) => PathMemberReceiverClassification::OrdinaryFree,
                        Ok(false) => PathMemberReceiverClassification::Value(
                            PathMemberValueBinding::Expression,
                        ),
                        Err(error) => {
                            self.errors.push(TypeCheckError::new(error.to_string()));
                            PathMemberReceiverClassification::Failed
                        }
                    };
                }
                PathMemberValueLookup::Failed => {
                    return PathMemberReceiverClassification::Failed;
                }
                PathMemberValueLookup::Absent => {}
            }
            match self.qualified_free_path_is_present(call, expected) {
                Ok(true) => return PathMemberReceiverClassification::OrdinaryFree,
                Ok(false) => {}
                Err(error) => {
                    self.errors.push(TypeCheckError::new(error.to_string()));
                    return PathMemberReceiverClassification::Failed;
                }
            }
            if select.target().dotted_path().is_some_and(|path| {
                (1..=path.segments().len()).rev().any(|segment_count| {
                    path.prefix(segment_count)
                        .is_some_and(|prefix| builtin_path_type(prefix.as_label()).is_some())
                })
            }) {
                return PathMemberReceiverClassification::Value(PathMemberValueBinding::Expression);
            }
        }

        let generics = self.active_generic_scope.clone();
        let self_scope = self.active_self_scope.clone();
        #[cfg(test)]
        {
            self.stats.associated_nominal_receiver_resolutions += 1;
        }
        let report =
            self.resolve_authored_type_report_unpublished(syntax.receiver(), &generics, self_scope);
        let unresolved_value = !syntax.separator().is_explicit_path()
            && matches!(syntax.receiver().value(), TypeRef::Path(_))
            && ResolvedAssociatedTypeReceiver::try_from_report(&report).is_err()
            && report.omitted_diagnostics() == 0
            && report.diagnostics().iter().all(|diagnostic| {
                matches!(diagnostic.kind(), NominalTypeDiagnosticKind::Unknown { .. })
            });
        if !unresolved_value {
            self.publish_authored_type_report(syntax.receiver(), &report);
        }
        PathMemberReceiverClassification::AssociatedType {
            report,
            unresolved_value,
        }
    }

    fn qualified_free_path_is_present(
        &self,
        call: &CallExpr,
        expected: Option<&TypeKind>,
    ) -> Result<bool, crate::callable::ResolveCallError> {
        let Some(path) = registered_call::callable_path(call.callee()) else {
            return Ok(false);
        };
        if self.registered_world.is_none() && self.function_type(&path.dotted_name()).is_some() {
            return Ok(true);
        }
        let module = self
            .current_module
            .clone()
            .unwrap_or_else(CanonicalModulePath::crate_root);
        let enum_variant = self
            .project_symbols
            .and_then(|symbols| self.registered_enum_seed(expected, &path, &module, symbols));
        match self.registered_world.zip(self.project_symbols) {
            Some((world, symbols)) => CallResolverAuthority::accepted(&module, symbols, world)
                .qualified_free_path_is_present(&path, enum_variant.is_some()),
            None => CallResolverAuthority::detached(self.env)
                .qualified_free_path_is_present(&path, enum_variant.is_some()),
        }
    }

    fn lookup_path_member_value(
        &mut self,
        receiver: &Expr,
        syntax: &PathMemberCalleeSyntax,
    ) -> PathMemberValueLookup {
        #[cfg(test)]
        {
            self.stats.associated_value_namespace_lookups += 1;
        }
        let Some(path) = receiver.dotted_path() else {
            self.errors.push(TypeCheckError::new(
                "path-member receiver has no structural value path".to_owned(),
            ));
            return PathMemberValueLookup::Failed;
        };

        let mut environment_value = None;
        for segment_count in (1..=path.segments().len()).rev() {
            let prefix = path
                .prefix(segment_count)
                .expect("bounded nonzero prefix count is valid");
            let value_name = prefix.as_label();
            if self.locals.contains_key(value_name)
                || self.global_symbols.contains_key(value_name)
                || self.global_functions.contains_key(value_name)
                || self.env.function_type(value_name).is_some()
            {
                return PathMemberValueLookup::Present(PathMemberValueBinding::Expression);
            }
            if self.env.symbol_type(value_name).is_some() {
                environment_value = Some(
                    segment_count == path.segments().len()
                        && self.env.is_function_namespace(value_name),
                );
                break;
            }
        }

        if let Some(symbols) = self.project_symbols {
            let module = self
                .current_module
                .clone()
                .unwrap_or_else(|| self.checked_module.module_path().clone());
            let Some(source) =
                self.source_span_for_current_range(*syntax.receiver().root_source().whole())
            else {
                self.errors.push(TypeCheckError::new(
                    "project value lookup requires the receiver's accepted source span".to_owned(),
                ));
                return PathMemberValueLookup::Failed;
            };
            let type_path = match arcweft_lang_syntax::types::TypePath::try_from(&path) {
                Ok(path) => path,
                Err(error) => {
                    self.errors.push(TypeCheckError::new(error.to_string()));
                    return PathMemberValueLookup::Failed;
                }
            };
            let reference = match SymbolPath::try_from(type_path.path()) {
                Ok(reference) => reference,
                Err(error) => {
                    self.errors.push(TypeCheckError::new(error.to_string()));
                    return PathMemberValueLookup::Failed;
                }
            };
            match symbols.resolve_value_target(&module, &reference, source) {
                Ok(ProjectValueLookup::Present(callable)) => {
                    return PathMemberValueLookup::Present(
                        PathMemberValueBinding::ProjectCallable(callable.declaration().clone()),
                    );
                }
                Ok(ProjectValueLookup::Absent) => {}
                Err(error) => {
                    self.errors
                        .push(TypeCheckError::project_value_lookup(error));
                    return PathMemberValueLookup::Failed;
                }
            }
        }

        match environment_value {
            Some(true) => PathMemberValueLookup::FunctionNamespace,
            Some(false) => PathMemberValueLookup::Present(PathMemberValueBinding::Expression),
            None => PathMemberValueLookup::Absent,
        }
    }

    fn check_typed_method_call(
        &mut self,
        receiver: &Expr,
        receiver_type: &TypeKind,
        method_name: &str,
        call: &CallExpr,
        receiver_expression: TypeExpressionId,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let args = call.args();
        match self.check_inherent_method_call(
            receiver,
            receiver_type,
            method_name,
            args,
            call,
            receiver_expression,
            expression_id,
        ) {
            InherentMethodCallOutcome::Missing => {}
            InherentMethodCallOutcome::Checked(return_type) => return return_type,
        }
        if self.registered_world.is_none() {
            match self.check_trait_method_call(receiver_type, method_name, args) {
                TraitMethodCallOutcome::Missing => {}
                TraitMethodCallOutcome::Typed(return_type) => return Some(return_type),
                TraitMethodCallOutcome::Rejected => return None,
            }
            if let Some(return_type) = self.check_data_last_method_fallback(
                receiver,
                receiver_type,
                method_name,
                args,
                expression_id,
            ) {
                return Some(return_type);
            }
        }
        self.check_untyped_method_args(args);
        if self.registered_world.is_none()
            && let Some(return_type) = self.env.method_type(receiver_type, method_name).cloned()
        {
            return Some(return_type);
        }
        self.errors.push(TypeCheckError::new(format!(
            "unknown method `{method_name}` on {}",
            type_kind_label(receiver_type)
        )));
        None
    }

    pub(super) fn check_select_expr(
        &mut self,
        expr: &Expr,
        select: &SelectExpr,
    ) -> Option<TypeKind> {
        let target = select.target();
        let field = select.member().as_str();
        if let Some(path) = expr_path_label(expr) {
            if let Some(ty) = self.locals.get(&path).cloned() {
                return Some(ty);
            }
            if let Some(ty) = self.env.symbol_type(&path).cloned() {
                return Some(ty);
            }
            if let Some(ty) = std_float_constant_type(&path) {
                return Some(ty);
            }
            if let Some(ty) = inline_failure_builtin_variant_type(&path) {
                return Some(ty);
            }
        }
        let receiver_type = self.check_expr(target);
        let field_type = receiver_type
            .as_ref()
            .and_then(|receiver| self.value_field_type(receiver, field));
        if field_type.is_some() {
            return field_type;
        }
        if let Some(receiver_type) = receiver_type.as_ref()
            && self.reject_method_value_reference(receiver_type, field)
        {
            return Some(TypeKind::Named("_".to_owned()));
        }
        None
    }

    pub(in crate::checker) fn value_field_type(
        &self,
        receiver_type: &TypeKind,
        field: &str,
    ) -> Option<TypeKind> {
        if let Some(field_type) = self.nominal_field_type(receiver_type, field) {
            return Some(field_type);
        }
        match receiver_type {
            TypeKind::Observation => agent_observation_field_type(field),
            TypeKind::ObservedObject => agent_observed_object_field_type(field),
            TypeKind::AgentBBox => agent_bbox_field_type(field),
            TypeKind::ActionTarget => agent_action_target_field_type(field),
            TypeKind::ActionResult => agent_action_result_field_type(field),
            TypeKind::CaptureRef => agent_capture_ref_field_type(field),
            TypeKind::AgentEntityMetadata => Self::agent_entity_metadata_field_type(field),
            TypeKind::AgentSourceAnchor => Self::agent_source_anchor_field_type(field),
            TypeKind::AgentProjectGraphNeighborhood => {
                Self::agent_project_graph_neighborhood_field_type(field)
            }
            TypeKind::AgentProjectGraphSymbol => Self::agent_project_graph_symbol_field_type(field),
            TypeKind::AgentProjectGraphEdge => Self::agent_project_graph_edge_field_type(field),
            TypeKind::AgentResource => agent_resource_field_type(field),
            TypeKind::AgentResourceBody => agent_resource_body_field_type(field),
            TypeKind::Ref(_) => {
                agent_entity_ref_field_type(field).or_else(|| well_known_field_type(field))
            }
            TypeKind::Map { value, .. } => Some(value.as_ref().clone()),
            TypeKind::Named(name) if name == "HttpRequestContext" => match field {
                "method" | "path" | "body" => Some(TypeKind::String),
                _ => None,
            },
            _ => well_known_field_type(field),
        }
    }

    fn reject_method_value_reference(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
    ) -> bool {
        if self
            .env
            .method_signature(receiver_type, method_name)
            .is_some()
        {
            self.errors
                .push(TypeCheckError::unsupported_method_value_reference(
                    receiver_type.clone(),
                    method_name,
                    "environment method values need an explicit receiver-binding contract; call the method directly or wrap it in an explicit closure",
                ));
            return true;
        }
        match self.trait_catalog.resolve_method(
            receiver_type,
            method_name,
            &self.active_trait_predicates(),
        ) {
            TraitMethodResolution::Missing => false,
            TraitMethodResolution::Inherent { .. } | TraitMethodResolution::Unique { .. } => {
                self.errors
                    .push(TypeCheckError::unsupported_method_value_reference(
                        receiver_type.clone(),
                        method_name,
                        "trait/impl method values need an explicit receiver-binding contract; call the method directly or wrap it in an explicit closure",
                    ));
                true
            }
            TraitMethodResolution::Ambiguous(candidates) => {
                self.errors.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::ambiguous_method(
                        method_name,
                        candidates
                            .iter()
                            .map(|candidate| candidate.trait_name.as_str())
                            .collect::<Vec<_>>(),
                    ),
                ));
                true
            }
        }
    }
}
