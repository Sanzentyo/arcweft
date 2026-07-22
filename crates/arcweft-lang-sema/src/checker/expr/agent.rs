use super::super::helpers::{expr_path_label, optional_type_kind_label, type_kind_label};
use super::super::{
    AgentActionEnvParam, DebugPathKind, EntityKind, Expr, MapKind, TypeCheckError, TypeChecker,
    TypeKind,
};
use super::support::{
    AgentInvokeArgs, agent_attach_resource_type, agent_result, set_agent_arg_slot, spread_item_type,
};
use arcweft_lang_syntax::expr::{CallArg, Literal};

impl TypeChecker<'_> {
    pub(super) fn check_agent_intrinsic_call_name(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        match name {
            "expect" => Some(self.check_agent_assert_intrinsic(name, args, "expect")),
            "deny" => Some(self.check_agent_assert_intrinsic(name, args, "deny")),
            "checkpoint" => Some(self.check_agent_record_text_intrinsic(
                name,
                args,
                "checkpoint name",
                &TypeKind::String,
            )),
            "note" => Some(self.check_agent_record_text_intrinsic(
                name,
                args,
                "note text",
                &TypeKind::DisplayText,
            )),
            "attach" => Some(self.check_agent_attach_intrinsic(name, args)),
            "choice_action" => Some(self.check_agent_choice_action_intrinsic(name, args)),
            "viewport" => {
                Some(self.check_agent_no_arg_intrinsic(name, args, TypeKind::CaptureTarget))
            }
            "layer" => Some(self.check_agent_layer_intrinsic(name, args)),
            "object" => Some(self.check_agent_object_intrinsic(name, args)),
            "capture" => Some(self.check_agent_capture_intrinsic(name, args)),
            "read_resource" => Some(self.check_agent_read_resource_intrinsic(name, args)),
            "entity_meta" => Some(self.check_agent_entity_meta_intrinsic(name, args)),
            "project_neighbors" => Some(self.check_agent_project_neighbors_intrinsic(name, args)),
            "signal" => {
                Some(self.check_agent_probe_intrinsic(name, args, &EntityKind::Signal, "signal"))
            }
            "metric" => {
                Some(self.check_agent_probe_intrinsic(name, args, &EntityKind::Metric, "metric"))
            }
            "state_path" => Some(self.check_agent_path_constructor_intrinsic(
                name,
                args,
                "debug state path",
                TypeKind::DebugStatePath,
            )),
            "observation_path" => Some(self.check_agent_path_constructor_intrinsic(
                name,
                args,
                "observation field path",
                TypeKind::ObservationFieldPath,
            )),
            "state" => Some(self.check_agent_path_probe_intrinsic(
                name,
                args,
                "debug state path",
                &TypeKind::DebugStatePath,
                DebugPathKind::State,
            )),
            "observation" => Some(self.check_agent_path_probe_intrinsic(
                name,
                args,
                "observation field path",
                &TypeKind::ObservationFieldPath,
                DebugPathKind::Observation,
            )),
            "diagnostics" => Some(self.check_agent_no_arg_intrinsic(
                name,
                args,
                TypeKind::Named("Diagnostics".to_owned()),
            )),
            "exists" => Some(self.check_agent_exists_intrinsic(name, args)),
            "action_enabled" => Some(self.check_agent_action_enabled_intrinsic(name, args)),
            "all" | "any" => Some(self.check_agent_predicate_list_intrinsic(name, args)),
            "not" => Some(self.check_agent_not_predicate_intrinsic(name, args)),
            "wait" => Some(self.check_agent_wait_intrinsic(name, args)),
            "advance_text" => {
                self.check_function_effects(name);
                Some(self.check_agent_no_arg_intrinsic(
                    name,
                    args,
                    agent_result(TypeKind::ActionResult),
                ))
            }
            "viewport_point" => Some(self.check_agent_viewport_point_intrinsic(name, args)),
            "pointer.click" => Some(self.check_agent_pointer_click_intrinsic(name, args)),
            "invoke" => Some(self.check_agent_invoke_intrinsic(name, args)),
            "rag.query" => Some(self.check_agent_rag_query_intrinsic(name, args)),
            _ => None,
        }
    }

    fn check_agent_assert_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
    ) -> TypeKind {
        let mut condition_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            condition_seen = true;
                            self.expect_expr_type(value, &TypeKind::Bool, context);
                        }
                        1 => {
                            self.expect_expr_type(value, &TypeKind::String, "assertion message");
                        }
                        _ => {
                            self.errors.push(TypeCheckError::new(format!(
                                "{name} received too many positional arguments"
                            )));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "message" => {
                    self.expect_expr_type(value, &TypeKind::String, "assertion message");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} does not accept spread arguments"
                    )));
                    self.check_expr(value);
                }
            }
        }
        if !condition_seen {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires a condition argument"
            )));
        }
        TypeKind::Unit
    }

    fn check_agent_record_text_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
        expected: &TypeKind,
    ) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Unit;
        };
        self.expect_expr_type(arg, expected, context);
        TypeKind::Unit
    }

    fn check_agent_attach_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Unit;
        };
        self.expect_expr_type(arg, &agent_attach_resource_type(), "attach resource");
        TypeKind::Unit
    }

    fn check_agent_choice_action_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::ActionTarget;
        };
        self.expect_expr_type(
            arg,
            &TypeKind::entity_ref(EntityKind::ChoiceOption),
            "choice_action choice",
        );
        TypeKind::ActionTarget
    }

    fn check_agent_no_arg_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        return_type: TypeKind,
    ) -> TypeKind {
        if !args.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} does not accept arguments"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
        }
        return_type
    }

    fn check_agent_layer_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::CaptureTarget;
        };
        self.expect_expr_type(
            arg,
            &TypeKind::entity_ref(EntityKind::Layer),
            "layer target",
        );
        TypeKind::CaptureTarget
    }

    fn check_agent_object_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::CaptureTarget;
        };
        self.expect_expr_type(
            arg,
            &TypeKind::Named("ObservedObjectId".to_owned()),
            "object id",
        );
        TypeKind::CaptureTarget
    }

    fn check_agent_capture_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut target_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        target_seen = true;
                        self.expect_expr_type(value, &TypeKind::CaptureTarget, "capture target");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "capture received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "name" => {
                    self.expect_expr_type(value, &TypeKind::String, "capture name");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "format" || arg_name == "kind" => {
                    self.check_expr(value);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "capture has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "capture does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !target_seen {
            self.errors.push(TypeCheckError::new(
                "capture requires a target argument".to_owned(),
            ));
        }
        agent_result(TypeKind::CaptureRef)
    }

    fn check_agent_viewport_point_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let mut x_seen = false;
        let mut y_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            x_seen = true;
                            self.expect_expr_type(value, &TypeKind::U32, "viewport_point x");
                        }
                        1 => {
                            y_seen = true;
                            self.expect_expr_type(value, &TypeKind::U32, "viewport_point y");
                        }
                        _ => {
                            self.errors.push(TypeCheckError::new(
                                "viewport_point received too many positional arguments".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "x" => {
                    x_seen = true;
                    self.expect_expr_type(value, &TypeKind::U32, "viewport_point x");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "y" => {
                    y_seen = true;
                    self.expect_expr_type(value, &TypeKind::U32, "viewport_point y");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "viewport_point does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !x_seen {
            self.errors
                .push(TypeCheckError::new("viewport_point requires x".to_owned()));
        }
        if !y_seen {
            self.errors
                .push(TypeCheckError::new("viewport_point requires y".to_owned()));
        }
        TypeKind::Named("ViewportPoint".to_owned())
    }

    fn check_agent_pointer_click_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut point_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        point_seen = true;
                        self.expect_expr_type(
                            value,
                            &TypeKind::Named("ViewportPoint".to_owned()),
                            "pointer.click point",
                        );
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "pointer.click received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "point" => {
                    point_seen = true;
                    self.expect_expr_type(
                        value,
                        &TypeKind::Named("ViewportPoint".to_owned()),
                        "pointer.click point",
                    );
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "button" => {
                    self.expect_expr_type(value, &TypeKind::ActionName, "pointer.click button");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "pointer.click has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "pointer.click does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !point_seen {
            self.errors.push(TypeCheckError::new(
                "pointer.click requires a point argument".to_owned(),
            ));
        }
        agent_result(TypeKind::ActionResult)
    }

    fn check_agent_read_resource_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut uri_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        uri_seen = true;
                        self.expect_expr_type(value, &TypeKind::String, "resource uri");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "read_resource received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "uri" => {
                    if uri_seen {
                        self.errors.push(TypeCheckError::new(
                            "read_resource received uri more than once".to_owned(),
                        ));
                    }
                    uri_seen = true;
                    self.expect_expr_type(value, &TypeKind::String, "resource uri");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "read_resource has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "read_resource does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !uri_seen {
            self.errors.push(TypeCheckError::new(
                "read_resource requires a uri argument".to_owned(),
            ));
        }
        agent_result(TypeKind::AgentResource)
    }

    fn check_agent_entity_meta_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return agent_result(TypeKind::AgentEntityMetadata);
        };
        match self.check_expr(arg) {
            Some(TypeKind::Ref(_)) | None => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "entity_meta argument must be an entity reference, found {}",
                type_kind_label(&actual)
            ))),
        }
        agent_result(TypeKind::AgentEntityMetadata)
    }

    pub(super) fn agent_entity_metadata_field_type(field: &str) -> Option<TypeKind> {
        Some(match field {
            "id" | "kind" | "semantic_hash" => TypeKind::String,
            "source" => TypeKind::AgentSourceAnchor,
            _ => return None,
        })
    }

    pub(super) fn agent_source_anchor_field_type(field: &str) -> Option<TypeKind> {
        Some(match field {
            "has_source" => TypeKind::Bool,
            "path" => TypeKind::String,
            "start_byte" | "end_byte" => TypeKind::U64,
            "start_line" | "start_column" | "end_line" | "end_column" => TypeKind::U32,
            _ => return None,
        })
    }

    fn check_agent_project_neighbors_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> TypeKind {
        self.check_function_effects(name);
        let mut root_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        root_seen = true;
                        self.check_agent_entity_ref_arg(value, "project_neighbors root");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "project_neighbors received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "root" => {
                    root_seen = true;
                    self.check_agent_entity_ref_arg(value, "project_neighbors root");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "depth" => {
                    self.expect_expr_type(value, &TypeKind::U32, "project_neighbors depth");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "project_neighbors has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "project_neighbors does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !root_seen {
            self.errors.push(TypeCheckError::new(
                "project_neighbors requires a root argument".to_owned(),
            ));
        }
        agent_result(TypeKind::AgentProjectGraphNeighborhood)
    }

    fn check_agent_entity_ref_arg(&mut self, value: &Expr, label: &str) {
        match self.check_expr(value) {
            Some(TypeKind::Ref(_)) | None => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "{label} must be an entity reference, found {}",
                type_kind_label(&actual)
            ))),
        }
    }

    pub(super) fn agent_project_graph_neighborhood_field_type(field: &str) -> Option<TypeKind> {
        Some(match field {
            "root" => TypeKind::String,
            "node_count" | "edge_count" => TypeKind::U32,
            "symbols" => TypeKind::Vec(Box::new(TypeKind::AgentProjectGraphSymbol)),
            "edges" => TypeKind::Vec(Box::new(TypeKind::AgentProjectGraphEdge)),
            _ => return None,
        })
    }

    pub(super) fn agent_project_graph_symbol_field_type(field: &str) -> Option<TypeKind> {
        Some(match field {
            "symbol_id" | "id" | "kind" | "semantic_hash" | "summary" => TypeKind::String,
            "has_entity"
            | "has_semantic_hash"
            | "has_flow_control"
            | "has_dynamic_control"
            | "has_project_summary" => TypeKind::Bool,
            "entity_count"
            | "agent_action_count"
            | "project_callable_count"
            | "relation_count"
            | "dependency_edge_count"
            | "dynamic_control_flow_count"
            | "debug_query_count"
            | "static_goto_count"
            | "dynamic_goto_count"
            | "branch_count"
            | "loop_count"
            | "await_count"
            | "thread_count"
            | "select_branch_count" => TypeKind::U32,
            _ => return None,
        })
    }

    pub(super) fn agent_project_graph_edge_field_type(field: &str) -> Option<TypeKind> {
        Some(match field {
            "from_symbol_id" | "to_symbol_id" | "kind" => TypeKind::String,
            _ => return None,
        })
    }

    fn check_agent_probe_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        expected_kind: &EntityKind,
        context: &str,
    ) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())));
        };
        match self.check_expr(arg) {
            Some(TypeKind::Ref(entity)) if entity.kind() == expected_kind => {
                if let Some(value) = entity.value() {
                    TypeKind::Probe(Box::new(value.clone()))
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "{context} probe requires a payload type in the project semantic index"
                    )));
                    TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())))
                }
            }
            Some(TypeKind::Ref(entity)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "{context} probe argument must be a {expected_kind:?} reference, found {:?}",
                    entity.kind()
                )));
                TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())))
            }
            Some(actual) => {
                self.errors.push(TypeCheckError::new(format!(
                    "{context} probe argument must be a {expected_kind:?} reference, found {}",
                    type_kind_label(&actual)
                )));
                TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())))
            }
            None => TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned()))),
        }
    }

    fn check_agent_path_probe_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
        expected_path: &TypeKind,
        path_kind: DebugPathKind,
    ) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Probe(Box::new(TypeKind::AgentValue));
        };
        match self.check_expr(arg) {
            Some(TypeKind::String) | None => {}
            Some(actual) if self.types_compatible(expected_path, &actual) => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "{context} must have type String or {}, found {}",
                type_kind_label(expected_path),
                type_kind_label(&actual)
            ))),
        }
        let value_type = Self::agent_debug_path_literal(arg)
            .and_then(|path| self.env.debug_path_type(path_kind, path))
            .cloned()
            .unwrap_or(TypeKind::AgentValue);
        TypeKind::Probe(Box::new(value_type))
    }

    fn check_agent_path_constructor_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
        result: TypeKind,
    ) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return result;
        };
        self.expect_expr_type(arg, &TypeKind::String, context);
        if Self::agent_string_literal(arg).is_some_and(str::is_empty) {
            self.errors
                .push(TypeCheckError::new(format!("{context} must not be empty")));
        }
        result
    }

    fn agent_debug_path_literal(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Literal(Literal::String(value)) => Some(value),
            Expr::Call(call)
                if matches!(
                    expr_path_label(call.callee()).as_deref(),
                    Some("state_path" | "observation_path")
                ) =>
            {
                let [CallArg::Positional(path)] = call.args() else {
                    return None;
                };
                Self::agent_string_literal(path)
            }
            _ => None,
        }
    }

    fn agent_string_literal(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Literal(Literal::String(value)) => Some(value),
            _ => None,
        }
    }

    fn check_agent_exists_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Predicate;
        };
        match self.check_expr(arg) {
            Some(TypeKind::Probe(_)) | None => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "exists argument must be a Probe, found {}",
                type_kind_label(&actual)
            ))),
        }
        TypeKind::Predicate
    }

    fn check_agent_action_enabled_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Predicate;
        };
        self.expect_expr_type(arg, &TypeKind::ActionTarget, "action_enabled target");
        TypeKind::Predicate
    }

    fn check_agent_predicate_list_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        if args.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires at least one predicate argument"
            )));
        }
        if let [CallArg::Positional(value)] = args
            && let Expr::BracketSeq(items) = value.as_ref()
        {
            if items.is_empty() {
                self.errors.push(TypeCheckError::new(format!(
                    "{name} predicate list cannot be empty"
                )));
            }
            for item in items {
                self.expect_expr_type(item, &TypeKind::Predicate, name);
            }
            return TypeKind::Predicate;
        }
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    self.expect_expr_type(value, &TypeKind::Predicate, name);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} arguments must be positional, got named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} arguments cannot be spread"
                    )));
                    self.check_expr(value);
                }
            }
        }
        TypeKind::Predicate
    }

    fn check_agent_not_predicate_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Predicate;
        };
        self.expect_expr_type(arg, &TypeKind::Predicate, "not predicate");
        TypeKind::Predicate
    }

    fn check_agent_wait_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut predicate_seen = false;
        let mut timeout_seen = false;
        let mut positional_index = 0usize;

        for arg in args {
            match arg {
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "timeout" => {
                    timeout_seen = true;
                    self.expect_expr_type(value, &TypeKind::Duration, "wait timeout");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "stable_frames" || arg_name == "poll_frames" => {
                    self.expect_expr_type(value, &TypeKind::U32, &format!("wait {arg_name}"));
                    self.check_wait_positive_u32_literal(arg_name, value);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "wait has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "wait does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            predicate_seen = true;
                            self.expect_expr_type(value, &TypeKind::Predicate, "wait predicate");
                        }
                        1 => {
                            timeout_seen = true;
                            self.expect_expr_type(value, &TypeKind::Duration, "wait timeout");
                        }
                        _ => {
                            self.errors.push(TypeCheckError::new(
                                "wait received too many positional arguments".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
            }
        }
        if !predicate_seen {
            self.errors.push(TypeCheckError::new(
                "wait requires a predicate argument".to_owned(),
            ));
        }
        if !timeout_seen {
            self.errors
                .push(TypeCheckError::new("wait requires timeout".to_owned()));
        }
        TypeKind::Result {
            ok: Box::new(TypeKind::Observation),
            error: Box::new(TypeKind::Named("WaitError".to_owned())),
        }
    }

    fn check_agent_invoke_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let parsed = self.collect_agent_invoke_args(name, args);
        let target_id = parsed
            .target
            .and_then(|target| self.check_agent_invoke_target(target));
        let action_name = parsed
            .action
            .and_then(|action| self.check_agent_action_name(action));
        self.finish_agent_invoke(parsed, target_id, action_name)
    }

    fn collect_agent_invoke_args<'a>(
        &mut self,
        name: &str,
        args: &'a [CallArg],
    ) -> AgentInvokeArgs<'a> {
        let mut target = None;
        let mut action = None;
        let mut action_args = None;
        let mut positional_index = 0usize;

        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            set_agent_arg_slot(
                                &mut target,
                                value,
                                name,
                                "target",
                                &mut self.errors,
                            );
                        }
                        1 => {
                            set_agent_arg_slot(
                                &mut action,
                                value,
                                name,
                                "action",
                                &mut self.errors,
                            );
                        }
                        2 => set_agent_arg_slot(
                            &mut action_args,
                            value,
                            name,
                            "args",
                            &mut self.errors,
                        ),
                        _ => {
                            self.errors.push(TypeCheckError::new(
                                "invoke received too many positional arguments".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "target" => {
                    set_agent_arg_slot(&mut target, value, name, arg_name, &mut self.errors);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "action" => {
                    set_agent_arg_slot(&mut action, value, name, arg_name, &mut self.errors);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "args" => {
                    set_agent_arg_slot(&mut action_args, value, name, arg_name, &mut self.errors);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "invoke has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "invoke does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        AgentInvokeArgs {
            target,
            action,
            action_args,
        }
    }

    fn finish_agent_invoke(
        &mut self,
        parsed: AgentInvokeArgs<'_>,
        target_id: Option<String>,
        action_name: Option<String>,
    ) -> TypeKind {
        if let (Some(target_id), Some(action_name)) = (target_id, action_name) {
            return self.check_resolved_agent_invoke(parsed.action_args, &target_id, &action_name);
        }
        if parsed.target.is_none() {
            self.errors.push(TypeCheckError::new(
                "invoke requires a target argument".to_owned(),
            ));
        }
        if parsed.action.is_none() {
            self.errors.push(TypeCheckError::new(
                "invoke requires an action argument".to_owned(),
            ));
        }
        if let Some(args) = parsed.action_args {
            self.check_agent_invoke_args(args, &[]);
        }
        agent_result(TypeKind::ActionResult)
    }

    fn check_resolved_agent_invoke(
        &mut self,
        action_args: Option<&Expr>,
        target_id: &str,
        action_name: &str,
    ) -> TypeKind {
        let Some(actions) = self.env.agent_actions(target_id) else {
            self.errors.push(TypeCheckError::new(format!(
                "invoke target `{target_id}` exposes no Agent actions"
            )));
            if let Some(args) = action_args {
                self.check_agent_invoke_args(args, &[]);
            }
            return agent_result(TypeKind::ActionResult);
        };
        let Some(signature) = actions
            .iter()
            .find(|signature| signature.action() == action_name)
            .cloned()
        else {
            self.errors.push(TypeCheckError::new(format!(
                "invoke target `{target_id}` has no Agent action `{action_name}`"
            )));
            if let Some(args) = action_args {
                self.check_agent_invoke_args(args, &[]);
            }
            return agent_result(TypeKind::ActionResult);
        };
        if let Some(args) = action_args {
            self.check_agent_invoke_args(args, signature.params());
        } else {
            self.check_agent_invoke_missing_args(target_id, action_name, signature.params());
        }
        agent_result(signature.return_type().clone())
    }

    fn check_agent_invoke_target(&mut self, target: &Expr) -> Option<String> {
        let actual = self.check_expr(target);
        if !actual
            .as_ref()
            .is_some_and(|ty| matches!(ty, TypeKind::Ref(_)))
        {
            self.errors.push(TypeCheckError::new(format!(
                "invoke target must be an entity reference, found {}",
                optional_type_kind_label(actual.as_ref())
            )));
        }
        match target {
            Expr::EntityRef(entity) => Some(entity.body().to_owned()),
            _ => None,
        }
    }

    fn check_agent_action_name(&mut self, action: &Expr) -> Option<String> {
        match action {
            Expr::Path(path) => Some(path.as_label().to_owned()),
            Expr::ShortVariant(name) => Some(name.to_string()),
            Expr::Literal(Literal::String(value)) => Some(value.clone()),
            _ => {
                self.errors.push(TypeCheckError::new(
                    "invoke action must be an ActionName literal such as `.open`".to_owned(),
                ));
                self.check_expr(action);
                None
            }
        }
    }

    fn check_agent_invoke_missing_args(
        &mut self,
        target_id: &str,
        action_name: &str,
        expected_params: &[AgentActionEnvParam],
    ) {
        let missing = expected_params
            .iter()
            .filter(|param| !param.has_default())
            .map(AgentActionEnvParam::name)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "invoke action `{action_name}` on `{target_id}` requires arg(s): {}",
                missing.join(", ")
            )));
        }
    }

    fn check_agent_invoke_args(&mut self, args: &Expr, expected_params: &[AgentActionEnvParam]) {
        if let Expr::RecordLiteral(fields) = args {
            self.check_agent_invoke_record_args(fields, expected_params);
            return;
        }

        let expected = TypeKind::Map {
            kind: MapKind::Sorted,
            key: Box::new(TypeKind::String),
            value: Box::new(TypeKind::AgentValue),
        };
        self.expect_expr_type(args, &expected, "invoke args");
    }

    fn check_agent_invoke_record_args(
        &mut self,
        fields: &[(String, Expr)],
        expected_params: &[AgentActionEnvParam],
    ) {
        let mut seen = std::collections::HashSet::new();
        for (field, value) in fields {
            if !seen.insert(field.as_str()) {
                self.errors.push(TypeCheckError::new(format!(
                    "invoke arg `{field}` was provided more than once"
                )));
            }
            let Some(param) = expected_params
                .iter()
                .find(|param| param.name() == field.as_str())
            else {
                self.errors.push(TypeCheckError::new(format!(
                    "invoke action has no arg named `{field}`"
                )));
                self.expect_expr_type(
                    value,
                    &TypeKind::AgentValue,
                    &format!("invoke arg `{field}`"),
                );
                continue;
            };
            self.expect_expr_type(value, param.ty(), &format!("invoke arg `{field}`"));
        }
        for param in expected_params
            .iter()
            .filter(|param| !param.has_default())
            .filter(|param| !seen.contains(param.name()))
        {
            self.errors.push(TypeCheckError::new(format!(
                "invoke action missing required arg `{}`",
                param.name()
            )));
        }
    }

    fn check_agent_rag_query_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut query_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        query_seen = true;
                        self.expect_expr_type(value, &TypeKind::String, "rag query");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "rag.query received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "roots" => {
                    self.check_agent_rag_roots_arg(value);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "graph_depth" => {
                    self.expect_expr_type(value, &TypeKind::U32, "rag graph_depth");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "limit" => {
                    self.expect_expr_type(value, &TypeKind::USize, "rag limit");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "rag.query has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "rag.query does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !query_seen {
            self.errors.push(TypeCheckError::new(
                "rag.query requires a query argument".to_owned(),
            ));
        }
        TypeKind::Result {
            ok: Box::new(TypeKind::RagContextPack),
            error: Box::new(TypeKind::Named("RagError".to_owned())),
        }
    }

    fn check_agent_rag_roots_arg(&mut self, value: &Expr) {
        if let Expr::BracketSeq(items) = value {
            for item in items {
                self.expect_agent_rag_root_expr(item);
            }
            return;
        }

        let Some(actual) = self.check_expr(value) else {
            return;
        };
        let Some(item) = spread_item_type(&actual) else {
            self.errors.push(TypeCheckError::new(format!(
                "rag.query roots must be a sequence of entity references, found {}",
                type_kind_label(&actual)
            )));
            return;
        };
        if !matches!(item, TypeKind::Ref(_)) {
            self.errors.push(TypeCheckError::new(format!(
                "rag.query roots items must be entity references, found {}",
                type_kind_label(item)
            )));
        }
    }

    fn expect_agent_rag_root_expr(&mut self, value: &Expr) {
        match self.check_expr(value) {
            Some(TypeKind::Ref(_)) | None => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "rag.query roots items must be entity references, found {}",
                type_kind_label(&actual)
            ))),
        }
    }

    fn check_wait_positive_u32_literal(&mut self, name: &str, value: &Expr) {
        if let Expr::Literal(Literal::Int(literal)) = value
            && literal.magnitude().is_ok_and(|magnitude| magnitude < 1)
        {
            self.errors.push(TypeCheckError::new(format!(
                "wait {name} must be at least 1"
            )));
        }
    }

    fn single_positional_agent_arg<'a>(
        &mut self,
        name: &str,
        args: &'a [CallArg],
    ) -> Option<&'a Expr> {
        let mut positional = args.iter().filter_map(|arg| match arg {
            CallArg::Positional(value) => Some(value),
            CallArg::Named {
                name: arg_name,
                value,
            } => {
                self.errors.push(TypeCheckError::new(format!(
                    "{name} arguments must be positional, got named `{arg_name}`"
                )));
                self.check_expr(value);
                None
            }
            CallArg::Spread { value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "{name} arguments cannot be spread"
                )));
                self.check_expr(value);
                None
            }
        });
        let first = positional.next();
        if positional.next().is_some() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires exactly one positional argument"
            )));
        }
        if first.is_none() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires one positional argument"
            )));
        }
        first.map(Box::as_ref)
    }
}
