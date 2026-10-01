//! Compact final ownership of selected eager execution regions.
//!
//! Direct edges are issued by the existing effect fold, including its latent
//! body decisions. Publication binds them to final owners. Expansion walks
//! this sealed DAG, never HIR, scope membership or a second execution policy.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    body_edges::HirBodyChild,
    identity::{ExprId, StmtId},
};

use super::{
    CheckedExecutableControlRole, CheckedExpression, CheckedStatement, CheckedSuspensionRole,
    FinalSemanticAnalysisError, statement_effects::PreparedExecutableSuspensionRow,
};

/// Membership in one eager execution frame. A place is addressed without
/// evaluating its source expression as a value or traversing value receivers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutionOperation {
    Value(ExprId),
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
    statements: BTreeMap<StmtId, Box<[CheckedExecutionOperation]>>,
}

impl PreparedExecutableSuspensionCatalog {
    pub(crate) fn new(
        expressions: BTreeMap<ExprId, PreparedExecutableSuspensionRow>,
        statements: BTreeMap<StmtId, Box<[CheckedExecutionOperation]>>,
    ) -> Self {
        Self {
            expressions,
            statements,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.expressions.is_empty() && self.statements.is_empty()
    }

    pub(super) fn publish(
        self,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        statements: &BTreeMap<StmtId, CheckedStatement>,
    ) -> Result<CheckedExpressionExecutionCatalog, FinalSemanticAnalysisError> {
        if !self.expressions.keys().eq(expressions.keys())
            || !self.statements.keys().eq(statements.keys())
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        for edges in self
            .expressions
            .values()
            .map(PreparedExecutableSuspensionRow::children)
            .chain(self.statements.values().map(AsRef::as_ref))
        {
            if edges.windows(2).any(|pair| pair[0] >= pair[1])
                || edges.iter().any(|edge| match edge {
                    CheckedExecutionOperation::Value(owner) => !expressions.contains_key(owner),
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
            .map(PreparedExecutableSuspensionRow::children)
            .chain(self.statements.values().map(AsRef::as_ref))
        {
            for child in edges {
                if matches!(child, CheckedExecutionOperation::Place(_)) {
                    incoming.entry(*child).or_insert(0);
                }
                *incoming
                    .get_mut(child)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)? += 1;
            }
        }
        let mut pending = incoming
            .iter()
            .filter_map(|(&owner, &count)| (count == 0).then_some(owner))
            .collect::<Vec<_>>();
        let mut completed = 0;
        while let Some(owner) = pending.pop() {
            completed += 1;
            let children = match owner {
                CheckedExecutionOperation::Value(owner) => self.expressions[&owner].children(),
                CheckedExecutionOperation::Place(_) => &[],
                CheckedExecutionOperation::Statement(owner) => &self.statements[&owner],
            };
            for child in children {
                let count = incoming
                    .get_mut(child)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                *count -= 1;
                if *count == 0 {
                    pending.push(*child);
                }
            }
        }
        if completed != incoming.len() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(CheckedExpressionExecutionCatalog {
            expressions: self.expressions,
            statements: self.statements,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct CheckedExpressionExecutionCatalog {
    expressions: BTreeMap<ExprId, PreparedExecutableSuspensionRow>,
    statements: BTreeMap<StmtId, Box<[CheckedExecutionOperation]>>,
}

impl CheckedExpressionExecutionCatalog {
    pub(super) fn region(&self, root: ExprId) -> Option<CheckedExpressionExecutionRegion> {
        let row = self.expressions.get(&root)?;
        let mut visited = BTreeSet::new();
        let mut pending = vec![CheckedExecutionOperation::Value(root)];
        let mut expressions = BTreeSet::new();
        let mut places = BTreeSet::new();
        let mut statements = BTreeSet::new();
        while let Some(owner) = pending.pop() {
            if !visited.insert(owner) {
                continue;
            }
            match owner {
                CheckedExecutionOperation::Value(owner) => {
                    expressions.insert(owner);
                    pending.extend(self.expressions.get(&owner)?.children());
                }
                CheckedExecutionOperation::Place(owner) => {
                    places.insert(owner);
                }
                CheckedExecutionOperation::Statement(owner) => {
                    statements.insert(owner);
                    pending.extend(self.statements.get(&owner)?);
                }
            }
        }
        Some(CheckedExpressionExecutionRegion {
            expressions: expressions.into_iter().collect(),
            places: places.into_iter().collect(),
            operations: visited.into_iter().collect(),
            statements: statements.into_iter().collect(),
            suspension: row.suspension(),
            control: row.control(),
        })
    }
}

pub(super) struct CheckedExpressionExecutionRegion {
    expressions: Box<[ExprId]>,
    places: Box<[ExprId]>,
    operations: Box<[CheckedExecutionOperation]>,
    statements: Box<[StmtId]>,
    suspension: CheckedSuspensionRole,
    control: CheckedExecutableControlRole,
}

impl CheckedExpressionExecutionRegion {
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
        assert!(statements.is_empty());
        let root = *expressions.keys().next().unwrap();
        let foreign = crate::final_analysis::tests::fixture("fn elsewhere() -> i64 { 3i64 }", None);
        let foreign_report = crate::final_analysis::tests::analyze(&foreign).unwrap();
        let foreign_owner = foreign_report.expressions().next().unwrap().0;
        for rejected_child in [root, foreign_owner] {
            let rows = expressions
                .keys()
                .map(|&owner| {
                    let children = if owner == root {
                        vec![CheckedExecutionOperation::Value(rejected_child)]
                    } else {
                        vec![]
                    };
                    (
                        owner,
                        PreparedExecutableSuspensionRow::new(
                            children.into_boxed_slice(),
                            CheckedSuspensionRole::NonSuspending,
                            CheckedExecutableControlRole::ExpressionCompatible,
                        ),
                    )
                })
                .collect();
            assert!(matches!(
                PreparedExecutableSuspensionCatalog::new(rows, BTreeMap::new())
                    .publish(&expressions, &statements),
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            ));
        }
    }
}
