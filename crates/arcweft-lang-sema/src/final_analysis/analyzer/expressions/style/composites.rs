//! Typed construction grammar for native composite Style values.

use super::{
    Analyzer, AnalyzerExpressionContext, AnalyzerExpressionError, CheckedExpression,
    CheckedExpressionResolution, CheckedStyleOperand, CompileTimeStyleType, ExprId,
    HirCallArgument, HirCallCallee, HirExpr, HirExprKind, HirLiteral, HirModule, HirStringLiteral,
    StyleCallableId, TypeKind, ViewSpecifiedValue, ViewStyleValueKind,
};
use crate::final_analysis::CheckedStyleDuration;
use arcweft_lang_hir::expr::HirCallInvocation;
use arcweft_view::style::{
    ViewBorderRadii, ViewClip, ViewFilter, ViewLengthMilli, ViewMask, ViewPropertyKind, ViewShadow,
    ViewStyleTransition,
};

#[derive(Clone, Copy)]
enum DefaultArgument {
    ZeroLength,
    ZeroDuration,
}

struct Parameter {
    name: &'static str,
    ty: CompileTimeStyleType,
    default: Option<DefaultArgument>,
}

impl Parameter {
    const fn required(name: &'static str, ty: CompileTimeStyleType) -> Self {
        Self {
            name,
            ty,
            default: None,
        }
    }

    const fn optional(
        name: &'static str,
        ty: CompileTimeStyleType,
        default: DefaultArgument,
    ) -> Self {
        Self {
            name,
            ty,
            default: Some(default),
        }
    }
}

struct Signature {
    result: CompileTimeStyleType,
    parameters: &'static [Parameter],
}

