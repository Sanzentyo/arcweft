//! Lower the sema-owned, HIR-free Fx definition carrier to presentation graphs.
//!
//! This module deliberately has no access to HIR, symbols, or call facts. The
//! final semantic analysis has already closed all of those boundaries into
//! `CheckedProjectFxDefinition`, `CheckedFxBody`, and the checked definition
//! catalog. Compiler lowering therefore only performs the typed projection
//! from that carrier to the presentation graph model.

use std::collections::BTreeMap;

use arcweft_lang_sema::final_analysis::{
    CheckedFxArgument, CheckedFxBindingDecision, CheckedFxBodyCall,
    CheckedFxConstructorArgumentValue, CheckedFxDefinition, CheckedFxDefinitionCatalog,
    CheckedFxDefinitionRef, CheckedFxGraphExpression, CheckedFxSourceParameter,
    CheckedFxSymbolicValue, CheckedProjectFxDefinition, CheckedSymbolicFxBinding,
};
use arcweft_presentation::fx::{
    BuiltinFxGraphArgument, BuiltinFxParameterBinding, BuiltinFxParameterId, FX_MAX_GRAPH_DEPTH,
    FxDefinition, FxDefinitionArgumentValue, FxDefinitionGraphContext, FxDefinitionParameterIndex,
    FxDefinitionParameterSchema, FxGraph, FxId, FxNode, FxNodeKind, FxProperty,
    FxSourceConstructor, FxSourceParameterRole, FxSourceParameterType, FxStaticType, FxStaticValue,
    build_builtin_fx_definition, build_builtin_fx_graph,
};

/// Lowers one sema-sealed project Fx definition.
///
/// The checked definition is the sole authority for its parameter schema and
/// body. The catalog is the sole authority for nested project and builtin
/// definitions. No HIR declaration, symbol table, or retained call fact is
/// consulted here.
pub(crate) fn lower_checked_definition(
    checked: &CheckedProjectFxDefinition,
    catalog: &CheckedFxDefinitionCatalog,
) -> Result<FxDefinition, String> {
    let schema = checked.parameter_schema();
    if schema.id() != checked.definition() {
        return Err("checked project Fx parameter schema has a foreign identity".to_owned());
    }
    if catalog
        .get(checked.definition())
        .is_none_or(|definition| definition.project() != Some(checked))
    {
        return Err("checked project Fx definition is absent from its catalog".to_owned());
    }

    let context = FxDefinitionGraphContext::new(schema);
    let mut lowerer = GraphLowerer {
        catalog,
        context,
        root_id: checked.definition().clone(),
        stack: vec![checked.definition().clone()],
    };
    let bindings = identity_bindings(schema);
    let graph = lowerer.lower_expression(checked.body().root(), &bindings, 0)?;
    FxDefinition::from_parameter_schema(schema.clone(), graph).map_err(|error| error.to_string())
}

/// A nested project body resolves its own parameter references through this
/// map. The values are already projected into the outer definition's static
/// domain, so recursive lowering never needs to inspect source expressions.
type StaticBindings = BTreeMap<FxDefinitionParameterIndex, FxStaticValue>;

fn identity_bindings(schema: &FxDefinitionParameterSchema) -> StaticBindings {
    schema
        .parameters()
        .iter()
        .map(|parameter| {
            (
                parameter.index(),
                FxStaticValue::Parameter(parameter.parameter_ref()),
            )
        })
        .collect()
}

struct GraphLowerer<'a> {
    catalog: &'a CheckedFxDefinitionCatalog,
    context: FxDefinitionGraphContext<'a>,
    root_id: FxId,
    stack: Vec<FxId>,
}

