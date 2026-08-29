//! Bottom-up final execution-effect authority for expressions and statements.
//!
//! The seal consumes the selected final-HIR graph after call sealing. It
//! replaces every prepared expression row with its complete effect fold and
//! constructs each checked statement exactly once from typed child/body
//! edges. Scope membership and source placement never participate.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    body_edges::{HirBodyChild, HirBodyProjection},
    expr::{
        HirComputationBlockKind, HirExprKind, HirExpressionChildOwnership, HirExpressionChildRole,
        HirExpressionOwnedChild,
    },
    identity::{ExprId, HirModuleId, StmtId},
    module::HirModule,
    project::{HirExpressionEvaluationEdge, HirProjectEvaluationTopology},
    source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite},
    stmt::{HirStatementBodyRole, HirStatementChild, HirStatementChildRole, HirStmtKind},
    symbol::CallableDeclarationKey,
};

use crate::{
    callable::{CallTargetFacts, CheckedCallableCatalog, CheckedCallableDeclaration},
    effects::{EffectId, EffectSet},
};

use super::{
    CheckedExpression, CheckedStatement, CheckedStatementPayload, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, PreparedExpressionFact, PreparedStatementPayload,
    match_edges::CheckedSelectedExpressionGraph,
};

/// One typed execution fold before final project-call rows are closed.
#[derive(Clone, Debug, Default)]
struct PreparedExecutionEffectRow {
    effects: EffectSet,
    expressions: BTreeSet<ExprId>,
}

impl PreparedExecutionEffectRow {
    fn union_with(&mut self, other: &Self) {
        self.effects.union_with(&other.effects);
        self.expressions.extend(other.expressions.iter().copied());
    }
}

/// One latent closure body and its exact selected expression inventory.
#[derive(Debug)]
pub(crate) struct PreparedClosureExecutionEffectRow {
    owner: ExprId,
    row: PreparedExecutionEffectRow,
}

impl PreparedClosureExecutionEffectRow {
    pub(crate) const fn owner(&self) -> ExprId {
        self.owner
    }

    pub(crate) const fn effects(&self) -> &EffectSet {
        &self.row.effects
    }

    pub(crate) fn expressions(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.row.expressions.iter().copied()
    }
}

/// Typed, selected-graph execution rows used to close callable contracts.
/// This is private preparation; final publication independently recomputes
/// the same roots from sealed calls and compares their complete rows.
#[derive(Debug)]
pub(crate) struct PreparedExecutionEffectCatalog {
    declarations: BTreeMap<CallableDeclarationKey, PreparedExecutionEffectRow>,
    items: BTreeMap<arcweft_lang_hir::identity::ItemId, PreparedExecutionEffectRow>,
    closures: BTreeMap<CallableDeclarationKey, Box<[PreparedClosureExecutionEffectRow]>>,
}

impl PreparedExecutionEffectCatalog {
    pub(crate) fn declaration_effects(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<&EffectSet> {
        self.declarations.get(declaration).map(|row| &row.effects)
    }

    pub(crate) fn declaration_expressions(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<impl Iterator<Item = ExprId> + '_> {
        self.declarations
            .get(declaration)
            .map(|row| row.expressions.iter().copied())
    }

    pub(crate) fn item_effects(
        &self,
        item: arcweft_lang_hir::identity::ItemId,
    ) -> Option<&EffectSet> {
        self.items.get(&item).map(|row| &row.effects)
    }

    pub(crate) fn item_owners(
        &self,
    ) -> impl Iterator<Item = arcweft_lang_hir::identity::ItemId> + '_ {
        self.items.keys().copied()
    }

    pub(crate) fn item_expressions(
        &self,
        item: arcweft_lang_hir::identity::ItemId,
    ) -> Option<impl Iterator<Item = ExprId> + '_> {
        self.items
            .get(&item)
            .map(|row| row.expressions.iter().copied())
    }

    pub(crate) fn replace_item_effects(
        &mut self,
        item: arcweft_lang_hir::identity::ItemId,
        effects: EffectSet,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let row = self
            .items
            .get_mut(&item)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        row.effects = effects;
        Ok(())
    }

    pub(crate) fn closures(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> &[PreparedClosureExecutionEffectRow] {
        self.closures.get(declaration).map_or(&[], Box::as_ref)
    }

    pub(crate) fn closure(&self, owner: ExprId) -> Option<&PreparedClosureExecutionEffectRow> {
        self.closures
            .values()
            .flat_map(|rows| rows.iter())
            .find(|row| row.owner == owner)
    }
}