impl Signature {
    fn for_callable(callable: StyleCallableId) -> Option<Self> {
        use CompileTimeStyleType::{
            Duration, Filter, Property, ResourceId, Shadow, Transition, Value,
        };
        use ViewStyleValueKind::{BorderRadii, Clip, Color, Length, Mask, Ratio, Resource, Scalar};
        Some(match callable {
            StyleCallableId::Radii => Self {
                result: Value(BorderRadii),
                parameters: const {
                    &[
                        Parameter::required("top_left", Value(Length)),
                        Parameter::required("top_right", Value(Length)),
                        Parameter::required("bottom_right", Value(Length)),
                        Parameter::required("bottom_left", Value(Length)),
                    ]
                },
            },
            StyleCallableId::Shadow | StyleCallableId::InsetShadow => Self {
                result: Shadow,
                parameters: const {
                    &[
                        Parameter::optional("x", Value(Length), DefaultArgument::ZeroLength),
                        Parameter::optional("y", Value(Length), DefaultArgument::ZeroLength),
                        Parameter::optional("blur", Value(Length), DefaultArgument::ZeroLength),
                        Parameter::optional("spread", Value(Length), DefaultArgument::ZeroLength),
                        Parameter::required("color", Value(Color)),
                    ]
                },
            },
            StyleCallableId::Blur => Self {
                result: Filter,
                parameters: const { &[Parameter::required("radius", Value(Length))] },
            },
            StyleCallableId::Brightness | StyleCallableId::Contrast => Self {
                result: Filter,
                parameters: const { &[Parameter::required("amount", Value(Scalar))] },
            },
            StyleCallableId::Opacity => Self {
                result: Filter,
                parameters: const { &[Parameter::required("amount", Value(Ratio))] },
            },
            StyleCallableId::RoundedRect => Self {
                result: Value(Clip),
                parameters: const { &[Parameter::required("radii", Value(BorderRadii))] },
            },
            StyleCallableId::Mask => Self {
                result: Value(Mask),
                parameters: const { &[Parameter::required("resource", Value(Resource))] },
            },
            StyleCallableId::Transition => Self {
                result: Transition,
                parameters: const {
                    &[
                        Parameter::required("property", Property),
                        Parameter::required("duration", Duration),
                        Parameter::optional("delay", Duration, DefaultArgument::ZeroDuration),
                    ]
                },
            },
            StyleCallableId::Resource => Self {
                result: Value(Resource),
                parameters: const { &[Parameter::required("id", ResourceId)] },
            },
            StyleCallableId::Rgba
            | StyleCallableId::Token
            | StyleCallableId::SystemColor
            | StyleCallableId::SystemFont => return None,
        })
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn check_style_composite(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        expression: &HirExpr,
        expected: CompileTimeStyleType,
    ) -> Result<Option<CheckedExpression>, AnalyzerExpressionError> {
        if let Some(operand) = Self::style_composite_leaf(owner, expression, expected)? {
            return Ok(Some(Self::checked_style_operand(operand)));
        }
        if let (CompileTimeStyleType::Value(kind), HirExprKind::BracketSequence(sequence)) =
            (expected, expression.kind())
        {
            let element_type = match kind {
                ViewStyleValueKind::ShadowList => CompileTimeStyleType::Shadow,
                ViewStyleValueKind::FilterList => CompileTimeStyleType::Filter,
                ViewStyleValueKind::Transition => CompileTimeStyleType::Transition,
                _ => return Ok(None),
            };
            let mut shadows = Vec::new();
            let mut filters = Vec::new();
            let mut transitions = Vec::new();
            for child in sequence.elements() {
                let checked = self.evaluate_expression(
                    context,
                    *child,
                    Some(&TypeKind::CompileTimeStyle(element_type)),
                )?;
                if !checked.effects().is_empty() {
                    return Err(AnalyzerExpressionError::rejected(*child));
                }
                match checked.checked_resolution() {
                    Some(CheckedExpressionResolution::StyleOperand(
                        CheckedStyleOperand::Shadow(value),
                    )) => shadows.push(*value),
                    Some(CheckedExpressionResolution::StyleOperand(
                        CheckedStyleOperand::Filter(value),
                    )) => filters.push(*value),
                    Some(CheckedExpressionResolution::StyleOperand(
                        CheckedStyleOperand::Transition(value),
                    )) => transitions.push(*value),
                    _ => return Err(AnalyzerExpressionError::rejected(*child)),
                }
            }
            let value = match kind {
                ViewStyleValueKind::ShadowList => ViewSpecifiedValue::ShadowList { value: shadows },
                ViewStyleValueKind::FilterList => ViewSpecifiedValue::FilterList { value: filters },
                ViewStyleValueKind::Transition => {
                    ViewSpecifiedValue::Transition { value: transitions }
                }
                _ => return Err(AnalyzerExpressionError::rejected(owner)),
            };
            return Ok(Some(Self::checked_style_value(value)));
        }
        let HirExprKind::Call(call) = expression.kind() else {
            return Ok(None);
        };
        let HirCallCallee::Value { value: callee } = call.callee() else {
            return Ok(None);
        };
        let Some(name) = Self::direct_callee_name(module, *callee)? else {
            return Ok(None);
        };
        let Some((callable, signature)) = [
            StyleCallableId::Radii,
            StyleCallableId::Shadow,
            StyleCallableId::InsetShadow,
            StyleCallableId::Blur,
            StyleCallableId::Brightness,
            StyleCallableId::Contrast,
            StyleCallableId::Opacity,
            StyleCallableId::RoundedRect,
            StyleCallableId::Mask,
            StyleCallableId::Transition,
            StyleCallableId::Resource,
        ]
        .into_iter()
        .find_map(|callable| {
            (callable.source_label() == name)
                .then(|| Signature::for_callable(callable).map(|signature| (callable, signature)))
                .flatten()
        }) else {
            return Ok(None);
        };
        if signature.result != expected {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let arguments = self.bind_style_arguments(context, module, owner, call, &signature)?;
        let result = Self::construct_style_composite(owner, callable, &arguments)?;
        self.publish_style_callee(*callee, callable)?;
        Ok(Some(result))
    }

    fn style_composite_leaf(
        owner: ExprId,
        expression: &HirExpr,
        expected: CompileTimeStyleType,
    ) -> Result<Option<CheckedStyleOperand>, AnalyzerExpressionError> {
        let string = match expression.kind() {
            HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(value))) => {
                Some(value.as_ref())
            }
            _ => None,
        };
        let operand = match expected {
            CompileTimeStyleType::Property => CheckedStyleOperand::Property(
                string
                    .and_then(ViewPropertyKind::from_source_name)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
            ),
            CompileTimeStyleType::ResourceId => CheckedStyleOperand::ResourceId(
                string
                    .and_then(|value| arcweft_id::PublicId::try_new(value).ok())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
            ),
            CompileTimeStyleType::Duration => {
                let HirExprKind::Literal(literal) = expression.kind() else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                CheckedStyleOperand::Duration(
                    CheckedStyleDuration::try_from(literal)
                        .map_err(|_| AnalyzerExpressionError::rejected(owner))?,
                )
            }
            _ => return Ok(None),
        };
        Ok(Some(operand))
    }

