//! One Source grammar shared by Style checking and token inference.

use crate::{
    final_analysis::CheckedStyleNumber,
    types::{CompileTimeStyleType, StyleCallableId, StyleNumericType},
};
use arcweft_lang_hir::{
    expr::{HirCallArgument, HirCallCallee, HirCallInvocation, HirExprKind, HirUnaryOp},
    identity::ExprId,
    leaf::{HirLiteral, HirPathRoot, HirPathSegment},
    module::HirModule,
};
use arcweft_view::style::{
    ViewAlignment, ViewBlendMode, ViewBoxAxisMode, ViewClip, ViewDisplay, ViewFlexDirection,
    ViewFlexWrap, ViewFontStyle, ViewMask, ViewOverflow, ViewPosition, ViewSpecifiedValue,
    ViewStyleValueKind,
};

#[derive(Clone, Copy)]
pub(in crate::final_analysis::analyzer) enum StyleDefaultArgument {
    ZeroLength,
    ZeroDuration,
}

#[derive(Clone, Copy)]
pub(in crate::final_analysis::analyzer) enum StyleArgumentType {
    Style(CompileTimeStyleType),
    Byte,
    TokenName,
}

pub(in crate::final_analysis::analyzer) struct StyleParameter {
    pub name: &'static str,
    pub ty: StyleArgumentType,
    pub default: Option<StyleDefaultArgument>,
}

impl StyleParameter {
    const fn required(name: &'static str, ty: CompileTimeStyleType) -> Self {
        Self {
            name,
            ty: StyleArgumentType::Style(ty),
            default: None,
        }
    }
    const fn optional(
        name: &'static str,
        ty: CompileTimeStyleType,
        default: StyleDefaultArgument,
    ) -> Self {
        Self {
            name,
            ty: StyleArgumentType::Style(ty),
            default: Some(default),
        }
    }
    const fn special(name: &'static str, ty: StyleArgumentType) -> Self {
        Self {
            name,
            ty,
            default: None,
        }
    }
}

pub(in crate::final_analysis::analyzer) struct StyleFactorySignature {
    /// Token references synthesize from the declaration rather than a fixed result.
    pub result: Option<CompileTimeStyleType>,
    pub parameters: &'static [StyleParameter],
}

#[derive(Debug)]
pub(in crate::final_analysis::analyzer) enum StyleArgumentBindingError {
    InvalidPosition,
    UnknownName,
    Duplicate,
    MissingRequired,
}

