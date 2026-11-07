//! Function bodies own an implicit lexical scope without a source operation.

use super::{FlowControlStackEntry, FlowControlStackEntryKind, NativeFlowExecutionFrame};
use crate::runtime_id::RuntimeFunctionSiteId;
use crate::scope::RuntimeScopeFrameOrigin;
use crate::value::{RuntimeInputOrder, RuntimeLocalBinding};
use std::sync::Arc;

#[cfg(test)]
mod tests;

impl super::Engine {
    /// EOF is an invocation exit, so its Unit completion waits for the same
    /// lexical cleanup boundary as an authored return. No source op is added.
    pub(super) fn defer_function_fallthrough(
        &mut self,
        output: &mut crate::step::RuntimeStepOutput,
    ) -> bool {
        use super::defer::{NativeDeferBoundary, NativeLexicalExit, NativeLexicalExitAction};
        let Some(target) = self
            .fiber
            .frame
            .defer_target(NativeDeferBoundary::Invocation)
        else {
            return false;
        };
        let Some((site, _)) = self.fiber.frame.env.function_contexts().next_back() else {
            return false;
        };
        let completes_unit = self
            .plan
            .function_sites()
            .get(site)
            .is_some_and(|function| {
                function.role().allows_implicit_unit_completion()
                    && matches!(
                        self.plan.checked_type(function.result()),
                        Ok(Some(crate::pattern::RuntimeCheckedType::Unit))
                    )
            });
        if !completes_unit {
            self.fail_eval(
                crate::value::RuntimeEvalError::FunctionFallthrough { site },
                output,
            );
            return true;
        }
        self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
            target,
            action: NativeLexicalExitAction::Fallthrough { site },
        });
        true
    }
}

impl NativeFlowExecutionFrame {
    pub(super) fn enter_function_body(&mut self, site: RuntimeFunctionSiteId) {
        self.env.push_scope();
        self.control_stack.push(FlowControlStackEntry {
            kind: FlowControlStackEntryKind::Scope {
                origin: RuntimeScopeFrameOrigin::FunctionBody(site),
                cleanups: Vec::new(),
                match_guard: None,
                defers: super::defer::NativeLexicalDeferScope::new(),
            },
        });
    }

    pub(super) fn bind_root_function(
        &mut self,
        site: RuntimeFunctionSiteId,
        instantiation: Option<Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        inputs: (
            impl IntoIterator<Item = RuntimeLocalBinding>,
            RuntimeInputOrder,
        ),
    ) {
        self.env.bind_root_function(site, instantiation, inputs);
        self.enter_function_body(site);
    }
}
