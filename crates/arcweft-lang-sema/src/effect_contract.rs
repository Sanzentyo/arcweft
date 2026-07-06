//! Lowers surface contract expressions into the canonical semantic effect model.

use arcweft_lang_syntax::{
    ast::flow::ContractClause,
    expr::{CallArg, Expr, Literal},
};
use thiserror::Error;

use crate::{
    effect_catalog::{EffectCatalog, EffectCatalogError},
    effect_model::EffectContract,
    effects::{EffectId, EffectIdError, EffectSet},
};

/// Failure to lower a contract selector into a canonical effect identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectContractLowerError {
    #[error("effect selector is not a capability path or scoped capability call: {expr:?}")]
    UnsupportedSelector { expr: Expr },
    #[error("effect scope arguments must be positional and statically named: {expr:?}")]
    UnsupportedScopeArgument { expr: Expr },
    #[error("pure callable cannot declare non-empty effects {effects}")]
    PureEffectConflict { effects: EffectSet },
    #[error(transparent)]
    InvalidEffectId(#[from] EffectIdError),
    #[error(transparent)]
    InvalidCatalogEffect(#[from] EffectCatalogError),
}

/// Lowers `effects` and `no_effect` clauses for one callable.
pub fn lower_effect_contract(
    contracts: &[ContractClause],
    pure: bool,
    catalog: &EffectCatalog,
) -> Result<EffectContract, Vec<EffectContractLowerError>> {
    let mut declared = None::<EffectSet>;
    let mut forbidden = EffectSet::new();
    let mut errors = Vec::new();

    for contract in contracts {
        match contract {
            ContractClause::Effects(expressions) => {
                let effects = declared.get_or_insert_with(EffectSet::new);
                for expression in expressions {
                    match effect_id_from_expr(expression) {
                        Ok(effect) => match catalog.validate(&effect) {
                            Ok(()) => {
                                effects.insert(effect);
                            }
                            Err(error) => errors.push(error.into()),
                        },
                        Err(error) => errors.push(error),
                    }
                }
            }
            ContractClause::NoEffect(expression) => match effect_id_from_expr(expression) {
                Ok(effect) => match catalog.validate(&effect) {
                    Ok(()) => {
                        forbidden.insert(effect);
                    }
                    Err(error) => errors.push(error.into()),
                },
                Err(error) => errors.push(error),
            },
            ContractClause::Requires { .. }
            | ContractClause::Ensures { .. }
            | ContractClause::Invariant { .. }
            | ContractClause::Assume { .. }
            | ContractClause::Reads(_)
            | ContractClause::Modifies(_)
            | ContractClause::Decreases(_) => {}
        }
    }

    if pure && declared.as_ref().is_some_and(|effects| !effects.is_empty()) {
        errors.push(EffectContractLowerError::PureEffectConflict {
            effects: declared.clone().unwrap_or_default(),
        });
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let contract = if pure {
        EffectContract::pure()
    } else if let Some(declared) = declared {
        EffectContract::bounded(declared)
    } else {
        EffectContract::inferred()
    }
    .with_forbidden(forbidden);

    Ok(contract)
}

/// Converts the existing expression-shaped effect selector to one canonical ID.
pub fn effect_id_from_expr(expression: &Expr) -> Result<EffectId, EffectContractLowerError> {
    EffectId::parse(effect_label(expression)?).map_err(EffectContractLowerError::InvalidEffectId)
}

fn effect_label(expression: &Expr) -> Result<String, EffectContractLowerError> {
    match expression {
        Expr::Path(path) => Ok(path.as_label().to_owned()),
        Expr::ShortVariant(name) => Ok(format!(".{name}")),
        Expr::Field { target, field } => {
            effect_label(target).map(|target| format!("{target}.{field}"))
        }
        Expr::Call { callee, args } => scoped_effect_label(callee, args),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let base = effect_label(receiver).map(|receiver| format!("{receiver}.{method}"))?;
            append_scope(base, args)
        }
        _ => Err(EffectContractLowerError::UnsupportedSelector {
            expr: expression.clone(),
        }),
    }
}

fn scoped_effect_label(
    callee: &Expr,
    args: &[CallArg],
) -> Result<String, EffectContractLowerError> {
    append_scope(effect_label(callee)?, args)
}

fn append_scope(base: String, args: &[CallArg]) -> Result<String, EffectContractLowerError> {
    if args.is_empty() {
        return Ok(base);
    }
    let scopes = args
        .iter()
        .map(effect_scope_arg)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{base}({})", scopes.join(",")))
}

fn effect_scope_arg(argument: &CallArg) -> Result<String, EffectContractLowerError> {
    let expression = match argument {
        CallArg::Positional(expression) => expression,
        CallArg::Named { value, .. } | CallArg::Spread { value } => {
            return Err(EffectContractLowerError::UnsupportedScopeArgument {
                expr: value.as_ref().clone(),
            });
        }
    };
    match expression {
        Expr::Path(path) => Ok(path
            .strip_prefix('\'')
            .unwrap_or(path.as_label())
            .to_owned()),
        Expr::ShortVariant(name) => Ok(format!(".{name}")),
        Expr::LifetimePath { key, .. } => Ok(key.scope().as_str().to_owned()),
        Expr::EntityRef(entity) => Ok(entity.body().to_owned()),
        Expr::Literal(Literal::String(value)) => Ok(value.clone()),
        _ => Err(EffectContractLowerError::UnsupportedScopeArgument {
            expr: expression.clone(),
        }),
    }
}
