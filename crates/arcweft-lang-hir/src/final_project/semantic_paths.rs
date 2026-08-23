//! Snapshot-bound declaration-root semantic path construction.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::HirExecutableProjectView;
use crate::{
    body_edges::{HirBodyChild, HirBodyChildEdge, HirBodyChildRole},
    expr::{
        HirExprKind, HirExpressionChildRole, HirExpressionOwnedBodyRole, HirExpressionOwnedChild,
        HirExpressionOwnedChildEdgeError,
    },
    identity::{ExprId, LocalId, PatternId, StmtId},
    item::{HirImplMember, HirItemKind, HirMethodParameter, HirParameter},
    module::HirModule,
    pattern::{HirPatternChild, HirPatternChildRole},
    source_index::HirCallableSourceOwner,
    stmt::{HirStatementBodyRole, HirStatementChild, HirStatementChildRole},
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};

/// Closed declaration roots for executable callable bodies.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationBodyRootRole {
    FunctionBody,
    PredicateBody,
    ProofBody,
    FlowBody,
    ImplFunctionBody,
    ViewValue { ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSemanticPathStep {
    DeclarationBody(HirDeclarationBodyRootRole),
    ExpressionOwned(HirExpressionOwnedBodyRole),
    Body(HirBodyChildRole),
    Statement(HirStatementChildRole),
    ThreadBody(HirStatementBodyRole),
    Expression(HirExpressionChildRole),
    MatchPattern { arm: u32 },
    Pattern(HirPatternChildRole),
    ParameterPattern { group: u32, parameter: u32 },
    ParameterDefault { group: u32, parameter: u32 },
}

/// Session-only expression hop used by sema to join the HIR role with the
/// exact checked edge fact. Raw IDs never enter [`HirSemanticPathStep`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpressionSemanticHop {
    parent: ExprId,
    child: ExprId,
    role: HirExpressionChildRole,
}

impl HirExpressionSemanticHop {
    pub const fn parent(&self) -> ExprId {
        self.parent
    }

    pub const fn child(&self) -> ExprId {
        self.child
    }

