//! Compilation of `#[fx] fn ... -> Fx` into renderer-independent graphs.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{flow::Stmt, pattern::Pattern},
        expr::{CallArg, Expr},
        types::FnParam,
    },
};
use arcweft_presentation::fx::{
    FxDefinition, FxGraph, FxId, FxNode, FxNodeKind, FxParameter, FxParameterSlot, FxProperty,
    FxRuntimeType, FxStaticType, FxStaticValue,
};

use crate::errors::RuntimePlanLowerError;

mod sampler;
mod value_lowering;

use sampler::lower_sampler;
use value_lowering::{lower_closed_runtime_value, lower_static_value, runtime_type};

/// Compiles all Fx graph factories in one linked HIR module.
pub fn lower_fx_definitions(
    module: &HirModule,
) -> Result<Vec<FxDefinition>, RuntimePlanLowerError> {
    lower_fx_definitions_for_package(module, "crate")
}

/// Compiles all Fx graph factories using the owning package identity.
pub fn lower_fx_definitions_for_package(
    module: &HirModule,
    package: &str,
) -> Result<Vec<FxDefinition>, RuntimePlanLowerError> {
    let functions = module
        .functions()
        .iter()
        .filter(|function| function.has_attribute("fx"))
        .map(|function| (function.name().to_owned(), function))
        .collect::<BTreeMap<_, _>>();
    let mut definitions = functions
        .values()
        .map(|function| compile_definition(function, package, &functions))
        .collect::<Result<Vec<_>, _>>()?;
    definitions.extend(crate::render_text::builtin_rich_text_fx_definitions(
        module,
    )?);
    definitions.sort_by(|left, right| left.id().cmp(right.id()));
    if let Some(pair) = definitions
        .windows(2)
        .find(|pair| pair[0].id() == pair[1].id())
    {
        return Err(RuntimePlanLowerError::new(format!(
            "duplicate Fx definition `{}`",
            pair[0].id()
        )));
    }
    Ok(definitions)
}

