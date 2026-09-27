//! Common outcome construction for checked `fmt` expressions.
//!
//! The plan evaluator and AWBC VM supply their own verified operand execution
//! and type projections. This function owns the resulting formatted value so
//! the two engines cannot silently choose different failure policies.

use arcweft_id::LocaleTag;
use fixed_decimal::{Decimal, FloatPrecision};
use icu_decimal::DecimalFormatter;
use icu_decimal::options::DecimalFormatterOptions;
use icu_experimental::dimension::currency::{
    CurrencyType, formatter::CurrencyFormatter, options::CurrencyFormatterOptions,
};
use icu_locale::Locale;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::pattern::{
    RuntimeBuiltinVariantCaseIdentity, RuntimeCheckedType, RuntimeSemanticTypeId,
};
use crate::plan::{RuntimePlan, RuntimePlanTypeProjection};
use crate::runtime_id::RuntimePlanTypeId;

use super::{
    RuntimeDialogueContentValue, RuntimeDialogueContentValueError,
    RuntimeDialogueFormattedFailureSelection, RuntimeDialogueFormattedOutcome,
    RuntimeDialogueFormattedSuccess, RuntimeDialogueFormattedValue,
    RuntimeDialogueFormattedValueError, RuntimeFmtParameterId, RuntimeInlineTextValue,
    RuntimeInlineTextValueError, RuntimeNominalRecordLayout, RuntimeNominalRecordValue,
    RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
};

/// Exact bundled ICU algorithms and CLDR data required by Core formatting.
/// Serialized format continuations reject a different data release.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeFormatDataIdentity {
    IcuDecimal230Experimental060Locale231Cldr4821,
}

impl RuntimeFormatDataIdentity {
    pub const CURRENT: Self = Self::IcuDecimal230Experimental060Locale231Cldr4821;

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IcuDecimal230Experimental060Locale231Cldr4821 => {
                "icu-decimal-2.3.0+experimental-0.6.0+locale-2.3.1+cldr-48.2.1"
            }
        }
    }
}

/// Immutable locale selected for one deterministic formatter attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFormatContext {
    active_locale: LocaleTag,
    data_identity: RuntimeFormatDataIdentity,
}

impl RuntimeFormatContext {
    #[must_use]
    pub fn new(active_locale: LocaleTag) -> Self {
        Self {
            active_locale,
            data_identity: RuntimeFormatDataIdentity::CURRENT,
        }
    }

    #[must_use]
    pub const fn active_locale(&self) -> &LocaleTag {
        &self.active_locale
    }

    #[must_use]
    pub const fn data_identity(&self) -> RuntimeFormatDataIdentity {
        self.data_identity
    }

    #[must_use]
    pub const fn has_current_data(&self) -> bool {
        matches!(
            self.data_identity,
            RuntimeFormatDataIdentity::IcuDecimal230Experimental060Locale231Cldr4821
        )
    }
}

impl Default for RuntimeFormatContext {
    fn default() -> Self {
        Self::new(LocaleTag::try_new("ja-JP").expect("the language default is canonical"))
    }
}

