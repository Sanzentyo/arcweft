//! Inclusive allocation budgets for the accepted full-source grammar.

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
    function_parameter_groups: usize,
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
            function_parameter_groups: 0,
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
    range: SourceRange,
    related_range: Option<SourceRange>,
}

/// Immutable work accounting committed with one accepted grammar snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxParseStats {
    accepted_source_bytes: usize,
    lexer_tokens: usize,
    grammar_events: usize,
    top_level_items: usize,
    statements: usize,
    expressions: usize,
    type_nodes: usize,
    pattern_nodes: usize,
    identity_bearing_nodes: usize,
    diagnostic_identities: usize,
}

/// Parser-control depths that a bounded candidate must preserve exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GrammarParserDepths {
    owners: usize,
    prefixes: usize,
}

impl SyntaxParseStats {
    /// Zero work, used only as the identity when aggregating already accepted
    /// module publications.
    pub const ZERO: Self = Self {
        accepted_source_bytes: 0,
        lexer_tokens: 0,
        grammar_events: 0,
        top_level_items: 0,
        statements: 0,
        expressions: 0,
        type_nodes: 0,
        pattern_nodes: 0,
        identity_bearing_nodes: 0,
        diagnostic_identities: 0,
    };

    /// Adds two accepted work records without silently wrapping any physical
    /// counter.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            accepted_source_bytes: self
                .accepted_source_bytes
                .checked_add(other.accepted_source_bytes)?,
            lexer_tokens: self.lexer_tokens.checked_add(other.lexer_tokens)?,
            grammar_events: self.grammar_events.checked_add(other.grammar_events)?,
            top_level_items: self.top_level_items.checked_add(other.top_level_items)?,
            statements: self.statements.checked_add(other.statements)?,
            expressions: self.expressions.checked_add(other.expressions)?,
            type_nodes: self.type_nodes.checked_add(other.type_nodes)?,
            pattern_nodes: self.pattern_nodes.checked_add(other.pattern_nodes)?,
            identity_bearing_nodes: self
                .identity_bearing_nodes
                .checked_add(other.identity_bearing_nodes)?,
            diagnostic_identities: self
                .diagnostic_identities
                .checked_add(other.diagnostic_identities)?,
        })
    }

    /// Confirms that committed source/diagnostic owners match this work record.
    pub(crate) const fn matches_publication(
        self,
        accepted_source_bytes: usize,
        diagnostic_identities: usize,
    ) -> bool {
        self.accepted_source_bytes == accepted_source_bytes
            && self.diagnostic_identities == diagnostic_identities
    }

    pub const fn accepted_source_bytes(self) -> usize {
        self.accepted_source_bytes
    }

    pub const fn lexer_tokens(self) -> usize {
        self.lexer_tokens
    }

    pub const fn grammar_events(self) -> usize {
        self.grammar_events
    }

    pub const fn top_level_items(self) -> usize {
        self.top_level_items
    }

    pub const fn statements(self) -> usize {
        self.statements
    }

    pub const fn expressions(self) -> usize {
        self.expressions
    }

    pub const fn type_nodes(self) -> usize {
        self.type_nodes
    }

    pub const fn pattern_nodes(self) -> usize {
        self.pattern_nodes
    }

    pub const fn identity_bearing_nodes(self) -> usize {
        self.identity_bearing_nodes
    }

    pub const fn diagnostic_identities(self) -> usize {
        self.diagnostic_identities
    }
}

impl GrammarBudget {
    #[cfg(test)]
    pub(crate) fn with_test_global_count(limit: SyntaxLimit, already_charged: usize) -> Self {
        assert!(already_charged <= limit.maximum());
        let mut budget = Self::default();
        match limit {
            SyntaxLimit::Statements => budget.statements = already_charged,
            SyntaxLimit::Expressions => budget.expressions = already_charged,
            SyntaxLimit::TypeNodes => budget.type_nodes = already_charged,
            SyntaxLimit::PatternNodes => budget.pattern_nodes = already_charged,
            SyntaxLimit::IdentityBearingNodes => budget.identity_nodes = already_charged,
            _ => panic!("{limit:?} is not a global grammar-node budget"),
        }
        budget
    }

