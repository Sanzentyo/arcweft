//! Expansion of compiled Fx graphs inside `RichText` spans.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    fx::FxConst,
    model::HirModule,
    syntax::{
        ast::{common::TextRange, dialogue::DialogueTag},
        expr::{CallArg, Expr, parse_expr},
    },
};
use arcweft_presentation::fx::{FxDefinition, FxGraph, FxNode, FxProperty, FxValue};
use arcweft_render_text::RichTextStyle;

use crate::{
    errors::RuntimePlanLowerError,
    fx::{closed_expr_to_fx_value, lower_fx_definitions},
};

use super::tag::lower_visual_fx_layer;

mod contributions;
mod expander;

#[cfg(test)]
mod tests;

pub(crate) use contributions::{FxInlineAssignment, append_fx_inline_contributions};
pub(crate) use expander::DialogueFxExpander;

/// Module-scoped compiled Fx definitions used by dialogue lowering.
#[derive(Clone, Debug, Default)]
pub(crate) struct FxCatalog {
    definitions: BTreeMap<String, FxDefinition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FxLayerKind {
    Em,
    Strong,
    Color,
    Font,
    Size,
    Style,
    Transform,
    Effect,
    Shader,
}

#[derive(Clone, Debug)]
struct ExpandedFxLayer {
    style: RichTextStyle,
    kind: FxLayerKind,
    selector: Option<String>,
    attrs: String,
    arguments: Vec<ExpandedFxArgument>,
}

#[derive(Clone, Debug)]
struct ExpandedFxArgument {
    name: String,
    value: String,
    invocation_range: Option<TextRange>,
}

impl FxCatalog {
    pub(crate) fn try_from_module(module: &HirModule) -> Result<Self, RuntimePlanLowerError> {
        let mut definitions = BTreeMap::new();
        for definition in lower_fx_definitions(module)? {
            let name = definition
                .id()
                .function()
                .rsplit('.')
                .next()
                .unwrap_or(definition.id().function())
                .to_owned();
            if definitions.insert(name.clone(), definition).is_some() {
                return Err(fx_error(format!("duplicate Fx function `{name}`")));
            }
        }
        Ok(Self { definitions })
    }

