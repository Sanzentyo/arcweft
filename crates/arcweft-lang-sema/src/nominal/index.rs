//! Immutable project-wide index of accepted nominal-resolution facts.

use std::collections::{BTreeMap, HashMap};

use arcweft_lang_syntax::types::TypeRefNodePath;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::types::TypeKind;

use super::{
    NominalAggregationLimits, NominalTypeDiagnostic, ResolvedTypeNode, TypeResolutionReport,
};

/// Exact address of one structural node inside an accepted authored type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalTypeNodeKey {
    root: SourceSpan,
    node: TypeRefNodePath,
}

/// Failure to combine one otherwise valid per-reference report into a project index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalResolutionIndexError {
    /// The same accepted root was resolved under two incompatible lexical contexts.
    ConflictingRoot { root: SourceSpan },
    /// The bounded project-wide resolution-work budget was exhausted.
    WorkLimit {
        attempted: u64,
        maximum: u64,
        root: SourceSpan,
    },
    /// A resolver node lost the accepted source evidence required by this index.
    DetachedNode {
        root: SourceSpan,
        node: TypeRefNodePath,
    },
}

/// Accepted nominal facts retained by one type-check transaction.
///
/// Roots are keyed by their exact revision-bound source span. Structural node
/// facts additionally retain their typed path within that root, avoiding both
/// display parsing and ambiguous range-only lookup for nested parenthesized
/// forms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalResolutionIndex {
    reports: BTreeMap<SourceSpan, TypeResolutionReport>,
    nodes: BTreeMap<NominalTypeNodeKey, ResolvedTypeNode>,
    diagnostics: Vec<NominalTypeDiagnostic>,
    omitted_diagnostics: u64,
    work_charged: u64,
    limits: NominalAggregationLimits,
}

impl NominalTypeNodeKey {
    pub const fn new(root: SourceSpan, node: TypeRefNodePath) -> Self {
        Self { root, node }
    }

    pub const fn root(&self) -> &SourceSpan {
        &self.root
    }

    pub const fn node(&self) -> &TypeRefNodePath {
        &self.node
    }
}

impl NominalResolutionIndex {
    /// Empty index using the fixed production aggregation budget.
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

    /// Returns the complete report for one exact accepted authored root.
    pub fn report(&self, root: &SourceSpan) -> Option<&TypeResolutionReport> {
        self.reports.get(root)
    }

    /// Returns the semantic type recovered for one exact accepted authored root.
    pub fn recovered_type(&self, root: &SourceSpan) -> Option<&TypeKind> {
        self.report(root)
            .map(|report| report.outcome().product().recovered())
    }

    /// Returns one typed structural-node fact without reconstructing a path.
    pub fn node(&self, root: &SourceSpan, node: &TypeRefNodePath) -> Option<&ResolvedTypeNode> {
        self.nodes
            .get(&NominalTypeNodeKey::new(root.clone(), node.clone()))
    }

    /// Returns the semantic type recovered for one exact nested type node.
    ///
    /// Constant and entity-family argument nodes do not denote types and
    /// therefore return `None` even when their structural fact exists.
    pub fn recovered_node_type(
        &self,
        root: &SourceSpan,
        node: &TypeRefNodePath,
    ) -> Option<&TypeKind> {
        self.node(root, node).and_then(ResolvedTypeNode::recovered)
    }

    /// Accepted roots in deterministic source order.
    pub fn roots(&self) -> impl ExactSizeIterator<Item = &SourceSpan> {
        self.reports.keys()
    }

