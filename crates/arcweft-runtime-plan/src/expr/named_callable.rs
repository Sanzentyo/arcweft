use super::{RuntimePureHelperLookup, lower_runtime_expr_strict_with_helpers};
use crate::function_values::RuntimeFunctionValueCandidate;
use arcweft_core::{plan::RuntimePureHelper, value::RuntimeExpr};
use arcweft_lang_hir::syntax::expr::CallArg;

#[derive(Clone, Debug)]
pub(super) struct PureHelperNamedPartialCall {
    pub(super) params: Vec<String>,
    pub(super) args: Vec<RuntimeExpr>,
}

#[derive(Clone, Debug)]
pub(super) enum PureHelperNamedCallLowering {
    Exact(Vec<RuntimeExpr>),
    Partial(PureHelperNamedPartialCall),
}

pub(super) fn lower_strict_pure_helper_named_call(
    callee: &str,
    args: &[CallArg],
    helper: &RuntimePureHelper,
    helpers: Option<RuntimePureHelperLookup<'_, '_>>,
) -> Result<RuntimeExpr, String> {
    match lower_strict_named_callable_args(
        "pure helper",
        callee,
        args,
        &helper.input_names,
        helpers,
    )? {
        PureHelperNamedCallLowering::Exact(args) => Ok(RuntimeExpr::PureCall {
            helper: helper.id,
            args,
        }),
        PureHelperNamedCallLowering::Partial(partial) => Ok(RuntimeExpr::Function {
            params: partial.params,
            body: Box::new(RuntimeExpr::PureCall {
                helper: helper.id,
                args: partial.args,
            }),
        }),
    }
}

pub(super) fn lower_strict_function_value_named_call(
    callee: &str,
    args: &[CallArg],
    candidate: &RuntimeFunctionValueCandidate,
    helpers: Option<RuntimePureHelperLookup<'_, '_>>,
) -> Result<RuntimeExpr, String> {
    match lower_strict_named_callable_args(
        "function",
        callee,
        args,
        candidate.input_names(),
        helpers,
    )? {
        PureHelperNamedCallLowering::Exact(args) => Ok(RuntimeExpr::Apply {
            callee: Box::new(candidate.value()),
            args,
        }),
        PureHelperNamedCallLowering::Partial(partial) => Ok(RuntimeExpr::Function {
            params: partial.params,
            body: Box::new(RuntimeExpr::Apply {
                callee: Box::new(candidate.value()),
                args: partial.args,
            }),
        }),
    }
}

pub(super) fn lower_strict_named_callable_args(
    callable_kind: &str,
    callee: &str,
    args: &[CallArg],
    input_names: &[String],
    helpers: Option<RuntimePureHelperLookup<'_, '_>>,
) -> Result<PureHelperNamedCallLowering, String> {
    let mut lowered = std::iter::repeat_with(|| None)
        .take(input_names.len())
        .collect::<Vec<_>>();
    let mut positional_index = 0usize;

    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                while positional_index < lowered.len() && lowered[positional_index].is_some() {
                    positional_index += 1;
                }
                let Some(slot) = lowered.get_mut(positional_index) else {
                    return Err(format!(
                        "{callable_kind} `{callee}` received too many positional arguments"
                    ));
                };
                *slot = Some(lower_runtime_expr_strict_with_helpers(value, helpers)?);
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let Some(index) = input_names.iter().position(|input| input == name) else {
                    return Err(format!(
                        "{callable_kind} `{callee}` has no input named `{name}`"
                    ));
                };
                if lowered[index].is_some() {
                    return Err(format!(
                        "{callable_kind} `{callee}` input `{name}` was provided more than once"
                    ));
                }
                lowered[index] = Some(lower_runtime_expr_strict_with_helpers(value, helpers)?);
            }
            CallArg::Spread { .. } => {
                return Err(format!(
                    "{callable_kind} `{callee}` does not accept spread arguments in named calls"
                ));
            }
        }
    }

    let mut missing = Vec::new();
    let args = lowered
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            value.unwrap_or_else(|| {
                let name = input_names[index].clone();
                missing.push(name.clone());
                RuntimeExpr::Local(name)
            })
        })
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(PureHelperNamedCallLowering::Exact(args))
    } else {
        Ok(PureHelperNamedCallLowering::Partial(
            PureHelperNamedPartialCall {
                params: missing,
                args,
            },
        ))
    }
}
