use super::{CallArg, Expr, Placeholder};
use crate::ast::flow::{FlowItem, Stmt, WaitTarget};

impl Expr {
    /// Returns whether this expression reads the surrounding pipe's left value.
    ///
    /// A nested pipe starts a new scope only for its RHS. Its LHS still belongs
    /// to the surrounding RHS, so `outer |> (^ |> inner(^))` finds only the
    /// first placeholder for the outer pipe.
    pub fn contains_pipe_left(&self) -> bool {
        match self {
            Self::Placeholder(Placeholder::PipeLeft) => true,
            Self::Placeholder(Placeholder::Partial)
            | Self::NumericBracketSeq(_)
            | Self::Literal(_)
            | Self::EntityRef(_)
            | Self::LifetimePath { .. }
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Raw(_) => false,
            Self::Tuple(items) | Self::BracketSeq(items) => {
                items.iter().any(Self::contains_pipe_left)
            }
            Self::ArrayRepeat { value, len }
            | Self::Binary {
                lhs: value,
                rhs: len,
                ..
            }
            | Self::Index {
                target: value,
                index: len,
            } => value.contains_pipe_left() || len.contains_pipe_left(),
            Self::Call { callee, args } => {
                callee.contains_pipe_left() || args.iter().any(call_arg_contains_pipe_left)
            }
            Self::Select(select) => select.target().contains_pipe_left(),
            Self::DialogueCall { callee, .. } => callee.contains_pipe_left(),
            Self::Pipe { lhs, .. } => lhs.contains_pipe_left(),
            Self::Try { expr }
            | Self::Await { expr, .. }
            | Self::Unary { expr, .. }
            | Self::Closure { body: expr, .. } => expr.contains_pipe_left(),
            Self::Borrow(borrow) => borrow.operand().contains_pipe_left(),
            Self::Deref(deref) => deref.operand().contains_pipe_left(),
            Self::Thread { block } => block.body().iter().any(flow_item_contains_pipe_left),
            Self::Range { start, end, .. } => {
                start.as_deref().is_some_and(Self::contains_pipe_left)
                    || end.as_deref().is_some_and(Self::contains_pipe_left)
            }
            Self::Record { fields, .. } | Self::RecordLiteral(fields) => {
                fields.iter().any(|(_, value)| value.contains_pipe_left())
            }
            Self::Block { statements, value }
            | Self::ComputationBlock {
                statements, value, ..
            }
            | Self::NamedBlock {
                statements, value, ..
            } => {
                statements.iter().any(stmt_contains_pipe_left)
                    || value.as_deref().is_some_and(Self::contains_pipe_left)
            }
            Self::MemoBlock {
                options,
                statements,
                value,
            } => {
                options.iter().any(|(_, value)| value.contains_pipe_left())
                    || statements.iter().any(stmt_contains_pipe_left)
                    || value.as_deref().is_some_and(Self::contains_pipe_left)
            }
            Self::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.contains_pipe_left()
                    || then_branch.contains_pipe_left()
                    || else_branch.as_deref().is_some_and(Self::contains_pipe_left)
            }
            Self::IfLet {
                expr,
                guard,
                then_branch,
                else_branch,
                ..
            } => {
                expr.contains_pipe_left()
                    || guard.as_deref().is_some_and(Self::contains_pipe_left)
                    || then_branch.contains_pipe_left()
                    || else_branch.as_deref().is_some_and(Self::contains_pipe_left)
            }
            Self::Match { scrutinee, arms } => {
                scrutinee.contains_pipe_left()
                    || arms.iter().any(|arm| {
                        arm.guard().is_some_and(Self::contains_pipe_left)
                            || arm.value().contains_pipe_left()
                    })
            }
        }
    }
}

fn call_arg_contains_pipe_left(arg: &CallArg) -> bool {
    arg.value().contains_pipe_left()
}

fn flow_items_contain_pipe_left(items: &[FlowItem]) -> bool {
    items.iter().any(flow_item_contains_pipe_left)
}

