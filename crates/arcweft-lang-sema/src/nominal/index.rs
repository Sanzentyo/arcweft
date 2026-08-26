//! Immutable project-wide index of accepted final-HIR nominal-resolution facts.

use std::collections::{BTreeMap, HashMap};

use arcweft_lang_hir::identity::TypeId;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::types::TypeKind;

use super::{
    NominalAggregationLimits, NominalTypeDiagnostic, ResolvedTypeNode, ResolvedTypeRefOutcome,
    TypeResolutionReport,
};

/// Exact final-HIR address of one node in an accepted resolution graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalTypeNodeKey {
    root: TypeId,
    node: TypeId,
}

/// Failure to combine one otherwise valid per-reference report into a project index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalResolutionIndexError {
    /// The same final-HIR root was resolved under incompatible lexical contexts.
    ConflictingRoot { root: TypeId },
    /// A deliberately detached report cannot enter an accepted project index.
    DetachedReport { root: TypeId },
    /// The bounded project-wide resolution-work budget was exhausted.
    WorkLimit {
        attempted: u64,
        maximum: u64,
        root: TypeId,
    },
}

/// Accepted nominal facts retained by one type-check transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalResolutionIndex {
    reports: BTreeMap<TypeId, TypeResolutionReport>,
    nodes: BTreeMap<NominalTypeNodeKey, ResolvedTypeNode>,
    diagnostics: Vec<NominalTypeDiagnostic>,
    omitted_diagnostics: u64,
    work_charged: u64,
    limits: NominalAggregationLimits,
}

impl NominalTypeNodeKey {
    pub const fn new(root: TypeId, node: TypeId) -> Self {
        Self { root, node }
    }

    pub const fn root(self) -> TypeId {
        self.root
    }

    pub const fn node(self) -> TypeId {
        self.node
    }
}

impl NominalResolutionIndex {
    pub fn production() -> Self {
        Self::with_limits(NominalAggregationLimits::PRODUCTION)
    }

    pub(crate) fn with_limits(limits: NominalAggregationLimits) -> Self {
        Self {
            reports: BTreeMap::new(),
            nodes: BTreeMap::new(),
            diagnostics: Vec::new(),
            omitted_diagnostics: 0,
            work_charged: 0,
            limits,
        }
    }

    /// Returns the complete report for one exact final-HIR root.
    pub fn report(&self, root: TypeId) -> Option<&TypeResolutionReport> {
        self.reports.get(&root)
    }

    /// Returns the semantic type recovered for one exact final-HIR root.
    pub fn recovered_type(&self, root: TypeId) -> Option<&TypeKind> {
        self.report(root)
            .map(|report| report.outcome().product().recovered())
    }

    /// Returns one typed node fact without source-range reconstruction.
    pub fn node(&self, root: TypeId, node: TypeId) -> Option<&ResolvedTypeNode> {
        self.nodes.get(&NominalTypeNodeKey::new(root, node))
    }

    pub fn roots(&self) -> impl ExactSizeIterator<Item = TypeId> + '_ {
        self.reports.keys().copied()
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (NominalTypeNodeKey, &ResolvedTypeNode)> {
        self.nodes.iter().map(|(key, node)| (*key, node))
    }

    pub fn diagnostics(&self) -> &[NominalTypeDiagnostic] {
        &self.diagnostics
    }

    pub const fn omitted_diagnostics(&self) -> u64 {
        self.omitted_diagnostics
    }

    pub const fn work_charged(&self) -> u64 {
        self.work_charged
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for report in self.reports.values() {
            report.visit_types(visitor)?;
        }
        Ok(())
    }

    /// Publishes one accepted report under the exact root retained by the report itself.
    pub(crate) fn record(
        &mut self,
        report: TypeResolutionReport,
    ) -> Result<(), NominalResolutionIndexError> {
        let root = report.outcome().product().root();
        if matches!(report.outcome(), ResolvedTypeRefOutcome::Detached(_)) {
            return Err(NominalResolutionIndexError::DetachedReport { root });
        }
        if let Some(existing) = self.reports.get(&root) {
            return if existing == &report {
                Ok(())
            } else {
                Err(NominalResolutionIndexError::ConflictingRoot { root })
            };
        }

        let attempted = self.work_charged.saturating_add(report.work_charged());
        if attempted > self.limits.work_per_project() {
            return Err(NominalResolutionIndexError::WorkLimit {
                attempted,
                maximum: self.limits.work_per_project(),
                root,
            });
        }

        let node_facts = report
            .outcome()
            .product()
            .nodes()
            .iter()
            .map(|node| (NominalTypeNodeKey::new(root, node.node()), node.clone()))
            .collect::<Vec<_>>();

        self.work_charged = attempted;
        self.nodes.extend(node_facts);
        self.reports.insert(root, report);
        self.rebuild_diagnostics();
        Ok(())
    }

    fn rebuild_diagnostics(&mut self) {
        let mut omitted = self
            .reports
            .values()
            .map(TypeResolutionReport::omitted_diagnostics)
            .sum::<u64>();
        let mut diagnostics = self
            .reports
            .values()
            .flat_map(|report| report.diagnostics().iter().cloned())
            .collect::<Vec<_>>();
        diagnostics.sort_by(|left, right| {
            left.primary()
                .project()
                .cmp(&right.primary().project())
                .then_with(|| left.primary().local().cmp(&right.primary().local()))
                .then_with(|| left.kind().cmp(right.kind()))
                .then_with(|| left.poison().cmp(&right.poison()))
        });
        diagnostics.dedup();

        let per_document = usize::from(self.limits.diagnostics_per_document());
        let mut document_counts = HashMap::<SourceDocumentIdentity, usize>::new();
        diagnostics.retain(|diagnostic| {
            let Some(source) = diagnostic.primary().project().map(SourceSpan::source) else {
                omitted = omitted.saturating_add(1);
                return false;
            };
            let count = document_counts.entry(source.clone()).or_default();
            if *count >= per_document {
                omitted = omitted.saturating_add(1);
                return false;
            }
            *count += 1;
            true
        });

        let project_cap = usize::from(self.limits.diagnostics_per_project());
        if diagnostics.len() > project_cap {
            omitted = omitted.saturating_add((diagnostics.len() - project_cap) as u64);
            diagnostics.truncate(project_cap);
        }
        self.diagnostics = diagnostics;
        self.omitted_diagnostics = omitted;
    }
}

impl Default for NominalResolutionIndex {
    fn default() -> Self {
        Self::production()
    }
}
