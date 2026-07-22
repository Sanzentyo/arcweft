//! Borrow, dereference, and indexed-access expression checking.

use super::super::helpers::{collection_index_type, type_kind_label};
use super::support::collection_index_key_type;
use super::{Expr, TypeCheckError, TypeChecker, TypeKind};
use arcweft_lang_syntax::reference::{BorrowExpr, DerefExpr};

impl TypeChecker<'_> {
    pub(super) fn check_borrow_expr(&mut self, borrow: &BorrowExpr) -> Option<TypeKind> {
        self.check_expr(borrow.operand())
            .map(|inner| TypeKind::BorrowRef {
                kind: borrow.kind(),
                lifetime: None,
                inner: Box::new(inner),
            })
    }

    pub(super) fn check_deref_expr(&mut self, deref: &DerefExpr) -> Option<TypeKind> {
        match self.check_expr(deref.operand()) {
            Some(TypeKind::BorrowRef { inner, .. }) => Some(*inner),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "dereference operand must be a reference, found {}",
                    type_kind_label(&other)
                )));
                None
            }
            None => None,
        }
    }

    pub(super) fn check_index_expr(&mut self, target: &Expr, index: &Expr) -> Option<TypeKind> {
        let target_type = self.check_expr(target);
        if let Some(expected_index) = target_type
            .as_ref()
            .and_then(collection_index_key_type)
            .or_else(|| {
                target_type
                    .as_ref()
                    .and_then(|target_type| self.env.index_type(target_type).map(|_| TypeKind::I64))
            })
        {
            self.expect_expr_type(index, &expected_index, "collection index");
        } else {
            self.check_expr(index);
        }
        target_type.and_then(|target_type| {
            collection_index_type(&target_type)
                .or_else(|| self.env.index_type(&target_type).cloned())
                .or_else(|| {
                    self.errors.push(TypeCheckError::new(format!(
                        "type {target_type:?} is not indexable"
                    )));
                    None
                })
        })
    }
}