    fn expand_tag(
        &self,
        tag: &DialogueTag,
    ) -> Result<(String, Vec<ExpandedFxLayer>), RuntimePlanLowerError> {
        let expr = parse_expr(tag.attrs().trim())
            .map_err(|error| fx_error(format!("invalid `[fx]` invocation: {error}")))?;
        let Expr::Call { callee, args } = expr else {
            return Err(fx_error("`[fx]` requires one Fx function call"));
        };
        let name = callee_name(&callee)
            .ok_or_else(|| fx_error("`[fx]` target must be a canonical function path"))?;
        let definition = self
            .definitions
            .get(name)
            .ok_or_else(|| fx_error(format!("unknown Fx function `{name}`")))?;
        let bindings = bind_invocation(definition, &args)?;
        let graph = instantiate_graph(definition.graph(), &bindings)?;
        let layers = lower_graph(name, &graph, tag.attrs_range())?;
        Ok((name.to_owned(), layers))
    }
}

fn bind_invocation(
    definition: &FxDefinition,
    args: &[CallArg],
) -> Result<BTreeMap<String, FxValue>, RuntimePlanLowerError> {
    let mut supplied = BTreeMap::new();
    for arg in args {
        let CallArg::Named { name, value } = arg else {
            return Err(fx_error(format!(
                "Fx function `{}` accepts named arguments only",
                definition.id().function()
            )));
        };
        if !rich_text_value_is_closed(value) {
            return Err(fx_error(format!(
                "RichText Fx argument `{name}` must be closed, found `{}`",
                crate::labels::expr_label(value)
            )));
        }
        if supplied
            .insert(name.clone(), closed_expr_to_fx_value(value))
            .is_some()
        {
            return Err(fx_error(format!(
                "Fx function `{}` receives duplicate argument `{name}`",
                definition.id().function()
            )));
        }
    }
    let mut bindings = BTreeMap::new();
    for parameter in definition.parameters() {
        let value = supplied.remove(parameter.name()).map_or_else(
            || {
                parameter.default().cloned().ok_or_else(|| {
                    fx_error(format!(
                        "Fx function `{}` is missing required argument `{}`",
                        definition.id().function(),
                        parameter.name()
                    ))
                })
            },
            Ok,
        )?;
        bindings.insert(parameter.name().to_owned(), value);
    }
    if let Some(unknown) = supplied.keys().next() {
        return Err(fx_error(format!(
            "Fx function `{}` has no parameter named `{unknown}`",
            definition.id().function()
        )));
    }
    Ok(bindings)
}

fn instantiate_graph(
    graph: &FxGraph,
    bindings: &BTreeMap<String, FxValue>,
) -> Result<FxGraph, RuntimePlanLowerError> {
    Ok(FxGraph::new(
        graph
            .nodes()
            .iter()
            .map(|node| instantiate_node(node, bindings))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn instantiate_node(
    node: &FxNode,
    bindings: &BTreeMap<String, FxValue>,
) -> Result<FxNode, RuntimePlanLowerError> {
    let properties = |properties: &[FxProperty]| {
        properties
            .iter()
            .map(|property| {
                Ok(FxProperty::new(
                    property.name(),
                    instantiate_value(property.value(), bindings)?,
                ))
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
    };
    Ok(match node {
        FxNode::Style(values) => FxNode::Style(properties(values)?),
        FxNode::Text(values) => FxNode::Text(properties(values)?),
        FxNode::Color(values) => FxNode::Color(properties(values)?),
        FxNode::Transform {
            fx,
            properties: values,
        } => FxNode::Transform {
            fx: fx.clone(),
            properties: properties(values)?,
        },
        FxNode::Mask {
            fx,
            properties: values,
        } => FxNode::Mask {
            fx: fx.clone(),
            properties: properties(values)?,
        },
        FxNode::Filter {
            fx,
            properties: values,
        } => FxNode::Filter {
            fx: fx.clone(),
            properties: properties(values)?,
        },
        FxNode::Shader {
            fx,
            properties: values,
        } => FxNode::Shader {
            fx: fx.clone(),
            properties: properties(values)?,
        },
        FxNode::Transition {
            fx,
            properties: values,
        } => FxNode::Transition {
            fx: fx.clone(),
            properties: properties(values)?,
        },
        FxNode::Conditional {
            condition,
            then_graph,
            else_graph,
        } => FxNode::Conditional {
            condition: instantiate_value(condition, bindings)?,
            then_graph: instantiate_graph(then_graph, bindings)?,
            else_graph: instantiate_graph(else_graph, bindings)?,
        },
        FxNode::Stack(children) => FxNode::Stack(
            children
                .iter()
                .map(|child| instantiate_graph(child, bindings))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    })
}

fn instantiate_value(
    value: &FxValue,
    bindings: &BTreeMap<String, FxValue>,
) -> Result<FxValue, RuntimePlanLowerError> {
    Ok(match value {
        FxValue::Parameter(name) => bindings
            .get(name)
            .cloned()
            .ok_or_else(|| fx_error(format!("unbound Fx parameter `{name}`")))?,
        FxValue::List(values) => FxValue::List(
            values
                .iter()
                .map(|value| instantiate_value(value, bindings))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        FxValue::Record(properties) => FxValue::Record(
            properties
                .iter()
                .map(|property| {
                    Ok(FxProperty::new(
                        property.name(),
                        instantiate_value(property.value(), bindings)?,
                    ))
                })
                .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?,
        ),
        other => other.clone(),
    })
}

fn lower_graph(
    owner: &str,
    graph: &FxGraph,
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    graph
        .nodes()
        .iter()
        .try_fold(Vec::new(), |mut layers, node| {
            layers.extend(lower_node(owner, node, invocation_range)?);
            Ok(layers)
        })
}

fn lower_node(
    owner: &str,
    node: &FxNode,
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    match node {
        FxNode::Text(properties) => lower_text(properties, invocation_range),
        FxNode::Color(properties) => lower_color(properties, invocation_range),
        FxNode::Style(properties) => lower_style(properties, invocation_range),
        FxNode::Transform { fx, properties } => {
            lower_effect_node(properties, fx, FxLayerKind::Transform, invocation_range)
        }
        FxNode::Mask { fx, properties }
        | FxNode::Filter { fx, properties }
        | FxNode::Transition { fx, properties } => {
            lower_effect_node(properties, fx, FxLayerKind::Effect, invocation_range)
        }
        FxNode::Shader { properties, .. } => lower_shader(properties, invocation_range),
        FxNode::Conditional {
            condition,
            then_graph,
            else_graph,
        } => match condition {
            FxValue::Bool(true) => lower_graph(owner, then_graph, invocation_range),
            FxValue::Bool(false) => lower_graph(owner, else_graph, invocation_range),
            _ => Err(fx_error(
                "RichText Fx conditions must resolve to a closed boolean",
            )),
        },
        FxNode::Stack(children) => children.iter().try_fold(Vec::new(), |mut layers, child| {
            layers.extend(lower_graph(owner, child, invocation_range)?);
            Ok(layers)
        }),
    }
}

fn lower_text(
    properties: &[FxProperty],
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    let mut layers = Vec::new();
    for property in properties {
        let value = value_label(property.value());
        let (builder, kind, attrs) = match property.name() {
            "weight" if value.trim_start_matches('.') == "strong" => {
                ("strong", FxLayerKind::Strong, String::new())
            }
            "style" if value.trim_start_matches('.') == "em" => {
                ("em", FxLayerKind::Em, String::new())
            }
            "color" => ("color", FxLayerKind::Color, format!("value={value}")),
            "font" => ("font", FxLayerKind::Font, format!("value={value}")),
            "size" => ("size", FxLayerKind::Size, format!("value={value}")),
            other => {
                return Err(fx_error(format!(
                    "Fx.text property `{other}` is not supported by RichText"
                )));
            }
        };
        let style = lower_visual_fx_layer(builder, None, &attrs).map_err(fx_error)?;
        layers.push(ExpandedFxLayer {
            style,
            kind,
            selector: None,
            attrs,
            arguments: vec![ExpandedFxArgument {
                name: property.name().to_owned(),
                value,
                invocation_range: Some(invocation_range),
            }],
        });
    }
    Ok(layers)
}

fn lower_color(
    properties: &[FxProperty],
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    let property = properties
        .iter()
        .find(|property| matches!(property.name(), "value" | "color"))
        .ok_or_else(|| fx_error("Fx.color requires `value = ...`"))?;
    lower_text(
        &[FxProperty::new("color", property.value().clone())],
        invocation_range,
    )
}

fn lower_style(
    properties: &[FxProperty],
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    if let Some(property) = properties
        .iter()
        .find(|property| matches!(property.name(), "italic" | "oblique" | "opacity"))
    {
        let selector = property.name();
        let attrs = format!("value={}", value_label(property.value()));
        let style = lower_visual_fx_layer("style", Some(selector), &attrs).map_err(fx_error)?;
        return Ok(vec![ExpandedFxLayer {
            style,
            kind: FxLayerKind::Style,
            selector: Some(selector.to_owned()),
            attrs,
            arguments: properties_to_arguments(properties, invocation_range),
        }]);
    }
    Err(fx_error(
        "Fx.style properties that are not text style properties are View-only",
    ))
}

fn lower_effect_node(
    properties: &[FxProperty],
    fx: &arcweft_presentation::fx::FxId,
    kind: FxLayerKind,
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    // The legacy renderer leaf still serializes its selector as text. Encode the
    // complete typed identity so an Fx function named `wave` cannot accidentally
    // execute the unrelated built-in `.wave` effect by basename collision.
    let selector = fx.to_string();
    let attrs = properties
        .iter()
        .filter(|property| property.name() != "sample")
        .map(|property| format!("{}={}", property.name(), value_label(property.value())))
        .collect::<Vec<_>>()
        .join(" ");
    let style = lower_visual_fx_layer("effect", Some(&selector), &attrs).map_err(fx_error)?;
    Ok(vec![ExpandedFxLayer {
        style,
        kind,
        selector: Some(selector),
        attrs,
        arguments: properties_to_arguments(properties, invocation_range),
    }])
}

fn lower_shader(
    properties: &[FxProperty],
    invocation_range: TextRange,
) -> Result<Vec<ExpandedFxLayer>, RuntimePlanLowerError> {
    let attrs = properties
        .iter()
        .map(|property| {
            let name = if property.name() == "resource" {
                "id"
            } else {
                property.name()
            };
            format!("{name}={}", value_label(property.value()))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let style = lower_visual_fx_layer("effect", Some("shader"), &attrs).map_err(fx_error)?;
    Ok(vec![ExpandedFxLayer {
        style,
        kind: FxLayerKind::Shader,
        selector: Some("shader".to_owned()),
        attrs,
        arguments: properties_to_arguments(properties, invocation_range),
    }])
}

fn properties_to_arguments(
    properties: &[FxProperty],
    invocation_range: TextRange,
) -> Vec<ExpandedFxArgument> {
    properties
        .iter()
        .filter(|property| property.name() != "sample")
        .map(|property| ExpandedFxArgument {
            name: property.name().to_owned(),
            value: value_label(property.value()),
            invocation_range: Some(invocation_range),
        })
        .collect()
}

fn value_label(value: &FxValue) -> String {
    match value {
        FxValue::Bool(value) => value.to_string(),
        FxValue::Integer(value) | FxValue::Decimal(value) | FxValue::Binding(value) => {
            value.clone()
        }
        FxValue::String(value) => value.clone(),
        FxValue::Scalar { value, unit } | FxValue::Duration { value, unit } => {
            format!("{value}{unit}")
        }
        FxValue::Selector(value) => format!(".{value}"),
        FxValue::Parameter(value) => format!("${value}"),
        FxValue::List(values) => format!(
            "[{}]",
            values.iter().map(value_label).collect::<Vec<_>>().join(",")
        ),
        FxValue::Record(properties) => format!(
            "{{{}}}",
            properties
                .iter()
                .map(|property| format!("{}={}", property.name(), value_label(property.value())))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn callee_name(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    path.segments()
        .last()
        .map(arcweft_lang_hir::syntax::expr::Name::as_str)
}

fn rich_text_value_is_closed(expr: &Expr) -> bool {
    if FxConst::from_expr(expr).is_some() {
        return true;
    }
    matches!(
        expr,
        Expr::Call { callee, args }
            if matches!(callee_name(callee), Some("rgb" | "vec2" | "vec3" | "vec4"))
                && args.iter().all(|arg| {
                    !matches!(arg, CallArg::Spread { .. })
                        && rich_text_value_is_closed(arg.value())
                })
    )
}

fn fx_error(message: impl Into<String>) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!("rich-text Fx: {}", message.into()))
}
