//! Binary-operator type checking.

use arcweft_lang_syntax::expr::{BinaryOp, Expr};

use super::super::helpers::{optional_type_kind_label, type_kind_label};
use super::support::{is_unit_number_type, rhs_expected_type_for_binary};
use super::{TypeCheckError, TypeChecker, TypeKind};

impl TypeChecker<'_> {
    pub(super) fn check_binary_expr(
        &mut self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let operand_expected = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        )
        .then_some(expected)
        .flatten()
        .filter(|ty| ty.is_integer() || ty.is_float());
        let lhs_type = self.check_expr_with_expected(lhs, operand_expected);
        if op == BinaryOp::In {
            return self.check_in_binary_expr(lhs_type.as_ref(), rhs);
        }
        let rhs_expected = rhs_expected_type_for_binary(op, lhs_type.as_ref());
        let rhs_type = self.check_expr_with_expected(rhs, rhs_expected);
        match op {
            BinaryOp::In => unreachable!("`in` is handled before rhs expected-type selection"),
            BinaryOp::Implies | BinaryOp::Or | BinaryOp::And => {
                if lhs_type != Some(TypeKind::Bool) || rhs_type != Some(TypeKind::Bool) {
                    self.errors.push(TypeCheckError::new(format!(
                        "logical contract expression must use bool operands, found {} and {}",
                        optional_type_kind_label(lhs_type.as_ref()),
                        optional_type_kind_label(rhs_type.as_ref())
                    )));
                    return None;
                }
                Some(TypeKind::Bool)
            }
            BinaryOp::Eq | BinaryOp::NotEq => match (lhs_type.as_ref(), rhs_type.as_ref()) {
                (Some(lhs), Some(rhs))
                    if self.types_compatible(lhs, rhs) || self.types_compatible(rhs, lhs) =>
                {
                    Some(TypeKind::Bool)
                }
                _ => {
                    self.errors.push(TypeCheckError::new(format!(
                        "equality operands must be compatible, found {} and {}",
                        optional_type_kind_label(lhs_type.as_ref()),
                        optional_type_kind_label(rhs_type.as_ref())
                    )));
                    None
                }
            },
            BinaryOp::Gte | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Lt => {
                match (lhs_type.as_ref(), rhs_type.as_ref()) {
                    (Some(lhs), Some(rhs))
                        if lhs == rhs && (lhs.is_integer() || lhs.is_float()) =>
                    {
                        Some(TypeKind::Bool)
                    }
                    _ => {
                        self.errors.push(TypeCheckError::new(format!(
                            "ordering operands must have the same ordered scalar type, found {} and {}",
                            optional_type_kind_label(lhs_type.as_ref()),
                            optional_type_kind_label(rhs_type.as_ref())
                        )));
                        None
                    }
                }
            }
            BinaryOp::Merge => match (lhs_type, rhs_type) {
                (Some(TypeKind::CharacterPatch(lhs)), Some(TypeKind::CharacterPatch(rhs)))
                    if lhs == rhs =>
                {
                    Some(TypeKind::CharacterPatch(lhs))
                }
                (Some(TypeKind::FocusPatch), Some(TypeKind::FocusPatch)) => {
                    Some(TypeKind::FocusPatch)
                }
                (lhs, rhs) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "merge operator `&` requires compatible patch operands, found {} and {}",
                        optional_type_kind_label(lhs.as_ref()),
                        optional_type_kind_label(rhs.as_ref())
                    )));
                    None
                }
            },
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if let Some(result) =
                    arithmetic_result_type(op, lhs_type.as_ref(), rhs_type.as_ref())
                {
                    Some(result)
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "arithmetic expression operands must have compatible numeric types or scale a unit value by a float, found {} and {}",
                        optional_type_kind_label(lhs_type.as_ref()),
                        optional_type_kind_label(rhs_type.as_ref())
                    )));
                    None
                }
            }
        }
    }

    fn check_in_binary_expr(
        &mut self,
        lhs_type: Option<&TypeKind>,
        rhs: &Expr,
    ) -> Option<TypeKind> {
        let expected_range = lhs_type
            .filter(|ty| ty.is_integer())
            .cloned()
            .map(|ty| TypeKind::Range(Box::new(ty)));
        let rhs_type = self.check_expr_with_expected(rhs, expected_range.as_ref());
        let Some(TypeKind::Range(item_type)) = rhs_type.as_ref() else {
            self.errors.push(TypeCheckError::new(format!(
                "`in` expression requires a range on the right, found {}",
                optional_type_kind_label(rhs_type.as_ref())
            )));
            return None;
        };
        if let Some(lhs_type) = lhs_type
            && !self.types_compatible(item_type, lhs_type)
        {
            self.errors.push(TypeCheckError::new(format!(
                "`in` expression left operand must have range item type {}, found {}",
                type_kind_label(item_type),
                type_kind_label(lhs_type)
            )));
            return None;
        }
        Some(TypeKind::Bool)
    }
}

fn arithmetic_result_type(
    op: BinaryOp,
    lhs: Option<&TypeKind>,
    rhs: Option<&TypeKind>,
) -> Option<TypeKind> {
    let (lhs, rhs) = (lhs?, rhs?);
    if lhs == rhs && (lhs.is_integer() || lhs.is_float()) {
        return Some(lhs.clone());
    }
    match op {
        BinaryOp::Mul if lhs.is_float() && is_unit_number_type(rhs) => Some(rhs.clone()),
        BinaryOp::Mul | BinaryOp::Div if is_unit_number_type(lhs) && rhs.is_float() => {
            Some(lhs.clone())
        }
        _ => None,
    }
}