pub(crate) struct PreparedExecutionEffectInput<'a> {
    pub(crate) modules: &'a BTreeMap<HirModuleId, &'a HirModule>,
    pub(crate) topology: &'a HirProjectEvaluationTopology,
    pub(crate) selected: &'a CheckedSelectedExpressionGraph,
    pub(crate) expressions: &'a [(ExprId, PreparedExpressionFact)],
    pub(crate) statements: &'a [(StmtId, PreparedStatementPayload)],
    pub(crate) control: FinalSemanticAnalysisControl<'a>,
}

/// Folds the exact selected expression and typed statement/body edges before
/// call-row closure. No lexical-scope membership scan participates.
pub(crate) fn prepare_execution_effects(
    input: PreparedExecutionEffectInput<'_>,
) -> Result<PreparedExecutionEffectCatalog, FinalSemanticAnalysisError> {
    PreparedExecutionEffectSealer::new(input)?.seal()
}

struct PreparedExecutionEffectSealer<'a> {
    modules: &'a BTreeMap<HirModuleId, &'a HirModule>,
    topology: &'a HirProjectEvaluationTopology,
    selected: &'a CheckedSelectedExpressionGraph,
    expression_facts: BTreeMap<ExprId, &'a PreparedExpressionFact>,
    statement_facts: BTreeMap<StmtId, &'a PreparedStatementPayload>,
    expression_rows: BTreeMap<ExprId, PreparedExecutionEffectRow>,
    statement_rows: BTreeMap<StmtId, PreparedExecutionEffectRow>,
    active_expressions: BTreeSet<ExprId>,
    active_statements: BTreeSet<StmtId>,
    declarations: BTreeMap<CallableDeclarationKey, PreparedExecutionEffectRow>,
    items: BTreeMap<arcweft_lang_hir::identity::ItemId, PreparedExecutionEffectRow>,
    closures: BTreeMap<(CallableDeclarationKey, ExprId), PreparedClosureExecutionEffectRow>,
    active_declaration: Option<CallableDeclarationKey>,
    control: FinalSemanticAnalysisControl<'a>,
}

impl<'a> PreparedExecutionEffectSealer<'a> {
    fn new(input: PreparedExecutionEffectInput<'a>) -> Result<Self, FinalSemanticAnalysisError> {
        let mut expression_facts = BTreeMap::new();
        for (owner, fact) in input.expressions {
            if expression_facts.insert(*owner, fact).is_some() {
                return Err(FinalSemanticAnalysisError::DuplicateFact {
                    family: super::SemanticFactFamily::Expression,
                });
            }
        }
        let mut statement_facts = BTreeMap::new();
        for (owner, fact) in input.statements {
            if statement_facts.insert(*owner, fact).is_some() {
                return Err(FinalSemanticAnalysisError::DuplicateFact {
                    family: super::SemanticFactFamily::Statement,
                });
            }
        }
        let selected = input.selected.owners().collect::<BTreeSet<_>>();
        if selected
            .iter()
            .any(|owner| !expression_facts.contains_key(owner))
        {
            return Err(FinalSemanticAnalysisError::MissingFact {
                family: super::SemanticFactFamily::Expression,
            });
        }
        if let Some(owner) = expression_facts
            .keys()
            .copied()
            .find(|owner| !selected.contains(owner))
        {
            return Err(FinalSemanticAnalysisError::UnexpectedExpressionFact { owner });
        }
        Ok(Self {
            modules: input.modules,
            topology: input.topology,
            selected: input.selected,
            expression_facts,
            statement_facts,
            expression_rows: BTreeMap::new(),
            statement_rows: BTreeMap::new(),
            active_expressions: BTreeSet::new(),
            active_statements: BTreeSet::new(),
            declarations: BTreeMap::new(),
            items: BTreeMap::new(),
            closures: BTreeMap::new(),
            active_declaration: None,
            control: input.control,
        })
    }

