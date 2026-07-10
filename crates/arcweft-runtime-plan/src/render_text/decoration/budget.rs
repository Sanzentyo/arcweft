//! Deterministic resource accounting for decoration composition expansion.

use arcweft_lang_hir::decoration::DecorationExpansionLimits;

use crate::errors::RuntimePlanLowerError;

use super::decoration_error;

#[derive(Clone, Debug)]
pub(super) struct DecorationExpansionState {
    limits: DecorationExpansionLimits,
    visits: usize,
    layers: usize,
    chain: Vec<String>,
}

impl DecorationExpansionState {
    pub(super) fn new(limits: DecorationExpansionLimits) -> Self {
        Self {
            limits,
            visits: 0,
            layers: 0,
            chain: Vec::new(),
        }
    }

    pub(super) fn enter(&mut self, name: &str) -> Result<(), RuntimePlanLowerError> {
        if let Some(cycle_start) = self.chain.iter().position(|active| active == name) {
            let mut cycle = self.chain[cycle_start..].to_vec();
            cycle.push(name.to_owned());
            return Err(decoration_error(format!(
                "rich-text decoration cycle: {}",
                cycle.join(" -> ")
            )));
        }
        if self.chain.len() >= self.limits.max_depth {
            return Err(decoration_error(format!(
                "rich-text decoration expansion exceeds maximum nesting depth of {} while entering `.{name}`",
                self.limits.max_depth
            )));
        }
        if self.visits >= self.limits.max_visits {
            return Err(decoration_error(format!(
                "rich-text decoration expansion exceeds maximum declaration visits of {} while entering `.{name}`",
                self.limits.max_visits
            )));
        }
        self.visits += 1;
        self.chain.push(name.to_owned());
        Ok(())
    }

    pub(super) fn leave(&mut self, name: &str) {
        let popped = self.chain.pop();
        debug_assert_eq!(popped.as_deref(), Some(name));
    }

    pub(super) fn record_layer(&mut self, owner: &str) -> Result<(), RuntimePlanLowerError> {
        if self.layers >= self.limits.max_layers {
            return Err(decoration_error(format!(
                "rich-text decoration expansion exceeds maximum expanded style layers of {} while expanding `.{owner}`",
                self.limits.max_layers
            )));
        }
        self.layers += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DecorationExpansionState;
    use arcweft_lang_hir::decoration::DecorationExpansionLimits;

    #[test]
    fn exact_expansion_limits_succeed_before_the_next_operation_fails() {
        let limits = DecorationExpansionLimits {
            max_depth: 2,
            max_visits: 2,
            max_layers: 2,
        };
        let mut state = DecorationExpansionState::new(limits);

        state.enter("outer").expect("first visit");
        state.enter("inner").expect("exact depth and visit limits");
        state.record_layer("inner").expect("first layer");
        state
            .record_layer("inner")
            .expect("exact expanded-layer limit");
        assert!(state.record_layer("inner").is_err());
        state.leave("inner");
        state.leave("outer");
        assert!(state.enter("third").is_err());
    }
}