    fn bind_style_arguments(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        call: &HirCallInvocation,
        signature: &Signature,
    ) -> Result<Vec<CheckedExpressionResolution>, AnalyzerExpressionError> {
        let mut bound = vec![None; signature.parameters.len()];
        let mut positional = 0;
        let mut named = false;
        // Bind and evaluate in authored order. Parameter order only selects storage.
        for argument in call.arguments() {
            let index = match argument {
                HirCallArgument::Positional { .. } if !named => {
                    let index = positional;
                    positional += 1;
                    index
                }
                HirCallArgument::Named { .. } => {
                    named = true;
                    let name = argument
                        .resolved_name()
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    signature
                        .parameters
                        .iter()
                        .position(|parameter| parameter.name == name.as_str())
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?
                }
                _ => return Err(AnalyzerExpressionError::rejected(owner)),
            };
            let parameter = signature
                .parameters
                .get(index)
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            if bound[index].is_some() {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
            let child = argument.value();
            let checked = self.evaluate_expression(
                context,
                child,
                Some(&TypeKind::CompileTimeStyle(parameter.ty)),
            )?;
            if !checked.effects().is_empty() {
                return Err(AnalyzerExpressionError::rejected(child));
            }
            let resolution = match checked.checked_resolution() {
                Some(CheckedExpressionResolution::StyleValue(value)) => {
                    CheckedExpressionResolution::StyleValue(self.resolve_style_constant(
                        context,
                        module,
                        child,
                        value.clone(),
                    )?)
                }
                Some(CheckedExpressionResolution::StyleOperand(operand)) => {
                    CheckedExpressionResolution::StyleOperand(operand.clone())
                }
                _ => return Err(AnalyzerExpressionError::rejected(child)),
            };
            bound[index] = Some(resolution);
        }
        bound
            .into_iter()
            .zip(signature.parameters)
            .map(|(bound, parameter)| {
                bound
                    .or_else(|| match parameter.default? {
                        DefaultArgument::ZeroLength => Some(
                            CheckedExpressionResolution::StyleValue(ViewSpecifiedValue::Length {
                                value: ViewLengthMilli::new(0),
                            }),
                        ),
                        DefaultArgument::ZeroDuration => {
                            Some(CheckedExpressionResolution::StyleOperand(
                                CheckedStyleOperand::Duration(CheckedStyleDuration::ZERO),
                            ))
                        }
                    })
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
            })
            .collect()
    }

    fn construct_style_composite(
        owner: ExprId,
        callable: StyleCallableId,
        arguments: &[CheckedExpressionResolution],
    ) -> Result<CheckedExpression, AnalyzerExpressionError> {
        use CheckedExpressionResolution::{StyleOperand as Operand, StyleValue as Value};
        use CheckedStyleOperand::{Duration, Property, ResourceId};
        use ViewSpecifiedValue::{BorderRadii, Color, Length, Ratio, Resource, Scalar};
        let result = match (callable, arguments) {
            (
                StyleCallableId::Radii,
                [
                    Value(Length { value: top_left }),
                    Value(Length { value: top_right }),
                    Value(Length {
                        value: bottom_right,
                    }),
                    Value(Length { value: bottom_left }),
                ],
            ) => Self::checked_style_value(BorderRadii {
                value: ViewBorderRadii {
                    top_left: *top_left,
                    top_right: *top_right,
                    bottom_right: *bottom_right,
                    bottom_left: *bottom_left,
                },
            }),
            (
                StyleCallableId::Shadow | StyleCallableId::InsetShadow,
                [
                    Value(Length { value: x }),
                    Value(Length { value: y }),
                    Value(Length { value: blur }),
                    Value(Length { value: spread }),
                    Value(Color { value: color }),
                ],
            ) => Self::checked_style_operand(CheckedStyleOperand::Shadow(ViewShadow {
                x: *x,
                y: *y,
                blur: *blur,
                spread: *spread,
                color: *color,
                inset: callable == StyleCallableId::InsetShadow,
            })),
            (StyleCallableId::Blur, [Value(Length { value: radius })]) => {
                Self::checked_style_operand(CheckedStyleOperand::Filter(ViewFilter::Blur {
                    radius: *radius,
                }))
            }
            (StyleCallableId::Brightness, [Value(Scalar { value: amount })]) => {
                Self::checked_style_operand(CheckedStyleOperand::Filter(ViewFilter::Brightness {
                    amount: *amount,
                }))
            }
            (StyleCallableId::Contrast, [Value(Scalar { value: amount })]) => {
                Self::checked_style_operand(CheckedStyleOperand::Filter(ViewFilter::Contrast {
                    amount: *amount,
                }))
            }
            (StyleCallableId::Opacity, [Value(Ratio { value: amount })]) => {
                Self::checked_style_operand(CheckedStyleOperand::Filter(ViewFilter::Opacity {
                    amount: *amount,
                }))
            }
            (StyleCallableId::RoundedRect, [Value(BorderRadii { value })]) => {
                Self::checked_style_value(ViewSpecifiedValue::Clip {
                    value: ViewClip::RoundedRect(*value),
                })
            }
            (StyleCallableId::Mask, [Value(Resource { value })]) => {
                Self::checked_style_value(ViewSpecifiedValue::Mask {
                    value: ViewMask::Resource(value.clone()),
                })
            }
            (
                StyleCallableId::Transition,
                [
                    Operand(Property(property)),
                    Operand(Duration(duration)),
                    Operand(Duration(delay)),
                ],
            ) => Self::checked_style_operand(CheckedStyleOperand::Transition(
                ViewStyleTransition::new(*property, duration.milliseconds(), delay.milliseconds())
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?,
            )),
            (StyleCallableId::Resource, [Operand(ResourceId(value))]) => {
                Self::checked_style_value(Resource {
                    value: value.clone(),
                })
            }
            _ => return Err(AnalyzerExpressionError::rejected(owner)),
        };
        Ok(result)
    }
}