fn compile_definition(
    function: &HirFunction,
    package: &str,
    functions: &BTreeMap<String, &HirFunction>,
) -> Result<FxDefinition, RuntimePlanLowerError> {
    let parameter_declarations = function_params(function)?;
    let mut parameters = Vec::with_capacity(parameter_declarations.len());
    for (name, parameter) in &parameter_declarations {
        let authored_ty = parameter.ty().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Fx function `{}` parameter `{name}` requires a type",
                function.name()
            ))
        })?;
        let ty = runtime_type(authored_ty.value())?;
        let default = parameter
            .default()
            .map(|expr| lower_closed_runtime_value(expr, ty))
            .transpose()?;
        parameters.push(FxParameter::try_new(name, ty, default).map_err(|source| {
            RuntimePlanLowerError::new(format!(
                "invalid Fx parameter `{name}` in `{}`: {source}",
                function.name()
            ))
        })?);
    }
    let parameter_types = parameters
        .iter()
        .map(FxParameter::value_type)
        .collect::<Vec<_>>();
    let bindings = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            (
                parameter.name().to_owned(),
                FxStaticValue::Parameter(FxParameterSlot {
                    index: u16::try_from(index).unwrap_or(u16::MAX),
                    ty: parameter.value_type(),
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let graph = FxGraphCompiler::new(package, functions, &parameter_types)
        .compile_function_graph(function, &bindings)?;
    let id = FxId::try_new(package, function.qualified_name()).map_err(|error| {
        RuntimePlanLowerError::new(format!(
            "invalid Fx identity for `{}`: {error}",
            function.name()
        ))
    })?;
    FxDefinition::new(id, parameters, graph).map_err(|source| {
        RuntimePlanLowerError::new(format!(
            "invalid Fx definition `{}`: {source}",
            function.name()
        ))
    })
}

struct FxGraphCompiler<'a> {
    package: &'a str,
    functions: &'a BTreeMap<String, &'a HirFunction>,
    parameter_types: &'a [FxRuntimeType],
    active: BTreeSet<String>,
}

impl<'a> FxGraphCompiler<'a> {
    fn new(
        package: &'a str,
        functions: &'a BTreeMap<String, &'a HirFunction>,
        parameter_types: &'a [FxRuntimeType],
    ) -> Self {
        Self {
            package,
            functions,
            parameter_types,
            active: BTreeSet::new(),
        }
    }

    fn compile_function_graph(
        &mut self,
        function: &HirFunction,
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxGraph, RuntimePlanLowerError> {
        if !self.active.insert(function.name().to_owned()) {
            return Err(RuntimePlanLowerError::new(format!(
                "Fx composition cycle reaches `{}`",
                function.name()
            )));
        }
        let result = (|| {
            let expr = function_value(function).ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "Fx function `{}` does not return a graph value",
                    function.name()
                ))
            })?;
            let owner =
                FxId::try_new(self.package, function.qualified_name()).map_err(|source| {
                    RuntimePlanLowerError::new(format!(
                        "invalid Fx identity for `{}`: {source}",
                        function.name()
                    ))
                })?;
            self.compile_graph_expr(expr, &owner, bindings)
        })();
        self.active.remove(function.name());
        result
    }

    fn compile_graph_expr(
        &mut self,
        expr: &Expr,
        owner: &FxId,
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxGraph, RuntimePlanLowerError> {
        let Expr::Call(call) = expr else {
            return Err(RuntimePlanLowerError::new(
                "Fx graph value must be a constructor or Fx function call".to_owned(),
            ));
        };
        if let Some(member) = fx_constructor_member(call.callee()) {
            return self.compile_constructor(member, owner, call.args(), bindings);
        }
        let Some(name) = simple_path(call.callee()) else {
            return Err(RuntimePlanLowerError::new(
                "Fx graph calls must use a canonical function symbol".to_owned(),
            ));
        };
        let function = *self.functions.get(name).ok_or_else(|| {
            RuntimePlanLowerError::new(format!("Fx graph references unknown Fx function `{name}`"))
        })?;
        let child_bindings = bind_call(function, call.args(), bindings)?;
        self.compile_function_graph(function, &child_bindings)
    }

    fn compile_constructor(
        &mut self,
        member: &str,
        owner: &FxId,
        args: &[CallArg],
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxGraph, RuntimePlanLowerError> {
        let node = match member {
            "stack" => self.compile_stack(owner, args, bindings)?,
            "conditional" => self.compile_conditional(owner, args, bindings)?,
            "shader" => Self::compile_shader(owner, args, bindings)?,
            "style" => self.compile_property_node(FxNodeKind::Style, owner, args, bindings)?,
            "text" => self.compile_property_node(FxNodeKind::Text, owner, args, bindings)?,
            "color" => self.compile_property_node(FxNodeKind::Color, owner, args, bindings)?,
            "transform" => {
                self.compile_property_node(FxNodeKind::Transform, owner, args, bindings)?
            }
            "mask" => self.compile_property_node(FxNodeKind::Mask, owner, args, bindings)?,
            "filter" => self.compile_property_node(FxNodeKind::Filter, owner, args, bindings)?,
            "transition" => {
                self.compile_property_node(FxNodeKind::Transition, owner, args, bindings)?
            }
            other => {
                return Err(RuntimePlanLowerError::new(format!(
                    "unknown Fx constructor `Fx.{other}`"
                )));
            }
        };
        Ok(FxGraph::new(vec![node]))
    }

    fn compile_stack(
        &mut self,
        owner: &FxId,
        args: &[CallArg],
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxNode, RuntimePlanLowerError> {
        let [CallArg::Positional(value)] = args else {
            return Err(RuntimePlanLowerError::new(
                "Fx.stack requires one ordered graph list".to_owned(),
            ));
        };
        let Expr::BracketSeq(children) = value.as_ref() else {
            return Err(RuntimePlanLowerError::new(
                "Fx.stack requires one ordered graph list".to_owned(),
            ));
        };
        Ok(FxNode::Stack {
            children: children
                .iter()
                .map(|child| self.compile_graph_expr(child, owner, bindings))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn compile_conditional(
        &mut self,
        owner: &FxId,
        args: &[CallArg],
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxNode, RuntimePlanLowerError> {
        let condition = lower_static_value(
            named_expr(args, "condition")?,
            FxStaticType::Runtime(FxRuntimeType::Bool),
            bindings,
        )?;
        Ok(FxNode::Conditional {
            condition,
            then_graph: self.compile_graph_expr(named_expr(args, "then")?, owner, bindings)?,
            else_graph: self.compile_graph_expr(named_expr(args, "else")?, owner, bindings)?,
        })
    }

    fn compile_shader(
        owner: &FxId,
        args: &[CallArg],
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxNode, RuntimePlanLowerError> {
        let mut properties = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            match arg {
                CallArg::Positional(value) if index == 0 => properties.push(FxProperty::new(
                    "resource",
                    lower_static_value(value, FxStaticType::Resource, bindings)?,
                )),
                CallArg::Named { name, value } => {
                    let expected = FxNodeKind::Shader.property_type(name).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "Fx.shader has no property named `{name}`"
                        ))
                    })?;
                    properties.push(FxProperty::new(
                        name,
                        lower_static_value(value, expected, bindings)?,
                    ));
                }
                _ => {
                    return Err(RuntimePlanLowerError::new(
                        "Fx.shader accepts only its resource positionally".to_owned(),
                    ));
                }
            }
        }
        Ok(FxNode::Shader {
            fx: owner.clone(),
            properties,
        })
    }

    fn compile_property_node(
        &self,
        kind: FxNodeKind,
        owner: &FxId,
        args: &[CallArg],
        bindings: &BTreeMap<String, FxStaticValue>,
    ) -> Result<FxNode, RuntimePlanLowerError> {
        let properties = compile_properties(kind, args, bindings, self.parameter_types)?;
        Ok(match kind {
            FxNodeKind::Style => FxNode::Style { properties },
            FxNodeKind::Text => FxNode::Text { properties },
            FxNodeKind::Color => FxNode::Color { properties },
            FxNodeKind::Transform => FxNode::Transform {
                fx: owner.clone(),
                properties,
            },
            FxNodeKind::Mask => FxNode::Mask {
                fx: owner.clone(),
                properties,
            },
            FxNodeKind::Filter => FxNode::Filter {
                fx: owner.clone(),
                properties,
            },
            FxNodeKind::Transition => FxNode::Transition {
                fx: owner.clone(),
                properties,
            },
            FxNodeKind::Shader
            | FxNodeKind::OffscreenPass
            | FxNodeKind::PostProcess
            | FxNodeKind::Conditional
            | FxNodeKind::Stack => {
                return Err(RuntimePlanLowerError::new(format!(
                    "Fx.{} requires a dedicated constructor contract",
                    kind.as_str()
                )));
            }
        })
    }
}

fn bind_call(
    function: &HirFunction,
    args: &[CallArg],
    parent: &BTreeMap<String, FxStaticValue>,
) -> Result<BTreeMap<String, FxStaticValue>, RuntimePlanLowerError> {
    let mut supplied = BTreeMap::new();
    for arg in args {
        let CallArg::Named { name, value } = arg else {
            return Err(RuntimePlanLowerError::new(format!(
                "Fx function `{}` accepts named arguments only",
                function.name()
            )));
        };
        if supplied.insert(name.clone(), value.as_ref()).is_some() {
            return Err(RuntimePlanLowerError::new(format!(
                "Fx function `{}` receives duplicate argument `{name}`",
                function.name()
            )));
        }
    }
    let mut result = BTreeMap::new();
    for (name, param) in function_params(function)? {
        let authored_ty = param.ty().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Fx function `{}` parameter `{name}` requires a type",
                function.name()
            ))
        })?;
        let ty = runtime_type(authored_ty.value())?;
        let expr = supplied.remove(&name).map_or_else(
            || {
                param.default().map_or_else(
                    || {
                        Err(RuntimePlanLowerError::new(format!(
                            "Fx function `{}` is missing required argument `{name}`",
                            function.name()
                        )))
                    },
                    Ok,
                )
            },
            Ok,
        )?;
        let value = lower_static_value(expr, FxStaticType::Runtime(ty), parent)?;
        result.insert(name, value);
    }
    if let Some(unknown) = supplied.keys().next() {
        return Err(RuntimePlanLowerError::new(format!(
            "Fx function `{}` has no parameter named `{unknown}`",
            function.name()
        )));
    }
    Ok(result)
}