impl GraphLowerer<'_> {
    fn lower_expression(
        &mut self,
        expression: &CheckedFxGraphExpression,
        bindings: &StaticBindings,
        depth: usize,
    ) -> Result<FxGraph, String> {
        if depth > FX_MAX_GRAPH_DEPTH {
            return Err("Fx graph expansion exceeds the compiler recursion limit".to_owned());
        }
        match expression {
            CheckedFxGraphExpression::Constructor(call) => {
                self.lower_constructor(call.constructor(), call.arguments(), bindings, depth)
            }
            CheckedFxGraphExpression::Builtin(call) => self.lower_builtin_call(call, bindings),
            CheckedFxGraphExpression::Project(call) => {
                self.lower_project_call(call, bindings, depth)
            }
        }
    }

    fn lower_constructor(
        &mut self,
        constructor: FxSourceConstructor,
        arguments: &[arcweft_lang_sema::final_analysis::CheckedFxConstructorArgument],
        bindings: &StaticBindings,
        depth: usize,
    ) -> Result<FxGraph, String> {
        let mut values = BTreeMap::<u16, &CheckedFxConstructorArgumentValue>::new();
        for argument in arguments {
            if constructor
                .parameter_schema()
                .get(usize::from(argument.parameter()))
                .is_none()
            {
                return Err(format!(
                    "Fx.{} constructor argument index {} is out of range",
                    constructor.source_name(),
                    argument.parameter()
                ));
            }
            if values
                .insert(argument.parameter(), argument.value())
                .is_some()
            {
                return Err(format!(
                    "Fx.{} constructor repeats parameter {}",
                    constructor.source_name(),
                    argument.parameter()
                ));
            }
        }

        match constructor {
            FxSourceConstructor::Conditional => {
                let condition =
                    self.constructor_value(constructor, &values, FxSourceParameterRole::Condition)?;
                let then_graph = self.constructor_graph(
                    constructor,
                    &values,
                    FxSourceParameterRole::ThenGraph,
                    bindings,
                    depth + 1,
                )?;
                let else_graph = self.constructor_graph(
                    constructor,
                    &values,
                    FxSourceParameterRole::ElseGraph,
                    bindings,
                    depth + 1,
                )?;
                let condition = self.lower_symbolic_value(
                    condition,
                    FxStaticType::Runtime(arcweft_presentation::fx::FxRuntimeType::Bool),
                    bindings,
                )?;
                FxGraph::try_new(vec![FxNode::Conditional {
                    condition,
                    then_graph,
                    else_graph,
                }])
                .map_err(|error| error.to_string())
            }
            FxSourceConstructor::Stack => {
                let index = parameter_index(constructor, FxSourceParameterRole::Graphs)?;
                let Some(CheckedFxConstructorArgumentValue::Graphs(children)) = values.get(&index)
                else {
                    return Err("Fx.stack requires a checked graph list".to_owned());
                };
                let children = children
                    .iter()
                    .map(|child| self.lower_expression(child, bindings, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                FxGraph::try_new(vec![FxNode::Stack { children }])
                    .map_err(|error| error.to_string())
            }
            _ => self.lower_leaf_constructor(constructor, &values, bindings),
        }
    }

    fn lower_leaf_constructor(
        &mut self,
        constructor: FxSourceConstructor,
        values: &BTreeMap<u16, &CheckedFxConstructorArgumentValue>,
        bindings: &StaticBindings,
    ) -> Result<FxGraph, String> {
        let mut properties = Vec::with_capacity(values.len());
        for (index, value) in values {
            let parameter = constructor
                .parameter_schema()
                .get(usize::from(*index))
                .ok_or_else(|| "Fx constructor parameter is out of range".to_owned())?;
            let FxSourceParameterRole::Property(property) = parameter.role() else {
                return Err(format!(
                    "Fx.{} leaf retained structural parameter `{}`",
                    constructor.source_name(),
                    parameter.source_name()
                ));
            };
            let expected = static_type(parameter.value_type())?;
            let CheckedFxConstructorArgumentValue::Value(value) = value else {
                return Err(format!(
                    "Fx.{} leaf property `{}` is not a closed value",
                    constructor.source_name(),
                    parameter.source_name()
                ));
            };
            let value = self.lower_symbolic_value(value, expected, bindings)?;
            properties.push(FxProperty::new(property, value));
        }
        let node = match constructor.node_kind() {
            FxNodeKind::Style => FxNode::Style { properties },
            FxNodeKind::Text => FxNode::Text { properties },
            FxNodeKind::Color => FxNode::Color { properties },
            FxNodeKind::Transform => FxNode::Transform {
                fx: self.root_id.clone(),
                properties,
            },
            FxNodeKind::Mask => FxNode::Mask {
                fx: self.root_id.clone(),
                properties,
            },
            FxNodeKind::Filter => FxNode::Filter {
                fx: self.root_id.clone(),
                properties,
            },
            FxNodeKind::Transition => FxNode::Transition {
                fx: self.root_id.clone(),
                properties,
            },
            FxNodeKind::Shader => {
                return Err(
                    "Fx shader constructors are not part of the checked source domain".to_owned(),
                );
            }
            FxNodeKind::OffscreenPass | FxNodeKind::PostProcess => {
                return Err("Fx constructor is not part of the checked source domain".to_owned());
            }
            FxNodeKind::Conditional | FxNodeKind::Stack => {
                return Err("Fx structural constructor reached leaf lowering".to_owned());
            }
        };
        FxGraph::try_new(vec![node]).map_err(|error| error.to_string())
    }

    fn constructor_value<'a>(
        &self,
        constructor: FxSourceConstructor,
        values: &'a BTreeMap<u16, &'a CheckedFxConstructorArgumentValue>,
        role: FxSourceParameterRole,
    ) -> Result<&'a CheckedFxSymbolicValue, String> {
        let index = parameter_index(constructor, role)?;
        match values.get(&index) {
            Some(CheckedFxConstructorArgumentValue::Value(value)) => Ok(value),
            Some(_) => Err(format!(
                "Fx.{} parameter `{}` is not a closed value",
                constructor.source_name(),
                role_name(role)
            )),
            None => Err(format!(
                "Fx.{} parameter `{}` is missing",
                constructor.source_name(),
                role_name(role)
            )),
        }
    }

    fn constructor_graph(
        &mut self,
        constructor: FxSourceConstructor,
        values: &BTreeMap<u16, &CheckedFxConstructorArgumentValue>,
        role: FxSourceParameterRole,
        bindings: &StaticBindings,
        depth: usize,
    ) -> Result<FxGraph, String> {
        let index = parameter_index(constructor, role)?;
        match values.get(&index) {
            Some(CheckedFxConstructorArgumentValue::Graph(value)) => {
                self.lower_expression(value, bindings, depth)
            }
            Some(_) => Err(format!(
                "Fx.{} parameter `{}` is not one graph",
                constructor.source_name(),
                role_name(role)
            )),
            None => Err(format!(
                "Fx.{} parameter `{}` is missing",
                constructor.source_name(),
                role_name(role)
            )),
        }
    }

    fn lower_builtin_call(
        &mut self,
        call: &CheckedFxBodyCall<CheckedSymbolicFxBinding>,
        bindings: &StaticBindings,
    ) -> Result<FxGraph, String> {
        let CheckedFxDefinitionRef::Builtin { specialization, .. } = call.definition() else {
            return Err(
                "checked builtin Fx body call has a project definition reference".to_owned(),
            );
        };
        self.validate_definition_reference(call.definition())?;
        let template = build_builtin_fx_definition(*specialization)
            .map_err(|error| format!("builtin Fx template: {error}"))?;
        let row = arcweft_presentation::fx::BUILTIN_FX_CALLABLE_CATALOG
            .get(specialization.row())
            .ok_or_else(|| "builtin Fx row is absent from the presentation catalog".to_owned())?;

        let arguments = row
            .parameters()
            .iter()
            .copied()
            .filter(|parameter| {
                parameter.binding() == BuiltinFxParameterBinding::Abi
                    && specialization
                        .active_abi_parameters()
                        .contains(parameter.id())
            })
            .map(|parameter| {
                let argument = call_argument(
                    call.arguments(),
                    CheckedFxSourceParameter::Builtin(parameter.id()),
                )
                .ok_or_else(|| {
                    format!(
                        "builtin Fx parameter `{}` is absent",
                        parameter.source_name()
                    )
                })?;
                match argument.decision() {
                    CheckedFxBindingDecision::Explicit(CheckedSymbolicFxBinding::Value(value)) => {
                        let value = self.lower_symbolic_value(
                            value,
                            parameter
                                .parameter_type()
                                .direct_definition_parameter_type()
                                .ok_or_else(|| {
                                    "builtin ABI parameter has no definition type".to_owned()
                                })?
                                .static_type(),
                            bindings,
                        )?;
                        static_to_graph_argument(value, parameter.id())
                    }
                    CheckedFxBindingDecision::Defaulted => {
                        let index = template
                            .binding_plan()
                            .parameters()
                            .iter()
                            .find(|row| row.id() == parameter.id())
                            .and_then(|row| match row.projection() {
                                arcweft_presentation::fx::BuiltinFxAbiProjection::Direct(index) => {
                                    Some(*index)
                                }
                            })
                            .ok_or_else(|| {
                                "builtin ABI parameter has no projected index".to_owned()
                            })?;
                        let value = template
                            .definition()
                            .parameters()
                            .get(usize::from(index.get()))
                            .and_then(|parameter| parameter.default())
                            .cloned()
                            .ok_or_else(|| {
                                format!(
                                    "builtin default for `{}` is absent",
                                    parameter.source_name()
                                )
                            })?;
                        Ok(BuiltinFxGraphArgument::constant(value))
                    }
                    CheckedFxBindingDecision::Explicit(
                        CheckedSymbolicFxBinding::Phase(_)
                        | CheckedSymbolicFxBinding::Target(_)
                        | CheckedSymbolicFxBinding::MotionFunction(_),
                    )
                    | CheckedFxBindingDecision::Omitted => Err(format!(
                        "active builtin ABI parameter `{}` has no ABI value",
                        parameter.source_name()
                    )),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        build_builtin_fx_graph(&self.context, *specialization, arguments)
            .map_err(|error| format!("builtin Fx graph: {error}"))
    }

    fn lower_project_call(
        &mut self,
        call: &CheckedFxBodyCall<CheckedSymbolicFxBinding>,
        bindings: &StaticBindings,
        depth: usize,
    ) -> Result<FxGraph, String> {
        let definition_id = call.definition().definition();
        self.validate_definition_reference(call.definition())?;
        if self.stack.iter().any(|item| item == definition_id) {
            return Err(format!(
                "Fx graph composition contains a cycle at `{definition_id}`"
            ));
        }
        let CheckedFxDefinition::Project(project) = self
            .catalog
            .get(definition_id)
            .ok_or_else(|| "nested project Fx definition is absent from the catalog".to_owned())?
        else {
            return Err("project Fx body call resolved to a builtin definition".to_owned());
        };
        let child_bindings = self.project_call_bindings(project, call.arguments(), bindings)?;
        self.stack.push(definition_id.clone());
        let graph = self.lower_expression(project.body().root(), &child_bindings, depth + 1);
        self.stack.pop();
        graph
    }

    fn project_call_bindings(
        &mut self,
        project: &CheckedProjectFxDefinition,
        arguments: &[CheckedFxArgument<CheckedSymbolicFxBinding>],
        bindings: &StaticBindings,
    ) -> Result<StaticBindings, String> {
        project
            .parameter_schema()
            .parameters()
            .iter()
            .map(|parameter| {
                let source = CheckedFxSourceParameter::Project(parameter.index());
                let argument = call_argument(arguments, source).ok_or_else(|| {
                    format!("project Fx parameter `{}` is absent", parameter.name())
                })?;
                let value = match argument.decision() {
                    CheckedFxBindingDecision::Explicit(CheckedSymbolicFxBinding::Value(value)) => {
                        self.lower_symbolic_value(
                            value,
                            parameter.parameter_type().static_type(),
                            bindings,
                        )?
                    }
                    CheckedFxBindingDecision::Defaulted => parameter
                        .default()
                        .cloned()
                        .map(|value| static_value_from_argument(&value))
                        .transpose()?
                        .ok_or_else(|| {
                            format!("project Fx default for `{}` is absent", parameter.name())
                        })?,
                    CheckedFxBindingDecision::Omitted => parameter
                        .default()
                        .cloned()
                        .map(|value| static_value_from_argument(&value))
                        .transpose()?
                        .ok_or_else(|| {
                            format!(
                                "project Fx omitted parameter `{}` has no default",
                                parameter.name()
                            )
                        })?,
                    CheckedFxBindingDecision::Explicit(
                        CheckedSymbolicFxBinding::Phase(_)
                        | CheckedSymbolicFxBinding::Target(_)
                        | CheckedSymbolicFxBinding::MotionFunction(_),
                    ) => {
                        return Err(format!(
                            "project Fx parameter `{}` has a structural value",
                            parameter.name()
                        ));
                    }
                };
                Ok((parameter.index(), value))
            })
            .collect()
    }

    fn lower_symbolic_value(
        &self,
        value: &CheckedFxSymbolicValue,
        expected: FxStaticType,
        bindings: &StaticBindings,
    ) -> Result<FxStaticValue, String> {
        let value = match value {
            CheckedFxSymbolicValue::Parameter(parameter) => {
                bindings.get(&parameter.index()).cloned().ok_or_else(|| {
                    "checked Fx parameter reference is outside the active body".to_owned()
                })?
            }
            CheckedFxSymbolicValue::Constant(value) => static_value_from_constant(value)?,
            CheckedFxSymbolicValue::Program(program) => FxStaticValue::Sampler(program.clone()),
        };
        if value.static_type() != expected {
            return Err(format!(
                "checked Fx value has type {:?}, expected {:?}",
                value.static_type(),
                expected
            ));
        }
        Ok(value)
    }

    fn validate_definition_reference(
        &self,
        reference: &CheckedFxDefinitionRef,
    ) -> Result<(), String> {
        let definition = self.catalog.get(reference.definition()).ok_or_else(|| {
            "checked Fx definition reference is absent from the catalog".to_owned()
        })?;
        if definition.reference() != *reference {
            return Err(
                "checked Fx definition reference does not match catalog authority".to_owned(),
            );
        }
        Ok(())
    }
}

fn call_argument<'a>(
    arguments: &'a [CheckedFxArgument<CheckedSymbolicFxBinding>],
    parameter: CheckedFxSourceParameter,
) -> Option<&'a CheckedFxArgument<CheckedSymbolicFxBinding>> {
    arguments
        .iter()
        .find(|argument| argument.parameter() == parameter)
}