    pub const fn role(&self) -> &HirExpressionChildRole {
        &self.role
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationSemanticPathIndex {
    declaration: CallableDeclarationKey,
    expressions: BTreeMap<ExprId, Box<[HirSemanticPathStep]>>,
    expression_hops: BTreeMap<ExprId, Box<[HirExpressionSemanticHop]>>,
    statements: BTreeMap<StmtId, Box<[HirSemanticPathStep]>>,
    patterns: BTreeMap<PatternId, Box<[HirSemanticPathStep]>>,
    locals: BTreeMap<LocalId, Box<[HirSemanticPathStep]>>,
}

impl HirDeclarationSemanticPathIndex {
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub fn expression(&self, owner: ExprId) -> Option<&[HirSemanticPathStep]> {
        self.expressions.get(&owner).map(Box::as_ref)
    }

    pub fn expression_hops(&self, owner: ExprId) -> Option<&[HirExpressionSemanticHop]> {
        self.expression_hops.get(&owner).map(Box::as_ref)
    }

    pub fn statement(&self, owner: StmtId) -> Option<&[HirSemanticPathStep]> {
        self.statements.get(&owner).map(Box::as_ref)
    }

    pub fn pattern(&self, owner: PatternId) -> Option<&[HirSemanticPathStep]> {
        self.patterns.get(&owner).map(Box::as_ref)
    }

    pub fn local(&self, owner: LocalId) -> Option<&[HirSemanticPathStep]> {
        self.locals.get(&owner).map(Box::as_ref)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSemanticPathError {
    #[error("semantic path symbol world does not match the executable project")]
    SymbolWorldMismatch,
    #[error("semantic path declaration is absent or ambiguous")]
    DeclarationUnavailable,
    #[error("semantic path declaration belongs to a foreign HIR snapshot")]
    ForeignSnapshot,
    #[error("semantic path declaration has no executable body")]
    MissingBody,
    #[error("semantic path references an unresolved HIR owner")]
    UnresolvedOwner,
    #[error("one HIR owner is reachable through more than one declaration-root path")]
    DuplicatePath,
    #[error("semantic path recursion is cyclic")]
    CyclicPath,
    #[error("a semantic path child ordinal does not fit u32")]
    OrdinalOverflow,
    #[error("an expression-owned semantic path lacks a structural coordinate")]
    InvalidOwnedPath,
}

impl HirExecutableProjectView<'_> {
    pub fn declaration_semantic_paths(
        self,
        symbols: &ProjectSymbolTable,
        declaration: &CallableDeclarationKey,
    ) -> Result<HirDeclarationSemanticPathIndex, HirSemanticPathError> {
        if symbols.world().package() != self.package() {
            return Err(HirSemanticPathError::SymbolWorldMismatch);
        }
        let mut candidates = symbols
            .callable_symbols()
            .filter(|symbol| symbol.declaration() == declaration);
        let symbol = candidates
            .next()
            .ok_or(HirSemanticPathError::DeclarationUnavailable)?;
        if candidates.next().is_some() {
            return Err(HirSemanticPathError::DeclarationUnavailable);
        }
        let module = self
            .modules()
            .find_map(|(_, module)| {
                (module.module_id() == symbol.source_item().module()).then_some(module.as_ref())
            })
            .ok_or(HirSemanticPathError::DeclarationUnavailable)?;
        if module.snapshot_id() != symbol.source_snapshot() {
            return Err(HirSemanticPathError::ForeignSnapshot);
        }
        let roots = declaration_body_roots(module, symbol.source_item(), symbol.source_owner())?;
        let mut builder = PathBuilder::new(module, declaration.clone());
        for root in
            declaration_parameter_roots(module, symbol.source_item(), symbol.source_owner())?
        {
            builder.walk_parameter(root)?;
        }
        for root in roots {
            builder.walk_declaration_body(root)?;
        }
        Ok(builder.finish())
    }
}

enum HirParameterRootChild {
    Pattern(PatternId),
    Expression(ExprId),
}

struct HirParameterRoot {
    child: HirParameterRootChild,
    step: HirSemanticPathStep,
}

fn declaration_parameter_roots(
    module: &HirModule,
    item: crate::identity::ItemId,
    owner: HirCallableSourceOwner,
) -> Result<Vec<HirParameterRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let mut roots = Vec::new();
    match owner {
        HirCallableSourceOwner::Item => match item.kind() {
            HirItemKind::Function(function) => {
                for (group, parameters) in function.parameter_groups().iter().enumerate() {
                    push_parameters(&mut roots, checked_ordinal(group)?, parameters.parameters())?;
                }
            }
            HirItemKind::Predicate(predicate) => {
                push_parameters(&mut roots, 0, predicate.parameters())?;
            }
            HirItemKind::Proof(proof) => push_parameters(&mut roots, 0, proof.parameters())?,
            HirItemKind::Flow(flow) => push_parameters(&mut roots, 0, flow.parameters())?,
            _ => {}
        },
        HirCallableSourceOwner::ImplFunction { member } => {
            let HirItemKind::Impl(implementation) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            let Some(HirImplMember::Function(function)) =
                implementation.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            for (group, parameters) in function.parameter_groups().iter().enumerate() {
                for (parameter, value) in parameters.parameters().iter().enumerate() {
                    let group = checked_ordinal(group)?;
                    let parameter = checked_ordinal(parameter)?;
                    match value {
                        HirMethodParameter::Receiver(receiver) => roots.push(HirParameterRoot {
                            child: HirParameterRootChild::Pattern(receiver.pattern()),
                            step: HirSemanticPathStep::ParameterPattern { group, parameter },
                        }),
                        HirMethodParameter::Typed(value) => {
                            push_parameter(&mut roots, group, parameter, value);
                        }
                    }
                }
            }
        }
        HirCallableSourceOwner::ViewItem => {
            let HirItemKind::View(view) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            push_parameters(&mut roots, 0, view.parameters())?;
        }
        HirCallableSourceOwner::ExternCapabilityFunction { .. }
        | HirCallableSourceOwner::TraitFunction { .. } => {}
    }
    Ok(roots)
}

fn push_parameters(
    roots: &mut Vec<HirParameterRoot>,
    group: u32,
    parameters: &[HirParameter],
) -> Result<(), HirSemanticPathError> {
    for (parameter, value) in parameters.iter().enumerate() {
        push_parameter(roots, group, checked_ordinal(parameter)?, value);
    }
    Ok(())
}

fn push_parameter(
    roots: &mut Vec<HirParameterRoot>,
    group: u32,
    parameter: u32,
    value: &HirParameter,
) {
    roots.push(HirParameterRoot {
        child: HirParameterRootChild::Pattern(value.pattern()),
        step: HirSemanticPathStep::ParameterPattern { group, parameter },
    });
    if let Some(default) = value.default() {
        roots.push(HirParameterRoot {
            child: HirParameterRootChild::Expression(default),
            step: HirSemanticPathStep::ParameterDefault { group, parameter },
        });
    }
}

fn checked_ordinal(value: usize) -> Result<u32, HirSemanticPathError> {
    u32::try_from(value).map_err(|_| HirSemanticPathError::OrdinalOverflow)
}

enum HirDeclarationBodyRootChild {
    Body(Vec<HirBodyChildEdge>),
    Expression(ExprId),
}

struct HirDeclarationBodyRoot {
    role: HirDeclarationBodyRootRole,
    child: HirDeclarationBodyRootChild,
}

fn declaration_body_roots(
    module: &HirModule,
    item: crate::identity::ItemId,
    owner: HirCallableSourceOwner,
) -> Result<Vec<HirDeclarationBodyRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    match owner {
        HirCallableSourceOwner::Item => match item.kind() {
            HirItemKind::Function(function) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::FunctionBody,
                function
                    .body()
                    .try_child_edges()
                    .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
            )]),
            HirItemKind::Predicate(predicate) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::PredicateBody,
                predicate
                    .body()
                    .try_child_edges()
                    .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
            )]),
            HirItemKind::Proof(proof) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::ProofBody,
                proof
                    .body()
                    .try_child_edges()
                    .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
            )]),
            HirItemKind::Flow(flow) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::FlowBody,
                flow.body()
                    .try_child_edges()
                    .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
            )]),
            _ => Err(HirSemanticPathError::MissingBody),
        },
        HirCallableSourceOwner::ImplFunction { member } => {
            let HirItemKind::Impl(implementation) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            let Some(HirImplMember::Function(function)) =
                implementation.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::ImplFunctionBody,
                function
                    .body()
                    .ok_or(HirSemanticPathError::MissingBody)?
                    .try_child_edges()
                    .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
            )])
        }
        HirCallableSourceOwner::ViewItem => {
            let HirItemKind::View(view) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            view.values()
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, expression)| {
                    Ok(HirDeclarationBodyRoot {
                        role: HirDeclarationBodyRootRole::ViewValue {
                            ordinal: checked_ordinal(ordinal)?,
                        },
                        child: HirDeclarationBodyRootChild::Expression(expression),
                    })
                })
                .collect()
        }
        HirCallableSourceOwner::ExternCapabilityFunction { .. }
        | HirCallableSourceOwner::TraitFunction { .. } => Err(HirSemanticPathError::MissingBody),
    }
}

