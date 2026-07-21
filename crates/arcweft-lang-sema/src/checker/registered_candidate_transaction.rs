//! Bounded mutation journal for speculative registered-call candidates.

use crate::{effect_collector::EffectCollectorCheckpoint, effect_model::CallableId};

use super::{
    BorrowStateCheckpoint, CallTargetFactRecorder, CurriedSignatureCallValue, ExprNodeKey,
    LifetimeKey, SignatureWorkChargeState, SuspensionBoundary, TypeCheckStats, TypeChecker,
    YieldContext,
};

pub(super) enum RegisteredCandidateMutation {
    ActivePresentationDefault {
        family: String,
        previous: Option<String>,
    },
    LifetimeGuarantee {
        key: LifetimeKey,
        was_present: bool,
    },
    DroppedLifetimeKey {
        key: LifetimeKey,
        was_present: bool,
    },
    AssertionEffectCondition {
        callable: CallableId,
        previous: Option<usize>,
    },
    ClosureEffectCallable {
        expression: ExprNodeKey,
        previous: Option<CallableId>,
    },
    ClosureFrameLocal {
        frame: usize,
        name: String,
    },
    ClosureCapture {
        frame: usize,
        name: String,
    },
    ClosureSuspensionBoundary {
        frame: usize,
        boundary: SuspensionBoundary,
    },
    HigherOrderParamInvocation {
        function_name: String,
        param_name: String,
    },
    HigherOrderParamClosureInvocation {
        function_name: String,
        param_name: String,
        callable: CallableId,
    },
}

pub(super) struct RegisteredCandidateCheckpoint {
    journal_start: usize,
    borrow: BorrowStateCheckpoint,
    effects: EffectCollectorCheckpoint,
    errors: usize,
    warnings: usize,
    stats: TypeCheckStats,
    judgments: usize,
    typed_lowering_evidence: usize,
    closure_captures: usize,
    numeric_fallbacks: usize,
    pending_higher_order_effect_calls: usize,
    for_iteration_evidence: usize,
    project_callable_references: usize,
    project_entity_references: usize,
    checked_speaker_lines: usize,
    loop_break_types: Vec<usize>,
    yield_stack: Vec<YieldContext>,
    last_checked_closure_effect_callable: Option<CallableId>,
    last_checked_curried_signature_call: Option<CurriedSignatureCallValue>,
    next_assertion_effect_scope: u64,
    next_semantic_scope: u32,
    next_semantic_binding: u32,
    allow_signed_min_literal: bool,
    call_target_fact_recorder: CallTargetFactRecorder,
    signature_work_charge: SignatureWorkChargeState,
}

