use arcweft_lang_syntax::{ast::flow::Stmt, expr::Expr};

#[derive(Clone, Copy)]
pub(super) struct FlowValueBlock<'a> {
    statements: &'a [Stmt],
    value: Option<&'a Expr>,
}

impl<'a> FlowValueBlock<'a> {
    pub(super) fn new(statements: &'a [Stmt], value: Option<&'a Expr>) -> Self {
        Self { statements, value }
    }

    pub(super) fn from_expr(expr: &'a Expr) -> Option<Self> {
        match expr {
            Expr::Block { statements, value }
            | Expr::ComputationBlock {
                statements, value, ..
            }
            | Expr::NamedBlock {
                statements, value, ..
            } => Some(Self::new(statements, value.as_deref())),
            _ => None,
        }
    }

    pub(super) fn statements(&self) -> &'a [Stmt] {
        self.statements
    }

    pub(super) fn value(&self) -> Option<&'a Expr> {
        self.value
    }
}