fn declaration_body(
    role: HirDeclarationBodyRootRole,
    edges: Vec<HirBodyChildEdge>,
) -> HirDeclarationBodyRoot {
    HirDeclarationBodyRoot {
        role,
        child: HirDeclarationBodyRootChild::Body(edges),
    }
}

struct PathBuilder<'module> {
    module: &'module HirModule,
    declaration: CallableDeclarationKey,
    expressions: BTreeMap<ExprId, Box<[HirSemanticPathStep]>>,
    expression_hops: BTreeMap<ExprId, Box<[HirExpressionSemanticHop]>>,
    statements: BTreeMap<StmtId, Box<[HirSemanticPathStep]>>,
    patterns: BTreeMap<PatternId, Box<[HirSemanticPathStep]>>,
    locals: BTreeMap<LocalId, Box<[HirSemanticPathStep]>>,
    active_expressions: BTreeSet<ExprId>,
    active_statements: BTreeSet<StmtId>,
    active_patterns: BTreeSet<PatternId>,
}

impl<'module> PathBuilder<'module> {
    fn new(module: &'module HirModule, declaration: CallableDeclarationKey) -> Self {
        Self {
            module,
            declaration,
            expressions: BTreeMap::new(),
            expression_hops: BTreeMap::new(),
            statements: BTreeMap::new(),
            patterns: BTreeMap::new(),
            locals: BTreeMap::new(),
            active_expressions: BTreeSet::new(),
            active_statements: BTreeSet::new(),
            active_patterns: BTreeSet::new(),
        }
    }