    fn seal(mut self) -> Result<PreparedExecutionEffectCatalog, FinalSemanticAnalysisError> {
        for module in self.topology.modules() {
            for entry in module.entries() {
                self.control.check()?;
                if let Some(body) = entry.body() {
                    if self
                        .active_declaration
                        .replace(body.declaration().clone())
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    let mut row = PreparedExecutionEffectRow::default();
                    for root in body.roots() {
                        row.union_with(&self.fold_body(root.projection())?);
                    }
                    if self
                        .declarations
                        .insert(body.declaration().clone(), row.clone())
                        .is_some()
                        || self.active_declaration.take().as_ref() != Some(body.declaration())
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    if matches!(body.declaration(), CallableDeclarationKey::Flow(_)) {
                        self.items
                            .entry(body.source_item())
                            .or_default()
                            .union_with(&row);
                    }
                }
                let mut item_row = self.items.remove(&entry.item()).unwrap_or_default();
                for root in entry.roots() {
                    item_row.union_with(&self.fold_body(root.projection())?);
                }
                self.items.insert(entry.item(), item_row);
            }
        }
        for owner in self.selected.owners().collect::<Vec<_>>() {
            self.seal_expression(owner)?;
        }
        if self.expression_rows.len() != self.expression_facts.len()
            || self.statement_rows.len() != self.statement_facts.len()
            || !self.active_expressions.is_empty()
            || !self.active_statements.is_empty()
            || self.active_declaration.is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let mut closures =
            BTreeMap::<CallableDeclarationKey, Vec<PreparedClosureExecutionEffectRow>>::new();
        for ((declaration, _), row) in self.closures {
            closures.entry(declaration).or_default().push(row);
        }
        for rows in closures.values_mut() {
            rows.sort_by_key(PreparedClosureExecutionEffectRow::owner);
        }
        Ok(PreparedExecutionEffectCatalog {
            declarations: self.declarations,
            items: self.items,
            closures: closures
                .into_iter()
                .map(|(owner, rows)| (owner, rows.into_boxed_slice()))
                .collect(),
        })
    }