fn parameter_index(
    constructor: FxSourceConstructor,
    role: FxSourceParameterRole,
) -> Result<u16, String> {
    constructor
        .parameter_schema()
        .iter()
        .position(|parameter| parameter.role() == role)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or_else(|| {
            format!(
                "Fx.{} has no `{}` parameter",
                constructor.source_name(),
                role_name(role)
            )
        })
}

fn role_name(role: FxSourceParameterRole) -> &'static str {
    match role {
        FxSourceParameterRole::Property(property) => property.source_name(),
        FxSourceParameterRole::Condition => "condition",
        FxSourceParameterRole::ThenGraph => "then",
        FxSourceParameterRole::ElseGraph => "else",
        FxSourceParameterRole::Graphs => "graphs",
    }
}

fn static_type(value_type: FxSourceParameterType) -> Result<FxStaticType, String> {
    match value_type {
        FxSourceParameterType::Static(value) => Ok(value),
        FxSourceParameterType::TransitionKind => Ok(FxStaticType::Selector(
            arcweft_presentation::fx::FxSelectorDomain::TransitionKind,
        )),
        FxSourceParameterType::TransitionEasing => Ok(FxStaticType::Selector(
            arcweft_presentation::fx::FxSelectorDomain::TransitionEasing,
        )),
        FxSourceParameterType::Fx | FxSourceParameterType::FxList => {
            Err("Fx graph parameter is not a closed static value".to_owned())
        }
    }
}

