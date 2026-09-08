use super::*;
use crate::final_analysis::PreparedProjectNominalTypeValueExpression;
use crate::final_analysis::fx_application::{
    checked_builtin_fx_binding, checked_project_fx_argument,
};

use crate::checked_rich_text::{
    CheckedDialogueHostEvent, PreparedCheckedDialogueToken, PreparedCheckedRichTextAction,
    RichTextContentChecker,
};
use arcweft_lang_hir::dialogue_application::{
    HirAttachedContentApplicationFamily, HirAttachedContentBodyPresence,
    HirContentCallSemanticEvidence, HirLinePlanItem,
};
use arcweft_lang_hir::expr::HirCallInvocationForm;
use arcweft_lang_hir::identity::StmtId;

fn content_expression_children(
    content: &arcweft_lang_hir::dialogue_application::HirDialogueContent,
) -> Vec<ExprId> {
    content
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            arcweft_lang_hir::dialogue_application::HirDialogueNodeKind::Interpolation(
                expression,
            )
            | arcweft_lang_hir::dialogue_application::HirDialogueNodeKind::ContentApplication(
                expression,
            ) => Some(*expression),
            arcweft_lang_hir::dialogue_application::HirDialogueNodeKind::PointAction(action) => {
                action.payload().expression()
            }
            _ => None,
        })
        .collect()
}

fn checked_compile_time_default(
    kind: arcweft_rich_text_schema::RichTextValueKind,
    value: arcweft_rich_text_schema::RichTextDefaultValue,
) -> Option<crate::final_analysis::CheckedCompileTimeValue> {
    use crate::checked_compile_time::CheckedCompileTimeScalar;
    use crate::checked_rich_text::{
        CheckedAngle, CheckedColor, CheckedDuration, CheckedLength, LengthUnit, Milli, RatioMilli,
    };
    use crate::final_analysis::{CheckedCompileTimeValue, CheckedCompileTimeVector};
    use arcweft_rich_text_schema::RichTextUnit;
    Some(match value {
        arcweft_rich_text_schema::RichTextDefaultValue::Bool(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Bool(value))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::Int(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Int(value))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::Milli(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Milli(Milli(value)))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::RatioMilli(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Ratio(RatioMilli(value)))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::Length { milli, unit } => {
            let unit = match unit {
                RichTextUnit::Px => LengthUnit::Px,
                RichTextUnit::Pt => LengthUnit::Pt,
                RichTextUnit::Ch => LengthUnit::Ch,
                RichTextUnit::Em => LengthUnit::Em,
                RichTextUnit::Unitless
                | RichTextUnit::Deg
                | RichTextUnit::Ms
                | RichTextUnit::S
                | RichTextUnit::Cps => return None,
            };
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Length(CheckedLength {
                milli,
                unit,
            }))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::AngleMilliDegrees(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Angle(CheckedAngle {
                milli_degrees: value,
            }))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::DurationMillis(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Duration(CheckedDuration {
                millis: value,
            }))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::EnumVariant(value) => {
            let arcweft_rich_text_schema::RichTextValueKind::ClosedEnum(domain) = kind else {
                return None;
            };
            CheckedCompileTimeValue::Enum(arcweft_id::closed_enum::ClosedEnumValueId::new(
                domain, value,
            ))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::PublicId(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::PublicId(
                arcweft_id::PublicId::try_new(value).ok()?,
            ))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::Text(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Text(value.into()))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::ColorRgba8(value) => {
            CheckedCompileTimeValue::scalar(CheckedCompileTimeScalar::Color(CheckedColor::Rgba8(
                value,
            )))
        }
        arcweft_rich_text_schema::RichTextDefaultValue::Vec2Milli(value) => {
            CheckedCompileTimeValue::Vector(CheckedCompileTimeVector::new(
                2,
                value.into_iter().map(Milli).collect(),
            )?)
        }
        arcweft_rich_text_schema::RichTextDefaultValue::Seed32(value) => {
            CheckedCompileTimeValue::Seed32(value)
        }
    })
}

fn decimal_to_milli(decimal: &arcweft_lang_hir::leaf::HirDecimal) -> Option<i32> {
    let text = decimal.to_decimal_string();
    let (negative, text) = text
        .strip_prefix('-')
        .map_or((false, text.as_str()), |value| (true, value));
    let (whole, fraction) = text.split_once('.').map_or((text, ""), |parts| parts);
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole = whole.parse::<i64>().ok()?;
    let mut fraction_digits = fraction.bytes().take(3).collect::<Vec<_>>();
    if fraction.len() > 3 && fraction.bytes().skip(3).any(|byte| byte != b'0') {
        return None;
    }
    while fraction_digits.len() < 3 {
        fraction_digits.push(b'0');
    }
    if !fraction_digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let fraction = fraction_digits
        .into_iter()
        .fold(0_i64, |value, digit| value * 10 + i64::from(digit - b'0'));
    let value = whole.checked_mul(1_000)?.checked_add(fraction)?;
    let value = if negative {
        value.checked_neg()?
    } else {
        value
    };
    i32::try_from(value).ok()
}

fn integer_to_i64(magnitude: &arcweft_lang_hir::leaf::HirBigUint, negative: bool) -> Option<i64> {
    let value = magnitude.to_decimal_string().parse::<u64>().ok()?;
    if negative {
        if value == (i64::MAX as u64).checked_add(1)? {
            Some(i64::MIN)
        } else {
            i64::try_from(value).ok()?.checked_neg()
        }
    } else {
        i64::try_from(value).ok()
    }
}

fn typed_milli_from_hir(
    module: &HirModule,
    expression: ExprId,
) -> Option<crate::checked_rich_text::Milli> {
    use arcweft_lang_hir::expr::{HirExprKind, HirUnaryOp};
    use arcweft_lang_hir::leaf::{HirIntegerLiteral, HirLiteral, HirUnitNumberLiteral};
    let expression = module.resolve_expr(expression).ok()?;
    match expression.kind() {
        HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
            magnitude, ..
        })) => Some(crate::checked_rich_text::Milli(
            integer_to_i64(magnitude, false)?.try_into().ok()?,
        )),
        HirExprKind::Literal(HirLiteral::Float(
            arcweft_lang_hir::leaf::HirFloatLiteral::Value { decimal, .. },
        )) => Some(crate::checked_rich_text::Milli(decimal_to_milli(decimal)?)),
        HirExprKind::Literal(HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
            decimal,
            ..
        })) => Some(crate::checked_rich_text::Milli(decimal_to_milli(decimal)?)),
        HirExprKind::Unary(unary) if unary.operator() == HirUnaryOp::Negate => {
            let operand = module.resolve_expr(unary.operand()).ok()?;
            match operand.kind() {
                HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
                    magnitude,
                    ..
                })) => Some(crate::checked_rich_text::Milli(
                    integer_to_i64(magnitude, true)?.try_into().ok()?,
                )),
                HirExprKind::Literal(HirLiteral::Float(
                    arcweft_lang_hir::leaf::HirFloatLiteral::Value { decimal, .. },
                ))
                | HirExprKind::Literal(HirLiteral::UnitNumber(HirUnitNumberLiteral::Value {
                    decimal,
                    ..
                })) => Some(crate::checked_rich_text::Milli(
                    decimal_to_milli(decimal)?.checked_neg()?,
                )),
                _ => None,
            }
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConditionalParameterDecision {
    AcceptExplicit,
    Omit,
    Materialize(arcweft_rich_text_schema::RichTextDefaultValue),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConditionalParameterError {
    InactiveExplicit,
    MissingDefault,
}

pub(crate) fn decide_conditional_parameter<'a, P, V>(
    presence: arcweft_rich_text_schema::RichTextCallableParameterPresence<P>,
    conditional_default: Option<arcweft_rich_text_schema::RichTextDefaultValue>,
    supplied: bool,
    value_for: impl FnMut(P) -> Option<&'a V>,
) -> Result<ConditionalParameterDecision, ConditionalParameterError>
where
    P: Copy + Eq + 'static,
    V: arcweft_rich_text_schema::RichTextPredicateValueView + ?Sized + 'a,
{
    let arcweft_rich_text_schema::RichTextCallableParameterPresence::Conditional { predicate } =
        presence
    else {
        return Err(ConditionalParameterError::MissingDefault);
    };
    if !predicate.holds(value_for) {
        return if supplied {
            Err(ConditionalParameterError::InactiveExplicit)
        } else {
            Ok(ConditionalParameterDecision::Omit)
        };
    }
    if supplied {
        Ok(ConditionalParameterDecision::AcceptExplicit)
    } else {
        conditional_default
            .map(ConditionalParameterDecision::Materialize)
            .ok_or(ConditionalParameterError::MissingDefault)
    }
}