    fn module(&self, owner: HirModuleId) -> Result<&'a HirModule, FinalSemanticAnalysisError> {
        self.modules
            .get(&owner)
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }

    fn fold_body(
        &mut self,
        body: &HirBodyProjection,
    ) -> Result<PreparedExecutionEffectRow, FinalSemanticAnalysisError> {
        let mut row = PreparedExecutionEffectRow::default();
        for edge in body.children() {
            let child = match edge.child() {
                HirBodyChild::Expression(owner) => self.seal_expression(owner)?,
                HirBodyChild::Statement(owner) => self.seal_statement(owner)?,
            };
            row.union_with(&child);
        }
        Ok(row)
    }

    fn seal_expression(
        &mut self,
        owner: ExprId,
    ) -> Result<PreparedExecutionEffectRow, FinalSemanticAnalysisError> {
        if let Some(row) = self.expression_rows.get(&owner) {
            return Ok(row.clone());
        }
        if !self.active_expressions.insert(owner) {
            return Err(FinalSemanticAnalysisError::ExpressionCycle { owner });
        }
        let fact = self
            .expression_facts
            .get(&owner)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let module = self.module(owner.module())?;
        let kind = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind();
        let mut row = PreparedExecutionEffectRow {
            effects: fact.effects().clone(),
            expressions: BTreeSet::from([owner]),
        };
        let latent_callable = matches!(
            fact.checked_resolution(),
            Some(super::CheckedExpressionResolution::ImplicitCallable(_))
        ) || matches!(kind, HirExprKind::Closure(_));
        let independent_computation = matches!(
            kind,
            HirExprKind::ComputationBlock(expression)
                if matches!(
                    expression.kind(),
                    HirComputationBlockKind::Seq | HirComputationBlockKind::Stream
                )
        );
        for edge in self.selected.expression_edges(owner).to_vec() {
            let HirExpressionEvaluationEdge::Expression {
                role,
                ownership: HirExpressionChildOwnership::Owning,
                child,
            } = edge
            else {
                continue;
            };
            let child_row = self.seal_expression(child)?;
            if matches!(kind, HirExprKind::Closure(_))
                && matches!(role, HirExpressionChildRole::ClosureBody)
            {
                let declaration = self
                    .active_declaration
                    .clone()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                if self
                    .closures
                    .insert(
                        (declaration, owner),
                        PreparedClosureExecutionEffectRow {
                            owner,
                            row: child_row.clone(),
                        },
                    )
                    .is_some()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
            if !latent_callable
                && !independent_computation
                && !matches!(role, HirExpressionChildRole::ClosureBody)
            {
                row.union_with(&child_row);
            }
        }
        if let Some(body) = kind
            .try_body_projection()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
        {
            let body_row = self.fold_body(&body)?;
            if !matches!(kind, HirExprKind::Thread(_))
                && !latent_callable
                && !independent_computation
            {
                row.union_with(&body_row);
            }
        }
        let eager_owned_children = matches!(kind, HirExprKind::Await(_));
        for edge in kind
            .expression_owned_child_edges()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
        {
            let child = match edge.child() {
                HirExpressionOwnedChild::Pattern(_) => continue,
                HirExpressionOwnedChild::Statement(owner) => self.seal_statement(owner)?,
                HirExpressionOwnedChild::Body(edge) => match edge.child() {
                    HirBodyChild::Expression(owner) => self.seal_expression(owner)?,
                    HirBodyChild::Statement(owner) => self.seal_statement(owner)?,
                },
            };
            if eager_owned_children {
                row.union_with(&child);
            }
        }
        if !self.active_expressions.remove(&owner)
            || self.expression_rows.insert(owner, row.clone()).is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(row)
    }

    fn seal_statement(
        &mut self,
        owner: StmtId,
    ) -> Result<PreparedExecutionEffectRow, FinalSemanticAnalysisError> {
        if let Some(row) = self.statement_rows.get(&owner) {
            return Ok(row.clone());
        }
        if !self.active_statements.insert(owner) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let module = self.module(owner.module())?;
        let statement = module
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if statement.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let kind = statement.kind();
        let mut row = PreparedExecutionEffectRow::default();
        for edge in kind
            .try_child_edges()
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
        {
            let child = match edge.child() {
                HirStatementChild::Expression(owner) => self.seal_expression(owner)?,
                HirStatementChild::Statement(owner)
                    if !matches!(edge.role(), HirStatementChildRole::BodyItem { .. }) =>
                {
                    self.seal_statement(owner)?
                }
                HirStatementChild::Statement(_)
                | HirStatementChild::Pattern(_)
                | HirStatementChild::Type(_)
                | HirStatementChild::Local(_) => continue,
            };
            row.union_with(&child);
        }
        for body in kind
            .body_projections()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
        {
            let body_row = self.fold_body(body.projection())?;
            if body.role() != &HirStatementBodyRole::On {
                row.union_with(&body_row);
            }
        }
        match self.statement_facts.get(&owner).copied().ok_or(
            FinalSemanticAnalysisError::MissingFact {
                family: super::SemanticFactFamily::Statement,
            },
        )? {
            PreparedStatementPayload::Suspension(_) | PreparedStatementPayload::Yield => {
                row.effects.insert(EffectId::control_suspend());
            }
            PreparedStatementPayload::HirOwned
            | PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Assertion(_)
            | PreparedStatementPayload::Iteration(_)
            | PreparedStatementPayload::EvaluatedEffect(_)
            | PreparedStatementPayload::SealedEvaluatedEffect(_) => {}
        }
        if !self.active_statements.remove(&owner)
            || self.statement_rows.insert(owner, row.clone()).is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(row)
    }
}

/// Move-only proof that the final typed child fold and the payload intrinsic
/// were completed together.
#[derive(Debug)]
pub(crate) struct CompletedStatementEffectFold {
    child_effects: EffectSet,
    intrinsic: CompletedStatementIntrinsicEffect,
}