fn function_params(
    function: &HirFunction,
) -> Result<Vec<(String, &FnParam)>, RuntimePlanLowerError> {
    function
        .signature()
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_hir::syntax::types::FnParamGroup::params)
        .map(|param| match param.pattern() {
            Pattern::Ident(name) => Ok((name.to_owned(), param)),
            _ => Err(RuntimePlanLowerError::new(format!(
                "Fx function `{}` has a non-identifier parameter",
                function.name()
            ))),
        })
        .collect()
}

fn function_value(function: &HirFunction) -> Option<&Expr> {
    function
        .value()
        .map(arcweft_lang_hir::syntax::ast::flow::AuthoredExpr::expr)
        .or_else(|| {
            function
                .statements()
                .last()
                .and_then(|statement| match statement {
                    Stmt::Return { expr, .. } => Some(expr),
                    _ => None,
                })
        })
}

fn compile_properties(
    kind: FxNodeKind,
    args: &[CallArg],
    bindings: &BTreeMap<String, FxStaticValue>,
    parameter_types: &[FxRuntimeType],
) -> Result<Vec<FxProperty>, RuntimePlanLowerError> {
    args.iter()
        .map(|arg| match arg {
            CallArg::Named { name, value } => {
                let canonical_name = if kind == FxNodeKind::Transform && name == "sample" {
                    "sampler"
                } else {
                    name
                };
                let expected = kind.property_type(canonical_name).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Fx.{} has no property named `{name}`",
                        kind.as_str()
                    ))
                })?;
                let value = if matches!(expected, FxStaticType::Runtime(FxRuntimeType::Transform2D))
                    && matches!(value.as_ref(), Expr::Closure { .. })
                {
                    FxStaticValue::Sampler(lower_sampler(value, bindings, parameter_types)?)
                } else {
                    lower_static_value(value, expected, bindings)?
                };
                Ok(FxProperty::new(canonical_name, value))
            }
            _ => Err(RuntimePlanLowerError::new(
                "Fx constructor arguments must be named".to_owned(),
            )),
        })
        .collect()
}

