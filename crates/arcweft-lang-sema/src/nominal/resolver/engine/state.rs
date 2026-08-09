use arcweft_lang_hir::{
    identity::TypeId,
    source_index::{HirSourceSite, HirTypeSourceRole},
    symbol::{
        ProjectTypeCandidate,
        nominal::{ProjectNominalBody, ProjectNominalDeclarationId},
    },
};

use crate::types::TypePoisonId;

use super::{
    NodeValue, NominalDiagnosticRelated, NominalRelatedMessage, NominalTypeDiagnostic,
    NominalTypeDiagnosticKind, ResolvedTypeNode, Resolver, SourceContext, TypeNameResolution,
    TypePoisonOrigin, TypePoisonRecord, TypeResolutionFailure, TypeSourceEvidence, diagnostic_kind,
    evidence_from_project, related_ordering,
};

impl Resolver<'_, '_> {
    pub(super) fn failed_node(
        &mut self,
        context: &SourceContext<'_>,
        node: TypeId,
        failure: TypeResolutionFailure,
        related: Vec<NominalDiagnosticRelated>,
    ) -> NodeValue {
        let poison = self.emit_failure(&failure, context.evidence(node, true), related);
        self.nodes.push(ResolvedTypeNode::new(
            node,
            context.evidence(node, false),
            context.terminal_evidence(node),
            context.reference_path(node),
            Some(crate::types::TypeKind::Error(poison)),
            TypeNameResolution::Failed(failure),
        ));
        NodeValue::error(poison, [])
    }

    pub(super) fn failed_name(
        &mut self,
        context: &SourceContext<'_>,
        node: TypeId,
        failure: TypeResolutionFailure,
        child_causes: Vec<TypePoisonId>,
        related: Vec<NominalDiagnosticRelated>,
    ) -> super::NameResult {
        let poison = self.emit_failure(&failure, context.evidence(node, true), related);
        super::NameResult {
            value: NodeValue::error(poison, child_causes),
            outcome: TypeNameResolution::Failed(failure),
        }
    }