#[derive(Debug)]
enum CompletedStatementIntrinsicEffect {
    None,
    ControlSuspend,
    EvaluatedEffect {
        application: crate::callable::CheckedCallApplicationSite,
        effects: EffectSet,
    },
}

impl CompletedStatementEffectFold {
    fn none(child_effects: EffectSet) -> Self {
        Self {
            child_effects,
            intrinsic: CompletedStatementIntrinsicEffect::None,
        }
    }

    fn control_suspend(child_effects: EffectSet) -> Self {
        Self {
            child_effects,
            intrinsic: CompletedStatementIntrinsicEffect::ControlSuspend,
        }
    }

    fn evaluated_effect(
        child_effects: EffectSet,
        application: crate::callable::CheckedCallApplicationSite,
        effects: EffectSet,
    ) -> Self {
        Self {
            child_effects,
            intrinsic: CompletedStatementIntrinsicEffect::EvaluatedEffect {
                application,
                effects,
            },
        }
    }

    pub(super) fn into_effects(self, payload: &CheckedStatementPayload) -> Option<EffectSet> {
        let mut effects = self.child_effects;
        match (self.intrinsic, payload) {
            (CompletedStatementIntrinsicEffect::None, payload)
                if !matches!(
                    payload,
                    CheckedStatementPayload::EvaluatedEffect(_)
                        | CheckedStatementPayload::Suspension(_)
                        | CheckedStatementPayload::Yield
                ) => {}
            (
                CompletedStatementIntrinsicEffect::ControlSuspend,
                CheckedStatementPayload::Suspension(_) | CheckedStatementPayload::Yield,
            ) => {
                effects.insert(EffectId::control_suspend());
            }
            (
                CompletedStatementIntrinsicEffect::EvaluatedEffect {
                    application,
                    effects: application_effects,
                },
                CheckedStatementPayload::EvaluatedEffect(effect),
            ) if effect.application() == &application => {
                effects.union_with(&application_effects);
            }
            _ => return None,
        }
        Some(effects)
    }
}

/// Payload construction boundary invoked only after every effectful child has
/// its completed row.
pub(crate) trait CheckedStatementPayloadSealer {
    fn seal_payload(
        &mut self,
        module: &HirModule,
        owner: StmtId,
        statement: &HirStmtKind,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        statements: &BTreeMap<StmtId, CheckedStatement>,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError>;

    fn finish(self) -> Result<(), FinalSemanticAnalysisError>;
}

/// Atomic output of the final expression/statement effect transaction.
pub(crate) struct CompletedStatementEffectCatalog {
    expressions: BTreeMap<ExprId, CheckedExpression>,
    statements: BTreeMap<StmtId, CheckedStatement>,
}

impl CompletedStatementEffectCatalog {
    pub(crate) fn into_parts(
        self,
    ) -> (
        BTreeMap<ExprId, CheckedExpression>,
        BTreeMap<StmtId, CheckedStatement>,
    ) {
        (self.expressions, self.statements)
    }
}

pub(crate) struct StatementEffectSealInput<'a, P> {
    pub(crate) modules: &'a BTreeMap<HirModuleId, &'a HirModule>,
    pub(crate) topology: &'a HirProjectEvaluationTopology,
    pub(crate) selected: &'a CheckedSelectedExpressionGraph,
    pub(crate) calls: &'a BTreeMap<ExprId, CallTargetFacts>,
    pub(crate) callables: &'a CheckedCallableCatalog,
    pub(crate) expressions: BTreeMap<ExprId, CheckedExpression>,
    pub(crate) payloads: P,
    pub(crate) control: FinalSemanticAnalysisControl<'a>,
}

/// Completes every selected expression and statement row, compares final
/// callable-body rows, and publishes neither map on any failure.
pub(crate) fn seal_statement_effects<P: CheckedStatementPayloadSealer>(
    input: StatementEffectSealInput<'_, P>,
) -> Result<CompletedStatementEffectCatalog, FinalSemanticAnalysisError> {
    StatementEffectSealer::new(input)?.seal()
}