fn flow_item_contains_pipe_left(item: &FlowItem) -> bool {
    match item {
        FlowItem::Stmt(stmt) => stmt_contains_pipe_left(stmt),
        FlowItem::AwaitWith(await_with) => await_with.expr().contains_pipe_left(),
        FlowItem::If(block) => {
            block.condition().contains_pipe_left()
                || flow_items_contain_pipe_left(block.body())
                || flow_items_contain_pipe_left(block.else_body())
        }
        FlowItem::IfLet(block) => {
            block.expr().contains_pipe_left()
                || block.guard().is_some_and(Expr::contains_pipe_left)
                || flow_items_contain_pipe_left(block.body())
                || flow_items_contain_pipe_left(block.else_body())
        }
        FlowItem::Match(block) => {
            block.expr().contains_pipe_left()
                || block.arms().iter().any(|arm| {
                    arm.guard().is_some_and(Expr::contains_pipe_left)
                        || flow_items_contain_pipe_left(arm.body())
                })
        }
        FlowItem::Loop(block) => flow_items_contain_pipe_left(block.body()),
        FlowItem::While(block) => {
            block.condition().contains_pipe_left() || flow_items_contain_pipe_left(block.body())
        }
        FlowItem::WhileLet(block) => {
            block.expr().contains_pipe_left()
                || block.guard().is_some_and(Expr::contains_pipe_left)
                || flow_items_contain_pipe_left(block.body())
        }
        FlowItem::For(block) => {
            block.source().contains_pipe_left() || flow_items_contain_pipe_left(block.body())
        }
        FlowItem::Select(block) => block
            .branches()
            .iter()
            .any(|branch| flow_items_contain_pipe_left(branch.body())),
        FlowItem::SourceLocale(block) => flow_items_contain_pipe_left(block.body()),
        FlowItem::Scope(block) => flow_items_contain_pipe_left(block.body()),
        FlowItem::Choice(_)
        | FlowItem::SpeakerLine(_)
        | FlowItem::ContentCall(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => false,
    }
}

fn stmts_contain_pipe_left(statements: &[Stmt]) -> bool {
    statements.iter().any(stmt_contains_pipe_left)
}

fn stmt_contains_pipe_left(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Assertion(assertion) => assertion.conditions().iter().any(Expr::contains_pipe_left),
        Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
            expr.contains_pipe_left()
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => expr.expr().contains_pipe_left() || stmts_contain_pipe_left(else_body),
        Stmt::Out { expr, .. }
        | Stmt::Defer { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Break {
            expr: Some(expr), ..
        } => expr.expr().contains_pipe_left(),
        Stmt::Assign { target, expr } | Stmt::LifetimeSet { target, expr } => {
            target.expr().contains_pipe_left() || expr.expr().contains_pipe_left()
        }
        Stmt::Signal { target, value } => {
            target.expr().contains_pipe_left() || value.expr().contains_pipe_left()
        }
        Stmt::LetActionReceive { action, .. } => action.expr().contains_pipe_left(),
        Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
            expr.expr().contains_pipe_left()
        }
        Stmt::Thread(thread) => flow_items_contain_pipe_left(thread.body()),
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::Loop { body: statements } => stmts_contain_pipe_left(statements),
        Stmt::UnsafeLifetime { reason, body, .. } => {
            reason.as_ref().is_some_and(Expr::contains_pipe_left) || stmts_contain_pipe_left(body)
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            condition.expr().contains_pipe_left()
                || stmts_contain_pipe_left(body)
                || stmts_contain_pipe_left(else_body)
        }
        Stmt::While { condition, body } => {
            condition.expr().contains_pipe_left() || stmts_contain_pipe_left(body)
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            expr.expr().contains_pipe_left()
                || guard
                    .as_ref()
                    .is_some_and(|guard| guard.expr().contains_pipe_left())
                || stmts_contain_pipe_left(body)
        }
        Stmt::For { source, body, .. } => {
            source.expr().contains_pipe_left() || stmts_contain_pipe_left(body)
        }
        Stmt::Match { expr, arms } => {
            expr.expr().contains_pipe_left()
                || arms.iter().any(|arm| {
                    arm.guard().is_some_and(Expr::contains_pipe_left)
                        || stmts_contain_pipe_left(arm.body())
                })
        }
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Break { expr: None, .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::expr::{Expr, Placeholder, parse_expr};

    #[test]
    fn finds_pipe_reads_inside_closures_and_control_flow() {
        let closure = parse_expr("|| (^, ^)").expect("closure fixture parses");
        let control = Expr::If {
            condition: Box::new(Expr::Path("ready".into())),
            then_branch: Box::new(Expr::Tuple(vec![Expr::Placeholder(Placeholder::PipeLeft)])),
            else_branch: None,
        };

        assert!(closure.contains_pipe_left());
        assert!(control.contains_pipe_left());
    }

    #[test]
    fn nested_pipe_rhs_owns_its_placeholders_but_not_its_lhs() {
        let inner_rhs = parse_expr("inner |> consume(^)").expect("nested pipe parses");
        let inner_lhs = parse_expr("^ |> consume(^)").expect("nested pipe lhs parses");

        assert!(!inner_rhs.contains_pipe_left());
        assert!(inner_lhs.contains_pipe_left());
    }
}
