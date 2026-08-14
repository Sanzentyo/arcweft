//! Runtime semantic owner domain for one executable final-HIR project.
//!
//! View and Style values are accepted presentation products, not runtime-plan
//! semantic facts. This module derives their complete typed owner closure from
//! final HIR and publishes the complementary runtime domain in canonical
//! project order.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use super::HirExecutableProjectView;
use crate::identity::{CaptureId, ExprId, LocalId, PatternId, ScopeId, StmtId, TypeId};
use crate::scope::HirScopeOwner;

/// Exact final-HIR owners admitted to runtime semantic fact publication.
///
/// The embedded executable view binds this inventory to one accepted project
/// generation. Callers cannot combine owner sets from another project lease.
pub struct HirRuntimeSemanticOwnerInventory<'project> {
    pub(super) project: HirExecutableProjectView<'project>,
    locals: Box<[LocalId]>,
    expressions: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    types: BTreeSet<TypeId>,
    patterns: BTreeSet<PatternId>,
    captures: BTreeSet<CaptureId>,
}

impl HirRuntimeSemanticOwnerInventory<'_> {
    /// Runtime locals in canonical executable-module and local-arena order.
    pub fn locals(&self) -> impl ExactSizeIterator<Item = LocalId> + '_ {
        self.locals.iter().copied()
    }

    /// Runtime patterns in qualified-ID order.
    pub fn patterns(&self) -> impl ExactSizeIterator<Item = PatternId> + '_ {
        self.patterns.iter().copied()
    }

    pub fn contains_local(&self, owner: LocalId) -> bool {
        self.locals.contains(&owner)
    }

    pub fn contains_expression(&self, owner: ExprId) -> bool {
        self.expressions.contains(&owner)
    }

    pub fn contains_statement(&self, owner: StmtId) -> bool {
        self.statements.contains(&owner)
    }

    pub fn contains_type(&self, owner: TypeId) -> bool {
        self.types.contains(&owner)
    }

    pub fn contains_pattern(&self, owner: PatternId) -> bool {
        self.patterns.contains(&owner)
    }

    pub fn contains_capture(&self, owner: CaptureId) -> bool {
        self.captures.contains(&owner)
    }

    pub(super) const fn expression_owners(&self) -> &BTreeSet<ExprId> {
        &self.expressions
    }
}

/// Failure to close presentation-owned typed HIR roots in an accepted project.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirRuntimeSemanticOwnerInventoryError {
    #[error("presentation owner inventory references unresolved scope {scope:?}")]
    UnresolvedScope { scope: ScopeId },
    #[error("presentation owner inventory references unresolved local {local:?}")]
    UnresolvedLocal { local: LocalId },
    #[error("presentation owner inventory references unresolved expression {expression:?}")]
    UnresolvedExpression { expression: ExprId },
    #[error("presentation owner inventory references unresolved statement {statement:?}")]
    UnresolvedStatement { statement: StmtId },
    #[error("presentation owner inventory references unresolved type {ty:?}")]
    UnresolvedType { ty: TypeId },
    #[error("presentation owner inventory references unresolved pattern {pattern:?}")]
    UnresolvedPattern { pattern: PatternId },
}

#[derive(Clone)]
struct ScopeEdges {
    children: Box<[ScopeId]>,
    locals: Box<[LocalId]>,
}

#[derive(Default)]
struct ScopedOwners {
    expressions: Vec<ExprId>,
    statements: Vec<StmtId>,
    types: Vec<TypeId>,
    patterns: Vec<PatternId>,
}

#[derive(Clone, Copy)]
enum PendingPresentationOwner {
    Scope(ScopeId),
    Local(LocalId),
    Expression(ExprId),
    Statement(StmtId),
    Type(TypeId),
    Pattern(PatternId),
}