struct StatementEffectSealer<'a, P> {
    modules: &'a BTreeMap<HirModuleId, &'a HirModule>,
    topology: &'a HirProjectEvaluationTopology,
    selected: &'a CheckedSelectedExpressionGraph,
    selected_owners: BTreeSet<ExprId>,
    calls: &'a BTreeMap<ExprId, CallTargetFacts>,
    callables: &'a CheckedCallableCatalog,
    pending_expressions: BTreeMap<ExprId, CheckedExpression>,
    expressions: BTreeMap<ExprId, CheckedExpression>,
    statements: BTreeMap<StmtId, CheckedStatement>,
    active_expressions: BTreeSet<ExprId>,
    active_statements: BTreeSet<StmtId>,
    declaration_effects: BTreeMap<arcweft_lang_hir::symbol::CallableDeclarationKey, EffectSet>,
    closure_effects: BTreeMap<ExprId, EffectSet>,
    payloads: P,
    control: FinalSemanticAnalysisControl<'a>,
}

impl<'a, P: CheckedStatementPayloadSealer> StatementEffectSealer<'a, P> {
    fn new(input: StatementEffectSealInput<'a, P>) -> Result<Self, FinalSemanticAnalysisError> {
        let selected_owners = input.selected.owners().collect::<BTreeSet<_>>();
        if selected_owners
            .iter()
            .any(|owner| !input.expressions.contains_key(owner))
        {
            return Err(FinalSemanticAnalysisError::MissingFact {
                family: super::SemanticFactFamily::Expression,
            });
        }
        if let Some(owner) = input
            .expressions
            .keys()
            .copied()
            .find(|owner| !selected_owners.contains(owner))
        {
            return Err(FinalSemanticAnalysisError::UnexpectedExpressionFact { owner });
        }
        Ok(Self {
            modules: input.modules,
            topology: input.topology,
            selected: input.selected,
            selected_owners,
            calls: input.calls,
            callables: input.callables,
            pending_expressions: input.expressions,
            expressions: BTreeMap::new(),
            statements: BTreeMap::new(),
            active_expressions: BTreeSet::new(),
            active_statements: BTreeSet::new(),
            declaration_effects: BTreeMap::new(),
            closure_effects: BTreeMap::new(),
            payloads: input.payloads,
            control: input.control,
        })
    }

    fn seal(mut self) -> Result<CompletedStatementEffectCatalog, FinalSemanticAnalysisError> {
        for module in self.topology.modules() {
            for entry in module.entries() {
                self.control.check()?;
                for root in entry.roots() {
                    self.fold_body(root.projection())?;
                }
                if let Some(declaration) = entry.body() {
                    let mut effects = EffectSet::new();
                    for root in declaration.roots() {
                        effects.union_with(&self.fold_body(root.projection())?);
                    }
                    if self
                        .declaration_effects
                        .insert(declaration.declaration().clone(), effects)
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                }
            }
        }

        for owner in self.selected_owners.iter().copied().collect::<Vec<_>>() {
            self.seal_expression(owner)?;
        }
        if !self.pending_expressions.is_empty() || !self.active_expressions.is_empty() {
            return Err(FinalSemanticAnalysisError::MissingFact {
                family: super::SemanticFactFamily::Expression,
            });
        }
        self.validate_callable_rows()?;
        self.payloads.finish()?;
        if !self.active_statements.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(CompletedStatementEffectCatalog {
            expressions: self.expressions,
            statements: self.statements,
        })
    }

