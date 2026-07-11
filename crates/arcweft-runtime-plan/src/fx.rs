//! Compilation of `#[fx] fn ... -> Fx` into renderer-independent graphs.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{flow::Stmt, pattern::Pattern},
        expr::{CallArg, Expr, Literal},
        types::{FnParam, TypeRef},
    },
};
use arcweft_presentation::fx::{
    FxAbiHash, FxDefinition, FxGraph, FxId, FxNode, FxParameter, FxProperty, FxSemanticHash,
    FxValue,
};

use crate::{errors::RuntimePlanLowerError, labels::expr_label};

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
    functions
        .values()
        .map(|function| compile_definition(function, package, &functions))
        .collect()
}

fn compile_definition(
    function: &HirFunction,
    package: &str,
    functions: &BTreeMap<String, &HirFunction>,
) -> Result<FxDefinition, RuntimePlanLowerError> {
    let parameters = function_params(function)?
        .into_iter()
        .map(|(name, param)| {
            let default = param
                .default()
                .map(|expr| expr_to_value(expr, &BTreeMap::new()));
            FxParameter::new(name, type_ref_label(param.ty()), default)
        })
        .collect::<Vec<_>>();
    let bindings = parameters
        .iter()
        .map(|parameter| {
            (
                parameter.name().to_owned(),
                FxValue::Parameter(parameter.name().to_owned()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let graph = compile_function_graph(
        function,
        package,
        functions,
        &bindings,
        &mut BTreeSet::new(),
    )?;
    let id = FxId::try_new(package, function.qualified_name()).map_err(|error| {
        RuntimePlanLowerError::new(format!(
            "invalid Fx identity for `{}`: {error}",
            function.name()
        ))
    })?;
    let abi_hash = FxAbiHash::for_definition(&parameters, &graph);
    let semantic_hash = FxSemanticHash::for_graph(&graph);
    Ok(FxDefinition::new(
        id,
        parameters,
        graph,
        abi_hash,
        semantic_hash,
    ))
}

fn compile_function_graph(
    function: &HirFunction,
    package: &str,
    functions: &BTreeMap<String, &HirFunction>,
    bindings: &BTreeMap<String, FxValue>,
    active: &mut BTreeSet<String>,
) -> Result<FxGraph, RuntimePlanLowerError> {
    if !active.insert(function.name().to_owned()) {
        return Err(RuntimePlanLowerError::new(format!(
            "Fx composition cycle reaches `{}`",
            function.name()
        )));
    }
    let expr = function_value(function).ok_or_else(|| {
        RuntimePlanLowerError::new(format!(
            "Fx function `{}` does not return a graph value",
            function.name()
        ))
    })?;
    let owner = FxId::try_new(package, function.qualified_name()).map_err(|error| {
        RuntimePlanLowerError::new(format!(
            "invalid Fx identity for `{}`: {error}",
            function.name()
        ))
    })?;
    let graph = compile_graph_expr(expr, &owner, package, functions, bindings, active);
    active.remove(function.name());
    graph
}

fn compile_graph_expr(
    expr: &Expr,
    owner: &FxId,
    package: &str,
    functions: &BTreeMap<String, &HirFunction>,
    bindings: &BTreeMap<String, FxValue>,
    active: &mut BTreeSet<String>,
) -> Result<FxGraph, RuntimePlanLowerError> {
    let Expr::Call { callee, args } = expr else {
        return Err(RuntimePlanLowerError::new(
            "Fx graph value must be a constructor or Fx function call".to_owned(),
        ));
    };
    if let Some(member) = fx_constructor_member(callee) {
        return compile_constructor(member, owner, package, args, functions, bindings, active);
    }
    let Some(name) = simple_path(callee) else {
        return Err(RuntimePlanLowerError::new(
            "Fx graph calls must use a canonical function symbol".to_owned(),
        ));
    };
    let function = functions.get(name).ok_or_else(|| {
        RuntimePlanLowerError::new(format!("Fx graph references unknown Fx function `{name}`"))
    })?;
    let child_bindings = bind_call(function, args, bindings)?;
    compile_function_graph(function, package, functions, &child_bindings, active)
}

fn compile_constructor(
    member: &str,
    owner: &FxId,
    package: &str,
    args: &[CallArg],
    functions: &BTreeMap<String, &HirFunction>,
    bindings: &BTreeMap<String, FxValue>,
    active: &mut BTreeSet<String>,
) -> Result<FxGraph, RuntimePlanLowerError> {
    let node = match member {
        "stack" => {
            let [CallArg::Positional(Expr::BracketSeq(children))] = args else {
                return Err(RuntimePlanLowerError::new(
                    "Fx.stack requires one ordered graph list".to_owned(),
                ));
            };
            FxNode::Stack(
                children
                    .iter()
                    .map(|child| {
                        compile_graph_expr(child, owner, package, functions, bindings, active)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
        "conditional" => {
            let properties = named_properties(args, bindings)?;
            let condition = property_value(&properties, "condition")?;
            let then_expr = named_expr(args, "then")?;
            let else_expr = named_expr(args, "else")?;
            FxNode::Conditional {
                condition,
                then_graph: compile_graph_expr(
                    then_expr, owner, package, functions, bindings, active,
                )?,
                else_graph: compile_graph_expr(
                    else_expr, owner, package, functions, bindings, active,
                )?,
            }
        }
        "shader" => {
            let mut properties = Vec::new();
            for (index, arg) in args.iter().enumerate() {
                match arg {
                    CallArg::Positional(value) if index == 0 => {
                        properties
                            .push(FxProperty::new("resource", expr_to_value(value, bindings)));
                    }
                    CallArg::Named { name, value } => {
                        properties.push(FxProperty::new(name, expr_to_value(value, bindings)));
                    }
                    _ => {
                        return Err(RuntimePlanLowerError::new(
                            "Fx.shader accepts only its resource positionally".to_owned(),
                        ));
                    }
                }
            }
            append_bound_parameters(&mut properties, bindings);
            FxNode::Shader {
                fx: owner.clone(),
                properties,
            }
        }
        "style" => FxNode::Style(named_properties(args, bindings)?),
        "text" => FxNode::Text(named_properties(args, bindings)?),
        "color" => FxNode::Color(named_properties(args, bindings)?),
        "transform" => FxNode::Transform {
            fx: owner.clone(),
            properties: bound_properties(args, bindings)?,
        },
        "mask" => FxNode::Mask {
            fx: owner.clone(),
            properties: bound_properties(args, bindings)?,
        },
        "filter" => FxNode::Filter {
            fx: owner.clone(),
            properties: bound_properties(args, bindings)?,
        },
        "transition" => FxNode::Transition {
            fx: owner.clone(),
            properties: bound_properties(args, bindings)?,
        },
        other => {
            return Err(RuntimePlanLowerError::new(format!(
                "unknown Fx constructor `Fx.{other}`"
            )));
        }
    };
    Ok(FxGraph::new(vec![node]))
}

fn bind_call(
    function: &HirFunction,
    args: &[CallArg],
    parent: &BTreeMap<String, FxValue>,
) -> Result<BTreeMap<String, FxValue>, RuntimePlanLowerError> {
    let mut supplied = BTreeMap::new();
    for arg in args {
        let CallArg::Named { name, value } = arg else {
            return Err(RuntimePlanLowerError::new(format!(
                "Fx function `{}` accepts named arguments only",
                function.name()
            )));
        };
        if supplied
            .insert(name.clone(), expr_to_value(value, parent))
            .is_some()
        {
            return Err(RuntimePlanLowerError::new(format!(
                "Fx function `{}` receives duplicate argument `{name}`",
                function.name()
            )));
        }
    }
    let mut result = BTreeMap::new();
    for (name, param) in function_params(function)? {
        let value = supplied.remove(&name).map_or_else(
            || {
                param.default().map_or_else(
                    || {
                        Err(RuntimePlanLowerError::new(format!(
                            "Fx function `{}` is missing required argument `{name}`",
                            function.name()
                        )))
                    },
                    |default| Ok(expr_to_value(default, parent)),
                )
            },
            Ok,
        )?;
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

fn named_properties(
    args: &[CallArg],
    bindings: &BTreeMap<String, FxValue>,
) -> Result<Vec<FxProperty>, RuntimePlanLowerError> {
    args.iter()
        .map(|arg| match arg {
            CallArg::Named { name, value } => {
                Ok(FxProperty::new(name, expr_to_value(value, bindings)))
            }
            _ => Err(RuntimePlanLowerError::new(
                "Fx constructor arguments must be named".to_owned(),
            )),
        })
        .collect()
}

fn bound_properties(
    args: &[CallArg],
    bindings: &BTreeMap<String, FxValue>,
) -> Result<Vec<FxProperty>, RuntimePlanLowerError> {
    let mut properties = named_properties(args, bindings)?;
    append_bound_parameters(&mut properties, bindings);
    Ok(properties)
}

fn append_bound_parameters(properties: &mut Vec<FxProperty>, bindings: &BTreeMap<String, FxValue>) {
    for (name, value) in bindings {
        if !properties.iter().any(|property| property.name() == name) {
            properties.push(FxProperty::new(name, value.clone()));
        }
    }
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

fn property_value(
    properties: &[FxProperty],
    target: &str,
) -> Result<FxValue, RuntimePlanLowerError> {
    properties
        .iter()
        .find(|property| property.name() == target)
        .map(|property| property.value().clone())
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!("Fx.conditional requires `{target} = ...`"))
        })
}

fn expr_to_value(expr: &Expr, bindings: &BTreeMap<String, FxValue>) -> FxValue {
    match expr {
        Expr::Literal(Literal::Bool(value)) => FxValue::Bool(*value),
        Expr::Literal(Literal::String(value)) => FxValue::String(value.clone()),
        Expr::Literal(Literal::Int(value)) => FxValue::Integer(value.raw().to_owned()),
        Expr::Literal(Literal::Float { raw, .. }) => FxValue::Decimal(raw.clone()),
        Expr::Literal(Literal::UnitNumber { raw, suffix }) => FxValue::Scalar {
            value: raw.strip_suffix(suffix.as_str()).unwrap_or(raw).to_owned(),
            unit: suffix.as_str().to_owned(),
        },
        Expr::Literal(Literal::Duration { amount, unit }) => FxValue::Duration {
            value: amount.clone(),
            unit: unit.as_str().to_owned(),
        },
        Expr::ShortVariant(selector) => FxValue::Selector(selector.as_str().to_owned()),
        Expr::Path(path) if path.segments().len() == 1 => bindings
            .get(path.as_label())
            .cloned()
            .unwrap_or_else(|| FxValue::Binding(path.as_label().to_owned())),
        Expr::BracketSeq(values) => FxValue::List(
            values
                .iter()
                .map(|value| expr_to_value(value, bindings))
                .collect(),
        ),
        Expr::NumericBracketSeq(values) => FxValue::List(
            values
                .literals()
                .iter()
                .map(|value| FxValue::Integer(value.raw().to_owned()))
                .collect(),
        ),
        Expr::RecordLiteral(fields) => FxValue::Record(
            fields
                .iter()
                .map(|(name, value)| FxProperty::new(name, expr_to_value(value, bindings)))
                .collect(),
        ),
        Expr::Call { callee, args } if simple_path(callee) == Some("rgb") => {
            args.first().map_or_else(
                || FxValue::Binding(expr_label(expr)),
                |arg| expr_to_value(arg.value(), bindings),
            )
        }
        _ => FxValue::Binding(expr_label(expr)),
    }
}

pub(crate) fn closed_expr_to_fx_value(expr: &Expr) -> FxValue {
    expr_to_value(expr, &BTreeMap::new())
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

fn type_ref_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.clone(),
        TypeRef::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(type_ref_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter()
                .map(type_ref_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::lower_fx_definitions;
    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_syntax::parser::parse_source;
    use arcweft_presentation::fx::{FxNode, FxValue};

    #[test]
    fn compiles_defaults_and_nested_stack_into_one_graph() {
        let tree = parse_source(
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
        )
        .into_typed_tree();
        let hir = lower_to_hir(&tree).expect("Fx fixture lowers");
        let definitions = lower_fx_definitions(&hir).expect("Fx graph compiles");
        let notice = definitions
            .iter()
            .find(|definition| definition.id().function() == "notice")
            .expect("notice definition");
        let [FxNode::Stack(children)] = notice.graph().nodes() else {
            panic!("notice keeps ordered stack");
        };
        let [FxNode::Text(properties)] = children[0].nodes() else {
            panic!("nested text graph expands");
        };
        assert!(properties.iter().any(|property| {
            property.name() == "color"
                && property.value() == &FxValue::Parameter("accent".to_owned())
        }));
    }
}