fn named_expr<'a>(args: &'a [CallArg], target: &str) -> Result<&'a Expr, RuntimePlanLowerError> {
    args.iter()
        .find_map(|arg| match arg {
            CallArg::Named { name, value } if name == target => Some(value.as_ref()),
            _ => None,
        })
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!("Fx.conditional requires `{target} = ...`"))
        })
}

pub(crate) fn closed_expr_to_fx_value(
    expr: &Expr,
    expected: FxRuntimeType,
) -> Result<arcweft_presentation::fx::FxRuntimeValue, RuntimePlanLowerError> {
    lower_closed_runtime_value(expr, expected)
}

fn fx_constructor_member(expr: &Expr) -> Option<&str> {
    let Expr::Select(select) = expr else {
        return None;
    };
    matches!(select.target(), Expr::Path(path) if path.is_single("Fx"))
        .then(|| select.member().as_str())
}

fn simple_path(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    (path.segments().len() == 1).then(|| path.as_label())
}

#[cfg(test)]
mod tests {
    use super::lower_fx_definitions;
    use arcweft_lang_hir::lower::lower_document_to_hir;
    use arcweft_lang_syntax::parser::parse_source;
    use arcweft_presentation::fx::{
        FiniteF32, FxEvaluationBudget, FxNode, FxParameterSlot, FxRuntimeType, FxRuntimeValue,
        FxSampleContext, FxStaticValue, Length, Seconds, ValueProgramInputs,
    };

    #[test]
    fn compiles_defaults_and_nested_stack_into_one_graph() {
        let parsed = parse_source(
            r##"
#[fx]
fn emphasis(accent: Color = rgb("#ffd060")) -> Fx {
    Fx.text(weight = .strong, color = accent)
}
#[fx]
fn notice(accent: Color = rgb("#ff4050")) -> Fx {
    Fx.stack([emphasis(accent = accent)])
}
"##,
        );
        let hir = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree())
            .expect("Fx fixture lowers");
        let definitions = lower_fx_definitions(&hir).expect("Fx graph compiles");
        let notice = definitions
            .iter()
            .find(|definition| definition.id().function() == "notice")
            .expect("notice definition");
        let [FxNode::Stack { children }] = notice.graph().nodes() else {
            panic!("notice keeps ordered stack");
        };
        let [FxNode::Text { properties }] = children[0].nodes() else {
            panic!("nested text graph expands");
        };
        assert!(properties.iter().any(|property| {
            property.name() == "color"
                && property.value()
                    == &FxStaticValue::Parameter(FxParameterSlot {
                        index: 0,
                        ty: FxRuntimeType::Color,
                    })
        }));
    }

    #[test]
    fn compiles_and_executes_transform_sampler_with_typed_parameter_slots() {
        let parsed = parse_source(
            r"
#[fx]
fn wave(amplitude: Length = 2px, speed: f32 = 1.0) -> Fx {
    Fx.transform(
        target = .glyph,
        sample = |ctx| Transform2D {
            translate_y: sin(ctx.time * speed + ctx.ordinal_phase()) * amplitude,
        },
    )
}
",
        );
        let hir = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree())
            .expect("Fx sampler fixture lowers");
        let definitions = lower_fx_definitions(&hir).expect("Fx sampler compiles");
        let [FxNode::Transform { properties, .. }] = definitions[0].graph().nodes() else {
            panic!("wave compiles to a transform node");
        };
        let FxStaticValue::Sampler(program) = properties
            .iter()
            .find(|property| property.name() == "sampler")
            .expect("typed sampler property")
            .value()
        else {
            panic!("sampler property owns executable IR");
        };
        let parameters = [
            FxRuntimeValue::Length(Length::try_pixels(2.0).expect("finite length")),
            FxRuntimeValue::F32(FiniteF32::try_new(1.0).expect("finite speed")),
        ];
        let context = FxSampleContext::from_elapsed(
            Seconds::try_seconds(0.5).expect("finite time"),
            1,
            7,
            false,
        );
        let mut budget = FxEvaluationBudget::new(128);
        let output = program
            .evaluate(
                ValueProgramInputs {
                    parameters: &parameters,
                    state: &[],
                },
                context,
                &mut budget,
            )
            .expect("shared evaluator executes compiled sampler");
        let FxRuntimeValue::Transform2D(transform) = output else {
            panic!("sampler returns Transform2D");
        };
        assert!(transform.translate_y.pixels().is_finite());
        assert!(transform.translate_y.pixels().abs() > f32::EPSILON);
    }
}
