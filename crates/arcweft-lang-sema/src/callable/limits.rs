//! Inclusive callable catalog and query limits.
#![allow(
    dead_code,
    reason = "work counters are consumed by the following catalog and resolver cuts"
)]

use super::{CallableBuildLimitError, CallableQueryLimitError};

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
            max_nested_calls: 32,
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

    pub(crate) fn charge(&mut self, units: u64) -> Result<(), CallableBuildLimitError> {
        let next = self
            .consumed
            .checked_add(units)
            .ok_or(CallableBuildLimitError::Work {
                requested: units,
                consumed: self.consumed,
                limit: self.limit,
            })?;
        if next > self.limit {
            return Err(CallableBuildLimitError::Work {
                requested: units,
                consumed: self.consumed,
                limit: self.limit,
            });
        }
        self.consumed = next;
        Ok(())
    }

    pub(crate) const fn consumed(self) -> u64 {
        self.consumed
    }
    pub(crate) const fn remaining(self) -> u64 {
        self.limit - self.consumed
    }
    pub(crate) const fn limit(self) -> u64 {
        self.limit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolverWork {
    consumed: u64,
    limit: u64,
}

impl ResolverWork {
    pub(crate) const fn new(limit: u64) -> Self {
        Self { consumed: 0, limit }
    }

    pub(crate) fn charge(&mut self, units: u64) -> Result<(), CallableQueryLimitError> {
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
        self.consumed = next;
        Ok(())
    }

    pub(crate) const fn consumed(self) -> u64 {
        self.consumed
    }
    pub(crate) const fn remaining(self) -> u64 {
        self.limit - self.consumed
    }
    pub(crate) const fn limit(self) -> u64 {
        self.limit
    }
}

/// Work performed while resolving and projecting one semantic signature query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureWorkReport {
    resolver: u64,
    argument_mapping: u64,
    type_checks: u64,
    recovery_nodes: usize,
    diagnostics: usize,
}

impl SignatureWorkReport {
    pub fn try_new(
        resolver: u64,
        argument_mapping: u64,
        type_checks: u64,
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
