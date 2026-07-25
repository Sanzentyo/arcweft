//! Inclusive allocation budgets for the staged full-source grammar.

#![allow(
    dead_code,
    reason = "the shadow grammar remains crate-private until the atomic syntax switch"
)]

use std::collections::BTreeSet;

use arcweft_source::SourceRange;

use super::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use super::kinds::{IdentityClass, SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

/// Transaction-local budget state shared by every parser over one document.
///
/// A one-over start event is rejected before it enters the event vector. Once
/// a limit fails, the parser may continue consuming source tokens for control
/// flow, but this state accepts no more events and the whole shadow build is
/// discarded.
#[derive(Debug, Default)]
pub(crate) struct GrammarBudget {
    stack: Vec<BudgetFrame>,
    prefix_depth: usize,
    top_level_items: usize,
    statements: usize,
    expressions: usize,
    type_nodes: usize,
    pattern_nodes: usize,
    identity_nodes: usize,
    diagnostics: BTreeSet<DiagnosticKey>,
    failure: Option<SyntaxLimit>,
}

#[derive(Debug)]
struct BudgetFrame {
    kind: SyntaxKind,
    generic_parameters: usize,
    where_predicates: usize,
    contract_clauses: usize,
    predicate_parameters: usize,
    proof_parameters: usize,
    assertion_conditions: usize,
    fixed_parameters: usize,
    declaration_members: usize,
    activity_ports: usize,
    metric_labels: usize,
    metric_buckets: usize,
    view_exports: usize,
    layer_members: usize,
}

impl BudgetFrame {
    const fn new(kind: SyntaxKind) -> Self {
        Self {
            kind,
            generic_parameters: 0,
            where_predicates: 0,
            contract_clauses: 0,
            predicate_parameters: 0,
            proof_parameters: 0,
            assertion_conditions: 0,
            fixed_parameters: 0,
            declaration_members: 0,
            activity_ports: 0,
            metric_labels: 0,
            metric_buckets: 0,
            view_exports: 0,
            layer_members: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DiagnosticKey {
    code: &'static str,
    start: usize,
    end: usize,
    related_start: Option<usize>,
    related_end: Option<usize>,
    message: String,
}

impl GrammarBudget {
    /// Enters one prefix-expression ancestor on the current typed expression
    /// path before emitting its node.
    pub(crate) fn enter_prefix_expression(&mut self) -> bool {
        if self.failure.is_some() {
            return false;
        }
        if let Err(limit) = charge(&mut self.prefix_depth, SyntaxLimit::PrefixDepth) {
            self.failure = Some(limit);
            return false;
        }
        true
    }

    /// Leaves one successfully entered prefix-expression ancestor.
    pub(crate) fn leave_prefix_expression(&mut self) {
        self.prefix_depth = self
            .prefix_depth
            .checked_sub(1)
            .expect("prefix-expression budget leaves only accepted entries");
    }

    /// Accepts one node start if doing so stays inside every inclusive budget.
    pub(crate) fn start(&mut self, kind: SyntaxKind, role: SyntaxRole) -> bool {
        if self.failure.is_some() {
            return false;
        }
        if let Err(limit) = self.charge_start(kind, role) {
            self.failure = Some(limit);
            return false;
        }
        self.stack.push(BudgetFrame::new(kind));
        true
    }

    /// Charges one direct condition in the current assertion statement.
    pub(crate) fn assertion_condition(&mut self) -> bool {
        if self.failure.is_some() {
            return false;
        }
        let Some(assertion) = self
            .stack
            .iter()
            .rposition(|frame| frame.kind == SyntaxKind::AssertionStatement)
        else {
            return true;
        };
        if let Err(limit) = charge(
            &mut self.stack[assertion].assertion_conditions,
            SyntaxLimit::AssertionConditions,
        ) {
            self.failure = Some(limit);
            return false;
        }
        true
    }

    /// Completes the most recently accepted node.
    pub(crate) fn finish(&mut self) -> bool {
        if self.failure.is_some() {
            return false;
        }
        self.stack.pop().is_some()
    }

    /// Accepts a non-node event, charging normalized diagnostics once.
    pub(crate) fn event(&mut self, event: &SyntaxEvent) -> bool {
        if self.failure.is_some() {
            return false;
        }
        let SyntaxEvent::Diagnostic(diagnostic) = event else {
            return true;
        };
        let key = DiagnosticKey::from(diagnostic);
        if self.diagnostics.contains(&key) {
            return true;
        }
        if self.diagnostics.len() >= SyntaxLimit::Diagnostics.maximum() {
            self.failure = Some(SyntaxLimit::Diagnostics);
            return false;
        }
        self.diagnostics.insert(key);
        true
    }

    /// First inclusive budget exceeded by the deterministic event stream.
    pub(crate) const fn failure(&self) -> Option<SyntaxLimit> {
        self.failure
    }

    fn charge_start(&mut self, kind: SyntaxKind, role: SyntaxRole) -> Result<(), SyntaxLimit> {
        if kind.identity_class() == IdentityClass::IdentityBearing {
            charge(&mut self.identity_nodes, SyntaxLimit::IdentityBearingNodes)?;
        }
        if kind.is_statement() {
            charge(&mut self.statements, SyntaxLimit::Statements)?;
        }
        if kind.is_expression() {
            charge(&mut self.expressions, SyntaxLimit::Expressions)?;
        }
        if kind.is_type_node() {
            charge(&mut self.type_nodes, SyntaxLimit::TypeNodes)?;
        }
        if kind.is_pattern_node() {
            charge(&mut self.pattern_nodes, SyntaxLimit::PatternNodes)?;
        }
        if kind.is_item() && !self.stack.iter().any(|frame| frame.kind.is_item()) {
            charge(&mut self.top_level_items, SyntaxLimit::TopLevelItems)?;
        }

        if kind.is_retained_declaration_member() {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        match kind {
            SyntaxKind::ActivityPort => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.activity_ports, SyntaxLimit::ActivityPorts)?;
            }
            SyntaxKind::MetricLabel => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.metric_labels, SyntaxLimit::MetricLabels)?;
            }
            SyntaxKind::ViewExportDeclaration => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.view_exports, SyntaxLimit::ViewExports)?;
            }
            SyntaxKind::LayerMember => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.layer_members, SyntaxLimit::LayerMembers)?;
            }
            _ => {}
        }
        if matches!(role, SyntaxRole::Bucket(_)) && kind.is_expression() {
            let frame = self.declaration_frame_mut()?;
            charge(&mut frame.metric_buckets, SyntaxLimit::MetricBuckets)?;
        }

        match kind {
            SyntaxKind::GenericParameter => {
                let frame = self.declaration_frame_mut()?;
                charge(
                    &mut frame.generic_parameters,
                    SyntaxLimit::GenericParameters,
                )?;
            }
            SyntaxKind::WherePredicate => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.where_predicates, SyntaxLimit::WherePredicates)?;
            }
            SyntaxKind::RequiresClause | SyntaxKind::EnsuresClause => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.contract_clauses, SyntaxLimit::ContractClauses)?;
            }
            SyntaxKind::Parameter => {
                let frame = self.declaration_frame_mut()?;
                match frame.kind {
                    SyntaxKind::ViewDeclarationItem | SyntaxKind::ActionDeclarationItem => {
                        charge(&mut frame.fixed_parameters, SyntaxLimit::FixedParameters)?;
                    }
                    SyntaxKind::PredicateItem => charge(
                        &mut frame.predicate_parameters,
                        SyntaxLimit::PredicateParameters,
                    )?,
                    SyntaxKind::ProofItem => {
                        charge(&mut frame.proof_parameters, SyntaxLimit::ProofParameters)?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn declaration_frame_mut(&mut self) -> Result<&mut BudgetFrame, SyntaxLimit> {
        self.stack
            .iter_mut()
            .rev()
            .find(|frame| frame.kind.is_item())
            .ok_or(SyntaxLimit::TopLevelItems)
    }

    fn is_direct_assertion_condition(&self, kind: SyntaxKind, role: SyntaxRole) -> bool {
        if !kind.is_expression() || role != SyntaxRole::Condition {
            return false;
        }
        let Some(assertion) = self
            .stack
            .iter()
            .rposition(|frame| frame.kind == SyntaxKind::AssertionStatement)
        else {
            return false;
        };
        !self.stack[assertion + 1..]
            .iter()
            .any(|frame| frame.kind.is_expression())
    }
}

impl From<&PendingSyntaxDiagnostic> for DiagnosticKey {
    fn from(diagnostic: &PendingSyntaxDiagnostic) -> Self {
        let range = diagnostic.range();
        Self {
            code: diagnostic.code(),
            start: range.start(),
            end: range.end(),
            related_start: diagnostic.related_range().map(SourceRange::start),
            related_end: diagnostic.related_range().map(SourceRange::end),
            message: diagnostic.message().to_owned(),
        }
    }
}

fn charge(counter: &mut usize, limit: SyntaxLimit) -> Result<(), SyntaxLimit> {
    if *counter >= limit.maximum() {
        return Err(limit);
    }
    *counter += 1;
    Ok(())
}

/// Revalidates budget behavior for event streams constructed outside the
/// shared parser cursor, including direct event-builder tests.
pub(crate) fn validate_events(events: &[SyntaxEvent]) -> Result<(), SyntaxLimit> {
    let mut budget = GrammarBudget::default();
    for event in events {
        match event {
            SyntaxEvent::StartNode { kind, role } => {
                if budget.is_direct_assertion_condition(*kind, *role)
                    && !budget.assertion_condition()
                {
                    return Err(budget
                        .failure()
                        .expect("failed assertion budget has a limit"));
                }
                if !budget.start(*kind, *role) {
                    return Err(budget.failure().expect("failed budget start has a limit"));
                }
            }
            SyntaxEvent::FinishNode => {
                budget.finish();
            }
            _ => {
                if !budget.event(event) {
                    return Err(budget.failure().expect("failed budget event has a limit"));
                }
            }
        }
    }
    budget.failure().map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use arcweft_source::SourceRange;

    use super::{GrammarBudget, PendingSyntaxDiagnostic, SyntaxEvent, SyntaxKind, SyntaxRole};
    use crate::incremental::SyntaxLimit;

    #[test]
    fn top_level_item_budget_accepts_exact_limit_and_rejects_one_over() {
        let mut budget = document_budget();
        for ordinal in 0..SyntaxLimit::TopLevelItems.maximum() {
            assert!(budget.start(
                SyntaxKind::ErrorItem,
                SyntaxRole::Element(u32::try_from(ordinal).expect("budget fits u32")),
            ));
            assert!(budget.finish());
        }
        assert!(!budget.start(
            SyntaxKind::ErrorItem,
            SyntaxRole::Element(
                u32::try_from(SyntaxLimit::TopLevelItems.maximum()).expect("budget fits u32"),
            ),
        ));
        assert_eq!(budget.failure(), Some(SyntaxLimit::TopLevelItems));

        let mut fresh = document_budget();
        assert!(fresh.start(SyntaxKind::ErrorItem, SyntaxRole::Element(0)));
    }

    #[test]
    fn identity_node_budget_accepts_exact_limit_and_rejects_one_over() {
        let mut budget = GrammarBudget::default();
        for _ in 0..SyntaxLimit::IdentityBearingNodes.maximum() {
            assert!(budget.start(SyntaxKind::NameDefinition, SyntaxRole::Name));
            assert!(budget.finish());
        }
        assert!(!budget.start(SyntaxKind::NameDefinition, SyntaxRole::Name));
        assert_eq!(budget.failure(), Some(SyntaxLimit::IdentityBearingNodes));

        let mut fresh = GrammarBudget::default();
        assert!(fresh.start(SyntaxKind::NameDefinition, SyntaxRole::Name));
    }

    #[test]
    fn diagnostic_budget_accepts_exact_limit_and_rejects_one_over() {
        let mut budget = GrammarBudget::default();
        for ordinal in 0..SyntaxLimit::Diagnostics.maximum() {
            assert!(budget.event(&diagnostic_event(ordinal)));
        }
        assert!(!budget.event(&diagnostic_event(SyntaxLimit::Diagnostics.maximum())));
        assert_eq!(budget.failure(), Some(SyntaxLimit::Diagnostics));

        let mut fresh = GrammarBudget::default();
        assert!(fresh.event(&diagnostic_event(0)));
    }

    #[test]
    fn prefix_depth_budget_accepts_exact_limit_and_rejects_one_over() {
        let mut budget = GrammarBudget::default();
        for _ in 0..SyntaxLimit::PrefixDepth.maximum() {
            assert!(budget.enter_prefix_expression());
        }
        assert!(!budget.enter_prefix_expression());
        assert_eq!(budget.failure(), Some(SyntaxLimit::PrefixDepth));
        for _ in 0..SyntaxLimit::PrefixDepth.maximum() {
            budget.leave_prefix_expression();
        }
        assert_eq!(budget.prefix_depth, 0);

        let mut fresh = GrammarBudget::default();
        for _ in 0..=SyntaxLimit::PrefixDepth.maximum() {
            assert!(fresh.enter_prefix_expression());
            fresh.leave_prefix_expression();
        }
    }

    fn document_budget() -> GrammarBudget {
        let mut budget = GrammarBudget::default();
        assert!(budget.start(SyntaxKind::SourceFile, SyntaxRole::Root));
        assert!(budget.start(SyntaxKind::ItemList, SyntaxRole::Element(0)));
        budget
    }

    fn diagnostic_event(ordinal: usize) -> SyntaxEvent {
        SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.test.budget",
            SourceRange::new(ordinal, ordinal),
            ordinal.to_string(),
        ))
    }
}