impl TypeChecker<'_> {
    pub(super) fn checkpoint_registered_candidate(&mut self) -> RegisteredCandidateCheckpoint {
        let stats = self.stats.clone();
        let borrow = self.checkpoint_borrow_state();
        let effects = self.effect_collector.checkpoint();
        self.registered_candidate_transaction_depth += 1;
        RegisteredCandidateCheckpoint {
            journal_start: self.registered_candidate_journal.len(),
            borrow,
            effects,
            errors: self.errors.len(),
            warnings: self.warnings.len(),
            stats,
            judgments: self.judgments.len(),
            typed_lowering_evidence: self.typed_lowering_evidence.len(),
            closure_captures: self.closure_captures.len(),
            numeric_fallbacks: self.numeric_fallbacks.len(),
            pending_higher_order_effect_calls: self.pending_higher_order_effect_calls.len(),
            for_iteration_evidence: self.for_iteration_evidence.len(),
            project_callable_references: self.project_callable_references.len(),
            project_entity_references: self.project_entity_references.len(),
            checked_speaker_lines: self.checked_speaker_lines.len(),
            loop_break_types: self
                .loop_stack
                .iter()
                .map(|context| context.break_types.len())
                .collect(),
            yield_stack: self.yield_stack.clone(),
            last_checked_closure_effect_callable: self.last_checked_closure_effect_callable.clone(),
            last_checked_curried_signature_call: self.last_checked_curried_signature_call.clone(),
            next_assertion_effect_scope: self.next_assertion_effect_scope,
            next_semantic_scope: self.next_semantic_scope,
            next_semantic_binding: self.next_semantic_binding,
            allow_signed_min_literal: self.allow_signed_min_literal,
            call_target_fact_recorder: self.call_target_fact_recorder.clone(),
            signature_work_charge: self.signature_work_charge,
        }
    }

    pub(super) fn rollback_registered_candidate(
        &mut self,
        checkpoint: RegisteredCandidateCheckpoint,
    ) {
        let terminal_query_error = self.call_target_fact_recorder.terminal_query_error();
        self.rollback_registered_candidate_mutations(checkpoint.journal_start);
        self.restore_registered_candidate_checkpoint(checkpoint, terminal_query_error);
    }

    pub(super) fn commit_registered_candidate(
        &mut self,
        checkpoint: &RegisteredCandidateCheckpoint,
    ) {
        let journal_start = checkpoint.journal_start;
        self.effect_collector.commit(&checkpoint.effects);
        self.registered_candidate_transaction_depth = self
            .registered_candidate_transaction_depth
            .checked_sub(1)
            .expect("registered candidate checkpoints commit exactly once");
        if self.registered_candidate_transaction_depth == 0 {
            self.registered_candidate_journal.truncate(journal_start);
        }
    }

    fn rollback_registered_candidate_mutations(&mut self, journal_start: usize) {
        while self.registered_candidate_journal.len() > journal_start {
            let mutation = self
                .registered_candidate_journal
                .pop()
                .expect("journal length was checked");
            self.rollback_registered_candidate_mutation(mutation);
        }
    }

    fn rollback_registered_candidate_mutation(&mut self, mutation: RegisteredCandidateMutation) {
        match mutation {
            RegisteredCandidateMutation::ActivePresentationDefault { family, previous } => {
                match previous {
                    Some(value) => {
                        self.active_presentation_defaults.insert(family, value);
                    }
                    None => {
                        self.active_presentation_defaults.remove(&family);
                    }
                }
            }
            RegisteredCandidateMutation::LifetimeGuarantee { key, was_present } => {
                if was_present {
                    self.lifetime_guarantees.insert(key);
                } else {
                    self.lifetime_guarantees.remove(&key);
                }
            }
            RegisteredCandidateMutation::DroppedLifetimeKey { key, was_present } => {
                if was_present {
                    self.dropped_lifetime_keys.insert(key);
                } else {
                    self.dropped_lifetime_keys.remove(&key);
                }
            }
            RegisteredCandidateMutation::AssertionEffectCondition { callable, previous } => {
                match previous {
                    Some(index) => {
                        self.assertion_effect_conditions.insert(callable, index);
                    }
                    None => {
                        self.assertion_effect_conditions.remove(&callable);
                    }
                }
            }
            RegisteredCandidateMutation::ClosureEffectCallable {
                expression,
                previous,
            } => match previous {
                Some(callable) => {
                    self.closure_effect_callables_by_expr
                        .insert(expression, callable);
                }
                None => {
                    self.closure_effect_callables_by_expr.remove(&expression);
                }
            },
            RegisteredCandidateMutation::ClosureFrameLocal { frame, name } => {
                if let Some(frame) = self.closure_capture_stack.get_mut(frame) {
                    frame.locals.remove(&name);
                }
            }
            RegisteredCandidateMutation::ClosureCapture { frame, name } => {
                if let Some(frame) = self.closure_capture_stack.get_mut(frame) {
                    frame.captures.remove(&name);
                }
            }
            RegisteredCandidateMutation::ClosureSuspensionBoundary { frame, boundary } => {
                if let Some(frame) = self.closure_capture_stack.get_mut(frame) {
                    frame.suspension_boundaries.remove(&boundary);
                }
            }
            RegisteredCandidateMutation::HigherOrderParamInvocation {
                function_name,
                param_name,
            } => self.rollback_higher_order_param_invocation(&function_name, &param_name),
            RegisteredCandidateMutation::HigherOrderParamClosureInvocation {
                function_name,
                param_name,
                callable,
            } => self.rollback_higher_order_param_closure_invocation(
                &function_name,
                &param_name,
                &callable,
            ),
        }
    }

    fn rollback_higher_order_param_invocation(&mut self, function_name: &str, param_name: &str) {
        let remove_function = self
            .higher_order_param_invocations
            .get_mut(function_name)
            .is_some_and(|params| {
                params.remove(param_name);
                params.is_empty()
            });
        if remove_function {
            self.higher_order_param_invocations.remove(function_name);
        }
    }

    fn rollback_higher_order_param_closure_invocation(
        &mut self,
        function_name: &str,
        param_name: &str,
        callable: &CallableId,
    ) {
        let mut remove_function = false;
        if let Some(params) = self
            .higher_order_param_closure_invocations
            .get_mut(function_name)
        {
            let remove_param = params.get_mut(param_name).is_some_and(|callables| {
                callables.remove(callable);
                callables.is_empty()
            });
            if remove_param {
                params.remove(param_name);
            }
            remove_function = params.is_empty();
        }
        if remove_function {
            self.higher_order_param_closure_invocations
                .remove(function_name);
        }
    }

    fn restore_registered_candidate_checkpoint(
        &mut self,
        checkpoint: RegisteredCandidateCheckpoint,
        terminal_query_error: Option<crate::callable::CallTargetFactError>,
    ) {
        self.effect_collector.rollback(checkpoint.effects);
        self.restore_borrow_state(checkpoint.borrow);
        self.errors.truncate(checkpoint.errors);
        self.warnings.truncate(checkpoint.warnings);
        self.judgments.truncate(checkpoint.judgments);
        self.typed_lowering_evidence
            .truncate(checkpoint.typed_lowering_evidence);
        self.closure_captures.truncate(checkpoint.closure_captures);
        self.numeric_fallbacks
            .truncate(checkpoint.numeric_fallbacks);
        self.pending_higher_order_effect_calls
            .truncate(checkpoint.pending_higher_order_effect_calls);
        self.for_iteration_evidence
            .truncate(checkpoint.for_iteration_evidence);
        self.project_callable_references
            .truncate(checkpoint.project_callable_references);
        self.project_entity_references
            .truncate(checkpoint.project_entity_references);
        self.checked_speaker_lines
            .truncate(checkpoint.checked_speaker_lines);
        debug_assert_eq!(self.loop_stack.len(), checkpoint.loop_break_types.len());
        for (context, len) in self.loop_stack.iter_mut().zip(checkpoint.loop_break_types) {
            context.break_types.truncate(len);
        }
        self.yield_stack = checkpoint.yield_stack;
        self.last_checked_closure_effect_callable = checkpoint.last_checked_closure_effect_callable;
        self.last_checked_curried_signature_call = checkpoint.last_checked_curried_signature_call;
        self.next_assertion_effect_scope = checkpoint.next_assertion_effect_scope;
        self.next_semantic_scope = checkpoint.next_semantic_scope;
        self.next_semantic_binding = checkpoint.next_semantic_binding;
        self.allow_signed_min_literal = checkpoint.allow_signed_min_literal;
        self.call_target_fact_recorder = checkpoint.call_target_fact_recorder;
        if let Some(error) = terminal_query_error {
            self.call_target_fact_recorder
                .record_terminal_query_error(error);
        }
        self.signature_work_charge = checkpoint.signature_work_charge;
        self.stats = checkpoint.stats;
        self.registered_candidate_transaction_depth = self
            .registered_candidate_transaction_depth
            .checked_sub(1)
            .expect("registered candidate checkpoints roll back exactly once");
    }

    fn record_registered_candidate_mutation(&mut self, mutation: RegisteredCandidateMutation) {
        if self.registered_candidate_transaction_depth != 0 {
            self.registered_candidate_journal.push(mutation);
        }
    }

    pub(super) fn set_active_presentation_default(
        &mut self,
        family: impl Into<String>,
        value: String,
    ) -> Option<String> {
        let family = family.into();
        let previous = self
            .active_presentation_defaults
            .insert(family.clone(), value);
        self.record_registered_candidate_mutation(
            RegisteredCandidateMutation::ActivePresentationDefault {
                family,
                previous: previous.clone(),
            },
        );
        previous
    }

    pub(super) fn clear_active_presentation_default(&mut self, family: &str) -> Option<String> {
        let previous = self.active_presentation_defaults.remove(family);
        self.record_registered_candidate_mutation(
            RegisteredCandidateMutation::ActivePresentationDefault {
                family: family.to_owned(),
                previous: previous.clone(),
            },
        );
        previous
    }

    pub(super) fn retain_lifetime_guarantee(&mut self, key: LifetimeKey) -> bool {
        let was_present = !self.lifetime_guarantees.insert(key.clone());
        self.record_registered_candidate_mutation(RegisteredCandidateMutation::LifetimeGuarantee {
            key,
            was_present,
        });
        !was_present
    }

    pub(super) fn release_lifetime_guarantee(&mut self, key: &LifetimeKey) -> bool {
        let was_present = self.lifetime_guarantees.remove(key);
        self.record_registered_candidate_mutation(RegisteredCandidateMutation::LifetimeGuarantee {
            key: key.clone(),
            was_present,
        });
        was_present
    }

    pub(super) fn retain_dropped_lifetime_key(&mut self, key: LifetimeKey) -> bool {
        let was_present = !self.dropped_lifetime_keys.insert(key.clone());
        self.record_registered_candidate_mutation(
            RegisteredCandidateMutation::DroppedLifetimeKey { key, was_present },
        );
        !was_present
    }

    pub(super) fn retain_assertion_effect_condition(&mut self, callable: CallableId, index: usize) {
        let previous = self
            .assertion_effect_conditions
            .insert(callable.clone(), index);
        self.record_registered_candidate_mutation(
            RegisteredCandidateMutation::AssertionEffectCondition { callable, previous },
        );
    }

    pub(super) fn retain_closure_effect_callable(
        &mut self,
        expression: ExprNodeKey,
        callable: CallableId,
    ) {
        let previous = self
            .closure_effect_callables_by_expr
            .insert(expression, callable);
        self.record_registered_candidate_mutation(
            RegisteredCandidateMutation::ClosureEffectCallable {
                expression,
                previous,
            },
        );
    }

    pub(super) fn retain_closure_frame_local(&mut self, frame: usize, name: String) {
        let inserted = self.closure_capture_stack[frame]
            .locals
            .insert(name.clone());
        if inserted {
            self.record_registered_candidate_mutation(
                RegisteredCandidateMutation::ClosureFrameLocal { frame, name },
            );
        }
    }

    pub(super) fn retain_closure_suspension_boundary(&mut self, boundary: SuspensionBoundary) {
        let Some(frame) = self.closure_capture_stack.len().checked_sub(1) else {
            return;
        };
        if self.closure_capture_stack[frame]
            .suspension_boundaries
            .insert(boundary)
        {
            self.record_registered_candidate_mutation(
                RegisteredCandidateMutation::ClosureSuspensionBoundary { frame, boundary },
            );
        }
    }

    pub(super) fn retain_closure_capture(&mut self, frame: usize, name: String) {
        self.record_registered_candidate_mutation(RegisteredCandidateMutation::ClosureCapture {
            frame,
            name,
        });
    }

    pub(super) fn retain_higher_order_param_invocation(
        &mut self,
        function_name: String,
        param_name: String,
    ) {
        let inserted = self
            .higher_order_param_invocations
            .entry(function_name.clone())
            .or_default()
            .insert(param_name.clone());
        if inserted {
            self.record_registered_candidate_mutation(
                RegisteredCandidateMutation::HigherOrderParamInvocation {
                    function_name,
                    param_name,
                },
            );
        }
    }

    pub(super) fn retain_higher_order_param_closure_invocation(
        &mut self,
        function_name: String,
        param_name: String,
        callable: CallableId,
    ) {
        let inserted = self
            .higher_order_param_closure_invocations
            .entry(function_name.clone())
            .or_default()
            .entry(param_name.clone())
            .or_default()
            .insert(callable.clone());
        if inserted {
            self.record_registered_candidate_mutation(
                RegisteredCandidateMutation::HigherOrderParamClosureInvocation {
                    function_name,
                    param_name,
                    callable,
                },
            );
        }
    }
}
