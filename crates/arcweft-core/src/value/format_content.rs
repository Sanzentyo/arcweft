//! Common outcome construction for checked `fmt` expressions.
//!
//! The plan evaluator and AWBC VM supply their own verified operand execution
//! and type projections. This function owns the resulting formatted value so
//! the two engines cannot silently choose different failure policies.

use thiserror::Error;

use crate::pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeSemanticTypeId};

use super::{
    RuntimeDialogueContentValue, RuntimeDialogueContentValueError,
    RuntimeDialogueFormattedFailureSelection, RuntimeDialogueFormattedOutcome,
    RuntimeDialogueFormattedSuccess, RuntimeDialogueFormattedValue,
    RuntimeDialogueFormattedValueError, RuntimeFmtParameterId, RuntimeInlineTextValue,
    RuntimeInlineTextValueError, RuntimeValue,
};

/// The admitted display type, projected from the owning executable's type
/// table. The semantic type of an Option is its Some payload's scalar type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFormatPrimaryKind {
    Scalar(RuntimeSemanticTypeId),
    Content,
    OptionScalar(RuntimeSemanticTypeId),
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
    #[error(transparent)]
    Content(#[from] RuntimeDialogueContentValueError),
    #[error(transparent)]
    Inline(#[from] RuntimeInlineTextValueError),
    #[error(transparent)]
    Formatted(#[from] RuntimeDialogueFormattedValueError),
}

/// Folds one source-ordered, once-evaluated formatter attempt. `None` denotes
/// only a previously classified recoverable expression failure. Fatal errors
/// must be returned by the evaluator before this function is called.
pub fn finish_format_content_attempt(
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
            value_plain: None,
        }
    } else {
        format_success(primary_kind, &values)?
    };
    RuntimeDialogueFormattedValue::try_new(outcome, failure).map_err(Into::into)
}

fn format_success(
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
    let mut unsupported = None;
    for (parameter, supported) in [
        (RuntimeFmtParameterId::Style, Some("number")),
        (RuntimeFmtParameterId::Locale, None),
        (RuntimeFmtParameterId::Currency, None),
    ] {
        if let Some(value) = values[parameter.index()] {
            let RuntimeValue::String(text) = value else {
                return Err(RuntimeFormatAttemptError::InvalidParameter(parameter));
            };
            if supported.is_none_or(|supported| text != supported) {
                unsupported = Some(parameter);
                break;
            }
        }
    }
    let rendered = if let Some(parameter) = unsupported {
        Err(format!(
            "fmt {:?} option is not supported by Core formatting",
            parameter
        ))
    } else {
        render_primary(primary_kind, primary, none_text)?
    };
    Ok(match rendered {
        Ok(value) => RuntimeDialogueFormattedOutcome::Success { value, color },
        Err(reason) => RuntimeDialogueFormattedOutcome::Failure {
            reason,
            value_plain: None,
        },
    })
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
        kind => (kind, value.clone()),
    };
    match kind {
        RuntimeFormatPrimaryKind::Content => {
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
        RuntimeFormatPrimaryKind::OptionScalar(_) => unreachable!("Option was decoded above"),
    }
}