    pub(super) fn emit_failure(
        &mut self,
        failure: &TypeResolutionFailure,
        primary: TypeSourceEvidence,
        related: Vec<NominalDiagnosticRelated>,
    ) -> TypePoisonId {
        let kind = diagnostic_kind(failure);
        if let Some(existing) = self
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.kind() == &kind && diagnostic.primary() == &primary)
        {
            return existing.poison();
        }
        let (related, omitted_related) =
            cap_related_labels(related, self.input.limits().related_labels_per_diagnostic());
        let work = 1_u64 + related.len() as u64;
        if let Err((attempted, maximum)) = self.charge(work) {
            return self.emit_work_overflow(primary, attempted, maximum);
        }
        let poison = self.allocate_poison();
        self.record_poison(
            poison,
            TypePoisonOrigin::NominalTypeDiagnostic,
            primary.clone(),
            true,
        );
        self.diagnostics.push(NominalTypeDiagnostic::new(
            poison,
            kind,
            primary,
            related,
            omitted_related,
        ));
        poison
    }

    fn emit_work_overflow(
        &mut self,
        primary: TypeSourceEvidence,
        attempted: u64,
        maximum: u64,
    ) -> TypePoisonId {
        if let Some(poison) = self.global_halt {
            return poison;
        }
        let poison = self.allocate_poison();
        self.record_poison(
            poison,
            TypePoisonOrigin::NominalTypeDiagnostic,
            primary.clone(),
            true,
        );
        self.diagnostics.push(NominalTypeDiagnostic::new(
            poison,
            NominalTypeDiagnosticKind::WorkOverflow { attempted, maximum },
            primary,
            [],
            0,
        ));
        self.work = maximum;
        self.global_halt = Some(poison);
        poison
    }

    pub(super) fn work_overflow_node(
        &mut self,
        context: &SourceContext<'_>,
        node: TypeId,
        attempted: u64,
        maximum: u64,
    ) -> NodeValue {
        let failure = TypeResolutionFailure::WorkOverflow { attempted, maximum };
        let poison = self.emit_work_overflow(context.evidence(node, true), attempted, maximum);
        self.nodes.push(ResolvedTypeNode::new(
            node,
            context.evidence(node, false),
            context.terminal_evidence(node),
            context.reference_path(node),
            Some(crate::types::TypeKind::Error(poison)),
            TypeNameResolution::Failed(failure),
        ));
        NodeValue::error(poison, [])
    }

    pub(super) fn charge(&mut self, units: u64) -> Result<(), (u64, u64)> {
        let maximum = self.input.limits().work_per_reference();
        let attempted = self.work.saturating_add(units);
        if attempted > maximum {
            return Err((attempted, maximum));
        }
        self.work = attempted;
        Ok(())
    }

    pub(super) fn charge_name_work(
        &mut self,
        units: u64,
        context: &SourceContext<'_>,
        node: TypeId,
        child_causes: Vec<TypePoisonId>,
    ) -> Option<super::NameResult> {
        let (attempted, maximum) = self.charge(units).err()?;
        let poison = self.emit_work_overflow(context.evidence(node, true), attempted, maximum);
        Some(super::NameResult {
            value: NodeValue::error(poison, child_causes),
            outcome: TypeNameResolution::Failed(TypeResolutionFailure::WorkOverflow {
                attempted,
                maximum,
            }),
        })
    }

    pub(super) fn allocate_poison(&mut self) -> TypePoisonId {
        loop {
            let index = self.next_poison_index;
            self.next_poison_index = self.next_poison_index.saturating_add(1);
            if !self.reserved_poison_indices.contains(&index)
                && !self
                    .poisons
                    .iter()
                    .any(|record| record.id().index() == index)
            {
                return TypePoisonId::from_index(index);
            }
            assert!(
                index != u32::MAX,
                "validated type references exhausted the resolver poison identity space"
            );
        }
    }

    pub(super) fn record_poison(
        &mut self,
        id: TypePoisonId,
        origin: TypePoisonOrigin,
        primary: TypeSourceEvidence,
        authoritative: bool,
    ) {
        if !self.poisons.iter().any(|record| record.id() == id) {
            self.poisons
                .push(TypePoisonRecord::new(id, origin, primary, authoritative));
        }
    }

    pub(super) fn replace_node_outcome(&mut self, owner: TypeId, outcome: TypeNameResolution) {
        if let Some(index) = self.nodes.iter().rposition(|node| node.node() == owner) {
            let source = self.nodes[index].source().clone();
            let terminal_source = self.nodes[index].terminal_source().cloned();
            let reference_path = self.nodes[index].reference_path().cloned();
            let recovered = self.nodes[index].recovered().cloned();
            self.nodes[index] = ResolvedTypeNode::new(
                owner,
                source,
                terminal_source,
                reference_path,
                recovered,
                outcome,
            );
        }
    }

    pub(super) fn candidate_related(
        candidates: &[ProjectTypeCandidate],
    ) -> Vec<NominalDiagnosticRelated> {
        candidates
            .iter()
            .flat_map(|candidate| {
                candidate
                    .declaration()
                    .cloned()
                    .map(|source| {
                        NominalDiagnosticRelated::new(
                            evidence_from_project(source),
                            NominalRelatedMessage::CandidateDeclaration,
                        )
                    })
                    .into_iter()
                    .chain(candidate.binding_sites().iter().cloned().map(|source| {
                        NominalDiagnosticRelated::new(
                            evidence_from_project(source),
                            NominalRelatedMessage::CandidateBinding,
                        )
                    }))
            })
            .collect()
    }

    pub(super) fn alias_cycle_related(
        &self,
        cycle: &[ProjectNominalDeclarationId],
    ) -> Vec<NominalDiagnosticRelated> {
        let Some(symbols) = self.input.world().symbols() else {
            return Vec::new();
        };
        cycle
            .iter()
            .filter_map(|id| symbols.nominal(id))
            .flat_map(|declaration| {
                let declaration_label = NominalDiagnosticRelated::new(
                    evidence_from_project(declaration.source().name().clone()),
                    NominalRelatedMessage::CycleDeclaration,
                );
                let target_label = match declaration.body() {
                    ProjectNominalBody::TypeAlias { target } => self
                        .input
                        .world()
                        .project()
                        .and_then(|project| project.module(declaration.id().module()))
                        .and_then(|module| {
                            module.type_source_site(*target, HirTypeSourceRole::Whole)
                        })
                        .and_then(|site| match site {
                            HirSourceSite::Span(span) => Some(NominalDiagnosticRelated::new(
                                TypeSourceEvidence::accepted(span.range(), span.clone()),
                                NominalRelatedMessage::CycleTarget,
                            )),
                            HirSourceSite::Insertion(_) => None,
                        }),
                    ProjectNominalBody::Struct { .. } | ProjectNominalBody::Enum { .. } => None,
                };
                core::iter::once(declaration_label).chain(target_label)
            })
            .collect()
    }
}

fn cap_related_labels(
    mut related: Vec<NominalDiagnosticRelated>,
    maximum: u16,
) -> (Vec<NominalDiagnosticRelated>, u64) {
    related.sort_by(related_ordering);
    related.dedup_by(|left, right| {
        left.source() == right.source() && left.message() == right.message()
    });
    let maximum = usize::from(maximum);
    let omitted = related.len().saturating_sub(maximum) as u64;
    related.truncate(maximum);
    (related, omitted)
}

#[cfg(test)]
mod tests {
    use arcweft_source::SourceRange;

    use crate::{
        nominal::{
            NominalDiagnosticRelated, NominalRelatedMessage, NominalTypeDiagnostic,
            NominalTypeDiagnosticKind, TypeSourceEvidence,
        },
        types::TypePoisonId,
    };

    use super::cap_related_labels;

    #[test]
    fn related_label_cap_retains_exact_omitted_count_after_ordering() {
        let related = (0..34)
            .rev()
            .map(|index| {
                NominalDiagnosticRelated::new(
                    TypeSourceEvidence::detached(SourceRange::new(index, index + 1)),
                    NominalRelatedMessage::CandidateBinding,
                )
            })
            .collect();
        let (retained, omitted) = cap_related_labels(related, 32);
        let diagnostic = NominalTypeDiagnostic::new(
            TypePoisonId::from_index(0),
            NominalTypeDiagnosticKind::SelfUnavailable,
            TypeSourceEvidence::detached(SourceRange::new(0, 0)),
            retained,
            omitted,
        );

        assert_eq!(diagnostic.secondary().len(), 32);
        assert_eq!(diagnostic.omitted_secondary(), 2);
        assert!(
            diagnostic
                .secondary()
                .windows(2)
                .all(|pair| pair[0].source().local() < pair[1].source().local())
        );
    }
}