    /// Control depths preserved by one parser-local candidate transaction.
    ///
    /// Work counters deliberately do not roll back: both bounded
    /// postfix-bracket attempts consume the shared document budget even when
    /// only one event stream is published.
    pub(crate) const fn parser_depths(&self) -> GrammarParserDepths {
        GrammarParserDepths {
            owners: self.stack.len(),
            prefixes: self.prefix_depth,
        }
    }

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

    /// Charges one grouped-use member against its owning use declaration.
    pub(crate) fn grouped_use_member(&mut self) -> bool {
        if self.failure.is_some() {
            return false;
        }
        let Some(use_declaration) = self
            .stack
            .iter()
            .rposition(|frame| frame.kind == SyntaxKind::UseDeclaration)
        else {
            self.failure = Some(SyntaxLimit::DeclarationMembers);
            return false;
        };
        if let Err(limit) = charge(
            &mut self.stack[use_declaration].declaration_members,
            SyntaxLimit::DeclarationMembers,
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

    /// Freezes the work already charged by this exact grammar transaction.
    pub(crate) fn final_stats(
        &self,
        accepted_source_bytes: usize,
        lexer_tokens: usize,
        grammar_events: usize,
    ) -> SyntaxParseStats {
        SyntaxParseStats {
            accepted_source_bytes,
            lexer_tokens,
            grammar_events,
            top_level_items: self.top_level_items,
            statements: self.statements,
            expressions: self.expressions,
            type_nodes: self.type_nodes,
            pattern_nodes: self.pattern_nodes,
            identity_bearing_nodes: self.identity_nodes,
            diagnostic_identities: self.diagnostics.len(),
        }
    }

    fn charge_start(&mut self, kind: SyntaxKind, role: SyntaxRole) -> Result<(), SyntaxLimit> {
        self.charge_node_shape(kind)?;
        self.validate_style_nesting(kind)?;
        self.charge_declaration_member(kind, role)?;
        self.charge_specialized_declaration_shape(kind, role)?;
        self.charge_generic_contract_or_parameter(kind)
    }

    fn charge_node_shape(&mut self, kind: SyntaxKind) -> Result<(), SyntaxLimit> {
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
        Ok(())
    }

    fn charge_specialized_declaration_shape(
        &mut self,
        kind: SyntaxKind,
        role: SyntaxRole,
    ) -> Result<(), SyntaxLimit> {
        match kind {
            SyntaxKind::FixedParameterGroup => {
                let frame = self.declaration_frame_mut()?;
                if frame.kind == SyntaxKind::FunctionItem {
                    charge(
                        &mut frame.function_parameter_groups,
                        SyntaxLimit::FixedParameters,
                    )?;
                }
            }
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
            SyntaxKind::ErrorDeclarationMember
                if self
                    .stack
                    .iter()
                    .rev()
                    .find(|frame| frame.kind.is_item())
                    .is_some_and(|frame| frame.kind == SyntaxKind::LayerDeclarationItem) =>
            {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.layer_members, SyntaxLimit::LayerMembers)?;
            }
            _ => {}
        }
        if matches!(role, SyntaxRole::Bucket(_)) && kind.is_expression() {
            let frame = self.declaration_frame_mut()?;
            charge(&mut frame.metric_buckets, SyntaxLimit::MetricBuckets)?;
        }
        Ok(())
    }

    fn charge_generic_contract_or_parameter(
        &mut self,
        kind: SyntaxKind,
    ) -> Result<(), SyntaxLimit> {
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
            kind if kind.is_contract_clause() => {
                let frame = self.declaration_frame_mut()?;
                charge(&mut frame.contract_clauses, SyntaxLimit::ContractClauses)?;
            }
            SyntaxKind::Parameter => {
                let recovered_parameter = self
                    .stack
                    .iter()
                    .rev()
                    .take_while(|frame| !frame.kind.is_item())
                    .any(|frame| frame.kind == SyntaxKind::ErrorNode);
                if recovered_parameter {
                    return Ok(());
                }
                let frame = self.declaration_frame_mut()?;
                match frame.kind {
                    SyntaxKind::FunctionItem
                    | SyntaxKind::ViewDeclarationItem
                    | SyntaxKind::ActionDeclarationItem
                    | SyntaxKind::FlowItem => {
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

    fn charge_declaration_member(
        &mut self,
        kind: SyntaxKind,
        role: SyntaxRole,
    ) -> Result<(), SyntaxLimit> {
        self.charge_retained_declaration_member(kind)?;
        if kind == SyntaxKind::ResourceFieldInitializer
            && self
                .stack
                .iter()
                .rev()
                .find(|frame| frame.kind.is_item())
                .is_some_and(|frame| frame.kind == SyntaxKind::ResourceDeclarationItem)
        {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        let is_activity_nested_member = matches!(
            kind,
            SyntaxKind::ActivityPort
                | SyntaxKind::RequiresClause
                | SyntaxKind::EnsuresClause
                | SyntaxKind::ErrorDeclarationMember
        ) && self
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind.is_item())
            .is_some_and(|frame| frame.kind == SyntaxKind::ActivityDeclarationItem);
        if is_activity_nested_member {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        let is_metric_nested_member = matches!(
            kind,
            SyntaxKind::MetricLabel | SyntaxKind::ErrorDeclarationMember
        ) && self
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind.is_item())
            .is_some_and(|frame| frame.kind == SyntaxKind::MetricDeclarationItem);
        if is_metric_nested_member {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        let is_layer_recovery_member = kind == SyntaxKind::ErrorDeclarationMember
            && self
                .stack
                .iter()
                .rev()
                .find(|frame| frame.kind.is_item())
                .is_some_and(|frame| frame.kind == SyntaxKind::LayerDeclarationItem);
        if is_layer_recovery_member {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        if kind == SyntaxKind::RecordField
            && let Some(frame) = self
                .stack
                .iter_mut()
                .rev()
                .find(|frame| frame.kind.is_item())
            && matches!(frame.kind, SyntaxKind::StructItem | SyntaxKind::EnumItem)
        {
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        let is_inline_item_member = matches!(
            kind,
            SyntaxKind::TypeAliasItem | SyntaxKind::FunctionItem | SyntaxKind::ErrorItem
        ) && self
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind.is_item())
            .is_some_and(|frame| {
                matches!(
                    frame.kind,
                    SyntaxKind::ExternCapabilityItem | SyntaxKind::TraitItem | SyntaxKind::ImplItem
                )
            });
        if is_inline_item_member {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
        }
        self.charge_style_member(kind, role)?;
        Ok(())
    }

    fn charge_retained_declaration_member(&mut self, kind: SyntaxKind) -> Result<(), SyntaxLimit> {
        if !kind.is_retained_declaration_member() {
            return Ok(());
        }
        let frame = self.declaration_frame_mut()?;
        charge(
            &mut frame.declaration_members,
            SyntaxLimit::DeclarationMembers,
        )
    }

    fn validate_style_nesting(&self, kind: SyntaxKind) -> Result<(), SyntaxLimit> {
        let one_over = kind == SyntaxKind::StyleEnvironmentBlock
            && self
                .stack
                .iter()
                .filter(|frame| frame.kind == SyntaxKind::StyleEnvironmentBlock)
                .count()
                >= SyntaxLimit::StyleNestingDepth.maximum();
        if one_over {
            Err(SyntaxLimit::StyleNestingDepth)
        } else {
            Ok(())
        }
    }

    fn charge_style_member(
        &mut self,
        kind: SyntaxKind,
        role: SyntaxRole,
    ) -> Result<(), SyntaxLimit> {
        let is_nested_member = matches!(
            kind,
            SyntaxKind::StyleTokenDeclaration
                | SyntaxKind::StyleRule
                | SyntaxKind::StyleSelectorSequence
                | SyntaxKind::StylePropertyDeclaration
                | SyntaxKind::StyleEnvironmentBlock
                | SyntaxKind::StyleEnvironmentClause
        ) || (kind == SyntaxKind::NameReference
            && matches!(role, SyntaxRole::Label(_))
            && self
                .stack
                .iter()
                .any(|frame| frame.kind == SyntaxKind::StyleSelectorSequence))
            || (kind == SyntaxKind::ErrorNode
                && matches!(role, SyntaxRole::Element(_))
                && self
                    .stack
                    .last()
                    .is_some_and(|frame| frame.kind == SyntaxKind::ItemList));
        if is_nested_member
            && self
                .stack
                .iter()
                .rev()
                .find(|frame| frame.kind.is_item())
                .is_some_and(|frame| frame.kind == SyntaxKind::StyleItem)
        {
            let frame = self.declaration_frame_mut()?;
            charge(
                &mut frame.declaration_members,
                SyntaxLimit::DeclarationMembers,
            )?;
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
        Self {
            code: diagnostic.code(),
            range: diagnostic.range(),
            related_range: diagnostic.related_range(),
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
pub(crate) fn validate_events(events: &[SyntaxEvent]) -> Result<GrammarBudget, SyntaxLimit> {
    let mut budget = GrammarBudget::default();
    for event in events {
        match event {
            SyntaxEvent::StartNode { kind, role, .. } => {
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
    budget.failure().map_or(Ok(budget), Err)
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
    fn flow_fixed_parameters_accept_exact_limit_and_reject_one_over() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::FlowItem, SyntaxRole::Element(0)));
        assert!(budget.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup,));
        for ordinal in 0..SyntaxLimit::FixedParameters.maximum() {
            assert!(budget.start(
                SyntaxKind::Parameter,
                SyntaxRole::Parameter(u16::try_from(ordinal).expect("parameter limit fits u16")),
            ));
            assert!(budget.finish());
        }
        assert!(
            !budget.start(
                SyntaxKind::Parameter,
                SyntaxRole::Parameter(
                    u16::try_from(SyntaxLimit::FixedParameters.maximum())
                        .expect("parameter limit fits u16"),
                ),
            )
        );
        assert_eq!(budget.failure(), Some(SyntaxLimit::FixedParameters));
    }

    #[test]
    fn rejected_flow_parameter_group_does_not_charge_fixed_parameters() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::FlowItem, SyntaxRole::Element(0)));
        assert!(budget.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup,));
        assert!(budget.start(SyntaxKind::Parameter, SyntaxRole::Parameter(0)));
        assert!(budget.finish());
        assert!(budget.finish());

        assert!(budget.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0)));
        assert!(budget.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup,));
        for ordinal in 0..SyntaxLimit::FixedParameters.maximum() {
            assert!(budget.start(
                SyntaxKind::Parameter,
                SyntaxRole::Parameter(u16::try_from(ordinal).expect("parameter limit fits u16")),
            ));
            assert!(budget.finish());
        }

        let flow = budget
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind == SyntaxKind::FlowItem)
            .expect("Flow budget frame");
        assert_eq!(flow.fixed_parameters, 1);
        assert_eq!(budget.failure(), None);
    }

    #[test]
    fn grouped_use_member_budget_accepts_exact_limit_and_rejects_one_over() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::UseDeclaration, SyntaxRole::Reference(0)));
        for _ in 0..SyntaxLimit::DeclarationMembers.maximum() {
            assert!(budget.grouped_use_member());
        }
        assert!(!budget.grouped_use_member());
        assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));

        let mut fresh = document_budget();
        assert!(fresh.start(SyntaxKind::UseDeclaration, SyntaxRole::Reference(0)));
        assert!(fresh.grouped_use_member());
    }

    #[test]
    fn grouped_use_member_budget_fails_closed_without_a_use_owner() {
        let mut budget = document_budget();
        assert!(!budget.grouped_use_member());
        assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));
    }

    #[test]
    fn nominal_record_member_budget_accepts_exact_limit_and_rejects_one_over() {
        for owner in [SyntaxKind::StructItem, SyntaxKind::EnumItem] {
            let mut budget = document_budget();
            assert!(budget.start(owner, SyntaxRole::Element(0)));
            for ordinal in 0..SyntaxLimit::DeclarationMembers.maximum() {
                assert!(budget.start(
                    SyntaxKind::RecordField,
                    SyntaxRole::Field(u16::try_from(ordinal).expect("member limit fits u16")),
                ));
                assert!(budget.finish());
            }
            assert!(
                !budget.start(
                    SyntaxKind::RecordField,
                    SyntaxRole::Field(
                        u16::try_from(SyntaxLimit::DeclarationMembers.maximum())
                            .expect("member limit fits u16"),
                    ),
                )
            );
            assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));
        }
    }

    #[test]
    fn resource_fields_accept_exact_declaration_member_limit_and_reject_one_over() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::ResourceDeclarationItem, SyntaxRole::Element(0)));
        for ordinal in 0..SyntaxLimit::DeclarationMembers.maximum() {
            assert!(budget.start(
                SyntaxKind::ResourceFieldInitializer,
                SyntaxRole::Field(u16::try_from(ordinal).expect("member limit fits u16")),
            ));
            assert!(budget.finish());
        }
        assert!(
            !budget.start(
                SyntaxKind::ResourceFieldInitializer,
                SyntaxRole::Field(
                    u16::try_from(SyntaxLimit::DeclarationMembers.maximum())
                        .expect("member limit fits u16"),
                ),
            )
        );
        assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));
    }

    #[test]
    fn activity_nested_rows_share_the_declaration_member_budget() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::ActivityDeclarationItem, SyntaxRole::Element(0)));
        for (kind, role) in [
            (SyntaxKind::ActivityModeMember, SyntaxRole::Member(0)),
            (SyntaxKind::ActivityPort, SyntaxRole::InputPort(0)),
            (SyntaxKind::RequiresClause, SyntaxRole::ContractClause(0)),
            (SyntaxKind::EnsuresClause, SyntaxRole::ContractClause(1)),
            (SyntaxKind::ErrorDeclarationMember, SyntaxRole::Recovery(0)),
        ] {
            assert!(budget.start(kind, role));
            assert!(budget.finish());
        }
        let activity = budget
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind == SyntaxKind::ActivityDeclarationItem)
            .expect("Activity budget frame");
        assert_eq!(activity.declaration_members, 5);
    }

    #[test]
    fn extern_capability_nested_items_share_the_declaration_member_budget() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::ExternCapabilityItem, SyntaxRole::Element(0)));
        for ordinal in 0..SyntaxLimit::DeclarationMembers.maximum() {
            let kind = match ordinal % 3 {
                0 => SyntaxKind::TypeAliasItem,
                1 => SyntaxKind::FunctionItem,
                _ => SyntaxKind::ErrorItem,
            };
            assert!(budget.start(kind, SyntaxRole::Element(u32::try_from(ordinal).unwrap())));
            assert!(budget.finish());
        }
        assert!(!budget.start(
            SyntaxKind::TypeAliasItem,
            SyntaxRole::Element(u32::try_from(SyntaxLimit::DeclarationMembers.maximum()).unwrap())
        ));
        assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));

        let capability = budget
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind == SyntaxKind::ExternCapabilityItem)
            .expect("external capability budget frame");
        assert_eq!(
            capability.declaration_members,
            SyntaxLimit::DeclarationMembers.maximum()
        );
    }

    #[test]
    fn trait_and_impl_inline_items_share_the_declaration_member_budget() {
        for owner in [SyntaxKind::TraitItem, SyntaxKind::ImplItem] {
            let mut budget = document_budget();
            assert!(budget.start(owner, SyntaxRole::Element(0)));
            for ordinal in 0..SyntaxLimit::DeclarationMembers.maximum() {
                let kind = match ordinal % 3 {
                    0 => SyntaxKind::TypeAliasItem,
                    1 => SyntaxKind::FunctionItem,
                    _ => SyntaxKind::ErrorItem,
                };
                assert!(budget.start(kind, SyntaxRole::Element(u32::try_from(ordinal).unwrap())));
                assert!(budget.finish());
            }
            assert!(!budget.start(
                SyntaxKind::TypeAliasItem,
                SyntaxRole::Element(
                    u32::try_from(SyntaxLimit::DeclarationMembers.maximum()).unwrap()
                )
            ));
            assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));
        }
    }

    #[test]
    fn activity_recovery_members_accept_exact_limit_and_reject_one_over() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::ActivityDeclarationItem, SyntaxRole::Element(0)));
        for ordinal in 0..SyntaxLimit::DeclarationMembers.maximum() {
            assert!(budget.start(
                SyntaxKind::ErrorDeclarationMember,
                SyntaxRole::Member(u16::try_from(ordinal).expect("member limit fits u16")),
            ));
            assert!(budget.finish());
        }
        assert!(
            !budget.start(
                SyntaxKind::ErrorDeclarationMember,
                SyntaxRole::Member(
                    u16::try_from(SyntaxLimit::DeclarationMembers.maximum())
                        .expect("member limit fits u16"),
                ),
            )
        );
        assert_eq!(budget.failure(), Some(SyntaxLimit::DeclarationMembers));
    }

    #[test]
    fn layer_recovery_members_charge_both_member_budgets_and_reject_one_over() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::LayerDeclarationItem, SyntaxRole::Element(0)));
        for ordinal in 0..SyntaxLimit::LayerMembers.maximum() {
            assert!(budget.start(
                SyntaxKind::ErrorDeclarationMember,
                SyntaxRole::Member(u16::try_from(ordinal).expect("member limit fits u16")),
            ));
            assert!(budget.finish());
        }
        let layer = budget
            .stack
            .iter()
            .rev()
            .find(|frame| frame.kind == SyntaxKind::LayerDeclarationItem)
            .expect("Layer budget frame");
        assert_eq!(
            layer.declaration_members,
            SyntaxLimit::LayerMembers.maximum()
        );
        assert_eq!(layer.layer_members, SyntaxLimit::LayerMembers.maximum());
        assert!(!budget.start(
            SyntaxKind::ErrorDeclarationMember,
            SyntaxRole::Member(
                u16::try_from(SyntaxLimit::LayerMembers.maximum()).expect("member limit fits u16"),
            ),
        ));
        assert_eq!(budget.failure(), Some(SyntaxLimit::LayerMembers));
    }

    #[test]
    fn expression_record_fields_do_not_charge_declaration_members() {
        let mut budget = GrammarBudget::default();
        assert!(budget.start(SyntaxKind::RecordExpression, SyntaxRole::Root));
        assert!(budget.start(SyntaxKind::RecordField, SyntaxRole::Field(0)));
        assert!(budget.finish());
        assert!(budget.finish());
        assert_eq!(budget.failure(), None);
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
    fn diagnostic_budget_uses_the_publication_identity_without_message_text() {
        let mut budget = GrammarBudget::default();
        let primary = SourceRange::new(0, 1);
        let first = SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.test.identity",
            primary,
            "first presentation",
        ));
        let message_only_duplicate = SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.test.identity",
            primary,
            "different presentation",
        ));
        let distinct_related = SyntaxEvent::Diagnostic(
            PendingSyntaxDiagnostic::new("syntax.test.identity", primary, "related evidence")
                .with_related_range(SourceRange::new(1, 2)),
        );

        assert!(budget.event(&first));
        assert!(budget.event(&message_only_duplicate));
        assert_eq!(budget.diagnostics.len(), 1);
        assert!(budget.event(&distinct_related));
        assert_eq!(budget.diagnostics.len(), 2);
        assert_eq!(budget.failure(), None);
    }

    #[test]
    fn final_stats_freeze_the_existing_transaction_accounting() {
        let mut budget = document_budget();
        assert!(budget.start(SyntaxKind::ErrorItem, SyntaxRole::Element(0)));
        for (kind, role) in [
            (SyntaxKind::LetStatement, SyntaxRole::Element(0)),
            (SyntaxKind::PathExpression, SyntaxRole::Element(1)),
            (SyntaxKind::PathType, SyntaxRole::Element(2)),
            (SyntaxKind::WholeBindingPattern, SyntaxRole::Element(3)),
            (SyntaxKind::NameDefinition, SyntaxRole::Name),
        ] {
            assert!(budget.start(kind, role));
            assert!(budget.finish());
        }
        assert!(budget.finish());
        assert!(budget.event(&diagnostic_event(0)));
        assert!(budget.event(&diagnostic_event(0)));

        let stats = budget.final_stats(89, 13, 47);
        assert_eq!(stats.accepted_source_bytes(), 89);
        assert_eq!(stats.lexer_tokens(), 13);
        assert_eq!(stats.grammar_events(), 47);
        assert_eq!(stats.top_level_items(), 1);
        assert_eq!(stats.statements(), 1);
        assert_eq!(stats.expressions(), 1);
        assert_eq!(stats.type_nodes(), 1);
        assert_eq!(stats.pattern_nodes(), 1);
        assert_eq!(stats.identity_bearing_nodes(), 7);
        assert_eq!(stats.diagnostic_identities(), 1);
        assert!(stats.matches_publication(89, 1));
        assert!(!stats.matches_publication(88, 1));
        assert!(!stats.matches_publication(89, 2));
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
