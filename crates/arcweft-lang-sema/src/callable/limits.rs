//! Inclusive callable catalog and query limits.
use super::{
    CallableBuildLimitError, CallableQueryLimitError, SignatureLimitExceeded, SignatureLimitKind,
    SignatureWorkKind,
};

/// Fixed resource limits shared by callable registration and semantic queries.
#[allow(
    clippy::struct_field_names,
    reason = "the contract names each inclusive bound as an explicit maximum"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableLimits {
    max_path_segments: usize,
    max_groups_per_callable: usize,
    max_parameters_per_callable: usize,
    max_overloads_per_key: usize,
    max_candidates_per_call: usize,
    max_nested_calls: usize,
    max_recovery_nodes: usize,
    max_diagnostics: usize,
    max_source_bytes: usize,
    max_project_modules: usize,
    max_catalog_records: usize,
    max_catalog_build_work: u64,
    max_query_work: u64,
}

/// Production callable limits. Every bound is inclusive.
pub const PRODUCTION_CALLABLE_LIMITS: CallableLimits = CallableLimits {
    max_path_segments: 32,
    max_groups_per_callable: 16,
    max_parameters_per_callable: 128,
    max_overloads_per_key: 32,
    max_candidates_per_call: 256,
    max_nested_calls: 32,
    max_recovery_nodes: 256,
    max_diagnostics: 128,
    max_source_bytes: 8_388_608,
    max_project_modules: 4_096,
    max_catalog_records: 262_144,
    max_catalog_build_work: 1_048_576,
    max_query_work: 4_096,
};

/// Inclusive public signature-search and result limits.
///
/// These limits are intentionally independent from callable catalog and
/// resolver staging. In particular, resolver facts fail closed at their own
/// diagnostic limit while the public result deterministically truncates its
/// diagnostic projection.
#[allow(
    clippy::struct_field_names,
    reason = "the contract names each inclusive bound as an explicit maximum"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureQueryLimits {
    candidate_calls: u64,
    overloads: u64,
    parameters_per_signature: u64,
    nested_calls: u64,
    recovery_nodes: u64,
    source_bytes: u64,
    diagnostics: u64,
    work_units: u64,
}

/// Production signature limits. Every bound is inclusive.
pub const PRODUCTION_SIGNATURE_LIMITS: SignatureQueryLimits = SignatureQueryLimits::PRODUCTION;

impl SignatureQueryLimits {
    pub const PRODUCTION: Self = Self {
        candidate_calls: 4_096,
        overloads: 64,
        parameters_per_signature: 128,
        nested_calls: 64,
        recovery_nodes: 512,
        source_bytes: 8_388_608,
        diagnostics: 32,
        work_units: 262_144,
    };

    pub const fn candidate_calls(self) -> u64 {
        self.candidate_calls
    }

    pub const fn overloads(self) -> u64 {
        self.overloads
    }

    pub const fn parameters_per_signature(self) -> u64 {
        self.parameters_per_signature
    }

    pub const fn nested_calls(self) -> u64 {
        self.nested_calls
    }

    pub const fn recovery_nodes(self) -> u64 {
        self.recovery_nodes
    }

    pub const fn source_bytes(self) -> u64 {
        self.source_bytes
    }

    pub const fn diagnostics(self) -> u64 {
        self.diagnostics
    }

    pub const fn work_units(self) -> u64 {
        self.work_units
    }
}

impl CallableLimits {
    pub const fn max_path_segments(self) -> usize {
        self.max_path_segments
    }
    pub const fn max_groups_per_callable(self) -> usize {
        self.max_groups_per_callable
    }
    pub const fn max_parameters_per_callable(self) -> usize {
        self.max_parameters_per_callable
    }
    pub const fn max_overloads_per_key(self) -> usize {
        self.max_overloads_per_key
    }
    pub const fn max_candidates_per_call(self) -> usize {
        self.max_candidates_per_call
    }
    pub const fn max_nested_calls(self) -> usize {
        self.max_nested_calls
    }
    pub const fn max_recovery_nodes(self) -> usize {
        self.max_recovery_nodes
    }
    pub const fn max_diagnostics(self) -> usize {
        self.max_diagnostics
    }
    pub const fn max_source_bytes(self) -> usize {
        self.max_source_bytes
    }
    pub const fn max_project_modules(self) -> usize {
        self.max_project_modules
    }
    pub const fn max_catalog_records(self) -> usize {
        self.max_catalog_records
    }
    pub const fn max_catalog_build_work(self) -> u64 {
        self.max_catalog_build_work
    }
    pub const fn max_query_work(self) -> u64 {
        self.max_query_work
    }