    /// Structural facts in deterministic root/path order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (&NominalTypeNodeKey, &ResolvedTypeNode)> {
        self.nodes.iter()
    }

    /// Bounded, deterministic project diagnostic inventory.
    pub fn diagnostics(&self) -> &[NominalTypeDiagnostic] {
        &self.diagnostics
    }

    /// Diagnostics omitted by per-reference, per-document, or project caps.
    pub const fn omitted_diagnostics(&self) -> u64 {
        self.omitted_diagnostics
    }

    /// Resolver work retained by the accepted transaction.
    pub const fn work_charged(&self) -> u64 {
        self.work_charged
    }

    pub(crate) fn record(
        &mut self,
        root: SourceSpan,
        report: TypeResolutionReport,
    ) -> Result<(), NominalResolutionIndexError> {
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
            .map(|node| {
                if node.source().project().is_none() {
                    return Err(NominalResolutionIndexError::DetachedNode {
                        root: root.clone(),
                        node: node.node().clone(),
                    });
                }
                Ok((
                    NominalTypeNodeKey::new(root.clone(), node.node().clone()),
                    node.clone(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arcweft_lang_syntax::{ast::common::TextRange, types::TypeRefNodePath};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSpan};

    use crate::types::{TypeKind, TypePoisonId};

    use super::{
        NominalResolutionIndex, NominalResolutionIndexError, NominalTypeDiagnostic,
        ResolvedTypeNode, TypeResolutionReport,
    };
    use crate::nominal::{
        BuiltinTypeConstructor, NominalTypeDiagnosticKind, ResolvedTypeProduct,
        ResolvedTypeRefOutcome, TypeNameResolution, TypeSourceEvidence,
    };

    fn document(name: &str, references: usize) -> Arc<SourceDocument> {
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("arcweft-project://nominal-index/{name}.arcw"))
                    .expect("document ID"),
                SourceName::path(format!("{name}.arcw")),
                "Self ".repeat(references),
            )
            .expect("source document"),
        )
    }

    fn recorded_report(root: &SourceSpan, local: TextRange, work: u64) -> TypeResolutionReport {
        let source = TypeSourceEvidence::accepted(local, root.clone());
        let poison = TypePoisonId::from_index(0);
        let diagnostic = NominalTypeDiagnostic::new(
            poison,
            NominalTypeDiagnosticKind::SelfUnavailable,
            source.clone(),
            [],
            0,
        );
        let node = ResolvedTypeNode::new(
            TypeRefNodePath::root(),
            source,
            None,
            None,
            Some(TypeKind::I32),
            TypeNameResolution::Builtin(BuiltinTypeConstructor::I32),
        );
        TypeResolutionReport::new(
            ResolvedTypeRefOutcome::Complete(ResolvedTypeProduct::new(TypeKind::I32, [node], [])),
            [diagnostic],
            [],
            0,
            work,
        )
    }

    fn root(document: &SourceDocument, index: usize) -> SourceSpan {
        let start = index * 5;
        document
            .span(SourceRange::new(start, start + 4))
            .expect("reference range is within the generated document")
    }

    #[test]
    fn production_diagnostic_caps_are_inclusive_and_report_omissions() {
        let document_cap =
            usize::from(super::NominalAggregationLimits::PRODUCTION.diagnostics_per_document());
        let document_source = document("document-cap", document_cap + 1);
        let mut index = NominalResolutionIndex::production();
        for position in 0..=document_cap {
            let root = root(&document_source, position);
            index
                .record(
                    root.clone(),
                    recorded_report(&root, TextRange::new(0, 4), 1),
                )
                .expect("diagnostic reports remain recordable");
        }
        assert_eq!(index.diagnostics().len(), document_cap);
        assert_eq!(index.omitted_diagnostics(), 1);

        let project_cap =
            usize::from(super::NominalAggregationLimits::PRODUCTION.diagnostics_per_project());
        let mut index = NominalResolutionIndex::production();
        let mut recorded = 0;
        let per_document = document_cap;
        while recorded <= project_cap {
            let count = (project_cap + 1 - recorded).min(per_document);
            let document = document(&format!("project-cap-{recorded}"), count);
            for position in 0..count {
                let root = root(&document, position);
                index
                    .record(
                        root.clone(),
                        recorded_report(&root, TextRange::new(0, 4), 1),
                    )
                    .expect("diagnostic reports remain recordable");
                recorded += 1;
            }
        }
        assert_eq!(index.diagnostics().len(), project_cap);
        assert_eq!(index.omitted_diagnostics(), 1);
    }

    #[test]
    fn production_work_cap_is_inclusive_and_rejects_one_over_with_typed_counts() {
        let maximum = super::NominalAggregationLimits::PRODUCTION.work_per_project();
        let document = document("work-cap", 2);
        let first = root(&document, 0);
        let mut index = NominalResolutionIndex::production();
        index
            .record(
                first.clone(),
                recorded_report(&first, TextRange::new(0, 4), maximum),
            )
            .expect("the exact project work limit is accepted");
        assert_eq!(index.work_charged(), maximum);

        let second = root(&document, 1);
        assert!(matches!(
            index.record(
                second.clone(),
                recorded_report(&second, TextRange::new(5, 9), 1),
            ),
            Err(NominalResolutionIndexError::WorkLimit {
                attempted,
                maximum: actual_maximum,
                root,
            }) if attempted == maximum + 1 && actual_maximum == maximum && root == second
        ));
        assert_eq!(index.work_charged(), maximum);
    }
}