/// The admitted display type, projected from the owning executable's type
/// table. The semantic type of an Option is its Some payload's scalar type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFormatPrimaryKind {
    Scalar(RuntimeSemanticTypeId),
    Content,
    OptionScalar(RuntimeSemanticTypeId),
    /// A selected project DisplayText method has already consumed the style,
    /// locale, and currency options and returned this Content.
    ProjectContent,
    /// A selected project method returned Content for Option::Some; Option::None
    /// is rendered by the checked `none` operand or failure policy.
    OptionProjectContent,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeFormatAttemptError {
    #[error("fmt operand {0:?} is duplicated")]
    DuplicateParameter(RuntimeFmtParameterId),
    #[error("fmt expression has no selected primary value")]
    MissingPrimary,
    #[error("fmt operand {0:?} has an invalid runtime value")]
    InvalidParameter(RuntimeFmtParameterId),
    #[error("fmt expression selects more than one failure policy")]
    ConflictingFailurePolicy,
    #[error("project DisplayText context does not match its accepted record schema")]
    InvalidProjectContext,
    #[error("project DisplayText returned an invalid Result<Content, DisplayError>")]
    InvalidProjectResult,
    #[error(transparent)]
    Content(#[from] RuntimeDialogueContentValueError),
    #[error(transparent)]
    Inline(#[from] RuntimeInlineTextValueError),
    #[error(transparent)]
    Formatted(#[from] RuntimeDialogueFormattedValueError),
}

/// Builds the standard, schema-checked context passed to a selected project
/// `DisplayText` implementation. A dynamic invalid locale is a recoverable
/// formatting failure, so the caller can apply its already selected policy.
pub fn project_display_context(
    layout: &RuntimeNominalRecordLayout,
    context: &RuntimeFormatContext,
    operands: &[(RuntimeFmtParameterId, Option<RuntimeValue>)],
) -> Result<Result<RuntimeValue, String>, RuntimeFormatAttemptError> {
    let mut selected = [false; 9];
    let mut values: [Option<&RuntimeValue>; 9] = std::array::from_fn(|_| None);
    for (parameter, value) in operands {
        let index = parameter.index();
        if std::mem::replace(&mut selected[index], true) {
            return Err(RuntimeFormatAttemptError::DuplicateParameter(*parameter));
        }
        values[index] = value.as_ref();
    }
    let style = format_string_operand(&values, RuntimeFmtParameterId::Style)?;
    let currency = format_string_operand(&values, RuntimeFmtParameterId::Currency)?;
    let locale = format_string_operand(&values, RuntimeFmtParameterId::Locale)?;
    let locale = match locale {
        Some(locale) => match LocaleTag::canonicalize(locale) {
            Ok(locale) => locale,
            Err(error) => return Ok(Err(format!("invalid fmt locale: {error}"))),
        },
        None => context.active_locale().clone(),
    };
    let fields = layout
        .fields()
        .iter()
        .map(|field| match field.name() {
            Some("locale") => Ok(RuntimeValue::String(locale.as_str().to_owned())),
            Some("style") => Ok(style.map_or_else(RuntimeValue::option_none, |value| {
                RuntimeValue::option_some(RuntimeValue::String(value.to_owned()))
            })),
            Some("currency") => Ok(currency.map_or_else(RuntimeValue::option_none, |value| {
                RuntimeValue::option_some(RuntimeValue::String(value.to_owned()))
            })),
            _ => Err(RuntimeFormatAttemptError::InvalidProjectContext),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if layout.fields().len() != 3 {
        return Err(RuntimeFormatAttemptError::InvalidProjectContext);
    }
    let record = RuntimeNominalRecordValue::try_from_accepted_layout(layout, fields)
        .map_err(|_| RuntimeFormatAttemptError::InvalidProjectContext)?;
    Ok(Ok(RuntimeValue::NominalRecord(record)))
}

/// Converts a project method's admitted result into the common formatter
/// success or recoverable failure consumed by native, pure, and AWBC paths.
pub fn project_display_result(
    result: RuntimeValue,
    error_layout: &RuntimeNominalRecordLayout,
) -> Result<Result<RuntimeValue, String>, RuntimeFormatAttemptError> {
    let (case, payload) = result
        .try_into_builtin_variant_case()
        .map_err(|_| RuntimeFormatAttemptError::InvalidProjectResult)?;
    let payload = payload.ok_or(RuntimeFormatAttemptError::InvalidProjectResult)?;
    match case {
        RuntimeBuiltinVariantCaseIdentity::ResultOk => {
            RuntimeDialogueContentValue::try_from_runtime_value(&payload)?;
            Ok(Ok(payload))
        }
        RuntimeBuiltinVariantCaseIdentity::ResultErr => {
            let record = payload
                .as_nominal_record()
                .ok_or(RuntimeFormatAttemptError::InvalidProjectResult)?;
            record
                .validate_against_layout(error_layout)
                .map_err(|_| RuntimeFormatAttemptError::InvalidProjectResult)?;
            let [RuntimeValue::String(message)] = record.fields() else {
                return Err(RuntimeFormatAttemptError::InvalidProjectResult);
            };
            Ok(Err(message.clone()))
        }
        _ => Err(RuntimeFormatAttemptError::InvalidProjectResult),
    }
}

/// Projects one sealed plan record domain into the same checked layout used by
/// native and AWBC value admission. No field names or ordinals are inferred
/// from authored source.
pub(crate) fn project_display_layout(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
) -> Result<RuntimeNominalRecordLayout, RuntimeFormatAttemptError> {
    let declaration = plan
        .type_table()
        .get(ty)
        .ok_or(RuntimeFormatAttemptError::InvalidProjectContext)?;
    let RuntimePlanTypeProjection::Nominal {
        nominal,
        layout,
        arguments,
    } = declaration.projection()
    else {
        return Err(RuntimeFormatAttemptError::InvalidProjectContext);
    };
    let domain = plan
        .nominal_record_domains()
        .get(ty)
        .ok_or(RuntimeFormatAttemptError::InvalidProjectContext)?;
    let checked = |field_ty| {
        plan.checked_type(field_ty)
            .ok()
            .flatten()
            .ok_or(RuntimeFormatAttemptError::InvalidProjectContext)
    };
    let arguments = arguments
        .iter()
        .copied()
        .map(checked)
        .collect::<Result<Vec<_>, _>>()?;
    let fields = domain
        .fields()
        .iter()
        .map(|field| {
            Ok(super::RuntimeNominalRecordLayoutField::new(
                field.field(),
                field.name().map(str::to_owned),
                checked(field.ty())?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeFormatAttemptError>>()?;
    RuntimeNominalRecordLayout::try_from_checked_projection(
        nominal.clone(),
        declaration.semantic_identity(),
        *layout,
        domain.shape(),
        arguments,
        fields,
    )
    .map_err(|_| RuntimeFormatAttemptError::InvalidProjectContext)
}

/// Folds one source-ordered, once-evaluated formatter attempt. `None` denotes
/// only a previously classified recoverable expression failure. Fatal errors
/// must be returned by the evaluator before this function is called.
pub fn finish_format_content_attempt(
    context: &RuntimeFormatContext,
    primary_kind: RuntimeFormatPrimaryKind,
    operands: &[(RuntimeFmtParameterId, Option<RuntimeValue>)],
    first_recoverable: Option<&str>,
) -> Result<RuntimeDialogueFormattedValue, RuntimeFormatAttemptError> {
    let mut selected = [false; 9];
    let mut values: [Option<&RuntimeValue>; 9] = std::array::from_fn(|_| None);
    for (parameter, value) in operands {
        let index = parameter.index();
        if std::mem::replace(&mut selected[index], true) {
            return Err(RuntimeFormatAttemptError::DuplicateParameter(*parameter));
        }
        values[index] = value.as_ref();
    }
    if !selected[RuntimeFmtParameterId::Value.index()] {
        return Err(RuntimeFormatAttemptError::MissingPrimary);
    }
    let policy_count = [
        RuntimeFmtParameterId::OnError,
        RuntimeFmtParameterId::Fallback,
        RuntimeFmtParameterId::DiscardError,
    ]
    .iter()
    .filter(|parameter| selected[parameter.index()])
    .count();
    if policy_count > 1 {
        return Err(RuntimeFormatAttemptError::ConflictingFailurePolicy);
    }
    let failure = if let Some(value) = values[RuntimeFmtParameterId::OnError.index()] {
        RuntimeDialogueFormattedFailureSelection::OnError(value.clone())
    } else if let Some(value) = values[RuntimeFmtParameterId::Fallback.index()] {
        let RuntimeValue::String(text) = value else {
            return Err(RuntimeFormatAttemptError::InvalidParameter(
                RuntimeFmtParameterId::Fallback,
            ));
        };
        RuntimeDialogueFormattedFailureSelection::Fallback(text.clone())
    } else if let Some(value) = values[RuntimeFmtParameterId::DiscardError.index()] {
        let RuntimeValue::Bool(discard) = value else {
            return Err(RuntimeFormatAttemptError::InvalidParameter(
                RuntimeFmtParameterId::DiscardError,
            ));
        };
        RuntimeDialogueFormattedFailureSelection::Discard(*discard)
    } else {
        RuntimeDialogueFormattedFailureSelection::Inherit
    };

    let outcome = if let Some(reason) = first_recoverable {
        RuntimeDialogueFormattedOutcome::Failure {
            reason: reason.to_owned(),
            value_plain: values[RuntimeFmtParameterId::Value.index()]
                .and_then(|primary| {
                    let none_text = match values[RuntimeFmtParameterId::NoneValue.index()] {
                        Some(RuntimeValue::String(text)) => Some(text.as_str()),
                        _ => None,
                    };
                    render_primary(primary_kind, primary, none_text)
                        .ok()
                        .and_then(Result::ok)
                })
                .and_then(|rendered| match rendered {
                    RuntimeDialogueFormattedSuccess::Text(text) => Some(text),
                    RuntimeDialogueFormattedSuccess::Content(_) => None,
                }),
        }
    } else {
        format_success(context, primary_kind, &values)?
    };
    RuntimeDialogueFormattedValue::try_new(outcome, failure).map_err(Into::into)
}

fn format_success(
    context: &RuntimeFormatContext,
    primary_kind: RuntimeFormatPrimaryKind,
    values: &[Option<&RuntimeValue>; 9],
) -> Result<RuntimeDialogueFormattedOutcome, RuntimeFormatAttemptError> {
    let primary = values[RuntimeFmtParameterId::Value.index()]
        .ok_or(RuntimeFormatAttemptError::MissingPrimary)?;
    let color = match values[RuntimeFmtParameterId::Color.index()] {
        Some(RuntimeValue::Color(color)) => Some(*color),
        Some(_) => {
            return Err(RuntimeFormatAttemptError::InvalidParameter(
                RuntimeFmtParameterId::Color,
            ));
        }
        None => None,
    };
    let none_text = match values[RuntimeFmtParameterId::NoneValue.index()] {
        Some(RuntimeValue::String(text)) => Some(text.as_str()),
        Some(_) => {
            return Err(RuntimeFormatAttemptError::InvalidParameter(
                RuntimeFmtParameterId::NoneValue,
            ));
        }
        None => None,
    };
    let rendered = render_primary(primary_kind, primary, none_text)?;
    let style = format_string_operand(values, RuntimeFmtParameterId::Style)?;
    let locale = format_string_operand(values, RuntimeFmtParameterId::Locale)?;
    let currency = format_string_operand(values, RuntimeFmtParameterId::Currency)?;
    if matches!(
        primary_kind,
        RuntimeFormatPrimaryKind::ProjectContent | RuntimeFormatPrimaryKind::OptionProjectContent
    ) {
        return Ok(match rendered {
            Ok(value) => RuntimeDialogueFormattedOutcome::Success { value, color },
            Err(reason) => RuntimeDialogueFormattedOutcome::Failure {
                reason,
                value_plain: None,
            },
        });
    }
    let numeric_mode = match (style, currency) {
        (Some("number") | None, Some(code)) => Some(Some(code)),
        (Some("number"), None) => Some(None),
        (None, None) if locale.is_none() => None,
        (None, None) => {
            return Ok(format_failure(
                "fmt locale requires number or currency formatting",
                &rendered,
            ));
        }
        (Some(_), _) => {
            return Ok(format_failure(
                "fmt style is not supported by Core formatting",
                &rendered,
            ));
        }
    };
    let rendered = if let Some(currency) = numeric_mode {
        match format_numeric(context, primary_kind, primary, none_text, locale, currency) {
            Ok(text) => {
                let semantic_type = match primary_kind {
                    RuntimeFormatPrimaryKind::Scalar(semantic_type)
                    | RuntimeFormatPrimaryKind::OptionScalar(semantic_type) => semantic_type,
                    RuntimeFormatPrimaryKind::Content
                    | RuntimeFormatPrimaryKind::ProjectContent
                    | RuntimeFormatPrimaryKind::OptionProjectContent => {
                        unreachable!("numeric formatter rejected Content")
                    }
                };
                let text = RuntimeInlineTextValue::try_new(semantic_type, text)?;
                Ok(RuntimeDialogueFormattedSuccess::Text(
                    text.text().to_owned(),
                ))
            }
            Err(reason) => return Ok(format_failure(&reason, &rendered)),
        }
    } else {
        rendered
    };
    Ok(match rendered {
        Ok(value) => RuntimeDialogueFormattedOutcome::Success { value, color },
        Err(reason) => RuntimeDialogueFormattedOutcome::Failure {
            reason,
            value_plain: None,
        },
    })
}

fn format_string_operand<'a>(
    values: &'a [Option<&RuntimeValue>; 9],
    parameter: RuntimeFmtParameterId,
) -> Result<Option<&'a str>, RuntimeFormatAttemptError> {
    match values[parameter.index()] {
        Some(RuntimeValue::String(text)) => Ok(Some(text)),
        Some(_) => Err(RuntimeFormatAttemptError::InvalidParameter(parameter)),
        None => Ok(None),
    }
}

fn format_failure(
    reason: &str,
    rendered: &Result<RuntimeDialogueFormattedSuccess, String>,
) -> RuntimeDialogueFormattedOutcome {
    RuntimeDialogueFormattedOutcome::Failure {
        reason: reason.to_owned(),
        value_plain: match rendered {
            Ok(RuntimeDialogueFormattedSuccess::Text(text)) => Some(text.clone()),
            Ok(RuntimeDialogueFormattedSuccess::Content(_)) | Err(_) => None,
        },
    }
}

#[derive(Clone, Copy)]
enum NumericKind {
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
}

impl NumericKind {
    fn from_semantic_type(semantic_type: RuntimeSemanticTypeId) -> Option<Self> {
        const SIGNED: [RuntimeSignedIntWidth; 6] = [
            RuntimeSignedIntWidth::I8,
            RuntimeSignedIntWidth::I16,
            RuntimeSignedIntWidth::I32,
            RuntimeSignedIntWidth::I64,
            RuntimeSignedIntWidth::I128,
            RuntimeSignedIntWidth::ISize,
        ];
        const UNSIGNED: [RuntimeUnsignedIntWidth; 6] = [
            RuntimeUnsignedIntWidth::U8,
            RuntimeUnsignedIntWidth::U16,
            RuntimeUnsignedIntWidth::U32,
            RuntimeUnsignedIntWidth::U64,
            RuntimeUnsignedIntWidth::U128,
            RuntimeUnsignedIntWidth::USize,
        ];
        SIGNED
            .into_iter()
            .find(|width| {
                RuntimeCheckedType::Signed(*width).semantic_identity_digest() == semantic_type
            })
            .map(Self::Signed)
            .or_else(|| {
                UNSIGNED
                    .into_iter()
                    .find(|width| {
                        RuntimeCheckedType::Unsigned(*width).semantic_identity_digest()
                            == semantic_type
                    })
                    .map(Self::Unsigned)
            })
            .or_else(|| {
                (RuntimeCheckedType::F32.semantic_identity_digest() == semantic_type)
                    .then_some(Self::F32)
            })
            .or_else(|| {
                (RuntimeCheckedType::F64.semantic_identity_digest() == semantic_type)
                    .then_some(Self::F64)
            })
    }

    fn decimal(self, value: &RuntimeValue) -> Result<Decimal, String> {
        match (self, value) {
            (Self::Signed(width), RuntimeValue::Int(value)) if value.width() == width => value
                .as_i128()
                .to_string()
                .parse()
                .map_err(|error| format!("fmt integer is outside decimal limits: {error}")),
            (Self::Unsigned(width), RuntimeValue::UInt(value)) if value.width() == width => value
                .as_u128()
                .to_string()
                .parse()
                .map_err(|error| format!("fmt integer is outside decimal limits: {error}")),
            (Self::F32, RuntimeValue::F32(value)) if value.is_finite() => {
                // f32 must take its own shortest round-trip text. Widening to
                // f64 first would expose the binary approximation of 0.1f32.
                value
                    .to_string()
                    .parse()
                    .map_err(|error| format!("fmt f32 is outside decimal limits: {error}"))
            }
            (Self::F64, RuntimeValue::F64(value)) if value.is_finite() => {
                Decimal::try_from_f64(*value, FloatPrecision::RoundTrip)
                    .map_err(|error| format!("fmt f64 is outside decimal limits: {error}"))
            }
            (Self::F32, RuntimeValue::F32(_)) => Err("fmt cannot format non-finite f32".to_owned()),
            (Self::F64, RuntimeValue::F64(_)) => Err("fmt cannot format non-finite f64".to_owned()),
            _ => Err("fmt numeric value does not match its checked type".to_owned()),
        }
    }
}

fn format_numeric(
    context: &RuntimeFormatContext,
    primary_kind: RuntimeFormatPrimaryKind,
    primary: &RuntimeValue,
    none_text: Option<&str>,
    explicit_locale: Option<&str>,
    currency: Option<&str>,
) -> Result<String, String> {
    let semantic_type = match primary_kind {
        RuntimeFormatPrimaryKind::Scalar(semantic_type)
        | RuntimeFormatPrimaryKind::OptionScalar(semantic_type) => semantic_type,
        RuntimeFormatPrimaryKind::Content
        | RuntimeFormatPrimaryKind::ProjectContent
        | RuntimeFormatPrimaryKind::OptionProjectContent => {
            return Err("fmt numeric style requires a numeric value".to_owned());
        }
    };
    let kind = NumericKind::from_semantic_type(semantic_type)
        .ok_or_else(|| "fmt numeric style requires a numeric value".to_owned())?;
    let locale = explicit_locale.map_or_else(
        || Ok(context.active_locale.clone()),
        |value| {
            LocaleTag::canonicalize(value).map_err(|error| format!("invalid fmt locale: {error}"))
        },
    )?;
    let locale: Locale = locale
        .as_str()
        .parse()
        .map_err(|error| format!("invalid ICU locale: {error}"))?;
    let currency = currency
        .map(|code| {
            if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                return Err("fmt currency must be a three-letter ASCII code".to_owned());
            }
            code.to_ascii_uppercase()
                .parse::<CurrencyType>()
                .map_err(|error| format!("invalid fmt currency: {error}"))
        })
        .transpose()?;
    let primary = match primary_kind {
        RuntimeFormatPrimaryKind::OptionScalar(_) => {
            match primary.clone().try_into_builtin_variant_case() {
                Ok((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => value,
                Ok((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => {
                    return none_text.map(ToOwned::to_owned).ok_or_else(|| {
                        "fmt received Option::None without a `none` operand".to_owned()
                    });
                }
                _ => return Err("fmt numeric Option has an invalid runtime value".to_owned()),
            }
        }
        RuntimeFormatPrimaryKind::Scalar(_) => primary.clone(),
        RuntimeFormatPrimaryKind::Content
        | RuntimeFormatPrimaryKind::ProjectContent
        | RuntimeFormatPrimaryKind::OptionProjectContent => {
            unreachable!("checked above")
        }
    };
    let value = kind.decimal(&primary)?;
    let formatted = if let Some(code) = currency {
        CurrencyFormatter::try_new_symbol(locale.into(), code, CurrencyFormatterOptions::default())
            .map_err(|error| format!("fmt currency data is unavailable: {error}"))?
            .format_fixed_decimal(&value)
            .to_string()
    } else {
        DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default())
            .map_err(|error| format!("fmt number data is unavailable: {error}"))?
            .format(&value)
            .to_string()
    };
    Ok(formatted)
}

fn render_primary(
    kind: RuntimeFormatPrimaryKind,
    value: &RuntimeValue,
    none_text: Option<&str>,
) -> Result<Result<RuntimeDialogueFormattedSuccess, String>, RuntimeFormatAttemptError> {
    let (kind, value) = match kind {
        RuntimeFormatPrimaryKind::OptionScalar(semantic_type) => {
            match value.clone().try_into_builtin_variant_case() {
                Ok((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => {
                    (RuntimeFormatPrimaryKind::Scalar(semantic_type), value)
                }
                Ok((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => {
                    return Ok(none_text.map_or_else(
                        || Err("fmt received Option::None without a `none` operand".to_owned()),
                        |text| Ok(RuntimeDialogueFormattedSuccess::Text(text.to_owned())),
                    ));
                }
                _ => {
                    return Err(RuntimeFormatAttemptError::InvalidParameter(
                        RuntimeFmtParameterId::Value,
                    ));
                }
            }
        }
        RuntimeFormatPrimaryKind::OptionProjectContent => {
            match value.clone().try_into_builtin_variant_case() {
                Ok((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => {
                    (RuntimeFormatPrimaryKind::ProjectContent, value)
                }
                Ok((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => {
                    return Ok(none_text.map_or_else(
                        || Err("fmt received Option::None without a `none` operand".to_owned()),
                        |text| Ok(RuntimeDialogueFormattedSuccess::Text(text.to_owned())),
                    ));
                }
                _ => {
                    return Err(RuntimeFormatAttemptError::InvalidParameter(
                        RuntimeFmtParameterId::Value,
                    ));
                }
            }
        }
        kind => (kind, value.clone()),
    };
    match kind {
        RuntimeFormatPrimaryKind::Content | RuntimeFormatPrimaryKind::ProjectContent => {
            let content = RuntimeDialogueContentValue::try_from_runtime_value(&value)?;
            Ok(Ok(RuntimeDialogueFormattedSuccess::Content(Box::new(
                content,
            ))))
        }
        RuntimeFormatPrimaryKind::Scalar(semantic_type) => Ok(
            match RuntimeInlineTextValue::try_format_runtime_value(semantic_type, &value) {
                Ok(text) => Ok(RuntimeDialogueFormattedSuccess::Text(
                    text.text().to_owned(),
                )),
                Err(error @ RuntimeInlineTextValueError::StringLimit { .. }) => {
                    return Err(error.into());
                }
                Err(error) => Err(error.to_string()),
            },
        ),
        RuntimeFormatPrimaryKind::OptionScalar(_)
        | RuntimeFormatPrimaryKind::OptionProjectContent => {
            unreachable!("Option was decoded above")
        }
    }
}

#[cfg(test)]
#[path = "format_content/tests.rs"]
mod tests;