    fn finish(self) -> HirDeclarationSemanticPathIndex {
        HirDeclarationSemanticPathIndex {
            declaration: self.declaration,
            expressions: self.expressions,
            expression_hops: self.expression_hops,
            statements: self.statements,
            patterns: self.patterns,
            locals: self.locals,
        }
    }

    fn walk_declaration_body(
        &mut self,
        root: HirDeclarationBodyRoot,
    ) -> Result<(), HirSemanticPathError> {
        let path = [HirSemanticPathStep::DeclarationBody(root.role)];
        match root.child {
            HirDeclarationBodyRootChild::Body(edges) => {
                for edge in edges {
                    self.walk_body(edge, &path)?;
                }
                Ok(())
            }
            HirDeclarationBodyRootChild::Expression(owner) => {
                self.walk_expression(owner, &path, &[])
            }
        }
    }

    fn walk_body(
        &mut self,
        edge: HirBodyChildEdge,
        parent: &[HirSemanticPathStep],
    ) -> Result<(), HirSemanticPathError> {
        let path = pushed(parent, HirSemanticPathStep::Body(edge.role()));
        match edge.child() {
            HirBodyChild::Expression(owner) => self.walk_expression(owner, &path, &[]),
            HirBodyChild::Statement(owner) => self.walk_statement(owner, &path),
        }
    }

    fn walk_parameter(&mut self, root: HirParameterRoot) -> Result<(), HirSemanticPathError> {
        let path = [root.step];
        match root.child {
            HirParameterRootChild::Pattern(owner) => self.walk_pattern(owner, &path),
            HirParameterRootChild::Expression(owner) => self.walk_expression(owner, &path, &[]),
        }
    }

    fn walk_expression(
        &mut self,
        owner: ExprId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
    ) -> Result<(), HirSemanticPathError> {
        if self.active_expressions.contains(&owner) {
            return Err(HirSemanticPathError::CyclicPath);
        }
        insert_unique(&mut self.expressions, owner, path)?;
        if self.expression_hops.insert(owner, hops.into()).is_some() {
            return Err(HirSemanticPathError::DuplicatePath);
        }
        self.active_expressions.insert(owner);
        let expression = self
            .module
            .resolve_expr(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        if let HirExprKind::Thread(thread) = expression.kind() {
            for edge in thread
                .body()
                .try_child_edges()
                .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
            {
                self.walk_body(edge, path)?;
            }
        }
        if let HirExprKind::Match(matched) = expression.kind() {
            for (arm, row) in matched.arms().iter().enumerate() {
                self.walk_pattern(
                    row.pattern(),
                    &pushed(
                        path,
                        HirSemanticPathStep::MatchPattern {
                            arm: checked_ordinal(arm)?,
                        },
                    ),
                )?;
            }
        }
        let owned_edges = expression
            .kind()
            .expression_owned_child_edges()
            .map_err(|error| match error {
                HirExpressionOwnedChildEdgeError::OrdinalOverflow => {
                    HirSemanticPathError::OrdinalOverflow
                }
                HirExpressionOwnedChildEdgeError::EmptyNestedPath => {
                    HirSemanticPathError::InvalidOwnedPath
                }
            })?;
        for edge in owned_edges {
            self.walk_expression_owned_edge(&edge, path)?;
        }
        for (ordinal, statement) in expression_statements(expression.kind()).iter().enumerate() {
            let role = HirBodyChildRole::Statement {
                ordinal: checked_ordinal(ordinal)?,
            };
            self.walk_statement(*statement, &pushed(path, HirSemanticPathStep::Body(role)))?;
        }
        for edge in expression
            .kind()
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            // For lowering publishes source, iterator, and next-value roots
            // directly on the owning statement. Following a synthetic
            // `ForInput` edge would reach the same source expression through
            // a second path, so the statement-owned edge remains the sole
            // coordinate authority for this lowered chain.
            if matches!(
                (expression.kind(), edge.role()),
                (
                    HirExprKind::ForSynthetic(_),
                    HirExpressionChildRole::ForInput
                )
            ) {
                continue;
            }
            let mut child_hops = hops.to_vec();
            child_hops.push(HirExpressionSemanticHop {
                parent: owner,
                child: edge.child(),
                role: edge.role().clone(),
            });
            self.walk_expression(
                edge.child(),
                &pushed(path, HirSemanticPathStep::Expression(edge.role().clone())),
                &child_hops,
            )?;
        }
        self.active_expressions.remove(&owner);
        Ok(())
    }

