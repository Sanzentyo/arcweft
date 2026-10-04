//! Compact final ownership of selected eager execution regions.
//!
//! Direct edges are issued by the existing effect fold, including its latent
//! body decisions. Publication binds them to final owners. Expansion walks
//! this sealed DAG, never HIR, scope membership or a second execution policy.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    body_edges::HirBodyChild,
    identity::{ExprId, StmtId},
    project::HirDeclarationBodyRootRole,
    symbol::CallableDeclarationKey,
};

use super::{
    CheckedExecutableControlRole, CheckedExpression, CheckedStatement, CheckedSuspensionRole,
    FinalSemanticAnalysisError, statement_effects::PreparedExecutableSuspensionRow,
};

/// Membership in one eager execution frame. A place is addressed without
/// evaluating its source expression as a value or traversing value receivers.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutionBodyOwner {
    CallableValue(ExprId),
    Declaration {
        declaration: CallableDeclarationKey,
        role: HirDeclarationBodyRootRole,
    },
}

/// One selected operation in the shared execution DAG. Body origins are
/// immutable shared keys; operation cloning never copies declaration names.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutionOperation {
    Value(ExprId),
    Body(Arc<CheckedExecutionBodyOwner>),
    Place(ExprId),
    Statement(StmtId),
}

impl From<HirBodyChild> for CheckedExecutionOperation {
    fn from(child: HirBodyChild) -> Self {
        match child {
            HirBodyChild::Expression(owner) => Self::Value(owner),
            HirBodyChild::Statement(owner) => Self::Statement(owner),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PreparedExecutableSuspensionCatalog {
    expressions: BTreeMap<ExprId, PreparedExecutableSuspensionRow>,
    bodies: BTreeMap<Arc<CheckedExecutionBodyOwner>, PreparedExecutableSuspensionRow>,
    statements: BTreeMap<StmtId, PreparedExecutableSuspensionRow>,
}

impl PreparedExecutableSuspensionCatalog {
    pub(crate) fn new(
        expressions: BTreeMap<ExprId, PreparedExecutableSuspensionRow>,
        statements: BTreeMap<StmtId, PreparedExecutableSuspensionRow>,
        bodies: BTreeMap<Arc<CheckedExecutionBodyOwner>, PreparedExecutableSuspensionRow>,
    ) -> Self {
        Self {
            expressions,
            bodies,
            statements,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.expressions.is_empty() && self.statements.is_empty() && self.bodies.is_empty()
    }

    pub(super) fn publish(
        self,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        statements: &BTreeMap<StmtId, CheckedStatement>,
        topology: &arcweft_lang_hir::project::HirProjectEvaluationTopology,
        selected: &super::match_edges::CheckedSelectedExpressionGraph,
    ) -> Result<CheckedExecutionCatalog, FinalSemanticAnalysisError> {
        if !self.expressions.keys().eq(expressions.keys())
            || !self.statements.keys().eq(statements.keys())
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let callable_owners = expressions.iter().filter_map(|(owner, expression)| {
            matches!(
                expression.resolution(),
                super::CheckedExpressionResolution::ImplicitCallable(_)
                    | super::CheckedExpressionResolution::Closure(_)
            )
            .then(|| Arc::new(CheckedExecutionBodyOwner::CallableValue(*owner)))
        });
        let callable_owners = callable_owners.collect::<BTreeSet<_>>();
        let actual_callable_owners = self
            .bodies
            .keys()
            .filter(|owner| matches!(owner.as_ref(), CheckedExecutionBodyOwner::CallableValue(_)))
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual_callable_owners != callable_owners {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let mut expected_bodies = callable_owners;
        for module in topology.modules() {
            for entry in module.entries() {
                let Some(body) = entry.body() else { continue };
                if selected.owns_fx_definition(body.declaration()) {
                    continue;
                }
                for root in body.roots() {
                    if root.projection().children().is_empty()
                        || root.projection().children().iter().any(|edge| {
                            selected.contains_owner(match edge.child() {
                                HirBodyChild::Expression(owner) => {
                                    arcweft_lang_hir::identity::SyntheticOwner::Expr(owner)
                                }
                                HirBodyChild::Statement(owner) => {
                                    arcweft_lang_hir::identity::SyntheticOwner::Stmt(owner)
                                }
                            })
                        })
                    {
                        expected_bodies.insert(Arc::new(CheckedExecutionBodyOwner::Declaration {
                            declaration: body.declaration().clone(),
                            role: root.role(),
                        }));
                    }
                }
            }
        }
        if !self.bodies.keys().eq(expected_bodies.iter()) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        for (owner, row) in &self.bodies {
            if let CheckedExecutionBodyOwner::Declaration { declaration, role } = owner.as_ref() {
                let declaration = topology
                    .declaration(declaration)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let body = declaration
                    .body()
                    .roots()
                    .iter()
                    .find(|root| root.role() == *role)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let expected = body
                    .projection()
                    .children()
                    .iter()
                    .map(|edge| CheckedExecutionOperation::from(edge.child()))
                    .collect::<BTreeSet<_>>();
                if row.children().iter().cloned().collect::<BTreeSet<_>>() != expected {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
        }
        for edges in self
            .expressions
            .values()
            .chain(self.bodies.values())
            .map(PreparedExecutableSuspensionRow::children)
            .chain(
                self.statements
                    .values()
                    .map(PreparedExecutableSuspensionRow::children),
            )
        {
            if edges.windows(2).any(|pair| pair[0] >= pair[1])
                || edges.iter().any(|edge| match edge {
                    CheckedExecutionOperation::Value(owner) => !expressions.contains_key(owner),
                    CheckedExecutionOperation::Body(owner) => !self.bodies.contains_key(owner),
                    CheckedExecutionOperation::Place(owner) => expressions
                        .get(owner)
                        .and_then(CheckedExpression::mutable_place)
                        .is_none(),
                    CheckedExecutionOperation::Statement(owner) => !statements.contains_key(owner),
                })
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        let mut incoming = self
            .expressions
            .keys()
            .copied()
            .map(CheckedExecutionOperation::Value)
            .chain(
                self.bodies
                    .keys()
                    .cloned()
                    .map(CheckedExecutionOperation::Body),
            )
            .chain(
                self.statements
                    .keys()
                    .copied()
                    .map(CheckedExecutionOperation::Statement),
            )
            .map(|owner| (owner, 0usize))
            .collect::<BTreeMap<_, _>>();
        for edges in self
            .expressions
            .values()
            .chain(self.bodies.values())
            .map(PreparedExecutableSuspensionRow::children)
            .chain(
                self.statements
                    .values()
                    .map(PreparedExecutableSuspensionRow::children),
            )
        {
            for child in edges {
                if matches!(child, CheckedExecutionOperation::Place(_)) {
                    incoming.entry(child.clone()).or_insert(0);
                }
                *incoming
                    .get_mut(child)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)? += 1;
            }
        }
        let mut pending = incoming
            .iter()
            .filter_map(|(owner, &count)| (count == 0).then(|| owner.clone()))
            .collect::<Vec<_>>();
        let mut completed = 0;
        while let Some(owner) = pending.pop() {
            completed += 1;
            let children = match owner {
                CheckedExecutionOperation::Value(owner) => self.expressions[&owner].children(),
                CheckedExecutionOperation::Body(ref owner) => self.bodies[owner].children(),
                CheckedExecutionOperation::Place(_) => &[],
                CheckedExecutionOperation::Statement(owner) => self.statements[&owner].children(),
            };
            for child in children {
                let count = incoming
                    .get_mut(child)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                *count -= 1;
                if *count == 0 {
                    pending.push(child.clone());
                }
            }
        }
        if completed != incoming.len() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(CheckedExecutionCatalog {
            expressions: self.expressions,
            bodies: self.bodies,
            statements: self.statements,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct CheckedExecutionCatalog {
    expressions: BTreeMap<ExprId, PreparedExecutableSuspensionRow>,
    bodies: BTreeMap<Arc<CheckedExecutionBodyOwner>, PreparedExecutableSuspensionRow>,
    statements: BTreeMap<StmtId, PreparedExecutableSuspensionRow>,
}

impl CheckedExecutionCatalog {
    pub(super) fn contains_body(&self, owner: &CheckedExecutionBodyOwner) -> bool {
        self.bodies.contains_key(owner)
    }
    pub(super) fn body_effects(
        &self,
        owner: &CheckedExecutionBodyOwner,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        statements: &BTreeMap<StmtId, CheckedStatement>,
    ) -> Option<crate::effects::EffectSet> {
        let mut effects = crate::effects::EffectSet::new();
        for child in self.bodies.get(owner)?.children() {
            match child {
                CheckedExecutionOperation::Value(owner) => {
                    effects.union_with(expressions.get(owner)?.effects());
                }
                CheckedExecutionOperation::Statement(owner) => {
                    effects.union_with(statements.get(owner)?.effects());
                }
                CheckedExecutionOperation::Body(owner) => {
                    effects.union_with(&self.body_effects(owner, expressions, statements)?);
                }
                CheckedExecutionOperation::Place(_) => {}
            }
        }
        Some(effects)
    }
    pub(super) fn body_row(&self, owner: ExprId) -> Option<&PreparedExecutableSuspensionRow> {
        self.bodies
            .get(&CheckedExecutionBodyOwner::CallableValue(owner))
    }
    pub(super) fn region(&self, root: CheckedExecutionOperation) -> Option<CheckedExecutionRegion> {
        let row = match &root {
            CheckedExecutionOperation::Value(owner) => self.expressions.get(owner)?,
            CheckedExecutionOperation::Body(owner) => self.bodies.get(owner)?,
            CheckedExecutionOperation::Statement(owner) => self.statements.get(owner)?,
            CheckedExecutionOperation::Place(_) => return None,
        };
        let mut visited = BTreeSet::new();
        let mut pending = vec![root];
        let mut expressions = BTreeSet::new();
        let mut places = BTreeSet::new();
        let mut statements = BTreeSet::new();
        while let Some(owner) = pending.pop() {
            if !visited.insert(owner.clone()) {
                continue;
            }
            match owner {
                CheckedExecutionOperation::Value(owner) => {
                    expressions.insert(owner);
                    pending.extend(self.expressions.get(&owner)?.children().iter().cloned());
                }
                CheckedExecutionOperation::Body(owner) => {
                    pending.extend(self.bodies.get(&owner)?.children().iter().cloned());
                }
                CheckedExecutionOperation::Place(owner) => {
                    places.insert(owner);
                }
                CheckedExecutionOperation::Statement(owner) => {
                    statements.insert(owner);
                    pending.extend(self.statements.get(&owner)?.children().iter().cloned());
                }
            }
        }
        Some(CheckedExecutionRegion {
            expressions: expressions.into_iter().collect(),
            places: places.into_iter().collect(),
            operations: visited.into_iter().collect(),
            statements: statements.into_iter().collect(),
            suspension: row.suspension(),
            control: row.control(),
        })
    }
}

#[derive(Debug)]
pub(super) struct CheckedExecutionRegion {
    expressions: Box<[ExprId]>,
    places: Box<[ExprId]>,
    operations: Box<[CheckedExecutionOperation]>,
    statements: Box<[StmtId]>,
    suspension: CheckedSuspensionRole,
    control: CheckedExecutableControlRole,
}

impl CheckedExecutionRegion {
    pub(super) fn places(&self) -> &[ExprId] {
        &self.places
    }

    pub(super) fn operations(&self) -> &[CheckedExecutionOperation] {
        &self.operations
    }
    pub(super) fn expressions(&self) -> &[ExprId] {
        &self.expressions
    }

    pub(super) fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub(super) const fn suspension(&self) -> CheckedSuspensionRole {
        self.suspension
    }

    pub(super) const fn control(&self) -> CheckedExecutableControlRole {
        self.control
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_rejects_cycles_and_foreign_eager_edges() {
        let world = crate::final_analysis::tests::fixture(
            "fn root(value: i64) -> i64 { value + 1i64 }",
            None,
        );
        let report = crate::final_analysis::tests::analyze(&world).unwrap();
        let expressions = report
            .expressions()
            .map(|(owner, fact)| (owner, fact.clone()))
            .collect::<BTreeMap<_, _>>();
        let statements = report
            .statements()
            .map(|(owner, fact)| (owner, fact.clone()))
            .collect::<BTreeMap<_, _>>();
        let prepared = expressions
            .iter()
            .map(|(owner, fact)| {
                (
                    *owner,
                    super::super::PreparedExpressionFact::Complete(fact.clone()),
                )
            })
            .collect();
        let selected =
            super::super::match_edges::CheckedSelectedExpressionGraph::seal_call_free_fixture(
                world.project.analysis_view().unwrap(),
                Arc::clone(report.accepted_root_catalog().topology()),
                &prepared,
            )
            .unwrap();
        assert!(statements.is_empty());
        let bodies = report
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .flat_map(|body| {
                body.roots().iter().map(|root| {
                    (
                        Arc::new(CheckedExecutionBodyOwner::Declaration {
                            declaration: body.declaration().clone(),
                            role: root.role(),
                        }),
                        PreparedExecutableSuspensionRow::new(
                            root.projection()
                                .children()
                                .iter()
                                .map(|edge| edge.child().into())
                                .collect(),
                            CheckedSuspensionRole::NonSuspending,
                            CheckedExecutableControlRole::ExpressionCompatible,
                        ),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let rows = expressions
            .keys()
            .map(|&owner| {
                let children = selected
                    .expression_edges(owner)
                    .iter()
                    .map(|edge| CheckedExecutionOperation::Value(edge.child()))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                (
                    owner,
                    PreparedExecutableSuspensionRow::new(
                        children,
                        CheckedSuspensionRole::NonSuspending,
                        CheckedExecutableControlRole::ExpressionCompatible,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            PreparedExecutableSuspensionCatalog::new(rows.clone(), BTreeMap::new(), bodies.clone())
                .publish(&expressions, &statements, report.hir_topology(), &selected)
                .is_ok()
        );
        // A missing independent declaration body must fail even when every
        // expression owner/edge remains valid and no eager node references it.
        assert!(matches!(
            PreparedExecutableSuspensionCatalog::new(
                rows.clone(),
                BTreeMap::new(),
                BTreeMap::new()
            )
            .publish(&expressions, &statements, report.hir_topology(), &selected),
            Err(FinalSemanticAnalysisError::WrongPayloadFamily)
        ));
        let root = *expressions.keys().next().unwrap();
        let foreign = crate::final_analysis::tests::fixture("fn elsewhere() -> i64 { 3i64 }", None);
        let foreign_report = crate::final_analysis::tests::analyze(&foreign).unwrap();
        let foreign_owner = foreign_report.expressions().next().unwrap().0;
        for rejected_child in [root, foreign_owner] {
            let mut rejected = rows.clone();
            *rejected.get_mut(&root).unwrap() = PreparedExecutableSuspensionRow::new(
                Box::new([CheckedExecutionOperation::Value(rejected_child)]),
                CheckedSuspensionRole::NonSuspending,
                CheckedExecutableControlRole::ExpressionCompatible,
            );
            assert!(matches!(
                PreparedExecutableSuspensionCatalog::new(rejected, BTreeMap::new(), bodies.clone())
                    .publish(&expressions, &statements, report.hir_topology(), &selected),
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            ));
        }
    }
}