impl StyleFactorySignature {
    pub fn for_callable(callable: StyleCallableId) -> Self {
        use CompileTimeStyleType::{
            Duration, Filter, Property, ResourceId, Shadow, Transition, Value,
        };
        use ViewStyleValueKind::{BorderRadii, Clip, Color, Length, Mask, Ratio, Resource, Scalar};
        match callable {
            StyleCallableId::Rgba => Self {
                result: Some(Value(Color)),
                parameters: const {
                    &[
                        StyleParameter::special("red", StyleArgumentType::Byte),
                        StyleParameter::special("green", StyleArgumentType::Byte),
                        StyleParameter::special("blue", StyleArgumentType::Byte),
                        StyleParameter::special("alpha", StyleArgumentType::Byte),
                    ]
                },
            },
            StyleCallableId::Token => Self {
                result: None,
                parameters: const {
                    &[StyleParameter::special(
                        "name",
                        StyleArgumentType::TokenName,
                    )]
                },
            },
            StyleCallableId::SystemColor => Self {
                result: Some(Value(Color)),
                parameters: const {
                    &[StyleParameter::required(
                        "role",
                        CompileTimeStyleType::SystemColor,
                    )]
                },
            },
            StyleCallableId::SystemFont => Self {
                result: Some(CompileTimeStyleType::FontFamily),
                parameters: const {
                    &[StyleParameter::required(
                        "family",
                        CompileTimeStyleType::SystemFontFamily,
                    )]
                },
            },
            StyleCallableId::Radii => Self {
                result: Some(Value(BorderRadii)),
                parameters: const {
                    &[
                        StyleParameter::required("top_left", Value(Length)),
                        StyleParameter::required("top_right", Value(Length)),
                        StyleParameter::required("bottom_right", Value(Length)),
                        StyleParameter::required("bottom_left", Value(Length)),
                    ]
                },
            },
            StyleCallableId::Shadow | StyleCallableId::InsetShadow => Self {
                result: Some(Shadow),
                parameters: const {
                    &[
                        StyleParameter::optional(
                            "x",
                            Value(Length),
                            StyleDefaultArgument::ZeroLength,
                        ),
                        StyleParameter::optional(
                            "y",
                            Value(Length),
                            StyleDefaultArgument::ZeroLength,
                        ),
                        StyleParameter::optional(
                            "blur",
                            Value(Length),
                            StyleDefaultArgument::ZeroLength,
                        ),
                        StyleParameter::optional(
                            "spread",
                            Value(Length),
                            StyleDefaultArgument::ZeroLength,
                        ),
                        StyleParameter::required("color", Value(Color)),
                    ]
                },
            },
            StyleCallableId::Blur => Self {
                result: Some(Filter),
                parameters: const { &[StyleParameter::required("radius", Value(Length))] },
            },
            StyleCallableId::Brightness | StyleCallableId::Contrast => Self {
                result: Some(Filter),
                parameters: const { &[StyleParameter::required("amount", Value(Scalar))] },
            },
            StyleCallableId::Opacity => Self {
                result: Some(Filter),
                parameters: const { &[StyleParameter::required("amount", Value(Ratio))] },
            },
            StyleCallableId::RoundedRect => Self {
                result: Some(Value(Clip)),
                parameters: const { &[StyleParameter::required("radii", Value(BorderRadii))] },
            },
            StyleCallableId::Mask => Self {
                result: Some(Value(Mask)),
                parameters: const { &[StyleParameter::required("resource", Value(Resource))] },
            },
            StyleCallableId::Transition => Self {
                result: Some(Transition),
                parameters: const {
                    &[
                        StyleParameter::required("property", Property),
                        StyleParameter::required("duration", Duration),
                        StyleParameter::optional(
                            "delay",
                            Duration,
                            StyleDefaultArgument::ZeroDuration,
                        ),
                    ]
                },
            },
            StyleCallableId::Resource => Self {
                result: Some(Value(Resource)),
                parameters: const { &[StyleParameter::required("id", ResourceId)] },
            },
        }
    }

    /// Returns explicit arguments in authored order, with their parameter slots.
    pub fn bind(
        &self,
        call: &HirCallInvocation,
    ) -> Result<Vec<(usize, ExprId)>, StyleArgumentBindingError> {
        if call.arguments().len() > self.parameters.len() {
            return Err(StyleArgumentBindingError::InvalidPosition);
        }
        let mut seen = vec![false; self.parameters.len()];
        let mut bound = Vec::with_capacity(call.arguments().len());
        let mut positional = 0;
        let mut named = false;
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
                        .ok_or(StyleArgumentBindingError::UnknownName)?;
                    self.parameters
                        .iter()
                        .position(|parameter| parameter.name == name.as_str())
                        .ok_or(StyleArgumentBindingError::UnknownName)?
                }
                _ => return Err(StyleArgumentBindingError::InvalidPosition),
            };
            let previous = seen
                .get_mut(index)
                .ok_or(StyleArgumentBindingError::InvalidPosition)?;
            if *previous {
                return Err(StyleArgumentBindingError::Duplicate);
            }
            *previous = true;
            bound.push((index, argument.value()));
        }
        if seen
            .iter()
            .zip(self.parameters)
            .any(|(seen, parameter)| !seen && parameter.default.is_none())
        {
            return Err(StyleArgumentBindingError::MissingRequired);
        }
        Ok(bound)
    }
}

/// Pure Source grammar. Complete value payloads remain native View values.
pub(in crate::final_analysis::analyzer) struct StyleSourceGrammar;

impl StyleSourceGrammar {
    pub fn callable(
        module: &HirModule,
        call: &HirCallInvocation,
    ) -> Option<(ExprId, StyleCallableId)> {
        let HirCallCallee::Value { value: callee } = call.callee() else {
            return None;
        };
        let HirExprKind::Path(path) = module.resolve_expr(*callee).ok()?.kind() else {
            return None;
        };
        let path = path.as_resolved()?;
        if path.root() != HirPathRoot::ImplicitCrate {
            return None;
        }
        let [HirPathSegment::Identifier(name)] = path.segments() else {
            return None;
        };
        StyleCallableId::ALL
            .into_iter()
            .find(|callable| callable.source_label() == name.as_str())
            .map(|callable| (*callee, callable))
    }

