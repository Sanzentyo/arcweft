//! Effect capability checks for contracts and scoped checker state.

use super::{
    ContractClause, EffectCapability, EffectScope, Expr, TypeCheckError, TypeChecker, TypeKind,
    capability_from_expr,
};
use std::collections::HashSet;

impl TypeChecker<'_> {
    pub(super) fn check_function_effects(&mut self, name: &str) {
        let source_effects = self
            .global_function_effects
            .get(name)
            .map_or(&[][..], Vec::as_slice);
        let env_effects = self.env.function_effects(name).unwrap_or(&[]);
        let missing = source_effects
            .iter()
            .map(String::as_str)
            .chain(env_effects.iter().map(EffectCapability::as_str))
            .filter(|capability| {
                !self.effect_capabilities.contains(*capability)
                    && !self.env.has_capability(capability)
            })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for capability in missing {
            self.errors.push(TypeCheckError::new(format!(
                "calling `{name}` requires effect capability `{capability}`"
            )));
        }
    }

    pub(super) fn check_contract_clause(&mut self, contract: &ContractClause) {
        match contract {
            ContractClause::Requires { expr, .. }
            | ContractClause::Ensures { expr, .. }
            | ContractClause::Invariant { expr, .. }
            | ContractClause::Assume { expr } => {
                self.expect_expr_type(expr, &TypeKind::Bool, "contract expression");
            }
            ContractClause::NoEffect(expr) | ContractClause::Decreases(expr) => {
                self.check_expr(expr);
            }
            ContractClause::Reads(items)
            | ContractClause::Effects(items)
            | ContractClause::Modifies(items) => {
                for item in items {
                    self.check_contract_selector(item);
                }
            }
        }
    }

    pub(super) fn check_contract_selector(&mut self, expr: &Expr) {
        // Contract selectors name capabilities or resources. They are not
        // executable expressions, so dotted selectors such as `signal.write`
        // must not resolve `signal` as a local value.
        if capability_from_expr(expr).is_some() || matches!(expr, Expr::EntityRef(_)) {
            return;
        }
        self.check_expr(expr);
    }

    pub(super) fn apply_effect_scope(&mut self, scope: &EffectScope) -> HashSet<String> {
        let snapshot = self.effect_capabilities.clone();
        self.effect_capabilities.extend(
            scope
                .iter()
                .map(|capability| capability.as_str().to_owned()),
        );
        snapshot
    }
}
