//! Assertion-specific type and effect checking.

use super::{
    CallableId, CallableKind, EffectContract, EffectSite, TypeCheckError, TypeChecker, TypeKind,
    Visibility,
};
use crate::effect_analysis::EffectAnalysisReport;
use arcweft_lang_syntax::assertion::AssertionStmt;
use std::collections::BTreeSet;

impl TypeChecker<'_> {
    pub(super) fn extend_effect_diagnostics(&mut self, effects: &EffectAnalysisReport) {
        let mut reported_assertion_conditions = BTreeSet::new();
        self.errors
            .extend(effects.errors().filter_map(|diagnostic| {
                let Some(index) = self
                    .assertion_effect_conditions
                    .get(diagnostic.callable())
                    .copied()
                else {
                    return Some(TypeCheckError::effect(diagnostic.clone()));
                };
                reported_assertion_conditions
                    .insert(diagnostic.callable().clone())
                    .then(|| {
                        TypeCheckError::assertion_condition_not_pure(index, diagnostic.clone())
                    })
            }));
    }

    pub(super) fn check_assertion(&mut self, assertion: &AssertionStmt) {
        for (index, condition) in assertion.conditions().iter().enumerate() {
            self.check_assertion_condition(condition, index);
        }
    }

    fn check_assertion_condition(
        &mut self,
        condition: &arcweft_lang_syntax::expr::Expr,
        index: usize,
    ) {
        let sequence = self.next_assertion_effect_scope;
        self.next_assertion_effect_scope = self
            .next_assertion_effect_scope
            .checked_add(1)
            .expect("syntax limits keep assertion effect-scope identity below u64::MAX");
        let callable = CallableId::new(format!("$assertion.{sequence}"));
        let source_name = format!("<assertion:{sequence}>");
        if let Err(error) = self.effect_collector.register_callable(
            source_name,
            callable.clone(),
            CallableKind::Function,
            Visibility::Private,
            EffectContract::pure(),
        ) {
            self.errors.push(TypeCheckError::new(error.to_string()));
            self.check_assertion_condition_type(condition, index);
            return;
        }

        self.retain_assertion_effect_condition(callable.clone(), index);
        let outer = self.effect_collector.current_callable();
        if let Some(outer) = outer.as_ref() {
            self.effect_collector.record_local_call_from(
                outer,
                callable.clone(),
                EffectSite::new(format!("assertion condition {index}")),
            );
        }
        let previous = self.effect_collector.enter(callable);
        self.check_assertion_condition_type(condition, index);
        self.effect_collector.restore(previous);
    }

    fn check_assertion_condition_type(
        &mut self,
        condition: &arcweft_lang_syntax::expr::Expr,
        index: usize,
    ) {
        let expected = TypeKind::Bool;
        let actual = self.check_expr_with_expected(condition, Some(&expected));
        if !actual
            .as_ref()
            .is_some_and(|actual| self.types_compatible(&expected, actual))
        {
            self.errors
                .push(TypeCheckError::assertion_condition_not_bool(index, actual));
        }
    }
}
