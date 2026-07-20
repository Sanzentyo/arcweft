//! Shared argument coordinates used by schemas, facts, and query results.

use arcweft_lang_syntax::expr::{CallArg, Expr};

use super::{
    CallableGroupIndex, CallableParameter, CallableParameterIndex, CallableParameterPassing,
    CallableParameterPresence, CallableSignatureSchema, SpreadArgumentPolicy,
    UnknownNamedArgumentPolicy,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterCoordinate {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
}

impl CallableParameterCoordinate {
    pub const fn new(group: CallableGroupIndex, parameter: CallableParameterIndex) -> Self {
        Self { group, parameter }
    }
    pub const fn group(self) -> CallableGroupIndex {
        self.group
    }
    pub const fn parameter(self) -> CallableParameterIndex {
        self.parameter
    }
}

pub(crate) fn call_shape_is_viable(
    schema: &CallableSignatureSchema,
    group: CallableGroupIndex,
    arguments: &[CallArg],
) -> bool {
    call_shape_is_viable_with_implicit(schema, group, arguments, None)
}

pub(crate) fn call_shape_is_viable_with_implicit(
    schema: &CallableSignatureSchema,
    group: CallableGroupIndex,
    arguments: &[CallArg],
    implicit: Option<CallableParameterIndex>,
) -> bool {
    let Some(group) = schema.group(group) else {
        return false;
    };
    let parameters = group.parameters();
    let mut provided = vec![false; parameters.len()];
    if let Some(implicit) = implicit {
        let Some(slot) = provided.get_mut(implicit.get()) else {
            return false;
        };
        *slot = true;
    }
    let mut positional = 0usize;
    for argument in arguments {
        match argument {
            CallArg::Positional(_) => {
                if !mark_viable_positional(parameters, &mut provided, &mut positional) {
                    return false;
                }
            }
            CallArg::Named { name, .. } => {
                let Some(parameter) = named_parameter(parameters, name) else {
                    if schema.argument_policy().unknown_named()
                        == UnknownNamedArgumentPolicy::Reject
                    {
                        return false;
                    }
                    continue;
                };
                if parameter.passing() != CallableParameterPassing::RestNamed {
                    let index = parameter.index().get();
                    if provided[index] {
                        return false;
                    }
                    provided[index] = true;
                }
            }
            CallArg::Spread { value } => match schema.argument_policy().spread() {
                SpreadArgumentPolicy::Reject => return false,
                SpreadArgumentPolicy::Unchecked => {}
                SpreadArgumentPolicy::FixedLiteralOnly => {
                    let Some(count) = fixed_literal_spread_slot_count(value) else {
                        return false;
                    };
                    if (0..count).any(|_| {
                        !mark_viable_positional(parameters, &mut provided, &mut positional)
                    }) {
                        return false;
                    }
                }
                SpreadArgumentPolicy::TypedRest => {
                    if let Some(count) = fixed_literal_spread_slot_count(value) {
                        if (0..count).any(|_| {
                            !mark_viable_positional(parameters, &mut provided, &mut positional)
                        }) {
                            return false;
                        }
                    } else if !parameters.iter().any(|parameter| {
                        parameter.passing() == CallableParameterPassing::RestPositional
                    }) || required_fixed_parameter_is_missing(parameters, &provided)
                    {
                        return false;
                    }
                }
            },
        }
    }
    !required_fixed_parameter_is_missing(parameters, &provided)
}

pub(crate) fn data_last_unsupported_spread_reason(arguments: &[CallArg]) -> Option<&'static str> {
    if !arguments.iter().any(CallArg::is_spread) {
        return None;
    }
    if arguments
        .iter()
        .filter_map(|argument| match argument {
            CallArg::Spread { value } => Some(value.as_ref()),
            CallArg::Positional(_) | CallArg::Named { .. } => None,
        })
        .all(|value| fixed_literal_spread_slot_count(value).is_some())
    {
        return None;
    }
    if arguments
        .iter()
        .filter(|argument| argument.is_spread())
        .count()
        > 1
    {
        return Some(
            "multiple spread arguments are not supported in data-last fallback; runtime expansion ranges are not specified",
        );
    }
    let spread_index = arguments.iter().position(CallArg::is_spread)?;
    if arguments
        .iter()
        .skip(spread_index + 1)
        .any(|argument| !argument.is_spread())
    {
        return Some(
            "spread arguments cannot be followed by fixed data-last fallback arguments; runtime argument order is not specified",
        );
    }
    Some("spread arguments are not supported; use positional arguments")
}

fn mark_viable_positional(
    parameters: &[CallableParameter],
    provided: &mut [bool],
    positional: &mut usize,
) -> bool {
    let Some(parameter) = next_positional_parameter(parameters, provided, positional) else {
        return false;
    };
    if parameter.passing() != CallableParameterPassing::RestPositional {
        let index = parameter.index().get();
        provided[index] = true;
        *positional = index + 1;
    }
    true
}

fn next_positional_parameter<'a>(
    parameters: &'a [CallableParameter],
    provided: &[bool],
    positional: &mut usize,
) -> Option<&'a CallableParameter> {
    while let Some(parameter) = parameters.get(*positional) {
        if provided[*positional]
            || matches!(
                parameter.passing(),
                CallableParameterPassing::NamedOnly | CallableParameterPassing::RestNamed
            )
        {
            *positional += 1;
        } else {
            break;
        }
    }
    parameters.get(*positional).or_else(|| {
        parameters
            .iter()
            .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
    })
}

fn named_parameter<'a>(
    parameters: &'a [CallableParameter],
    name: &str,
) -> Option<&'a CallableParameter> {
    parameters
        .iter()
        .find(|parameter| {
            parameter
                .name()
                .is_some_and(|candidate| candidate.as_str() == name)
                && matches!(
                    parameter.passing(),
                    CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::NamedOnly
                )
        })
        .or_else(|| {
            parameters
                .iter()
                .find(|parameter| parameter.passing() == CallableParameterPassing::RestNamed)
        })
}

fn required_fixed_parameter_is_missing(
    parameters: &[CallableParameter],
    provided: &[bool],
) -> bool {
    parameters.iter().any(|parameter| {
        parameter.presence() == CallableParameterPresence::Required
            && !matches!(
                parameter.passing(),
                CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
            )
            && !provided[parameter.index().get()]
    })
}

fn fixed_literal_spread_slot_count(value: &Expr) -> Option<usize> {
    match value {
        Expr::BracketSeq(items) => Some(items.len()),
        Expr::NumericBracketSeq(sequence) => Some(sequence.len()),
        _ => None,
    }
}