    fn walk_expression_owned_edge(
        &mut self,
        edge: &crate::expr::HirExpressionOwnedChildEdge,
        parent: &[HirSemanticPathStep],
    ) -> Result<(), HirSemanticPathError> {
        let path = pushed(
            parent,
            HirSemanticPathStep::ExpressionOwned(edge.role().clone()),
        );
        match edge.child() {
            HirExpressionOwnedChild::Pattern(owner) => self.walk_pattern(owner, &path),
            HirExpressionOwnedChild::Statement(owner) => self.walk_statement(owner, &path),
            HirExpressionOwnedChild::Body(edge) => self.walk_body(edge, &path),
        }
    }

    fn walk_statement(
        &mut self,
        owner: StmtId,
        path: &[HirSemanticPathStep],
    ) -> Result<(), HirSemanticPathError> {
        if self.active_statements.contains(&owner) {
            return Err(HirSemanticPathError::CyclicPath);
        }
        insert_unique(&mut self.statements, owner, path)?;
        self.active_statements.insert(owner);
        let statement = self
            .module
            .resolve_stmt(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        for edge in statement
            .kind()
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            let path = pushed(path, HirSemanticPathStep::Statement(edge.role()));
            match edge.child() {
                HirStatementChild::Expression(owner) => self.walk_expression(owner, &path, &[])?,
                HirStatementChild::Statement(owner) => self.walk_statement(owner, &path)?,
                HirStatementChild::Pattern(owner) => self.walk_pattern(owner, &path)?,
                HirStatementChild::Type(_) => {}
                HirStatementChild::Local(owner) => {
                    insert_unique(&mut self.locals, owner, &path)?;
                }
            }
        }
        for (role, edges) in statement
            .kind()
            .try_thread_body_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            let body_path = pushed(path, HirSemanticPathStep::ThreadBody(role));
            for edge in edges {
                self.walk_body(edge, &body_path)?;
            }
        }
        self.active_statements.remove(&owner);
        Ok(())
    }

    fn walk_pattern(
        &mut self,
        owner: PatternId,
        path: &[HirSemanticPathStep],
    ) -> Result<(), HirSemanticPathError> {
        if self.active_patterns.contains(&owner) {
            return Err(HirSemanticPathError::CyclicPath);
        }
        insert_unique(&mut self.patterns, owner, path)?;
        self.active_patterns.insert(owner);
        let pattern = self
            .module
            .resolve_pattern(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        for edge in pattern
            .kind()
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            let path = pushed(path, HirSemanticPathStep::Pattern(edge.role()));
            match edge.child() {
                HirPatternChild::Pattern(owner) => self.walk_pattern(owner, &path)?,
                HirPatternChild::Type(_) => {}
                HirPatternChild::Local(owner) => {
                    insert_unique(&mut self.locals, owner, &path)?;
                }
            }
        }
        self.active_patterns.remove(&owner);
        Ok(())
    }
}

fn expression_statements(kind: &HirExprKind) -> &[StmtId] {
    match kind {
        HirExprKind::Block(block) => block.statements(),
        HirExprKind::ComputationBlock(block) => block.statements(),
        HirExprKind::NamedBlock(block) => block.statements(),
        HirExprKind::Loop(block) => block.statements(),
        _ => &[],
    }
}

fn pushed(parent: &[HirSemanticPathStep], step: HirSemanticPathStep) -> Vec<HirSemanticPathStep> {
    let mut path = Vec::with_capacity(parent.len() + 1);
    path.extend_from_slice(parent);
    path.push(step);
    path
}

fn insert_unique<K: Ord + Copy + std::fmt::Debug>(
    rows: &mut BTreeMap<K, Box<[HirSemanticPathStep]>>,
    owner: K,
    path: &[HirSemanticPathStep],
) -> Result<(), HirSemanticPathError> {
    if rows.insert(owner, path.into()).is_some() {
        Err(HirSemanticPathError::DuplicatePath)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "semantic_paths/tests.rs"]
mod tests;