impl Analyzer<'_, '_, '_> {
    pub(in crate::final_analysis::analyzer) fn checked_compile_time_value(
        &self,
        module: &HirModule,
        expression: ExprId,
        expected: &TypeKind,
    ) -> Result<crate::final_analysis::CheckedCompileTimeValue, AnalyzerExpressionError> {
        let fact = self
            .facts
            .expressions()
            .get(&expression)
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        if let PreparedExpressionFact::CompileTimeScalar(value) = fact {
            return Ok(crate::final_analysis::CheckedCompileTimeValue::scalar(
                value.value().clone(),
            ));
        }
        match expected {
            TypeKind::CompileTimeEnum(enum_type) => {
                let Some(CheckedExpressionResolution::CompileTimeEnum(value)) =
                    fact.checked_resolution()
                else {
                    return Err(AnalyzerExpressionError::rejected(expression));
                };
                let actual =
                    crate::types::CompileTimeEnumType::exact(value.domain(), value.variant());
                if !enum_type.accepts(actual) {
                    return Err(AnalyzerExpressionError::rejected(expression));
                }
                Ok(crate::final_analysis::CheckedCompileTimeValue::Enum(*value))
            }
            TypeKind::CompileTimeFx(_) => Err(AnalyzerExpressionError::rejected(expression)),
            TypeKind::FixedVector(vector) => {
                self.checked_compile_time_vector(module, expression, vector)
            }
            TypeKind::F32 => typed_milli_from_hir(module, expression)
                .map(crate::checked_compile_time::CheckedCompileTimeScalar::Milli)
                .map(crate::final_analysis::CheckedCompileTimeValue::scalar)
                .ok_or_else(|| AnalyzerExpressionError::rejected(expression)),
            TypeKind::U32 => self
                .checked_seed32(module, expression)
                .map(crate::final_analysis::CheckedCompileTimeValue::Seed32)
                .ok_or_else(|| AnalyzerExpressionError::rejected(expression)),
            _ => self
                .checked_compile_time_scalar(module, expression, fact, expected)
                .ok_or_else(|| AnalyzerExpressionError::rejected(expression)),
        }
    }

    fn checked_compile_time_scalar(
        &self,
        module: &HirModule,
        expression: ExprId,
        fact: &PreparedExpressionFact,
        expected: &TypeKind,
    ) -> Option<crate::final_analysis::CheckedCompileTimeValue> {
        let scalars = self.catalogs.world.environment().compile_time_scalars();
        let role = crate::registration::CompileTimeScalarTypeRoleId::ALL
            .into_iter()
            .find(|role| scalars.type_for(*role) == expected)?;
        let kind = match role {
            crate::registration::CompileTimeScalarTypeRoleId::Bool => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Bool
            }
            crate::registration::CompileTimeScalarTypeRoleId::Int => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Int
            }
            crate::registration::CompileTimeScalarTypeRoleId::Milli => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Milli
            }
            crate::registration::CompileTimeScalarTypeRoleId::Ratio => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Ratio
            }
            crate::registration::CompileTimeScalarTypeRoleId::Length => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Length
            }
            crate::registration::CompileTimeScalarTypeRoleId::Angle => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Angle
            }
            crate::registration::CompileTimeScalarTypeRoleId::Duration => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Duration
            }
            crate::registration::CompileTimeScalarTypeRoleId::PublicId => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::PublicId
            }
            crate::registration::CompileTimeScalarTypeRoleId::Color => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Color
            }
            crate::registration::CompileTimeScalarTypeRoleId::Text => {
                crate::checked_compile_time::CheckedCompileTimeScalarKind::Text
            }
        };
        let value = if let PreparedExpressionFact::CompileTimeScalar(value) = fact {
            value.value().clone()
        } else if role == crate::registration::CompileTimeScalarTypeRoleId::Color
            && self
                .facts
                .calls()
                .get(&expression)
                .and_then(crate::callable::CallTargetFacts::selected_application)
                .is_some_and(|application| {
                    matches!(
                        application.core().candidates().selected().id(),
                        crate::callable::CallableCandidateId::Builtin(
                            crate::callable::BuiltinCallableId::Rgb
                        )
                    )
                })
        {
            crate::checked_text_proxy::reduce_color_argument(module, expression).ok()?
        } else {
            self.reduce_compile_time_scalar_expression(module, expression, &kind, fact)
                .ok()?
        };
        Some(crate::final_analysis::CheckedCompileTimeValue::scalar(
            value,
        ))
    }

    fn checked_seed32(&self, module: &HirModule, expression: ExprId) -> Option<u32> {
        use arcweft_lang_hir::expr::{HirExprKind, HirUnaryOp};
        use arcweft_lang_hir::leaf::{HirIntegerLiteral, HirLiteral};
        let expression_node = module.resolve_expr(expression).ok()?;
        let (magnitude, negative) = match expression_node.kind() {
            HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
                magnitude,
                ..
            })) => (magnitude, false),
            HirExprKind::Unary(unary) if unary.operator() == HirUnaryOp::Negate => {
                let operand = module.resolve_expr(unary.operand()).ok()?;
                let HirExprKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
                    magnitude,
                    ..
                })) = operand.kind()
                else {
                    return None;
                };
                (magnitude, true)
            }
            _ => return None,
        };
        (!negative)
            .then(|| magnitude.to_decimal_string().parse::<u32>().ok())
            .flatten()
    }

    pub(in crate::final_analysis::analyzer) fn checked_content_fx_application(
        &mut self,
        module: &HirModule,
        content_application: &crate::callable::CheckedCallApplication,
        ordinal: crate::final_analysis::CheckedFxApplicationOrdinal,
    ) -> Result<crate::final_analysis::CheckedContentFxApplication, AnalyzerExpressionError> {
        let owner = content_application.core().site().expression();
        let selected = content_application.core().candidates().selected();
        let crate::callable::CallableValidator::Content(identity) = selected.schema().validator()
        else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        if !matches!(
            identity,
            crate::callable::ContentCallableIdentity::Language {
                definition:
                    arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Fx,
                ..
            }
        ) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let group = selected
            .schema()
            .group(crate::callable::CallableGroupIndex::ZERO)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let [parameter] = group.parameters() else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let coordinate =
            crate::callable::CallableParameterCoordinate::new(group.index(), parameter.index());
        let mut slots = content_application
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .filter(|slot| {
                matches!(
                    slot.destination(),
                    crate::callable::CheckedCallOperandDestination::Parameter(actual)
                        if *actual == coordinate
                )
            });
        let slot = slots
            .next()
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        if slots.next().is_some() {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let crate::callable::CheckedCallArgumentSlotSource::Expression(expression) =
            slot.source().raw()
        else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        self.checked_closed_fx_application(module, expression, ordinal)
    }

    fn checked_closed_fx_application(
        &mut self,
        module: &HirModule,
        expression: ExprId,
        ordinal: crate::final_analysis::CheckedFxApplicationOrdinal,
    ) -> Result<crate::final_analysis::CheckedContentFxApplication, AnalyzerExpressionError> {
        let fact = self
            .facts
            .expressions()
            .get(&expression)
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        let PreparedExpressionFact::Complete(checked) = fact else {
            return Err(AnalyzerExpressionError::rejected(expression));
        };
        let application = self
            .facts
            .calls()
            .get(&expression)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .cloned()
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        if application.core().site() != crate::callable::CheckedCallSite::HirCall(expression) {
            return Err(AnalyzerExpressionError::rejected(expression));
        }
        let Some(TypeKind::CompileTimeFx(actual)) = checked.value_type() else {
            return Err(AnalyzerExpressionError::rejected(expression));
        };
        let actual = actual.clone();
        let mut definitions = self
            .fx_definitions
            .take()
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        let result = crate::final_analysis::fx_application::seal_shared_fx_application(
            expression,
            &application,
            &actual,
            ordinal,
            &mut definitions,
            |request, selected, source| {
                let declared = selected.declared_type().ok_or(())?;
                let value = self
                    .checked_compile_time_value(module, source, declared)
                    .map_err(|_| ())?;
                match request {
                    crate::final_analysis::fx_application::CheckedFxBindingRequest::Builtin(
                        parameter,
                    ) => checked_builtin_fx_binding(parameter.parameter_type(), &value),
                    crate::final_analysis::fx_application::CheckedFxBindingRequest::Project(
                        expected,
                    ) => checked_project_fx_argument(expected, &value)
                        .map(crate::final_analysis::CheckedContentFxBinding::abi),
                }
            },
        );
        self.fx_definitions = Some(definitions);
        result
            .map(crate::final_analysis::CheckedContentFxApplication::from_inner)
            .map_err(|_| AnalyzerExpressionError::rejected(expression))
    }

    fn checked_compile_time_vector(
        &self,
        module: &HirModule,
        expression: ExprId,
        expected: &crate::types::FixedVectorType,
    ) -> Result<crate::final_analysis::CheckedCompileTimeValue, AnalyzerExpressionError> {
        let application = self
            .facts
            .calls()
            .get(&expression)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        let crate::callable::CallableCandidateId::Builtin(
            crate::callable::BuiltinCallableId::Vector { dimensions },
        ) = application.core().candidates().selected().id()
        else {
            return Err(AnalyzerExpressionError::rejected(expression));
        };
        if *dimensions
            != match expected.dimensions() {
                crate::callable::VectorDimensions::Two => crate::callable::VectorDimensions::Two,
                crate::callable::VectorDimensions::Three => {
                    crate::callable::VectorDimensions::Three
                }
                crate::callable::VectorDimensions::Four => crate::callable::VectorDimensions::Four,
            }
        {
            return Err(AnalyzerExpressionError::rejected(expression));
        }
        let mut components = Vec::new();
        for argument in application.core().execution().arguments() {
            for slot in argument.slots() {
                let crate::callable::CheckedCallArgumentSlotSource::Expression(source) =
                    slot.source().raw()
                else {
                    return Err(AnalyzerExpressionError::rejected(expression));
                };
                components.push(
                    typed_milli_from_hir(module, source)
                        .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?,
                );
            }
        }
        let dimensions = match expected.dimensions() {
            crate::callable::VectorDimensions::Two => 2,
            crate::callable::VectorDimensions::Three => 3,
            crate::callable::VectorDimensions::Four => 4,
        };
        let value = crate::final_analysis::CheckedCompileTimeVector::new(dimensions, components)
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        Ok(crate::final_analysis::CheckedCompileTimeValue::Vector(
            value,
        ))
    }

    pub(in crate::final_analysis::analyzer) fn content_parameter_values(
        &self,
        module: &HirModule,
        application: &crate::callable::CheckedCallApplication,
        definition: arcweft_presentation::rich_text::PresentationContentCallableDefinitionId,
    ) -> Result<Vec<crate::checked_rich_text::CheckedContentParameter>, AnalyzerExpressionError>
    {
        let row = arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
            .get(definition)
            .ok_or_else(|| {
                AnalyzerExpressionError::rejected(application.core().site().expression())
            })?;
        let schema = application.core().candidates().selected().schema();
        let group = schema
            .group(crate::callable::CallableGroupIndex::ZERO)
            .ok_or_else(|| {
                AnalyzerExpressionError::rejected(application.core().site().expression())
            })?;
        let mut values = Vec::<crate::checked_rich_text::CheckedContentParameter>::new();
        for (index, spec) in row.parameters().iter().enumerate() {
            let parameter_index = crate::callable::CallableParameterIndex::try_from_usize(index)
                .map_err(|_| {
                    AnalyzerExpressionError::rejected(application.core().site().expression())
                })?;
            let parameter = group.parameter(parameter_index).ok_or_else(|| {
                AnalyzerExpressionError::rejected(application.core().site().expression())
            })?;
            let coordinate =
                crate::callable::CallableParameterCoordinate::new(group.index(), parameter_index);
            let slot = application
                .core()
                .execution()
                .arguments()
                .iter()
                .flat_map(|argument| argument.slots())
                .find(|slot| {
                    matches!(
                        slot.destination(),
                        crate::callable::CheckedCallOperandDestination::Parameter(actual)
                            if *actual == coordinate
                    )
                });
            let value = if let Some(slot) = slot {
                let crate::callable::CheckedCallArgumentSlotSource::Expression(source) =
                    slot.source().raw()
                else {
                    return Err(AnalyzerExpressionError::rejected(
                        application.core().site().expression(),
                    ));
                };
                let value = self.checked_compile_time_value(
                    module,
                    source,
                    parameter.declared_type().ok_or_else(|| {
                        AnalyzerExpressionError::rejected(application.core().site().expression())
                    })?,
                )?;
                if matches!(
                    spec.presence,
                    arcweft_rich_text_schema::RichTextCallableParameterPresence::Conditional { .. }
                ) {
                    decide_conditional_parameter(
                        spec.presence,
                        spec.conditional_default,
                        true,
                        |property| {
                            values
                                .iter()
                                .find(|parameter| parameter.id() == property)
                                .map(crate::checked_rich_text::CheckedContentParameter::value)
                        },
                    )
                    .map_err(|_| {
                        AnalyzerExpressionError::rejected(application.core().site().expression())
                    })?;
                }
                value
            } else {
                let default = match spec.presence {
                    arcweft_rich_text_schema::RichTextCallableParameterPresence::Defaulted(
                        value,
                    ) => Some(value),
                    arcweft_rich_text_schema::RichTextCallableParameterPresence::Conditional {
                        ..
                    } => match decide_conditional_parameter(
                        spec.presence,
                        spec.conditional_default,
                        false,
                        |property| {
                            values
                                .iter()
                                .find(|parameter| parameter.id() == property)
                                .map(crate::checked_rich_text::CheckedContentParameter::value)
                        },
                    )
                    .map_err(|_| {
                        AnalyzerExpressionError::rejected(application.core().site().expression())
                    })? {
                        ConditionalParameterDecision::Omit => continue,
                        ConditionalParameterDecision::Materialize(value) => Some(value),
                        ConditionalParameterDecision::AcceptExplicit => {
                            return Err(AnalyzerExpressionError::rejected(
                                application.core().site().expression(),
                            ));
                        }
                    },
                    arcweft_rich_text_schema::RichTextCallableParameterPresence::Required
                    | arcweft_rich_text_schema::RichTextCallableParameterPresence::Optional => None,
                };
                let Some(default) = default else {
                    if matches!(
                        spec.presence,
                        arcweft_rich_text_schema::RichTextCallableParameterPresence::Conditional {
                            predicate: _
                        }
                    ) {
                        return Err(AnalyzerExpressionError::rejected(
                            application.core().site().expression(),
                        ));
                    }
                    continue;
                };
                checked_compile_time_default(spec.kind, default).ok_or_else(|| {
                    AnalyzerExpressionError::rejected(application.core().site().expression())
                })?
            };
            values.push(crate::checked_rich_text::CheckedContentParameter::new(
                spec.id, value,
            ));
        }
        Ok(values)
    }

    fn prepare_language_content_emission(
        &self,
        owner: ExprId,
        content: &arcweft_lang_hir::dialogue_application::HirDialogueContent,
        identity: crate::callable::ContentCallableIdentity,
    ) -> Result<crate::final_analysis::PreparedContentEmission, AnalyzerExpressionError> {
        let crate::callable::ContentCallableIdentity::Language { definition, schema } = identity
        else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        let row = arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
            .get(definition)
            .filter(|row| row.schema_digest() == schema)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        if matches!(
            row.emission_family(),
            arcweft_presentation::rich_text::PresentationContentEmissionFamily::Raw
        ) && content.raw_literal().is_none()
        {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        Ok(crate::final_analysis::PreparedContentEmission::LanguageCallable(identity))
    }

    pub(super) fn prepare_dialogue_content_application(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirAttachedContentApplication,
        expectation: &AnalyzerExpressionExpectation<'_>,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        if let HirAttachedContentApplicationFamily::ContentCall { invocation, .. } =
            application.family()
        {
            return self.prepare_content_call_application(
                context,
                module,
                owner,
                application,
                application.content(),
                invocation,
                application.body_presence(),
            );
        }
        let HirAttachedContentApplicationFamily::DialogueLine {
            target,
            plan,
            coordinates,
        } = application.family()
        else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        };
        if matches!(
            application.body_presence(),
            HirAttachedContentBodyPresence::Absent
        ) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }
        let target_owner = *target;
        let _ = coordinates;
        let expected = expectation.contextual_shape();
        // Immediate id/text_key arguments are compile-time application
        // coordinates. Publish their accepted project identities before the
        // shared parenthesized-call resolver evaluates argument facts.
        let dialogue_application_metadata =
            self.publish_dialogue_coordinates(module, owner, application)?;
        let target_expression = module.resolve_expr(target_owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let checked_target = match target_expression.kind() {
            HirExprKind::Call(call) => {
                let dialogue_application_metadata =
                    dialogue_application_metadata.as_ref().ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                let checked = self.check_call_expression_in_context(
                    context,
                    module,
                    target_owner,
                    call,
                    &AnalyzerExpressionExpectation::Unconstrained,
                    Some(dialogue_application_metadata),
                )?;
                let write = if context.is_candidate()
                    && self.facts.expressions().contains_key(&target_owner)
                {
                    self.facts
                        .replace_existing_expression(target_owner, checked.clone())
                } else {
                    self.facts
                        .publish_new_expression(target_owner, checked.clone())
                };
                write.map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
                checked.into()
            }
            _ => self.evaluate_expression(context, target_owner, None)?,
        };
        let target = checked_character_dialogue_target(target_owner, &checked_target)
            .map_err(|error| AnalyzerExpressionError::Call {
                owner,
                failure: super::super::calls::CallAnalysisFailure::Invariant(
                    super::super::calls::CallAnalysisInvariant::Constraint(error),
                ),
            })?
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let application_patch = match checked_target.checked_resolution() {
            Some(CheckedExpressionResolution::CharacterDialogueFactory(factory)) => {
                Some(factory.patch().clone())
            }
            Some(CheckedExpressionResolution::CharacterDialogueReconfigure(reconfigure)) => {
                Some(reconfigure.patch().clone())
            }
            _ => None,
        };
        let rich_text_check = RichTextContentChecker::check(module, application.content())
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RichTextSourceQuery {
                    owner,
                })
            })?;
        if !rich_text_check.report().is_valid() {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::InvalidRichTextContent {
                    owner,
                    diagnostics: rich_text_check
                        .report()
                        .diagnostics()
                        .to_vec()
                        .into_boxed_slice(),
                    declaration_diagnostics: self
                        .text_proxies
                        .as_ref()
                        .map(|catalog| catalog.diagnostics().to_vec().into_boxed_slice())
                        .unwrap_or_default(),
                },
            ));
        }
        let line_result = plan.as_ref().map_or(Ok(TypeKind::Unit), |plan| {
            self.check_dialogue_line_plan_output(context, owner, plan.items())
        })?;
        let application_children = module
            .resolve_expr(owner)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
            .kind()
            .direct_expression_children();
        for child in application_children {
            if child != target_owner {
                self.evaluate_expression(context, child, None)?;
            }
        }

        let rich_text_check = self
            .prepare_rich_text_effect_plan(module, rich_text_check)
            .map_err(AnalyzerExpressionError::fatal)?;
        let call_target = target.clone();
        let ty = self.publish_dialogue_content_application_call(
            module,
            owner,
            &call_target,
            expected,
            &line_result,
            plan.is_some(),
        )?;
        let selection = match expected.map(|expected| expected.accepts(&ty)) {
            Some(true) => CheckedTypeSelection::Expected,
            None => CheckedTypeSelection::Inferred,
            Some(false) => {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
        };
        let shell = PreparedExpressionShell::value(ty, selection, EffectSet::new());
        let content = rich_text_check.content_id();
        let prepared = PreparedDialogueApplication::try_new(
            shell,
            target,
            application_patch,
            content,
            line_result,
            None,
        )
        .ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
        })?;
        self.facts
            .publish_checked_content(owner, rich_text_check)
            .map_err(AnalyzerExpressionError::fact)?;
        Ok(PreparedExpressionFact::DialogueApplication(prepared))
    }

    /// Checks a `#call(args)[body]` wrapper as one ordinary call-site
    /// application. The attached body is checked once into the affine content
    /// catalog; the call result is retained only when the ordinary resolver
    /// produced the exact accepted `DialogueContent` nominal.
    fn prepare_content_call_application(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirAttachedContentApplication,
        content: &arcweft_lang_hir::dialogue_application::HirDialogueContent,
        invocation: &arcweft_lang_hir::expr::HirCallInvocation,
        body_presence: HirAttachedContentBodyPresence,
    ) -> Result<PreparedExpressionFact, AnalyzerExpressionError> {
        let object_discriminator = match application.family() {
            HirAttachedContentApplicationFamily::ContentCall {
                evidence: HirContentCallSemanticEvidence::TextProxyObject {
                    nominal_discriminator:
                        arcweft_lang_hir::dialogue_application::HirRequiredContentCallNominalDiscriminator::Present(
                            nominal_discriminator,
                        ),
                },
                ..
            } => Some(*nominal_discriminator),
            HirAttachedContentApplicationFamily::ContentCall {
                evidence: HirContentCallSemanticEvidence::TextProxyObject {
                    nominal_discriminator:
                        arcweft_lang_hir::dialogue_application::HirRequiredContentCallNominalDiscriminator::Invalid,
                },
                ..
            } => return Err(AnalyzerExpressionError::rejected(owner)),
            _ => None,
        };

        // Attached content is evaluated bottom-up. A nested content call is a
        // real expression fact and must be present before either the object or
        // language call publishes its prepared graph node; otherwise the
        // affine graph sees an in-flight parent without its child fact.
        for child in content_expression_children(content) {
            self.evaluate_expression(context, child, None)?;
        }

        if let Some(discriminator) = object_discriminator {
            let type_argument = discriminator.argument();
            let type_expression = discriminator.source();
            let type_actual = self
                .resolve_type(discriminator.type_root(), false)
                .map_err(AnalyzerExpressionError::fatal)?;
            let TypeKind::ProjectNominal(type_nominal) = &type_actual else {
                return Err(AnalyzerExpressionError::rejected(owner));
            };
            let declaration = self
                .symbols
                .nominal(type_nominal.declaration())
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            let type_nominal = crate::final_analysis::CheckedProjectNominal::new(
                type_nominal.declaration().clone(),
                declaration.owner(),
                type_actual.semantic_identity_digest().map_err(|error| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::from(error))
                })?,
                type_nominal.arguments().to_vec(),
            );
            if !matches!(body_presence, HirAttachedContentBodyPresence::Present) {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
            let definition = self
                .text_proxies
                .as_ref()
                .ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?
                .definition_for_type(&type_actual)
                .cloned()
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            let schema = definition
                .callable_schema(self.catalogs.world().environment().compile_time_scalars())
                .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
            let mut type_coordinate = None;
            for group in schema.groups() {
                for parameter in group.parameters() {
                    if !matches!(
                        parameter.consumer(),
                        crate::callable::CallableParameterConsumer::Content(
                            crate::callable::CallableContentParameterConsumer::ObjectType
                        )
                    ) {
                        continue;
                    }
                    let coordinate = crate::callable::CallableParameterCoordinate::new(
                        group.index(),
                        parameter.index(),
                    );
                    if type_coordinate.replace(coordinate).is_some() {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                }
            }
            let type_coordinate =
                type_coordinate.ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            let type_value = PreparedProjectNominalTypeValueExpression::try_new(type_nominal)
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            let checked = self.publish_text_proxy_object_application(
                context,
                module,
                owner,
                invocation,
                &definition,
                (
                    type_argument,
                    type_expression,
                    type_actual,
                    type_value,
                    type_coordinate,
                ),
            )?;
            let expression_facts = self.facts.expressions().clone();
            let prepared_application = definition
                .prepare_application(invocation, &expression_facts)
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            let rich_text_check = RichTextContentChecker::check(module, content).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RichTextSourceQuery {
                    owner,
                })
            })?;
            let rich_text_check = self
                .prepare_rich_text_effect_plan(module, rich_text_check)
                .map_err(AnalyzerExpressionError::fatal)?;
            if !rich_text_check.report().is_valid() {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::InvalidRichTextContent {
                        owner,
                        diagnostics: rich_text_check
                            .report()
                            .diagnostics()
                            .to_vec()
                            .into_boxed_slice(),
                        declaration_diagnostics: self
                            .text_proxies
                            .as_ref()
                            .map(|catalog| catalog.diagnostics().to_vec().into_boxed_slice())
                            .unwrap_or_default(),
                    },
                ));
            }
            let identity = definition
                .callable_identity()
                .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
            let result_matches = match checked.result() {
                crate::final_analysis::CheckedExpressionResult::NonValue(
                    crate::final_analysis::CheckedNonValueExpressionResult::ContentEmission(actual),
                ) => *actual == identity,
                _ => false,
            };
            if !result_matches {
                return Err(AnalyzerExpressionError::rejected(owner));
            }
            let prepared =
                crate::final_analysis::PreparedContentApplication::try_new_content_emission(
                    owner,
                    checked.effects().clone(),
                    Some(rich_text_check.content_id()),
                    identity,
                    crate::final_analysis::PreparedContentEmission::ObjectSpan(
                        prepared_application,
                    ),
                )
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            self.facts
                .publish_checked_content(owner, rich_text_check)
                .map_err(AnalyzerExpressionError::fact)?;
            return Ok(PreparedExpressionFact::ContentApplication(prepared));
        }
        let expected_dialogue_content = self
            .catalogs
            .world
            .environment()
            .typecheck_env()
            .standard_dialogue_content_type()
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;

        let (checked, is_call) = match invocation.form() {
            HirCallInvocationForm::Value => {
                if !invocation.arguments().is_empty()
                    || invocation.explicit_type_application().spelling().is_some()
                {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                let target = invocation.callee().value_expression().ok_or_else(|| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
                let checked = self
                    .evaluate_expression(context, target, None)?
                    .into_complete()
                    .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
                if checked.value_type() != Some(&expected_dialogue_content) {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                (checked, false)
            }
            HirCallInvocationForm::Parenthesized => (
                self.check_content_call_expression_in_context(
                    context,
                    module,
                    owner,
                    invocation,
                    &AnalyzerExpressionExpectation::Unconstrained,
                )?,
                true,
            ),
        };

        let rich_text_check = if matches!(body_presence, HirAttachedContentBodyPresence::Present) {
            let rich_text_check = RichTextContentChecker::check(module, content).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::RichTextSourceQuery {
                    owner,
                })
            })?;
            let rich_text_check = self
                .prepare_rich_text_effect_plan(module, rich_text_check)
                .map_err(AnalyzerExpressionError::fatal)?;
            if !rich_text_check.report().is_valid() {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::InvalidRichTextContent {
                        owner,
                        diagnostics: rich_text_check
                            .report()
                            .diagnostics()
                            .to_vec()
                            .into_boxed_slice(),
                        declaration_diagnostics: self
                            .text_proxies
                            .as_ref()
                            .map(|catalog| catalog.diagnostics().to_vec().into_boxed_slice())
                            .unwrap_or_default(),
                    },
                ));
            }
            Some(rich_text_check)
        } else {
            None
        };

        // Language-owned body-bearing operations are ordinary callable
        // applications whose result is a typed non-value emission. Project
        // the selected catalog row and its mapped/defaulted operands here,
        // while the call fact is still the sole argument-mapping authority.
        if is_call
            && let crate::final_analysis::CheckedExpressionResult::NonValue(
                crate::final_analysis::CheckedNonValueExpressionResult::ContentEmission(operation),
            ) = checked.result()
            && matches!(
                operation,
                crate::callable::ContentCallableIdentity::Language { .. }
            )
        {
            let emission = self.prepare_language_content_emission(owner, content, *operation)?;
            let prepared =
                crate::final_analysis::PreparedContentApplication::try_new_content_emission(
                    owner,
                    checked.effects().clone(),
                    rich_text_check.as_ref().map(|check| check.content_id()),
                    *operation,
                    emission,
                )
                .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
            if let Some(rich_text_check) = rich_text_check {
                self.facts
                    .publish_checked_content(owner, rich_text_check)
                    .map_err(AnalyzerExpressionError::fact)?;
            }
            return Ok(PreparedExpressionFact::ContentApplication(prepared));
        }

        if is_call && checked.value_type() != Some(&expected_dialogue_content) {
            return Err(AnalyzerExpressionError::rejected(owner));
        }

        let checked_type = checked.value_type().cloned().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner,
            })
        })?;
        let checked_selection = checked.type_selection().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                owner,
            })
        })?;
        let shell = PreparedExpressionShell::value(
            checked_type,
            checked_selection,
            checked.effects().clone(),
        );
        let dialogue_content_type = self
            .catalogs
            .world
            .environment()
            .typecheck_env()
            .standard_dialogue_content_type();
        let prepared = crate::final_analysis::PreparedContentApplication::try_new(
            owner,
            shell,
            rich_text_check.as_ref().map(|check| check.content_id()),
            crate::final_analysis::PreparedContentEmission::ContentResult,
            dialogue_content_type.as_ref(),
        )
        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        if let Some(rich_text_check) = rich_text_check {
            self.facts
                .publish_checked_content(owner, rich_text_check)
                .map_err(AnalyzerExpressionError::fact)?;
        }
        Ok(PreparedExpressionFact::ContentApplication(prepared))
    }

    fn prepare_rich_text_effect_plan(
        &self,
        module: &HirModule,
        check: crate::checked_rich_text::PreparedCheckedRichTextCheck,
    ) -> Result<crate::checked_rich_text::PreparedCheckedRichTextCheck, FinalSemanticAnalysisError>
    {
        let effect_plan = self.prepared_dialogue_effect_plan(module, check.report())?;
        Ok(check.with_effect_plan(effect_plan))
    }

    fn prepared_dialogue_effect_plan(
        &self,
        module: &HirModule,
        rich_text: &crate::checked_rich_text::PreparedCheckedRichTextReport,
    ) -> Result<PreparedDialogueEffectPlan, FinalSemanticAnalysisError> {
        let mut effect_sites = Vec::new();
        for token in rich_text.content().tokens() {
            let PreparedCheckedDialogueToken::PointAction(action) = token else {
                continue;
            };
            match action {
                PreparedCheckedRichTextAction::Marker { .. } => {}
                PreparedCheckedRichTextAction::Host {
                    action:
                        event @ (CheckedDialogueHostEvent::TimedCue { .. }
                        | CheckedDialogueHostEvent::Call { .. }),
                    ..
                } => {
                    let id = CheckedDialogueEffectSiteOrdinal::new(
                        u32::try_from(effect_sites.len())
                            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                    );
                    let (expression, trigger) = match event {
                        CheckedDialogueHostEvent::TimedCue { at, call } => {
                            (*call, CheckedDialogueEffectTrigger::Delay(*at))
                        }
                        CheckedDialogueHostEvent::Call { call } => {
                            (*call, CheckedDialogueEffectTrigger::Content)
                        }
                        _ => unreachable!("the grouped checked RichText effect event is closed"),
                    };
                    let effect = self
                        .prepare_evaluated_effect_expression(module, expression)?
                        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                    effect_sites.push(PreparedDialogueEffectSite::new(id, trigger, effect));
                }
                PreparedCheckedRichTextAction::Control { .. }
                | PreparedCheckedRichTextAction::Host { .. } => {}
            }
        }
        Ok(PreparedDialogueEffectPlan::new(effect_sites))
    }

    fn publish_dialogue_content_application_call(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        target: &super::super::CheckedCharacterDialogueTarget,
        expected: Option<&TypeKind>,
        line_result: &TypeKind,
        has_line_plan: bool,
    ) -> Result<TypeKind, AnalyzerExpressionError> {
        let callee = match target {
            super::super::CheckedCharacterDialogueTarget::Character { character, .. } => {
                DialogueCalleeIdentity::Character {
                    character: character.clone(),
                }
            }
            super::super::CheckedCharacterDialogueTarget::Dialogue { ty, .. } => {
                DialogueCalleeIdentity::CharacterDialogue {
                    character: ty.character().clone(),
                }
            }
        };
        let prepared = PreparedCallCallee::Dialogue {
            id: DialogueCallableId::ContentApplication,
            callee: &callee,
            patch_context: CharacterDialoguePatchContext::ImmediateContentApplication,
            result: crate::callable::DialogueCallableResultContext::ContentApplication {
                line_result,
            },
        };
        let authority = CallResolverAuthority::accepted(
            self.project,
            module,
            self.symbols,
            self.catalogs.world,
        );
        let staged = self.staged_callables.as_ref().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        let request = CallResolverRequest::try_new_dialogue_application(
            prepared,
            &super::super::CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                presentation_character_owner: None,
                expression: owner,
                cancellation: self.control.cancellation(),
                prepared_continuations: self
                    .facts
                    .prepared_calls()
                    .map_err(AnalyzerExpressionError::fact)?,
                limits: &self.catalogs.callable_limits,
                implicit_extension_receiver: None,
            },
            &mut work,
        )
        .map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        let outcome = resolve_call_target(request);
        let candidates = match outcome {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => candidates,
            ResolveCallOutcome::Invariant(error) => {
                return Err(AnalyzerExpressionError::Call {
                    owner,
                    failure: crate::final_analysis::analyzer::calls::CallAnalysisFailure::Invariant(
                        crate::final_analysis::analyzer::calls::CallAnalysisInvariant::Constraint(
                            error,
                        ),
                    ),
                });
            }
            _ => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CallResolutionFailed { owner },
                ));
            }
        };
        let considered = candidates.into_shared().map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        let selected = considered.first().cloned().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CallResolutionFailed {
                owner,
            })
        })?;
        if selected.id()
            != &crate::callable::CallableCandidateId::Dialogue(
                DialogueCallableId::ContentApplication,
            )
            || !matches!(
                selected.schema().value_type(),
                Some(TypeKind::DialogueLine(_))
            )
        {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::CallResolutionFailed { owner },
            ));
        }
        self.publish_resolved_dialogue_application(
            module,
            owner,
            expected,
            target.expression(),
            target.ty(),
            has_line_plan,
            considered,
            work,
        )
    }

    fn check_dialogue_line_plan_output(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        application: ExprId,
        items: &[HirLinePlanItem],
    ) -> Result<TypeKind, AnalyzerExpressionError> {
        let mut output: Option<TypeKind> = None;
        let mut output_statements = BTreeSet::new();
        let mut check_statement = |statement: StmtId| {
            let module = self
                .module(statement.module())
                .map_err(AnalyzerExpressionError::fatal)?;
            self.evaluate_block_statement_uses(context, module, &[statement])?;
            let payload = module.resolve_stmt(statement).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            let arcweft_lang_hir::stmt::HirStmtEvaluationPlan::Value {
                kind: arcweft_lang_hir::stmt::HirStmtValuePlanKind::Out,
                expression: Some(value),
                ..
            } = payload.kind().evaluation_plan()
            else {
                return Ok(());
            };
            let transfer = self.topology.control_transfer_row(statement).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
            })?;
            if transfer.kind() != arcweft_lang_hir::project::HirControlTransferKind::Out
                || transfer.target().output_application() != Some(application)
            {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                ));
            }
            if !output_statements.insert(statement) {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                ));
            }
            let checked = self.evaluate_expression(context, value, output.as_ref())?;
            let checked_type = checked.value_type().ok_or_else(|| {
                AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: value },
                )
            })?;
            match &output {
                Some(expected) if !expected.accepts(checked_type) => {
                    Err(AnalyzerExpressionError::rejected(value))
                }
                Some(_) => Ok(()),
                None => {
                    output = Some(checked_type.clone());
                    Ok(())
                }
            }
        };
        let mut pending = vec![items];
        while let Some(items) = pending.pop() {
            for item in items {
                match item {
                    HirLinePlanItem::Init(statements) => {
                        for statement in statements {
                            check_statement(*statement)?;
                        }
                    }
                    HirLinePlanItem::Thread(statement)
                    | HirLinePlanItem::On(statement)
                    | HirLinePlanItem::Statement(statement)
                    | HirLinePlanItem::CancelRule(statement)
                    | HirLinePlanItem::Error(statement) => check_statement(*statement)?,
                    HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                        pending.push(items)
                    }
                }
            }
        }
        Ok(output.unwrap_or(TypeKind::Unit))
    }

    fn publish_dialogue_coordinates(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        application: &arcweft_lang_hir::dialogue_application::HirAttachedContentApplication,
    ) -> Result<
        Option<crate::callable::PreparedDialogueApplicationMetadataInventory>,
        AnalyzerExpressionError,
    > {
        let HirAttachedContentApplicationFamily::DialogueLine {
            target,
            plan: _,
            coordinates,
        } = application.family()
        else {
            return Err(AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::WrongPayloadFamily,
            ));
        };
        let target = module.resolve_expr(*target).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        if !matches!(target.kind(), HirExprKind::Call(_)) {
            return coordinates.is_empty().then_some(None).ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            });
        }
        let projection = module
            .dialogue_application_metadata_projection(owner)
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        let site = if coordinates.is_empty() {
            None
        } else {
            Some(
                module
                    .dialogue_line_sites()
                    .for_semantic_expr(owner)
                    .ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?,
            )
        };
        let mut prepared = Vec::with_capacity(projection.coordinates().len());
        for coordinate in projection.coordinates() {
            let site = site.ok_or_else(|| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
            let value = module.dialogue_coordinate_value(coordinate).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
            let (metadata_coordinate, ty, evidence, resolution) = match (coordinate.kind(), value) {
                (
                    arcweft_lang_hir::dialogue_application::HirDialogueCoordinateKind::Id,
                    arcweft_lang_hir::dialogue_application::HirDialogueCoordinateValueRef::IdRef(
                        reference,
                    ),
                ) => {
                    let (id, _) = site
                        .resolve_explicit_line_id(module.key(), &reference)
                        .ok_or_else(|| {
                            AnalyzerExpressionError::fatal(
                                FinalSemanticAnalysisError::WrongPayloadFamily,
                            )
                        })?;
                    (
                        crate::callable::DialogueApplicationMetadataCoordinate::Id,
                        TypeKind::entity_ref(EntityKind::DialogueLine),
                        crate::callable::PreparedDialogueApplicationMetadataEvidence::Id(
                            id.clone(),
                        ),
                        CheckedExpressionResolution::DialogueLineCoordinate(id),
                    )
                }
                (
                    arcweft_lang_hir::dialogue_application::HirDialogueCoordinateKind::TextKey,
                    arcweft_lang_hir::dialogue_application::HirDialogueCoordinateValueRef::IdRef(
                        reference,
                    ),
                ) => {
                    let key = site.resolve_explicit_text_key(&reference).ok_or_else(|| {
                        AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::WrongPayloadFamily,
                        )
                    })?;
                    (
                        crate::callable::DialogueApplicationMetadataCoordinate::TextKey,
                        TypeKind::entity_ref(EntityKind::Text),
                        crate::callable::PreparedDialogueApplicationMetadataEvidence::TextKey(
                            key.clone(),
                        ),
                        CheckedExpressionResolution::DialogueTextKeyCoordinate(key),
                    )
                }
                _ => {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                    ));
                }
            };
            prepared.push(
                crate::callable::PreparedDialogueApplicationMetadataArgument::seal(
                    coordinate.argument(),
                    coordinate.value(),
                    metadata_coordinate,
                    ty.clone(),
                    evidence,
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?,
            );
            self.facts
                .publish_new_expression(
                    coordinate.value(),
                    CheckedExpression::value(
                        ty,
                        CheckedTypeSelection::Inferred,
                        EffectSet::new(),
                        resolution,
                    ),
                )
                .map_err(|_| {
                    AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
                })?;
        }
        crate::callable::PreparedDialogueApplicationMetadataInventory::seal(
            &projection,
            prepared.into_boxed_slice(),
        )
        .map(Some)
        .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConditionalParameterDecision, ConditionalParameterError, decide_conditional_parameter,
    };
    use crate::final_analysis::CheckedCompileTimeValue;
    use arcweft_rich_text_schema::{
        RichTextCallableParameterPresence, RichTextDefaultValue, RichTextPropertyPredicate,
    };

    const PREDICATE_PROPERTY: u8 = 7;

    fn presence() -> RichTextCallableParameterPresence<u8> {
        RichTextCallableParameterPresence::Conditional {
            predicate: RichTextPropertyPredicate::BoolEquals {
                property: PREDICATE_PROPERTY,
                value: true,
            },
        }
    }

    #[test]
    fn conditional_presence_decision_covers_active_and_inactive_defaults() {
        let active = CheckedCompileTimeValue::scalar(
            crate::checked_compile_time::CheckedCompileTimeScalar::Bool(true),
        );
        let inactive = CheckedCompileTimeValue::scalar(
            crate::checked_compile_time::CheckedCompileTimeScalar::Bool(false),
        );
        let lookup_active = || |property| (property == PREDICATE_PROPERTY).then_some(&active);
        let lookup_inactive = || |property| (property == PREDICATE_PROPERTY).then_some(&inactive);

        assert_eq!(
            decide_conditional_parameter(presence(), None, false, lookup_active()),
            Err(ConditionalParameterError::MissingDefault)
        );
        assert_eq!(
            decide_conditional_parameter(
                presence(),
                Some(RichTextDefaultValue::RatioMilli(350)),
                false,
                lookup_active(),
            ),
            Ok(ConditionalParameterDecision::Materialize(
                RichTextDefaultValue::RatioMilli(350)
            ))
        );
        assert_eq!(
            decide_conditional_parameter(presence(), None, true, lookup_active()),
            Ok(ConditionalParameterDecision::AcceptExplicit)
        );
        assert_eq!(
            decide_conditional_parameter(
                presence(),
                Some(RichTextDefaultValue::RatioMilli(350)),
                false,
                lookup_inactive(),
            ),
            Ok(ConditionalParameterDecision::Omit)
        );
        assert_eq!(
            decide_conditional_parameter(presence(), None, true, lookup_inactive()),
            Err(ConditionalParameterError::InactiveExplicit)
        );
    }
}
