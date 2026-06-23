//! Effect capability checks for contracts and scoped checker state.

use super::{
    ContractClause, EffectCapability, EffectScope, Expr, TypeCheckError, TypeChecker, TypeKind,
    capability_from_expr,
};
use crate::effect_model::EffectSite;
use crate::effects::{EffectId, EffectSet};
use std::collections::HashSet;

impl TypeChecker<'_> {
    pub(super) fn check_function_effects(&mut self, name: &str) {
        let source_effects = self
            .global_function_effects
            .get(name)
            .into_iter()
            .flatten()
            .map(String::as_str);
        let env_effects = self
            .env
            .function_effects(name)
            .into_iter()
            .flatten()
            .map(EffectCapability::as_str);
        let external = effect_set_from_labels(source_effects.chain(env_effects), &mut self.errors);
        self.effect_collector.record_named_call(
            name,
            external,
            EffectSite::new(format!("call `{name}`")),
        );
    }

    pub(super) fn record_static_effect(&mut self, effect: &str, site: impl Into<String>) {
        match EffectId::parse(effect) {
            Ok(effect) => self
                .effect_collector
                .record_effect(effect, EffectSite::new(site)),
            Err(error) => self.errors.push(TypeCheckError::new(error.to_string())),
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
            ContractClause::NoEffect(_) => {}
            ContractClause::Decreases(expr) => {
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

    pub(super) fn check_function_contract_clause(
        &mut self,
        contract: &ContractClause,
        result_type: Option<&TypeKind>,
    ) {
        let ContractClause::Ensures { .. } = contract else {
            self.check_contract_clause(contract);
            return;
        };
        let Some(result_type) = result_type else {
            self.check_contract_clause(contract);
            return;
        };
        let previous_result = self.locals.insert("result".to_owned(), result_type.clone());
        self.check_contract_clause(contract);
        if let Some(previous_result) = previous_result {
            self.locals.insert("result".to_owned(), previous_result);
        } else {
            self.locals.remove("result");
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

fn effect_set_from_labels<'a>(
    labels: impl IntoIterator<Item = &'a str>,
    errors: &mut Vec<TypeCheckError>,
) -> Option<EffectSet> {
    let mut effects = EffectSet::new();
    for label in labels {
        match EffectId::parse(label) {
            Ok(effect) => {
                effects.insert(effect);
            }
            Err(error) => errors.push(TypeCheckError::new(error.to_string())),
        }
    }
    (!effects.is_empty()).then_some(effects)
}