fn static_value_from_constant(
    value: &arcweft_lang_sema::final_analysis::CheckedFxConstant,
) -> Result<FxStaticValue, String> {
    match value {
        arcweft_lang_sema::final_analysis::CheckedFxConstant::Abi(value) => {
            static_value_from_argument(value)
        }
        arcweft_lang_sema::final_analysis::CheckedFxConstant::Selector(value) => {
            Ok(FxStaticValue::Selector(value.clone()))
        }
        arcweft_lang_sema::final_analysis::CheckedFxConstant::ShaderStage(value) => {
            Ok(FxStaticValue::ShaderStage(*value))
        }
        arcweft_lang_sema::final_analysis::CheckedFxConstant::FontFamily(value) => {
            Ok(FxStaticValue::FontFamily(value.clone()))
        }
        arcweft_lang_sema::final_analysis::CheckedFxConstant::Target(value) => {
            Ok(FxStaticValue::Target(*value))
        }
        arcweft_lang_sema::final_analysis::CheckedFxConstant::Phase(value) => {
            Ok(FxStaticValue::Phase(*value))
        }
    }
}

fn static_value_from_argument(value: &FxDefinitionArgumentValue) -> Result<FxStaticValue, String> {
    Ok(match value {
        FxDefinitionArgumentValue::Runtime(value) => FxStaticValue::Runtime(*value),
        FxDefinitionArgumentValue::Resource(value) => FxStaticValue::Resource(value.clone()),
        FxDefinitionArgumentValue::UniformRecord(value) => {
            FxStaticValue::UniformRecord(value.clone())
        }
    })
}

fn static_to_graph_argument(
    value: FxStaticValue,
    parameter: BuiltinFxParameterId,
) -> Result<BuiltinFxGraphArgument, String> {
    match value {
        FxStaticValue::Parameter(reference) => Ok(BuiltinFxGraphArgument::parameter(reference)),
        FxStaticValue::Runtime(value) => Ok(BuiltinFxGraphArgument::constant(
            FxDefinitionArgumentValue::Runtime(value),
        )),
        FxStaticValue::Resource(value) => Ok(BuiltinFxGraphArgument::constant(
            FxDefinitionArgumentValue::Resource(value),
        )),
        FxStaticValue::UniformRecord(value) => Ok(BuiltinFxGraphArgument::constant(
            FxDefinitionArgumentValue::UniformRecord(value),
        )),
        FxStaticValue::Sampler(_)
        | FxStaticValue::Selector(_)
        | FxStaticValue::ShaderStage(_)
        | FxStaticValue::FontFamily(_)
        | FxStaticValue::Target(_)
        | FxStaticValue::Phase(_) => Err(format!(
            "builtin ABI parameter `{parameter:?}` has a non-ABI static value"
        )),
    }
}