    fn module(&self, owner: HirModuleId) -> Result<&'a HirModule, FinalSemanticAnalysisError> {
        self.modules
            .get(&owner)
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }

    fn fold_body(
        &mut self,
        body: &HirBodyProjection,
    ) -> Result<EffectSet, FinalSemanticAnalysisError> {
        let mut effects = EffectSet::new();
        for edge in body.children() {
            self.control.check()?;
            match edge.child() {
                HirBodyChild::Expression(owner) => {
                    effects.union_with(&self.seal_expression(owner)?);
                }
                HirBodyChild::Statement(owner) => {
                    effects.union_with(&self.seal_statement(owner)?);
                }
            }
        }
        Ok(effects)
    }

    fn seal_expression(&mut self, owner: ExprId) -> Result<EffectSet, FinalSemanticAnalysisError> {
        if let Some(checked) = self.expressions.get(&owner) {
            return Ok(checked.effects().clone());
        }
        if !self.selected_owners.contains(&owner) || !self.active_expressions.insert(owner) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let checked = self
            .pending_expressions
            .remove(&owner)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let module = self.module(owner.module())?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let kind = expression.kind();
        let mut effects = checked.effects().clone();
        self.include_final_call_effect(owner, &mut effects)?;

        let latent_callable = matches!(
            checked.resolution(),
            super::CheckedExpressionResolution::ImplicitCallable(_)
        ) || matches!(kind, HirExprKind::Closure(_));
        let independent_computation = matches!(
            kind,
            HirExprKind::ComputationBlock(expression)
                if matches!(
                    expression.kind(),
                    HirComputationBlockKind::Seq | HirComputationBlockKind::Stream
                )
        );
        let selected_edges = self.selected.expression_edges(owner).to_vec();
        for edge in selected_edges {
            let HirExpressionEvaluationEdge::Expression {
                role,
                ownership: HirExpressionChildOwnership::Owning,
                child,
            } = edge
            else {
                continue;
            };
            let child_effects = self.seal_expression(child)?;
            if matches!(kind, HirExprKind::Closure(_))
                && matches!(role, HirExpressionChildRole::ClosureBody)
                && self
                    .closure_effects
                    .insert(owner, child_effects.clone())
                    .is_some()
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            if !latent_callable
                && !independent_computation
                && !matches!(role, HirExpressionChildRole::ClosureBody)
            {
                effects.union_with(&child_effects);
            }
        }

        if let Some(body) = kind
            .try_body_projection()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
        {
            let body_effects = self.fold_body(&body)?;
            if !matches!(kind, HirExprKind::Thread(_))
                && !latent_callable
                && !independent_computation
            {
                effects.union_with(&body_effects);
            }
        }

        let owned_children = kind
            .expression_owned_child_edges()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?;
        let owned_children_are_eager = matches!(kind, HirExprKind::Await(_));
        for edge in owned_children {
            let child_effects = match edge.child() {
                HirExpressionOwnedChild::Pattern(_) => continue,
                HirExpressionOwnedChild::Statement(owner) => self.seal_statement(owner)?,
                HirExpressionOwnedChild::Body(edge) => match edge.child() {
                    HirBodyChild::Expression(owner) => self.seal_expression(owner)?,
                    HirBodyChild::Statement(owner) => self.seal_statement(owner)?,
                },
            };
            if owned_children_are_eager {
                effects.union_with(&child_effects);
            }
        }

        if !self.active_expressions.remove(&owner) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        self.expressions
            .insert(owner, checked.with_completed_effects(effects.clone()));
        Ok(effects)
    }

    fn seal_statement(&mut self, owner: StmtId) -> Result<EffectSet, FinalSemanticAnalysisError> {
        if let Some(checked) = self.statements.get(&owner) {
            return Ok(checked.effects().clone());
        }
        if !self.active_statements.insert(owner) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let module = self.module(owner.module())?;
        let statement = module
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if statement.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let kind = statement.kind();
        let mut child_effects = EffectSet::new();
        for edge in kind
            .try_child_edges()
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
        {
            match edge.child() {
                HirStatementChild::Expression(expression) => {
                    child_effects.union_with(&self.seal_expression(expression)?);
                }
                HirStatementChild::Statement(statement)
                    if !matches!(edge.role(), HirStatementChildRole::BodyItem { .. }) =>
                {
                    child_effects.union_with(&self.seal_statement(statement)?);
                }
                HirStatementChild::Statement(_)
                | HirStatementChild::Pattern(_)
                | HirStatementChild::Type(_)
                | HirStatementChild::Local(_) => {}
            }
        }
        for body in kind
            .body_projections()
            .map_err(|_| FinalSemanticAnalysisError::RecoveredOwner)?
        {
            let body_effects = self.fold_body(body.projection())?;
            if body.role() != &HirStatementBodyRole::On {
                child_effects.union_with(&body_effects);
            }
        }

        let payload =
            self.payloads
                .seal_payload(module, owner, kind, &self.expressions, &self.statements)?;
        let fold = self.complete_statement_fold(child_effects, &payload)?;
        let checked = CheckedStatement::new(payload, fold)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let effects = checked.effects().clone();
        if !self.active_statements.remove(&owner)
            || self.statements.insert(owner, checked).is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(effects)
    }

    fn complete_statement_fold(
        &self,
        child_effects: EffectSet,
        payload: &CheckedStatementPayload,
    ) -> Result<CompletedStatementEffectFold, FinalSemanticAnalysisError> {
        match payload {
            CheckedStatementPayload::Suspension(_) | CheckedStatementPayload::Yield => {
                Ok(CompletedStatementEffectFold::control_suspend(child_effects))
            }
            CheckedStatementPayload::EvaluatedEffect(effect) => {
                let site = effect.application().raw();
                let application = self
                    .calls
                    .get(&site.expression())
                    .and_then(CallTargetFacts::selected_application)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if application.core().site() != site
                    || application.core().application_site() != effect.application()
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                Ok(CompletedStatementEffectFold::evaluated_effect(
                    child_effects,
                    effect.application().clone(),
                    application.core().effects().concrete().clone(),
                ))
            }
            CheckedStatementPayload::Structural
            | CheckedStatementPayload::Assertion(_)
            | CheckedStatementPayload::Assignment(_)
            | CheckedStatementPayload::Iteration(_)
            | CheckedStatementPayload::Defer(_)
            | CheckedStatementPayload::ControlTransfer(_)
            | CheckedStatementPayload::Trigger(_)
            | CheckedStatementPayload::UnsafeAudit(_)
            | CheckedStatementPayload::Select(_)
            | CheckedStatementPayload::SourceLocale(_)
            | CheckedStatementPayload::Scope(_)
            | CheckedStatementPayload::Include(_) => {
                Ok(CompletedStatementEffectFold::none(child_effects))
            }
        }
    }

    fn include_final_call_effect(
        &self,
        owner: ExprId,
        effects: &mut EffectSet,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let Some(call) = self.calls.get(&owner) else {
            return Ok(());
        };
        let Some(application) = call.selected_application() else {
            // Rejected, ambiguous, missing, and non-callable evidence has no
            // executable call row. Its selected child expressions still fold
            // normally, while the call site contributes no intrinsic effect.
            return Ok(());
        };
        if application.core().site().expression() != owner {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        effects.union_with(application.core().effects().concrete());
        Ok(())
    }

    fn validate_callable_rows(&self) -> Result<(), FinalSemanticAnalysisError> {
        for facts in self.callables.records() {
            let Some(actual) = facts.actual_row() else {
                continue;
            };
            let CheckedCallableDeclaration::Project(declaration) = facts.id().declaration() else {
                continue;
            };
            let completed = self
                .declaration_effects
                .get(declaration)
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            if actual.concrete() != completed {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        if self.callables.closure_rows().len() != self.closure_effects.len() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        for (owner, effects) in &self.closure_effects {
            let checked = self
                .expressions
                .get(owner)
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *owner })?;
            if !matches!(
                checked.resolution(),
                super::CheckedExpressionResolution::Closure(closure) if closure.owner() == *owner
            ) {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
            let module = self.module(owner.module())?;
            let source = module
                .source_site(
                    module.provenance().source_identity(),
                    HirSourceQuery::Expr {
                        owner: *owner,
                        role: HirExprSourceRole::Whole,
                    },
                )
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            let HirSourcePresence::Present(HirSourceSite::Span(source)) = source.presence() else {
                return Err(FinalSemanticAnalysisError::RecoveredOwner);
            };
            let row = self
                .callables
                .closure_at_source(source)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            if row.concrete() != effects {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
        }
        Ok(())
    }
}