    #[cfg(test)]
    #[allow(
        clippy::too_many_arguments,
        reason = "test fixtures need independent exact and one-over limit controls"
    )]
    pub(crate) const fn for_test(
        max_path_segments: usize,
        max_groups_per_callable: usize,
        max_parameters_per_callable: usize,
        max_overloads_per_key: usize,
        max_candidates_per_call: usize,
        max_nested_calls: usize,
        max_recovery_nodes: usize,
        max_diagnostics: usize,
        max_catalog_build_work: u64,
        max_query_work: u64,
    ) -> Self {
        Self {
            max_path_segments,
            max_groups_per_callable,
            max_parameters_per_callable,
            max_overloads_per_key,
            max_candidates_per_call,
            max_nested_calls,
            max_recovery_nodes,
            max_diagnostics,
            max_source_bytes: 8_388_608,
            max_project_modules: 4_096,
            max_catalog_records: 262_144,
            max_catalog_build_work,
            max_query_work,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CatalogBuildWork {
    consumed: u64,
    limit: u64,
}

impl CatalogBuildWork {
    pub(crate) const fn new(limit: u64) -> Self {
        Self { consumed: 0, limit }
    }

    pub(crate) fn charge(&mut self, units: u64) -> Result<(), super::CallableCatalogBuildError> {
        let next = self
            .consumed
            .checked_add(units)
            .ok_or(super::CallableCatalogBuildError::WorkOverflow)?;
        if next > self.limit {
            return Err(CallableBuildLimitError::Work {
                requested: units,
                consumed: self.consumed,
                limit: self.limit,
            }
            .into());
        }
        self.consumed = next;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolverWork {
    consumed: u64,
    limit: u64,
    resolver: u64,
    argument_mapping: u64,
    type_checks: u64,
    call: CallResolverAccountingReport,
}

/// Closed resolver-work and committed-publication counters for one final Call
/// transaction.
///
/// Candidate probe/replay entries count candidate × authored-argument visits;
/// fixed spread slots deliberately remain separate in the crate-owned
/// `physical_candidate_argument_evaluations` trace. These counters are
/// observations of work already charged through [`ResolverWork`], not a
/// second work budget.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CallResolverAccountingReport {
    logical_argument_checks: u64,
    resolver_invocations: u64,
    candidate_argument_probes: u64,
    selected_replay_argument_visits: u64,
    retained_argument_fact_publications: u64,
}

impl CallResolverAccountingReport {
    const ZERO: Self = Self {
        logical_argument_checks: 0,
        resolver_invocations: 0,
        candidate_argument_probes: 0,
        selected_replay_argument_visits: 0,
        retained_argument_fact_publications: 0,
    };

    pub const fn logical_argument_checks(self) -> u64 {
        self.logical_argument_checks
    }

    pub const fn resolver_invocations(self) -> u64 {
        self.resolver_invocations
    }

    pub const fn candidate_argument_probes(self) -> u64 {
        self.candidate_argument_probes
    }

    pub const fn selected_replay_argument_visits(self) -> u64 {
        self.selected_replay_argument_visits
    }

    pub const fn retained_argument_fact_publications(self) -> u64 {
        self.retained_argument_fact_publications
    }

    #[cfg(test)]
    pub(crate) fn delta_from(self, before: Self) -> Result<Self, CallableQueryLimitError> {
        Ok(Self {
            logical_argument_checks: self
                .logical_argument_checks
                .checked_sub(before.logical_argument_checks)
                .ok_or(CallableQueryLimitError::ArithmeticOverflow)?,
            resolver_invocations: self
                .resolver_invocations
                .checked_sub(before.resolver_invocations)
                .ok_or(CallableQueryLimitError::ArithmeticOverflow)?,
            candidate_argument_probes: self
                .candidate_argument_probes
                .checked_sub(before.candidate_argument_probes)
                .ok_or(CallableQueryLimitError::ArithmeticOverflow)?,
            selected_replay_argument_visits: self
                .selected_replay_argument_visits
                .checked_sub(before.selected_replay_argument_visits)
                .ok_or(CallableQueryLimitError::ArithmeticOverflow)?,
            retained_argument_fact_publications: self
                .retained_argument_fact_publications
                .checked_sub(before.retained_argument_fact_publications)
                .ok_or(CallableQueryLimitError::ArithmeticOverflow)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallResolverAccountingEvent {
    LogicalArgumentCheck,
    ResolverInvocation,
    CandidateArgumentProbe,
    SelectedReplayArgumentVisit,
    RetainedArgumentFactPublication,
}

/// Current registered-candidate recursion owned by one focused callable query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CallableQueryDepth {
    current: usize,
    limit: usize,
}

impl CallableQueryDepth {
    pub(crate) const fn new(limits: CallableLimits) -> Self {
        Self {
            current: 0,
            limit: limits.max_nested_calls(),
        }
    }

    pub(crate) fn try_enter(&mut self) -> Result<(), CallableQueryLimitError> {
        let actual = self
            .current
            .checked_add(1)
            .ok_or(CallableQueryLimitError::ArithmeticOverflow)?;
        if actual > self.limit {
            return Err(CallableQueryLimitError::NestedCalls {
                actual,
                limit: self.limit,
            });
        }
        self.current = actual;
        Ok(())
    }

    pub(crate) fn leave(&mut self) {
        self.current = self
            .current
            .checked_sub(1)
            .expect("focused callable depth exits exactly once");
    }

    pub(crate) const fn is_active(self) -> bool {
        self.current != 0
    }
}

impl ResolverWork {
    pub(crate) const fn new(limit: u64) -> Self {
        Self {
            consumed: 0,
            limit,
            resolver: 0,
            argument_mapping: 0,
            type_checks: 0,
            call: CallResolverAccountingReport::ZERO,
        }
    }

    #[cfg(test)]
    pub(crate) fn reset(&mut self) {
        self.consumed = 0;
        self.resolver = 0;
        self.argument_mapping = 0;
        self.type_checks = 0;
        self.call = CallResolverAccountingReport::ZERO;
    }

    pub(crate) fn charge(&mut self, units: u64) -> Result<(), CallableQueryLimitError> {
        self.charge_component(units, ResolverWorkComponent::Resolver)
    }

    pub(crate) fn charge_argument_mapping(
        &mut self,
        units: u64,
    ) -> Result<(), CallableQueryLimitError> {
        self.charge_component(units, ResolverWorkComponent::ArgumentMapping)
    }

    pub(crate) fn charge_type_check(&mut self, units: u64) -> Result<(), CallableQueryLimitError> {
        self.charge_component(units, ResolverWorkComponent::TypeCheck)
    }

    pub(crate) fn record_logical_argument_checks(
        &mut self,
        units: u64,
    ) -> Result<(), CallableQueryLimitError> {
        self.record_call_event(CallResolverAccountingEvent::LogicalArgumentCheck, units)
    }

    pub(crate) fn record_resolver_invocation(&mut self) -> Result<(), CallableQueryLimitError> {
        self.record_call_event(CallResolverAccountingEvent::ResolverInvocation, 1)
    }

    pub(crate) fn record_candidate_argument_probes(
        &mut self,
        units: u64,
    ) -> Result<(), CallableQueryLimitError> {
        self.record_call_event(CallResolverAccountingEvent::CandidateArgumentProbe, units)
    }

    pub(crate) fn record_selected_replay_argument_visits(
        &mut self,
        units: u64,
    ) -> Result<(), CallableQueryLimitError> {
        self.record_call_event(
            CallResolverAccountingEvent::SelectedReplayArgumentVisit,
            units,
        )
    }

    pub(crate) fn record_retained_argument_fact_publications(
        &mut self,
        units: u64,
    ) -> Result<(), CallableQueryLimitError> {
        self.record_call_event(
            CallResolverAccountingEvent::RetainedArgumentFactPublication,
            units,
        )
    }

    pub(crate) const fn call_accounting(&self) -> CallResolverAccountingReport {
        self.call
    }

    fn record_call_event(
        &mut self,
        event: CallResolverAccountingEvent,
        units: u64,
    ) -> Result<(), CallableQueryLimitError> {
        let counter = match event {
            CallResolverAccountingEvent::LogicalArgumentCheck => {
                &mut self.call.logical_argument_checks
            }
            CallResolverAccountingEvent::ResolverInvocation => &mut self.call.resolver_invocations,
            CallResolverAccountingEvent::CandidateArgumentProbe => {
                &mut self.call.candidate_argument_probes
            }
            CallResolverAccountingEvent::SelectedReplayArgumentVisit => {
                &mut self.call.selected_replay_argument_visits
            }
            CallResolverAccountingEvent::RetainedArgumentFactPublication => {
                &mut self.call.retained_argument_fact_publications
            }
        };
        *counter = counter
            .checked_add(units)
            .ok_or(CallableQueryLimitError::ArithmeticOverflow)?;
        Ok(())
    }

    fn charge_component(
        &mut self,
        units: u64,
        component: ResolverWorkComponent,
    ) -> Result<(), CallableQueryLimitError> {
        let next = self
            .consumed
            .checked_add(units)
            .ok_or(CallableQueryLimitError::ArithmeticOverflow)?;
        if next > self.limit {
            return Err(CallableQueryLimitError::Work {
                requested: units,
                consumed: self.consumed,
                limit: self.limit,
            });
        }
        let next_component = match component {
            ResolverWorkComponent::Resolver => self.resolver.checked_add(units),
            ResolverWorkComponent::ArgumentMapping => self.argument_mapping.checked_add(units),
            ResolverWorkComponent::TypeCheck => self.type_checks.checked_add(units),
        }
        .ok_or(CallableQueryLimitError::ArithmeticOverflow)?;
        self.consumed = next;
        match component {
            ResolverWorkComponent::Resolver => self.resolver = next_component,
            ResolverWorkComponent::ArgumentMapping => self.argument_mapping = next_component,
            ResolverWorkComponent::TypeCheck => self.type_checks = next_component,
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ResolverWorkComponent {
    Resolver,
    ArgumentMapping,
    TypeCheck,
}

/// Work performed while resolving and projecting one semantic signature query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureWorkReport {
    resolver: u64,
    argument_mapping: u64,
    type_checks: u64,
    call: CallResolverAccountingReport,
    recovery_nodes: usize,
    diagnostics: usize,
}

impl SignatureWorkReport {
    /// Builds the public work envelope from immutable final-call facts without
    /// replaying resolution or argument checking.
    pub(crate) fn from_final_call_facts(
        call: CallResolverAccountingReport,
        recovery_nodes: usize,
        diagnostics: usize,
        limits: &CallableLimits,
    ) -> Result<Self, CallableQueryLimitError> {
        Self::try_new(0, 0, 0, call, recovery_nodes, diagnostics, limits)
    }

    pub(crate) fn try_new(
        resolver: u64,
        argument_mapping: u64,
        type_checks: u64,
        call: CallResolverAccountingReport,
        recovery_nodes: usize,
        diagnostics: usize,
        limits: &CallableLimits,
    ) -> Result<Self, CallableQueryLimitError> {
        if recovery_nodes > limits.max_recovery_nodes() {
            return Err(CallableQueryLimitError::RecoveryNodes {
                actual: recovery_nodes,
                limit: limits.max_recovery_nodes(),
            });
        }
        if diagnostics > limits.max_diagnostics() {
            return Err(CallableQueryLimitError::Diagnostics {
                actual: diagnostics,
                limit: limits.max_diagnostics(),
            });
        }
        let report = Self {
            resolver,
            argument_mapping,
            type_checks,
            call,
            recovery_nodes,
            diagnostics,
        };
        let total = report.total_work()?;
        if total > limits.max_query_work() {
            return Err(CallableQueryLimitError::Work {
                requested: total,
                consumed: 0,
                limit: limits.max_query_work(),
            });
        }
        Ok(report)
    }

    pub const fn resolver(&self) -> u64 {
        self.resolver
    }
    pub const fn argument_mapping(&self) -> u64 {
        self.argument_mapping
    }
    pub const fn type_checks(&self) -> u64 {
        self.type_checks
    }
    pub const fn call_accounting(&self) -> CallResolverAccountingReport {
        self.call
    }
    pub const fn recovery_nodes(&self) -> usize {
        self.recovery_nodes
    }
    pub const fn diagnostics(&self) -> usize {
        self.diagnostics
    }

    pub fn total_work(&self) -> Result<u64, CallableQueryLimitError> {
        self.resolver
            .checked_add(self.argument_mapping)
            .and_then(|value| value.checked_add(self.type_checks))
            .ok_or(CallableQueryLimitError::ArithmeticOverflow)
    }
}

/// Search-stage work for the outer position-aware signature query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureQuerySearchWork {
    node_visits: u64,
    candidate_calls: u64,
    nested_calls: u64,
    arguments: u64,
    recovery_nodes: u64,
}

impl SignatureQuerySearchWork {
    pub const fn new(
        node_visits: u64,
        candidate_calls: u64,
        nested_calls: u64,
        arguments: u64,
        recovery_nodes: u64,
    ) -> Self {
        Self {
            node_visits,
            candidate_calls,
            nested_calls,
            arguments,
            recovery_nodes,
        }
    }

    pub const fn node_visits(self) -> u64 {
        self.node_visits
    }
    pub const fn candidate_calls(self) -> u64 {
        self.candidate_calls
    }
    pub const fn nested_calls(self) -> u64 {
        self.nested_calls
    }
    pub const fn arguments(self) -> u64 {
        self.arguments
    }
    pub const fn recovery_nodes(self) -> u64 {
        self.recovery_nodes
    }
}

/// Resolver and checker-transaction work charged by the outer query meter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureQueryResolutionWork {
    resolver: u64,
    argument_bindings: u64,
    specificity_checks: u64,
}

impl SignatureQueryResolutionWork {
    pub const fn new(resolver: u64, argument_bindings: u64, specificity_checks: u64) -> Self {
        Self {
            resolver,
            argument_bindings,
            specificity_checks,
        }
    }

    pub const fn resolver(self) -> u64 {
        self.resolver
    }
    pub const fn argument_bindings(self) -> u64 {
        self.argument_bindings
    }
    pub const fn specificity_checks(self) -> u64 {
        self.specificity_checks
    }
}

/// Public-result projection work charged by the outer query meter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureQueryProjectionWork {
    overloads: u64,
    parameters: u64,
    argument_projections: u64,
    diagnostic_considerations: u64,
}

impl SignatureQueryProjectionWork {
    pub const fn new(
        overloads: u64,
        parameters: u64,
        argument_projections: u64,
        diagnostic_considerations: u64,
    ) -> Self {
        Self {
            overloads,
            parameters,
            argument_projections,
            diagnostic_considerations,
        }
    }

    pub const fn overloads(self) -> u64 {
        self.overloads
    }
    pub const fn parameters(self) -> u64 {
        self.parameters
    }
    pub const fn argument_projections(self) -> u64 {
        self.argument_projections
    }
    pub const fn diagnostic_considerations(self) -> u64 {
        self.diagnostic_considerations
    }
}

/// Exact outer-query operation counts, separated by owning stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureQueryWorkReport {
    search: SignatureQuerySearchWork,
    resolution: SignatureQueryResolutionWork,
    projection: SignatureQueryProjectionWork,
    total: u64,
}

impl SignatureQueryWorkReport {
    pub const fn search(self) -> SignatureQuerySearchWork {
        self.search
    }
    pub const fn resolution(self) -> SignatureQueryResolutionWork {
        self.resolution
    }
    pub const fn projection(self) -> SignatureQueryProjectionWork {
        self.projection
    }

    pub const fn total_work(self) -> u64 {
        self.total
    }
}

/// Failure while charging caller-owned semantic signature work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureAccountingError {
    /// A configured signature-query limit was exceeded.
    Limit(SignatureLimitExceeded),
    /// A checked work counter overflowed without mutating the counter.
    Arithmetic { counter: SignatureWorkKind },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SignatureQueryWorkMeter {
    limits: SignatureQueryLimits,
    node_visits: u64,
    candidate_calls: u64,
    nested_calls: u64,
    arguments: u64,
    recovery_nodes: u64,
    resolver: u64,
    argument_bindings: u64,
    specificity_checks: u64,
    overloads: u64,
    parameters: u64,
    argument_projections: u64,
    diagnostic_considerations: u64,
    total: u64,
}

impl SignatureQueryWorkMeter {
    pub(crate) const fn new(limits: SignatureQueryLimits) -> Self {
        Self {
            limits,
            node_visits: 0,
            candidate_calls: 0,
            nested_calls: 0,
            arguments: 0,
            recovery_nodes: 0,
            resolver: 0,
            argument_bindings: 0,
            specificity_checks: 0,
            overloads: 0,
            parameters: 0,
            argument_projections: 0,
            diagnostic_considerations: 0,
            total: 0,
        }
    }

    pub(crate) fn charge(
        &mut self,
        kind: SignatureWorkKind,
        units: u64,
    ) -> Result<(), SignatureAccountingError> {
        let counter = self.counter(kind);
        let next_counter = counter
            .checked_add(units)
            .ok_or(SignatureAccountingError::Arithmetic { counter: kind })?;
        if let Some((limit_kind, maximum)) = self.operation_limit(kind)
            && next_counter > maximum
        {
            return Err(SignatureAccountingError::Limit(SignatureLimitExceeded {
                kind: limit_kind,
                observed: next_counter,
                maximum,
            }));
        }
        let next_total = self
            .total
            .checked_add(units)
            .ok_or(SignatureAccountingError::Arithmetic { counter: kind })?;
        if next_total > self.limits.work_units() {
            return Err(SignatureAccountingError::Limit(SignatureLimitExceeded {
                kind: SignatureLimitKind::WorkUnits,
                observed: next_total,
                maximum: self.limits.work_units(),
            }));
        }
        *self.counter_mut(kind) = next_counter;
        self.total = next_total;
        Ok(())
    }

    pub(crate) fn charge_parameter(
        &mut self,
        parameters_in_signature: &mut u64,
    ) -> Result<(), SignatureAccountingError> {
        let observed =
            parameters_in_signature
                .checked_add(1)
                .ok_or(SignatureAccountingError::Arithmetic {
                    counter: SignatureWorkKind::Parameters,
                })?;
        if observed > self.limits.parameters_per_signature() {
            return Err(SignatureAccountingError::Limit(SignatureLimitExceeded {
                kind: SignatureLimitKind::ParametersPerSignature,
                observed,
                maximum: self.limits.parameters_per_signature(),
            }));
        }
        self.charge(SignatureWorkKind::Parameters, 1)?;
        *parameters_in_signature = observed;
        Ok(())
    }

    pub(crate) fn report(&self) -> SignatureQueryWorkReport {
        SignatureQueryWorkReport {
            search: SignatureQuerySearchWork::new(
                self.node_visits,
                self.candidate_calls,
                self.nested_calls,
                self.arguments,
                self.recovery_nodes,
            ),
            resolution: SignatureQueryResolutionWork::new(
                self.resolver,
                self.argument_bindings,
                self.specificity_checks,
            ),
            projection: SignatureQueryProjectionWork::new(
                self.overloads,
                self.parameters,
                self.argument_projections,
                self.diagnostic_considerations,
            ),
            total: self.total,
        }
    }

    const fn counter(&self, kind: SignatureWorkKind) -> u64 {
        match kind {
            SignatureWorkKind::SourceBytes => 0,
            SignatureWorkKind::NodeVisits => self.node_visits,
            SignatureWorkKind::CandidateCalls => self.candidate_calls,
            SignatureWorkKind::NestedCalls => self.nested_calls,
            SignatureWorkKind::Arguments => self.arguments,
            SignatureWorkKind::RecoveryNodes => self.recovery_nodes,
            SignatureWorkKind::Resolver => self.resolver,
            SignatureWorkKind::ArgumentBindings => self.argument_bindings,
            SignatureWorkKind::SpecificityChecks => self.specificity_checks,
            SignatureWorkKind::Overloads => self.overloads,
            SignatureWorkKind::Parameters => self.parameters,
            SignatureWorkKind::ArgumentProjections => self.argument_projections,
            SignatureWorkKind::DiagnosticConsiderations => self.diagnostic_considerations,
        }
    }

    fn counter_mut(&mut self, kind: SignatureWorkKind) -> &mut u64 {
        match kind {
            SignatureWorkKind::SourceBytes => {
                unreachable!("source bytes are checked before the work meter is created")
            }
            SignatureWorkKind::NodeVisits => &mut self.node_visits,
            SignatureWorkKind::CandidateCalls => &mut self.candidate_calls,
            SignatureWorkKind::NestedCalls => &mut self.nested_calls,
            SignatureWorkKind::Arguments => &mut self.arguments,
            SignatureWorkKind::RecoveryNodes => &mut self.recovery_nodes,
            SignatureWorkKind::Resolver => &mut self.resolver,
            SignatureWorkKind::ArgumentBindings => &mut self.argument_bindings,
            SignatureWorkKind::SpecificityChecks => &mut self.specificity_checks,
            SignatureWorkKind::Overloads => &mut self.overloads,
            SignatureWorkKind::Parameters => &mut self.parameters,
            SignatureWorkKind::ArgumentProjections => &mut self.argument_projections,
            SignatureWorkKind::DiagnosticConsiderations => &mut self.diagnostic_considerations,
        }
    }

    const fn operation_limit(&self, kind: SignatureWorkKind) -> Option<(SignatureLimitKind, u64)> {
        match kind {
            SignatureWorkKind::CandidateCalls => Some((
                SignatureLimitKind::CandidateCalls,
                self.limits.candidate_calls(),
            )),
            SignatureWorkKind::NestedCalls => {
                Some((SignatureLimitKind::NestedCalls, self.limits.nested_calls()))
            }
            SignatureWorkKind::RecoveryNodes => Some((
                SignatureLimitKind::RecoveryNodes,
                self.limits.recovery_nodes(),
            )),
            SignatureWorkKind::Overloads => {
                Some((SignatureLimitKind::Overloads, self.limits.overloads()))
            }
            SignatureWorkKind::SourceBytes
            | SignatureWorkKind::NodeVisits
            | SignatureWorkKind::Arguments
            | SignatureWorkKind::Resolver
            | SignatureWorkKind::ArgumentBindings
            | SignatureWorkKind::SpecificityChecks
            | SignatureWorkKind::Parameters
            | SignatureWorkKind::ArgumentProjections
            | SignatureWorkKind::DiagnosticConsiderations => None,
        }
    }
}

#[cfg(test)]
mod final_call_accounting_tests {
    use super::*;

    #[test]
    fn final_call_counters_remain_separate_and_reset_together() {
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let before = work.call_accounting();
        work.record_logical_argument_checks(2)
            .expect("logical argument checks");
        work.record_resolver_invocation()
            .expect("one shared resolver entry");
        work.record_candidate_argument_probes(4)
            .expect("two candidates probe two arguments");
        work.record_selected_replay_argument_visits(2)
            .expect("selected replay visits each retained argument");
        work.record_retained_argument_fact_publications(2)
            .expect("fact publication retains each argument");

        let report = work
            .call_accounting()
            .delta_from(before)
            .expect("monotonic accounting delta");
        assert_eq!(report.logical_argument_checks(), 2);
        assert_eq!(report.resolver_invocations(), 1);
        assert_eq!(report.candidate_argument_probes(), 4);
        assert_eq!(report.selected_replay_argument_visits(), 2);
        assert_eq!(report.retained_argument_fact_publications(), 2);

        work.reset();
        assert_eq!(work.call_accounting(), CallResolverAccountingReport::ZERO);
    }

    #[test]
    fn signature_argument_projection_has_a_distinct_counter() {
        let mut meter = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
        meter
            .charge(SignatureWorkKind::Arguments, 3)
            .expect("surface argument traversal");
        meter
            .charge(SignatureWorkKind::ArgumentProjections, 2)
            .expect("public argument projection");
        let report = meter.report();
        assert_eq!(report.search().arguments(), 3);
        assert_eq!(report.projection().argument_projections(), 2);
        assert_eq!(report.total_work(), 5);
    }
}