impl<'project> HirExecutableProjectView<'project> {
    /// Derives the sole runtime semantic owner domain from accepted final HIR.
    ///
    /// A View's complete callable-scope subtree and every Style value subtree
    /// belong to presentation products. Their typed descendants are excluded
    /// together so local IDs, normalized types, and operational facts cannot
    /// cross the product boundary independently.
    #[expect(
        clippy::too_many_lines,
        reason = "one fixed-point transaction indexes and closes every typed HIR owner family without exposing a partial owner graph"
    )]
    pub fn runtime_semantic_owner_inventory(
        self,
    ) -> Result<HirRuntimeSemanticOwnerInventory<'project>, HirRuntimeSemanticOwnerInventoryError>
    {
        let mut all_locals = Vec::new();
        let mut all_expressions = BTreeSet::new();
        let mut all_statements = BTreeSet::new();
        let mut all_types = BTreeSet::new();
        let mut all_patterns = BTreeSet::new();
        let mut all_captures = BTreeMap::new();
        let mut scopes = BTreeMap::new();
        let mut scope_members = BTreeMap::<ScopeId, ScopedOwners>::new();
        let mut local_types = BTreeMap::new();
        let mut expression_edges = BTreeMap::new();
        let mut statement_types = BTreeMap::new();
        let mut type_edges = BTreeMap::new();
        let mut pattern_types = BTreeMap::new();
        let mut owned_scopes = BTreeMap::<HirScopeOwner, Vec<ScopeId>>::new();

        for (_, module) in self.modules() {
            for (owner, scope) in module.scopes() {
                scopes.insert(
                    owner,
                    ScopeEdges {
                        children: scope.children().into(),
                        locals: scope.locals().into(),
                    },
                );
                owned_scopes.entry(*scope.owner()).or_default().push(owner);
            }
            for (owner, local) in module.locals() {
                all_locals.push(owner);
                local_types.insert(owner, local.annotation());
            }
            for (owner, expression) in module.expressions() {
                all_expressions.insert(owner);
                scope_members
                    .entry(expression.scope())
                    .or_default()
                    .expressions
                    .push(owner);
                expression_edges.insert(
                    owner,
                    (
                        expression.kind().direct_expression_children(),
                        expression.kind().direct_type_roots(),
                    ),
                );
            }
            for (owner, statement) in module.statements() {
                all_statements.insert(owner);
                scope_members
                    .entry(statement.scope())
                    .or_default()
                    .statements
                    .push(owner);
                statement_types.insert(owner, statement.kind().direct_type_roots());
            }
            for (owner, ty) in module.types() {
                all_types.insert(owner);
                scope_members
                    .entry(ty.scope())
                    .or_default()
                    .types
                    .push(owner);
                type_edges.insert(owner, ty.kind().direct_type_children());
            }
            for (owner, pattern) in module.patterns() {
                all_patterns.insert(owner);
                scope_members
                    .entry(pattern.scope())
                    .or_default()
                    .patterns
                    .push(owner);
                pattern_types.insert(owner, pattern.kind().authored_type());
            }
            for (owner, capture) in module.captures() {
                all_captures.insert(owner, capture.closure());
            }
        }

        let mut pending = VecDeque::new();
        for item in self.items() {
            let Some(roots) = item.item().presentation_semantic_roots() else {
                continue;
            };
            pending.extend(roots.scope().map(PendingPresentationOwner::Scope));
            pending.extend(
                roots
                    .expressions()
                    .map(PendingPresentationOwner::Expression),
            );
            pending.extend(roots.types().map(PendingPresentationOwner::Type));
        }

        let mut presentation_scopes = BTreeSet::new();
        let mut presentation_locals = BTreeSet::new();
        let mut presentation_expressions = BTreeSet::new();
        let mut presentation_statements = BTreeSet::new();
        let mut presentation_types = BTreeSet::new();
        let mut presentation_patterns = BTreeSet::new();

        while let Some(owner) = pending.pop_front() {
            match owner {
                PendingPresentationOwner::Scope(owner) => {
                    if !presentation_scopes.insert(owner) {
                        continue;
                    }
                    let edges = scopes.get(&owner).ok_or(
                        HirRuntimeSemanticOwnerInventoryError::UnresolvedScope { scope: owner },
                    )?;
                    pending.extend(
                        edges
                            .children
                            .iter()
                            .copied()
                            .map(PendingPresentationOwner::Scope),
                    );
                    pending.extend(
                        edges
                            .locals
                            .iter()
                            .copied()
                            .map(PendingPresentationOwner::Local),
                    );
                    if let Some(members) = scope_members.get(&owner) {
                        pending.extend(
                            members
                                .expressions
                                .iter()
                                .copied()
                                .map(PendingPresentationOwner::Expression),
                        );
                        pending.extend(
                            members
                                .statements
                                .iter()
                                .copied()
                                .map(PendingPresentationOwner::Statement),
                        );
                        pending.extend(
                            members
                                .types
                                .iter()
                                .copied()
                                .map(PendingPresentationOwner::Type),
                        );
                        pending.extend(
                            members
                                .patterns
                                .iter()
                                .copied()
                                .map(PendingPresentationOwner::Pattern),
                        );
                    }
                }
                PendingPresentationOwner::Local(owner) => {
                    if !presentation_locals.insert(owner) {
                        continue;
                    }
                    let ty = local_types.get(&owner).ok_or(
                        HirRuntimeSemanticOwnerInventoryError::UnresolvedLocal { local: owner },
                    )?;
                    pending.extend(ty.iter().copied().map(PendingPresentationOwner::Type));
                }
                PendingPresentationOwner::Expression(owner) => {
                    if !presentation_expressions.insert(owner) {
                        continue;
                    }
                    let (expressions, types) = expression_edges.get(&owner).ok_or(
                        HirRuntimeSemanticOwnerInventoryError::UnresolvedExpression {
                            expression: owner,
                        },
                    )?;
                    pending.extend(
                        expressions
                            .iter()
                            .copied()
                            .map(PendingPresentationOwner::Expression),
                    );
                    pending.extend(types.iter().copied().map(PendingPresentationOwner::Type));
                    pending.extend(
                        owned_scopes
                            .get(&HirScopeOwner::Expr(owner))
                            .into_iter()
                            .flatten()
                            .copied()
                            .map(PendingPresentationOwner::Scope),
                    );
                }
                PendingPresentationOwner::Statement(owner) => {
                    if !presentation_statements.insert(owner) {
                        continue;
                    }
                    let types = statement_types.get(&owner).ok_or(
                        HirRuntimeSemanticOwnerInventoryError::UnresolvedStatement {
                            statement: owner,
                        },
                    )?;
                    pending.extend(types.iter().copied().map(PendingPresentationOwner::Type));
                    pending.extend(
                        owned_scopes
                            .get(&HirScopeOwner::Stmt(owner))
                            .into_iter()
                            .flatten()
                            .copied()
                            .map(PendingPresentationOwner::Scope),
                    );
                }
                PendingPresentationOwner::Type(owner) => {
                    if !presentation_types.insert(owner) {
                        continue;
                    }
                    let children = type_edges.get(&owner).ok_or(
                        HirRuntimeSemanticOwnerInventoryError::UnresolvedType { ty: owner },
                    )?;
                    pending.extend(children.iter().copied().map(PendingPresentationOwner::Type));
                }
                PendingPresentationOwner::Pattern(owner) => {
                    if !presentation_patterns.insert(owner) {
                        continue;
                    }
                    let ty = pattern_types.get(&owner).ok_or(
                        HirRuntimeSemanticOwnerInventoryError::UnresolvedPattern { pattern: owner },
                    )?;
                    pending.extend(ty.iter().copied().map(PendingPresentationOwner::Type));
                }
            }
        }

        let locals = all_locals
            .into_iter()
            .filter(|owner| !presentation_locals.contains(owner))
            .collect();
        let expressions = all_expressions
            .difference(&presentation_expressions)
            .copied()
            .collect();
        let statements = all_statements
            .difference(&presentation_statements)
            .copied()
            .collect();
        let types = all_types.difference(&presentation_types).copied().collect();
        let patterns = all_patterns
            .difference(&presentation_patterns)
            .copied()
            .collect();
        let captures = all_captures
            .into_iter()
            .filter_map(|(owner, closure)| {
                (!presentation_expressions.contains(&closure)).then_some(owner)
            })
            .collect();

        Ok(HirRuntimeSemanticOwnerInventory {
            project: self,
            locals,
            expressions,
            statements,
            types,
            patterns,
            captures,
        })
    }
}