    pub fn sequence_element(kind: ViewStyleValueKind) -> Option<CompileTimeStyleType> {
        Some(match kind {
            ViewStyleValueKind::FontFamilyList => CompileTimeStyleType::FontFamily,
            ViewStyleValueKind::ShadowList => CompileTimeStyleType::Shadow,
            ViewStyleValueKind::FilterList => CompileTimeStyleType::Filter,
            ViewStyleValueKind::Transition => CompileTimeStyleType::Transition,
            _ => return None,
        })
    }

    pub fn number(
        module: &HirModule,
        owner: ExprId,
        kind: StyleNumericType,
    ) -> Option<(CheckedStyleNumber, Option<ExprId>)> {
        let (literal, operand) = match module.resolve_expr(owner).ok()?.kind() {
            HirExprKind::Literal(literal) => (literal, None),
            HirExprKind::Unary(unary) if unary.operator() == HirUnaryOp::Negate => {
                let HirExprKind::Literal(literal) =
                    module.resolve_expr(unary.operand()).ok()?.kind()
                else {
                    return None;
                };
                (literal, Some(unary.operand()))
            }
            _ => return None,
        };
        CheckedStyleNumber::try_from((kind, literal))
            .ok()
            .map(|number| (number, operand))
    }

    pub fn closed_variant(kind: ViewStyleValueKind, name: &str) -> Option<ViewSpecifiedValue> {
        match kind {
            ViewStyleValueKind::BoxAxes => ViewBoxAxisMode::from_source_name(name)
                .map(|value| ViewSpecifiedValue::BoxAxes { value }),
            ViewStyleValueKind::FontStyle => ViewFontStyle::from_source_name(name)
                .map(|value| ViewSpecifiedValue::FontStyle { value }),
            ViewStyleValueKind::Display => ViewDisplay::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Display { value }),
            ViewStyleValueKind::Position => ViewPosition::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Position { value }),
            ViewStyleValueKind::Overflow => ViewOverflow::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Overflow { value }),
            ViewStyleValueKind::FlexDirection => ViewFlexDirection::from_source_name(name)
                .map(|value| ViewSpecifiedValue::FlexDirection { value }),
            ViewStyleValueKind::FlexWrap => ViewFlexWrap::from_source_name(name)
                .map(|value| ViewSpecifiedValue::FlexWrap { value }),
            ViewStyleValueKind::Alignment => ViewAlignment::from_source_name(name)
                .map(|value| ViewSpecifiedValue::Alignment { value }),
            ViewStyleValueKind::Clip => {
                ViewClip::from_source_name(name).map(|value| ViewSpecifiedValue::Clip { value })
            }
            ViewStyleValueKind::Mask => {
                ViewMask::from_source_name(name).map(|value| ViewSpecifiedValue::Mask { value })
            }
            ViewStyleValueKind::BlendMode => ViewBlendMode::from_source_name(name)
                .map(|value| ViewSpecifiedValue::BlendMode { value }),
            _ => None,
        }
    }

    /// Principal literal families, before contextual Length-to-radii conversion.
    pub fn literal_kinds(module: &HirModule, owner: ExprId) -> Vec<ViewStyleValueKind> {
        let Ok(expression) = module.resolve_expr(owner) else {
            return Vec::new();
        };
        if matches!(
            expression.kind(),
            HirExprKind::Literal(HirLiteral::Boolean(_))
        ) {
            return vec![ViewStyleValueKind::Bool];
        }
        if let HirExprKind::ShortVariant(name) = expression.kind() {
            return name.as_resolved().map_or_else(Vec::new, |name| {
                ViewStyleValueKind::ALL
                    .iter()
                    .copied()
                    .filter(|kind| Self::closed_variant(*kind, name.as_str()).is_some())
                    .collect()
            });
        }
        StyleNumericType::ALL
            .into_iter()
            .filter_map(|kind| {
                let (number, operand) = Self::number(module, owner, kind)?;
                number
                    .specified_value(operand.is_some())
                    .ok()
                    .map(|_| kind.value_kind())
            })
            .collect()
    }
}
