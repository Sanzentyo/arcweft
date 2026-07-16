//! Deterministic sheet-local token dependency validation.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::style::HirStyleTokenDecl;
use arcweft_lang_syntax::expr::{CallArg, Expr};

use super::diagnostic::{StyleDiagnostic, StyleDiagnosticCode};

pub(crate) struct TokenGraphResult {
    pub(crate) order: Vec<usize>,
    pub(crate) diagnostics: Vec<StyleDiagnostic>,
}

pub(crate) fn token_dependency_order(
    owner_sheet: &str,
    tokens: &[HirStyleTokenDecl],
) -> TokenGraphResult {
    let mut diagnostics = Vec::new();
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();
    for (index, token) in tokens.iter().enumerate() {
        if let Some(previous) = indices.get(token.public_id()).copied() {
            diagnostics.push(
                StyleDiagnostic::new(
                    StyleDiagnosticCode::DuplicateToken,
                    format!("duplicate style token `{}`", token.public_id()),
                    token.range(),
                )
                .with_subject(token.public_id())
                .with_owner_sheet(owner_sheet)
                .with_related_range(tokens[previous].range()),
            );
        } else {
            indices.insert(token.public_id().to_owned(), index);
        }
    }

    let dependencies = tokens
        .iter()
        .map(|token| token_references(token.value().expr()))
        .collect::<Vec<_>>();
    for (index, references) in dependencies.iter().enumerate() {
        for reference in references {
            if !indices.contains_key(reference) {
                diagnostics.push(
                    StyleDiagnostic::new(
                        StyleDiagnosticCode::UnresolvedToken,
                        format!("unknown style token `{reference}`"),
                        tokens[index].value().range(),
                    )
                    .with_subject(reference)
                    .with_owner_sheet(owner_sheet),
                );
            }
        }
    }

    let mut state = vec![VisitState::Unvisited; tokens.len()];
    let mut stack = Vec::new();
    let mut order = Vec::with_capacity(tokens.len());
    {
        let mut walk = TokenGraphWalk {
            tokens,
            indices: &indices,
            dependencies: &dependencies,
            state: &mut state,
            stack: &mut stack,
            order: &mut order,
            diagnostics: &mut diagnostics,
            owner_sheet,
        };
        for index in 0..tokens.len() {
            walk.visit(index);
        }
    }
    TokenGraphResult { order, diagnostics }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

struct TokenGraphWalk<'a> {
    tokens: &'a [HirStyleTokenDecl],
    indices: &'a BTreeMap<String, usize>,
    dependencies: &'a [BTreeSet<String>],
    state: &'a mut [VisitState],
    stack: &'a mut Vec<usize>,
    order: &'a mut Vec<usize>,
    diagnostics: &'a mut Vec<StyleDiagnostic>,
    owner_sheet: &'a str,
}

impl TokenGraphWalk<'_> {
    fn visit(&mut self, index: usize) {
        match self.state[index] {
            VisitState::Visited => return,
            VisitState::Visiting => {
                let start = self
                    .stack
                    .iter()
                    .position(|candidate| *candidate == index)
                    .unwrap_or(0);
                let cycle_indices = self.stack[start..]
                    .iter()
                    .chain(std::iter::once(&index))
                    .copied()
                    .collect::<Vec<_>>();
                let cycle_ids = cycle_indices
                    .iter()
                    .map(|index| self.tokens[*index].public_id().to_owned())
                    .collect::<Vec<_>>();
                let cycle = cycle_ids.join(" -> ");
                let diagnostic = StyleDiagnostic::new(
                    StyleDiagnosticCode::TokenCycle,
                    format!("style token dependency cycle: {cycle}"),
                    self.tokens[index].range(),
                )
                .with_subject(cycle)
                .with_owner_sheet(self.owner_sheet)
                .with_ordered_subjects(cycle_ids);
                self.diagnostics.push(
                    cycle_indices.iter().fold(diagnostic, |diagnostic, index| {
                        diagnostic.with_related_range(self.tokens[*index].range())
                    }),
                );
                return;
            }
            VisitState::Unvisited => {}
        }

        self.state[index] = VisitState::Visiting;
        self.stack.push(index);
        for dependency in &self.dependencies[index] {
            if let Some(dependency) = self.indices.get(dependency).copied() {
                self.visit(dependency);
            }
        }
        self.stack.pop();
        self.state[index] = VisitState::Visited;
        self.order.push(index);
    }
}

fn token_references(expr: &Expr) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    collect_token_references(expr, &mut references);
    references
}

fn collect_token_references(expr: &Expr, references: &mut BTreeSet<String>) {
    match expr {
        Expr::Call { callee, args } => {
            if callee.dotted_selector_label().as_deref() == Some("token")
                && let Some(reference) = args.iter().find_map(|arg| match arg {
                    CallArg::Positional(value) => value.dotted_selector_label(),
                    CallArg::Named { .. } | CallArg::Spread { .. } => None,
                })
            {
                references.insert(reference);
            }
            collect_token_references(callee, references);
            for arg in args {
                match arg {
                    CallArg::Positional(value) => collect_token_references(value, references),
                    CallArg::Named { value, .. } | CallArg::Spread { value } => {
                        collect_token_references(value, references);
                    }
                }
            }
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_token_references(item, references);
            }
        }
        Expr::ArrayRepeat { value, len }
        | Expr::Index {
            target: value,
            index: len,
        }
        | Expr::Pipe {
            lhs: value,
            rhs: len,
        }
        | Expr::Binary {
            lhs: value,
            rhs: len,
            ..
        } => {
            collect_token_references(value, references);
            collect_token_references(len, references);
        }
        Expr::Select(select) => collect_token_references(select.target(), references),
        Expr::Try { expr }
        | Expr::Await { expr, .. }
        | Expr::Unary { expr, .. }
        | Expr::Closure { body: expr, .. } => collect_token_references(expr, references),
        Expr::Borrow(borrow) => collect_token_references(borrow.operand(), references),
        Expr::Deref(deref) => collect_token_references(deref.operand(), references),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                collect_token_references(value, references);
            }
        }
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_token_references(start, references);
            }
            if let Some(end) = end {
                collect_token_references(end, references);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_token_references(condition, references);
            collect_token_references(then_branch, references);
            if let Some(else_branch) = else_branch {
                collect_token_references(else_branch, references);
            }
        }
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::DialogueCall { .. }
        | Expr::Thread { .. }
        | Expr::Block { .. }
        | Expr::ComputationBlock { .. }
        | Expr::NamedBlock { .. }
        | Expr::IfLet { .. }
        | Expr::Match { .. }
        | Expr::Raw(_) => {}
    }
}
